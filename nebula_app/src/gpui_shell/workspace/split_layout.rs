//! Split structure, focus, rendering geometry, and divider interaction authority.

use super::*;

impl NebulaWorkspace {
    /// 在聚焦 pane 上开分屏（ctrl+shift+d / ctrl+shift+s，对齐旧壳
    /// SplitRight/SplitDown）：新 pane 继承聚焦 pane 的 cwd，spawn 网格按
    /// 切割方向对半预估——首帧 prepaint 回写真实矩形后自动收敛。
    pub(super) fn split_focused(
        &mut self,
        direction: SplitDirection,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Result<u64, crate::runtime_api::ApiError> {
        let active = self.active;
        let Some(WorkspaceTab::Terminal { panes, focused, .. }) = self.tabs.get(active) else {
            return Err(crate::runtime_api::ApiError::new(
                "invalid_state",
                "the active tab cannot be split",
            ));
        };
        let focused = *focused;
        let Some(anchor) = panes.iter().find(|pane| pane.id == focused) else {
            return Err(crate::runtime_api::ApiError::new(
                "action_failed",
                "the focused pane is missing from the active split tree",
            ));
        };
        let (cols, rows, launch) = {
            let view = anchor.view.read(cx);
            let (launch, cwd) = tab_duplication::copy_launch(
                view.session_launch.clone(),
                tab_duplication::CopyKind::Split,
                tab_duplication::PaneOrigin::of(view),
            );
            let launch = Self::terminal_launch_from_session(&launch, cwd);
            (view.grid_cols() as u16, view.grid_rows() as u16, launch)
        };
        let grid = match direction {
            SplitDirection::LeftRight => ((cols / 2).max(2), rows.max(2)),
            SplitDirection::TopBottom => (cols.max(2), (rows / 2).max(2)),
        };
        let pane = self.new_pane(grid, launch, None, window, cx);
        let new_id = pane.id;
        let Some(WorkspaceTab::Terminal { panes, tree, focused, zoomed, .. }) =
            self.tabs.get_mut(active)
        else {
            // new_pane 之后 tab 结构不可能已变（同一同步调用栈），但防御住：
            // 树上挂不进去就立即回收，不留孤儿 PTY。
            pane.view.read(cx).shutdown();
            return Err(crate::runtime_api::ApiError::new(
                "action_failed",
                "the active terminal tab changed while creating a split",
            ));
        };
        if !tree.split_leaf(*focused, new_id, direction, 0.5) {
            pane.view.read(cx).shutdown();
            return Err(crate::runtime_api::ApiError::new(
                "action_failed",
                "the focused pane could not be attached to the split tree",
            ));
        }
        panes.push(pane);
        *focused = new_id;
        *zoomed = false;
        self.mark_structural_resize(active, cx);
        self.focus_active(window, cx);
        self.sync_side_panel_to_active(true, cx);
        cx.notify();
        Ok(new_id)
    }

    /// 结构性布局变化：让 tab 内每个 pane 的下一次网格观测直接下发到 Term 与
    /// ConPTY，不进尾沿去抖。
    ///
    /// 分屏创建/关闭、pane 替换、zoom 切换都属此列——几何只改一次，去抖没有
    /// 可合并的后续帧，而多等的每一毫秒都是 Term（已按新网格渲染）与 ConPTY
    /// （仍是旧几何）不一致的窗口期：这段时间到达的字节按旧宽度进网格，等提交
    /// 时本地 reflow 与 conhost rewrap 各折一套行数，提示符与回显就此错开几行。
    /// 旧壳 `window_context/split.rs` 的 `resize_active_layout()` 在同样的路径
    /// 上同步下发 grid + PTY，这里复刻同一条合同。
    pub(super) fn mark_structural_resize(&mut self, tab_ix: usize, cx: &mut App) {
        let Some(WorkspaceTab::Terminal { panes, .. }) = self.tabs.get(tab_ix) else {
            return;
        };
        let views: Vec<_> = panes.iter().map(|pane| pane.view.clone()).collect();
        for view in views {
            view.update(cx, |view, _| view.mark_structural_resize());
        }
    }

    /// 关一个 pane（pane 退出 / ctrl+shift+w）。树裁定结局：最后一个叶子
    /// 关整个 tab，否则兄弟收编、焦点交给幸存子树首叶。
    pub(super) fn close_pane(
        &mut self,
        tab_ix: usize,
        pane_id: u64,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Some(edit) = self.pane_rename.as_ref()
            && self.tab_of_pane(edit.pane_id) == Some(tab_ix)
            && let Some(WorkspaceTab::Terminal { panes, .. }) = self.tabs.get(tab_ix)
            && panes.iter().any(|pane| pane.id == pane_id)
        {
            if edit.pane_id == pane_id {
                self.pane_rename = None;
            } else if panes.len() == 2 {
                // The survivor's header disappears when the split collapses.
                self.commit_pane_rename(false, window, cx);
            }
        }
        let outcome = match self.tabs.get_mut(tab_ix) {
            Some(WorkspaceTab::Terminal { tree, .. }) => tree.remove_leaf(pane_id),
            _ => return,
        };
        if !matches!(outcome, RemoveOutcome::NotFound) {
            self.runtime_hub.record_pane_closed(self.runtime_window_id, pane_id);
        }
        match outcome {
            RemoveOutcome::NotFound => {},
            RemoveOutcome::WasRoot => self.close_tab(tab_ix, window, cx),
            RemoveOutcome::Collapsed(next_focus) => {
                if let Some(WorkspaceTab::Terminal { panes, focused, zoomed, broadcast, .. }) =
                    self.tabs.get_mut(tab_ix)
                {
                    if let Some(pos) = panes.iter().position(|pane| pane.id == pane_id) {
                        let pane = panes.remove(pos);
                        pane.view.read(cx).shutdown();
                    }
                    if *focused == pane_id {
                        *focused = next_focus;
                    }
                    *zoomed = false;
                    // 收敛到单 pane：广播没有语义了，留着开关状态只会骗人
                    // ——标题条此时也不再绘制，用户根本没有入口关掉它。
                    if panes.len() < 2 {
                        *broadcast = false;
                    }
                }
                self.remote_browser.forget(pane_id);
                self.pane_bounds.borrow_mut().remove(&pane_id);
                self.mark_structural_resize(tab_ix, cx);
                if tab_ix == self.active {
                    if self.pane_rename.is_none() {
                        self.focus_active(window, cx);
                    }
                    self.sync_side_panel_to_active(true, cx);
                }
                cx.notify();
            },
        }
    }

    /// 聚焦另一个 pane（点击上报或方向导航落点）。
    pub(super) fn focus_pane(
        &mut self,
        tab_ix: usize,
        pane_id: u64,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        {
            let Some(WorkspaceTab::Terminal { panes, focused, .. }) = self.tabs.get_mut(tab_ix)
            else {
                return;
            };
            if *focused == pane_id || !panes.iter().any(|pane| pane.id == pane_id) {
                return;
            }
            *focused = pane_id;
        }
        if tab_ix == self.active {
            self.focus_active(window, cx);
            self.sync_side_panel_to_active(true, cx);
        }
        cx.notify();
    }

    /// ctrl+alt+方向：按上一帧 pane 矩形找目标方向最近邻（垂直漂移 4 倍
    /// 惩罚，`nebula_split::nav_target` 与旧壳同式）。缩放态先退出缩放再
    /// 导航，pane 矩形下一帧才有——本次导航按缩放前记录的矩形执行。
    pub(super) fn navigate_pane(
        &mut self,
        nav: SplitNav,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let active = self.active;
        let Some(WorkspaceTab::Terminal { panes, focused, zoomed, .. }) = self.tabs.get_mut(active)
        else {
            return;
        };
        if panes.len() < 2 {
            return;
        }
        *zoomed = false;
        let focused_id = *focused;
        let bounds = self.pane_bounds.borrow();
        let rects: Vec<(u64, nebula_split::Rect)> = panes
            .iter()
            .filter_map(|pane| bounds.get(&pane.id).map(|b| (pane.id, to_split_rect(b))))
            .collect();
        drop(bounds);
        if let Some(target) = nebula_split::nav_target(&rects, focused_id, nav) {
            self.focus_pane(active, target, window, cx);
        } else {
            cx.notify();
        }
    }

    /// ctrl+shift+enter：聚焦 pane 满卡缩放开关（旧壳 ToggleZoom）。
    pub(super) fn toggle_zoom(&mut self, cx: &mut Context<Self>) {
        let active = self.active;
        let toggled = match self.tabs.get_mut(active) {
            Some(WorkspaceTab::Terminal { panes, zoomed, .. }) if panes.len() > 1 => {
                *zoomed = !*zoomed;
                true
            },
            _ => false,
        };
        if toggled {
            // 缩放把一个 pane 的几何从半卡拉到整卡（其余 pane 反之），是一次性
            // 结构变化，同 split/close 走同步下发。
            self.mark_structural_resize(active, cx);
            cx.notify();
        }
    }

    /// 按视图实体反查 (tab 下标, pane id)。

    /// Terminal tab 的内容区：单叶直渲、缩放态聚焦 pane 满卡，否则按
    /// 分屏树递归铺陈。
    pub(super) fn render_terminal_tab(
        &self,
        tab_ix: usize,
        cx: &mut Context<Self>,
    ) -> gpui::AnyElement {
        let Some(WorkspaceTab::Terminal { tree, panes, focused, zoomed, broadcast }) =
            self.tabs.get(tab_ix)
        else {
            return div().into_any_element();
        };
        if *zoomed || tree.is_leaf() {
            let pane = self.tabs[tab_ix].primary_terminal_pane();
            let view = pane.map(|pane| pane.view.clone());
            let probe = pane.map(|pane| {
                let pane_id = pane.id;
                let store = self.pane_bounds.clone();
                // 普通单 Pane 也必须记录窗口坐标；dock 命中与旧壳一样依赖
                // 当前终端区域，不能只在进入分屏递归后才得到边界。
                gpui::canvas(
                    move |bounds, _, _| {
                        store.borrow_mut().insert(pane_id, bounds);
                    },
                    |_, _, _, _| (),
                )
                .absolute()
                .inset_0()
            });
            // 缩放态也要画标题条：否则进了独占就没有退出按钮，只剩快捷键。
            // 满卡时标题条两个上角都贴着卡片圆角。
            let header = pane.filter(|_| pane_header::header_visible(panes.len())).map(|pane| {
                let ordinal = pane_header::pane_order(tree)
                    .iter()
                    .position(|id| *id == pane.id)
                    .map_or(1, |at| at + 1);
                self.render_pane_header(
                    tab_ix,
                    pane.id,
                    ordinal,
                    &pane.view,
                    pane.custom_name.as_deref(),
                    true,
                    *zoomed,
                    *broadcast,
                    pane_header::HeaderCorners { top_left: true, top_right: true },
                    cx,
                )
            });
            return v_flex()
                .size_full()
                .children(header)
                .child(div().flex_1().min_h_0().relative().children(probe).children(view))
                .into_any_element();
        }
        let mut path = Vec::new();
        let order = pane_header::pane_order(tree);
        self.render_split_node(
            tab_ix,
            tree,
            &mut path,
            panes,
            *focused,
            &order,
            *broadcast,
            pane_header::HeaderCorners { top_left: true, top_right: true },
            cx,
        )
    }

    /// 递归渲染分屏树。Split 节点 = flex 容器：first 占 `relative(ratio)`、
    /// 分隔条固定 `DIVIDER_GAP`、second 吃剩余（与 `nebula_split::layout`
    /// 的切割数学等价到 ±divider×ratio ≤ 2px；比例在提交时按真实视口吸附
    /// 整格，不累积漂移）。节点视口与 pane 矩形经 canvas prepaint 回写
    /// 帧记录，供拖拽换算与方向导航消费。
    /// `order` 是本 tab 的 pane 视觉次序（序号徽章的来源）：每层递归都传同一
    /// 份，不在叶子里重新遍历子树——那样每个叶子都会把自己算成 1。
    ///
    /// `corners` 标记这个子树是否还贴着卡片的上两角。GPUI 的 `overflow_hidden`
    /// 只按**矩形**裁剪，不跟圆角，所以带不透明底色的标题条会直接盖住卡片
    /// 圆角（用户 08-23 报的「上面一行越界」）。谁该收角只能由树位置决定，
    /// 在这里逐层往下传。
    #[allow(clippy::too_many_arguments)]
    pub(super) fn render_split_node(
        &self,
        tab_ix: usize,
        node: &SplitTree<u64>,
        path: &mut Vec<bool>,
        panes: &[TerminalPane],
        focused: u64,
        order: &[u64],
        broadcast: bool,
        corners: pane_header::HeaderCorners,
        cx: &mut Context<Self>,
    ) -> gpui::AnyElement {
        match node {
            SplitTree::Leaf(id) => {
                let id = *id;
                let pane = panes.iter().find(|pane| pane.id == id);
                let view = pane.map(|pane| pane.view.clone());
                let store = self.pane_bounds.clone();
                let is_focused = id == focused;
                let dim = gpui::black().opacity(crate::display::NEBULA_UNFOCUSED_SPLIT_DIM);
                let veil = div().absolute().inset_0().bg(dim);
                let ordinal = order.iter().position(|other| *other == id).map_or(1, |at| at + 1);
                let header = pane.map(|pane| {
                    self.render_pane_header(
                        tab_ix,
                        id,
                        ordinal,
                        &pane.view,
                        pane.custom_name.as_deref(),
                        is_focused,
                        false,
                        broadcast,
                        corners,
                        cx,
                    )
                });
                // 布局：标题条固定高 + 终端吃剩余。canvas 探针仍量**整个叶子**
                // ——方向导航与 dock 命中读的是 pane 矩形，不是终端矩形。
                v_flex()
                    .size_full()
                    .relative()
                    .min_w_0()
                    .min_h_0()
                    .child(
                        gpui::canvas(
                            move |bounds, _, _| {
                                store.borrow_mut().insert(id, bounds);
                            },
                            |_, _, _, _| (),
                        )
                        .absolute()
                        .inset_0(),
                    )
                    .children(header)
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .min_h_0()
                            .relative()
                            .children(view)
                            // 与旧壳一致：不用焦点描边，仅给非活动 pane 覆 30%
                            // 黑色 veil。veil 只盖终端区——压暗标题条会把四个
                            // pane 的标题一起糊成灰。
                            .when(
                                !is_focused
                                    && cx
                                        .try_global::<crate::gpui_shell::config::Settings>()
                                        .is_none_or(|settings| settings.dim_inactive_panes),
                                |pane| pane.child(veil),
                            ),
                    )
                    .into_any_element()
            },
            SplitTree::Split { direction, ratio, dragging, first, second, .. } => {
                let direction = *direction;
                // Interactive drag uses a lightweight overlay. Keeping the
                // committed ratio here avoids rebuilding/reflowing both terminal
                // grids for every high-frequency pointer event.
                let show_ratio = *ratio;
                let dragging = *dragging;
                let key = (tab_ix, path.clone());
                let store = self.split_bounds.clone();
                let probe = gpui::canvas(
                    move |bounds, _, _| {
                        store.borrow_mut().insert(key.clone(), bounds);
                    },
                    |_, _, _, _| (),
                )
                .absolute()
                .inset_0();

                path.push(false);
                let first_el = self.render_split_node(
                    tab_ix,
                    first,
                    path,
                    panes,
                    focused,
                    order,
                    broadcast,
                    corners.first_child(direction),
                    cx,
                );
                path.pop();
                path.push(true);
                let second_el = self.render_split_node(
                    tab_ix,
                    second,
                    path,
                    panes,
                    focused,
                    order,
                    broadcast,
                    corners.second_child(direction),
                    cx,
                );
                path.pop();

                let drag_path = path.clone();
                let drag_start_ratio = *ratio;
                let divider_id =
                    SharedString::from(format!("split-divider-{tab_ix}-{drag_path:?}"));
                let bar_color =
                    if dragging { cx.theme().primary } else { cx.theme().border.opacity(0.35) };
                // 视觉线 DIVIDER_GAP，命中区向两侧各扩 HIT_SLOP（旧壳同参）。
                let hit = div()
                    .id(divider_id)
                    .absolute()
                    .map(|hit| match direction {
                        SplitDirection::LeftRight => hit
                            .top_0()
                            .bottom_0()
                            .left(px(-HIT_SLOP))
                            .w(px(DIVIDER_GAP + HIT_SLOP * 2.0))
                            .cursor_col_resize(),
                        SplitDirection::TopBottom => hit
                            .left_0()
                            .right_0()
                            .top(px(-HIT_SLOP))
                            .h(px(DIVIDER_GAP + HIT_SLOP * 2.0))
                            .cursor_row_resize(),
                    })
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |this, _: &MouseDownEvent, _, cx| {
                            this.split_drag = Some(SplitDrag {
                                tab: tab_ix,
                                path: drag_path.clone(),
                                direction,
                                preview_ratio: drag_start_ratio,
                                close_target: None,
                                last_notified: std::time::Instant::now(),
                            });
                            if let Some(WorkspaceTab::Terminal { tree, .. }) =
                                this.tabs.get_mut(tab_ix)
                                && let Some(SplitTree::Split { preview_ratio, dragging, .. }) =
                                    tree.node_mut(&drag_path)
                            {
                                *preview_ratio = None;
                                *dragging = true;
                            }
                            cx.notify();
                        }),
                    );
                let divider = div()
                    .relative()
                    .flex_shrink_0()
                    .map(|bar| match direction {
                        SplitDirection::LeftRight => bar.w(px(DIVIDER_GAP)).h_full(),
                        SplitDirection::TopBottom => bar.h(px(DIVIDER_GAP)).w_full(),
                    })
                    .bg(bar_color)
                    .child(hit);

                div()
                    .size_full()
                    .relative()
                    .flex()
                    .map(|el| match direction {
                        SplitDirection::LeftRight => el.flex_row(),
                        SplitDirection::TopBottom => el.flex_col(),
                    })
                    .child(probe)
                    .child(
                        div()
                            .min_w_0()
                            .min_h_0()
                            .map(|el| match direction {
                                SplitDirection::LeftRight => el.h_full().w(relative(show_ratio)),
                                SplitDirection::TopBottom => el.w_full().h(relative(show_ratio)),
                            })
                            .child(first_el),
                    )
                    .child(divider)
                    .child(div().flex_1().min_w_0().min_h_0().child(second_el))
                    .into_any_element()
            },
        }
    }

    /// 分隔条拖拽的指针移动：指针位置 → 原始比例 → 轻量预览线（常规带跟手、
    /// 关闭区钉边示意"松手即关"）。pane flex、终端网格与 PTY 都不追预览，
    /// 松手后才做一次结构提交。高频鼠标事件最多按 120Hz 通知绘制。
    pub(super) fn update_split_drag(
        &mut self,
        event: &gpui::MouseMoveEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.split_drag.is_none() {
            return;
        }
        if event.pressed_button != Some(MouseButton::Left) {
            let position = event.position;
            self.finish_split_drag(position, window, cx);
            return;
        }
        let raw = match self.split_drag_raw_ratio(event.position, window, cx) {
            Some(raw) => raw,
            None => return,
        };
        let preview_ratio = nebula_split::preview_ratio(raw);
        let close_target = nebula_split::drag_close_target(raw);
        let Some(drag) = self.split_drag.as_mut() else { return };
        let changed = (drag.preview_ratio - preview_ratio).abs() > f32::EPSILON
            || drag.close_target != close_target;
        if !changed {
            return;
        }
        let close_changed = drag.close_target != close_target;
        drag.preview_ratio = preview_ratio;
        drag.close_target = close_target;
        if close_changed || drag.last_notified.elapsed() >= Duration::from_millis(8) {
            drag.last_notified = std::time::Instant::now();
            cx.notify();
        }
    }

    /// Visual-only divider/close preview. It lives above the stable pane tree,
    /// so moving it does not invoke `TerminalElement::prepaint` with new bounds.
    pub(super) fn split_drag_visual(&self, cx: &mut Context<Self>) -> Option<gpui::AnyElement> {
        let drag = self.split_drag.as_ref()?;
        let viewport = self.split_bounds.borrow().get(&(drag.tab, drag.path.clone())).copied()?;
        let visual = split_drag_visual_geometry(
            drag.direction,
            to_split_rect(&viewport),
            drag.preview_ratio,
            drag.close_target,
        );
        let line = visual.divider;
        let close = visual.close_area.map(|area| {
            div()
                .absolute()
                .left(px(area.x))
                .top(px(area.y))
                .w(px(area.w))
                .h(px(area.h))
                .bg(cx.theme().warning.opacity(0.12))
        });
        Some(
            div()
                .absolute()
                .inset_0()
                .children(close)
                .child(
                    div()
                        .absolute()
                        .left(px(line.x))
                        .top(px(line.y))
                        .w(px(line.w))
                        .h(px(line.h))
                        .bg(cx.theme().primary.opacity(0.88)),
                )
                .into_any_element(),
        )
    }

    /// 当前拖拽的指针位置 → 未钳制原始比例（依赖上一帧节点视口记录）。
    pub(super) fn split_drag_raw_ratio(
        &self,
        position: gpui::Point<Pixels>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<f32> {
        let drag = self.split_drag.as_ref()?;
        let viewport = self.split_bounds.borrow().get(&(drag.tab, drag.path.clone())).copied()?;
        let (cell_w, line_h) = TerminalView::cell_metrics(window, cx);
        let cell = match drag.direction {
            SplitDirection::LeftRight => f32::from(cell_w),
            SplitDirection::TopBottom => f32::from(line_h),
        };
        Some(nebula_split::drag_ratio(
            drag.direction,
            to_split_rect(&viewport),
            DIVIDER_GAP,
            cell,
            f32::from(position.x),
            f32::from(position.y),
        ))
    }

    /// 松手：关闭区按被挤压侧收尾（仅当那一侧是单叶——子树不做整树误杀，
    /// 按钳制带提交）；常规带比例吸附整数单元格后写回提交值。
    pub(super) fn finish_split_drag(
        &mut self,
        position: gpui::Point<Pixels>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let raw = self.split_drag_raw_ratio(position, window, cx);
        let Some(drag) = self.split_drag.take() else { return };
        let viewport = self.split_bounds.borrow().get(&(drag.tab, drag.path.clone())).copied();
        let (cell_w, line_h) = TerminalView::cell_metrics(window, cx);
        let cell = match drag.direction {
            SplitDirection::LeftRight => f32::from(cell_w),
            SplitDirection::TopBottom => f32::from(line_h),
        };

        // 先裁定拖拽关闭；借用两段式（close_pane 需要 &mut self）。
        let mut close_target: Option<u64> = None;
        if let (Some(raw), Some(WorkspaceTab::Terminal { tree, .. })) =
            (raw, self.tabs.get_mut(drag.tab))
        {
            if let Some(squeezed_second) = nebula_split::drag_close_target(raw) {
                if let Some(SplitTree::Split { first, second, .. }) = tree.node_mut(&drag.path) {
                    let child = if squeezed_second { second.as_ref() } else { first.as_ref() };
                    if let SplitTree::Leaf(id) = child {
                        close_target = Some(*id);
                    }
                }
            }
        }
        if let Some(pane_id) = close_target {
            // 繁忙 pane 会先弹确认框而不会立刻 remove_leaf；必须先清掉拖拽态，
            // 否则确认框背后会残留一条永久高亮的分隔线。
            if let Some(WorkspaceTab::Terminal { tree, .. }) = self.tabs.get_mut(drag.tab)
                && let Some(SplitTree::Split { preview_ratio, dragging, .. }) =
                    tree.node_mut(&drag.path)
            {
                *preview_ratio = None;
                *dragging = false;
            }
            self.request_close_pane(drag.tab, pane_id, window, cx);
            cx.notify();
            return;
        }

        if let Some(WorkspaceTab::Terminal { tree, .. }) = self.tabs.get_mut(drag.tab) {
            if let Some(SplitTree::Split { ratio, preview_ratio, dragging, .. }) =
                tree.node_mut(&drag.path)
            {
                if let (Some(raw), Some(viewport)) = (raw, viewport) {
                    let vp = to_split_rect(&viewport);
                    let extent = match drag.direction {
                        SplitDirection::LeftRight => vp.w,
                        SplitDirection::TopBottom => vp.h,
                    };
                    *ratio = nebula_split::commit_ratio(
                        nebula_split::preview_ratio(raw),
                        extent,
                        DIVIDER_GAP,
                        cell,
                    );
                }
                *preview_ratio = None;
                *dragging = false;
            }
        }
        cx.notify();
    }
}
