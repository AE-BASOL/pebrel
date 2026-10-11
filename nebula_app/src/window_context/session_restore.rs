//! Legacy workspace import, export, and terminal restore authority.

use super::*;

impl WindowContext {
    /// Export the whole window (`None`) or a single tab (`Some(index)`) as a
    /// workspace file — the same schema the crash-restore session uses, so
    /// "打开工作区" and session restore share one rebuild path.
    pub(super) fn export_workspace(&mut self, tab_index: Option<usize>) {
        let exportable =
            |tab: &&TabEntry| tab.doc.is_none() && tab.image.is_none() && !tab.settings;
        let tabs: Vec<_> = match tab_index {
            Some(index) => self
                .tabs
                .get(index)
                .filter(exportable)
                .map(|tab| self.tab_session(tab))
                .into_iter()
                .collect(),
            None => self.tabs.iter().filter(exportable).map(|tab| self.tab_session(tab)).collect(),
        };
        let Some(first) = tabs.first() else { return };

        // Single-tab exports name the file after the tab; whole-window exports
        // after the workspace. Path separators and friends must not leak into
        // the suggested file name.
        let stem = match tab_index {
            Some(_) => first
                .custom_name
                .clone()
                .or_else(|| {
                    first.cwd.rsplit(['/', '\\']).find(|part| !part.is_empty()).map(str::to_owned)
                })
                .unwrap_or_else(|| "tab".to_owned()),
            None => "workspace".to_owned(),
        };
        let stem: String = stem
            .chars()
            .map(|c| {
                if matches!(c, '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|') {
                    '-'
                } else {
                    c
                }
            })
            .collect();

        let Some(path) =
            self.display.save_workspace_dialog(&format!("{stem}.pebrel-workspace.json"))
        else {
            return;
        };
        let session = session::Session::new(0, tabs);
        if let Err(err) = session::save_to(&path, &session) {
            let user_error = crate::ux::UserFacingError::new(
                "工作区导出失败",
                "无法写入所选的工作区文件。",
                "确认该位置可写(或换一个目录)后重试。",
            )
            .details(err.to_string());
            self.message_buffer.push(crate::message_bar::Message::user_error(&user_error));
            self.dirty = true;
        }
    }

    /// Pick a workspace file and append its tabs — launch identity and split
    /// trees included — to this window, then focus the first imported tab.
    pub(super) fn import_workspace(&mut self) {
        let Some(path) = self.display.pick_workspace_dialog() else { return };
        let Some(session) = session::load_from(&path) else {
            let user_error = crate::ux::UserFacingError::new(
                "工作区导入失败",
                "所选文件不是可识别的 Pebrel 工作区。",
                "确认选择的是导出生成的 .pebrel-workspace.json 或旧版 .nebula-workspace.json 文件。",
            );
            self.message_buffer.push(crate::message_bar::Message::user_error(&user_error));
            self.dirty = true;
            return;
        };

        let before = self.tabs.len();
        for tab in &session.tabs {
            self.append_session_tab(tab);
        }
        let added = self.tabs.len() - before;
        if added > 0 {
            // append_session_tab leaves the last new tab active; land on the
            // first one, matching the saved workspace's reading order.
            self.select_tab(self.active_tab + 1 - added);
            self.sync_chrome_tabs();
            self.mark_session_dirty();
        } else {
            let user_error = crate::ux::UserFacingError::new(
                "工作区导入失败",
                "工作区文件里没有可恢复的标签页。",
                "该文件可能为空,或其中的会话都无法启动。",
            );
            self.message_buffer.push(crate::message_bar::Message::user_error(&user_error));
            self.dirty = true;
        }
    }

    /// Rebuild every saved tab of a restored session and refocus the tab that
    /// was active at close. The boot path only spawned a seed pane so the
    /// window exists; the real tabs — launch identity and split tree included
    /// — are appended here through the same path workspace import uses, then
    /// the seed is dismantled unless the CLI pinned its working directory.
    pub(super) fn restore_session_tabs(&mut self, session: &session::Session, keep_seed: bool) {
        let first_new = self.tabs.len();
        for tab in &session.tabs {
            self.append_session_tab(tab);
        }
        let restored = self.tabs.len() - first_new;
        if restored == 0 {
            // Every spawn failed: keep the seed rather than an empty window.
            return;
        }
        // Guarded: a failed spawn above leaves fewer tabs than were saved.
        if session.active_tab < restored {
            self.active_tab = first_new + session.active_tab;
        }
        if !keep_seed {
            // The seed is a plain single-pane tab at index 0 that never saw
            // user state — dismantle it in place instead of going through
            // close_tab's confirmation and focus logic.
            let seed = self.tabs.remove(0);
            let mut ids = Vec::new();
            seed.layout.leaves(&mut ids);
            for id in ids {
                if let Some(i) = self.pane_index(id) {
                    let pane = self.panes.remove(i);
                    let _ = pane.notifier.0.send(Msg::Shutdown);
                }
            }
            self.active_tab = self.active_tab.saturating_sub(1).min(self.tabs.len() - 1);
        }
        self.resize_active_layout();
        self.sync_chrome_tabs();
        self.dirty = true;
    }

    /// Append one saved tab: spawn its first pane per launch identity, rebuild
    /// its split tree, and re-apply name/color/focus. Returns `false` when the
    /// first pane could not spawn — the tab is skipped so the session's
    /// remaining tabs still restore.
    pub(super) fn append_session_tab(&mut self, tab: &session::TabSession) -> bool {
        let before = self.tabs.len();
        let launch = tab.launch.as_ref().unwrap_or(&session::LaunchSession::Default);
        match launch {
            session::LaunchSession::Default => {
                self.append_default_tab(tab);
            },
            session::LaunchSession::Shell { name, program, args } => {
                let shell = nebula_terminal::tty::Shell::new(program.clone(), args.clone());
                self.spawn_tab_shell(name.clone(), shell, TabPlacement::AfterActive);
            },
            session::LaunchSession::Profile { name, command, args, cwd, shell_id } => {
                self.spawn_tab_profile_value(
                    crate::config::ui_config::Profile {
                        name: name.clone(),
                        command: command.clone(),
                        args: args.clone(),
                        cwd: cwd.as_ref().map(std::path::PathBuf::from),
                        shell_id: shell_id.clone(),
                        terminal_profile_id: None,
                    },
                    TabPlacement::AfterActive,
                );
            },
            session::LaunchSession::Ssh { host } => {
                self.spawn_tab_ssh(host.clone(), TabPlacement::AfterActive)
            },
        }
        if self.tabs.len() == before {
            // Cross-platform degradation: a workspace made on another OS may
            // name a program this machine lacks (wsl.exe on Linux, a distro
            // shell on Windows). The tab must survive as a default shell in
            // its saved directory rather than silently vanish; the SSH path
            // reports its own failure and gets no fallback tab.
            let is_command = matches!(
                launch,
                session::LaunchSession::Shell { .. } | session::LaunchSession::Profile { .. }
            );
            if !(is_command && self.append_default_tab(tab)) {
                return false;
            }
        }

        // The new tab is active now. Grow the saved split tree around its
        // first pane, then re-apply the saved presentation.
        let first_pane = self.tabs[self.active_tab].active_pane;
        if let Some(saved) = tab.layout.as_ref().filter(|layout| layout.pane_count() > 1) {
            let mut seed = Some(first_pane);
            if let Some(built) = self.rebuild_layout(saved, &mut seed) {
                let entry = &mut self.tabs[self.active_tab];
                entry.layout = built;
                let mut leaves = Vec::new();
                entry.layout.leaves(&mut leaves);
                entry.active_pane =
                    leaves.get(tab.active_pane).or(leaves.first()).copied().unwrap_or(first_pane);
                self.resize_active_layout();
            }
        }
        let entry = &mut self.tabs[self.active_tab];
        if tab.custom_name.is_some() {
            // `None` must not clobber the launch-derived label (SSH host,
            // shell name) that spawn_tab_* already set.
            entry.custom_name = tab.custom_name.clone();
        }
        entry.custom_color = tab.color;
        self.resume_agent_sessions(tab);
        true
    }

    /// 冷恢复接续 AI 对话（T1-2）：把保存的叶子与重建后的活动布局按同一
    /// DFS 序配对，给记录了前台对话的 pane 敲入 resume 命令。
    ///
    /// 注入走 ConPTY 输入队列（fastfetch intro 同款机制）：字节安静排队，
    /// shell 就绪后读到第一行输入才执行，不需要等提示符出现。首叶继承 tab
    /// 的 launch 身份，只有确定它是裸 shell（Default / Shell）才注入——
    /// Profile 可能直接启动任意程序（甚至就是 claude，命令会变成发给新
    /// 对话的假消息），SSH 连接期还有密码/口令交互，字节会落进错误的
    /// 输入框；其余叶子由 `rebuild_layout` 统一以默认 shell 重建，安全。
    pub(super) fn resume_agent_sessions(&mut self, tab: &session::TabSession) {
        if !self.display.nebula_resume_ai {
            return;
        }
        let Some(saved) = tab.layout.as_ref() else { return };
        let mut live = Vec::new();
        self.tabs[self.active_tab].layout.leaves(&mut live);
        let launch = tab.launch.as_ref().unwrap_or(&session::LaunchSession::Default);
        let seed_is_plain_shell = matches!(
            launch,
            session::LaunchSession::Default | session::LaunchSession::Shell { .. }
        );
        // 某个叶子 spawn 失败时 rebuild_layout 会折叠父节点，后续配对右移
        // 错位——zip 在较短一侧截止。错位注入最多把 resume 敲进错的兄弟
        // pane，claude/codex 自己会报「找不到会话」，不会破坏任何数据。
        for (index, (leaf, pane_id)) in saved.leaves().iter().zip(live).enumerate() {
            let session::LayoutSession::Pane { agent: Some(agent), .. } = leaf else {
                continue;
            };
            if index == 0 && !seed_is_plain_shell {
                continue;
            }
            let Some(command) = agent.resume_command() else { continue };
            let Some(i) = self.pane_index(pane_id) else { continue };
            let pane = &mut self.panes[i];
            pane.notifier.notify(command.into_bytes());
            pane.notifier.notify(vec![b'\r']);
        }
    }

    /// Spawn a default-shell tab in a saved tab's first-leaf directory —
    /// both the `Default` launch path and the cross-platform fallback when a
    /// saved program does not exist on this machine.
    pub(super) fn append_default_tab(&mut self, tab: &session::TabSession) -> bool {
        // The first pane adopts the tree's first leaf, so it must start in
        // that leaf's directory, not the focused pane's.
        let saved_cwd = tab.layout.as_ref().map(|layout| layout.first_cwd()).unwrap_or(&tab.cwd);
        let cwd = session::valid_dir(saved_cwd).or_else(|| self.display.startup_directory());
        let Some(id) = self.spawn_pane_detached(cwd, self.display.size_info) else {
            return false;
        };
        // 恢复不读新标签插入策略：保存的顺序才是权威，逐个追加在活动标签
        // 之后即可复现它。
        self.insert_tab(
            TabEntry {
                layout: Layout::Leaf(id),
                active_pane: id,
                has_bell: false,
                custom_name: None,
                custom_color: None,
                launch: TabLaunch::Default,
                doc: None,
                image: None,
                settings: false,
            },
            TabPlacement::AfterActive,
        );
        self.run_fastfetch_intro(id);
        true
    }

    /// Materialize a saved split tree. `seed` is the already-spawned first
    /// pane, adopted by the depth-first first leaf; every other leaf spawns a
    /// default shell in its saved cwd — matching live behaviour, where only a
    /// tab's first pane carries the launch identity. A leaf whose spawn fails
    /// collapses its parent to the surviving side, so a partially failed
    /// restore still yields a working tab.
    pub(super) fn rebuild_layout(
        &mut self,
        node: &session::LayoutSession,
        seed: &mut Option<PaneId>,
    ) -> Option<Layout> {
        match node {
            session::LayoutSession::Pane { cwd, .. } => {
                if let Some(id) = seed.take() {
                    return Some(Layout::Leaf(id));
                }
                let cwd = session::valid_dir(cwd).or_else(|| self.display.startup_directory());
                self.spawn_pane_detached(cwd, self.display.size_info).map(Layout::Leaf)
            },
            session::LayoutSession::Split { axis, ratio_permille, first, second } => {
                let first = self.rebuild_layout(first, seed);
                let second = self.rebuild_layout(second, seed);
                match (first, second) {
                    (Some(first), Some(second)) => Some(Layout::Split {
                        direction: match axis {
                            session::SplitAxis::LeftRight => {
                                crate::display::SplitDirection::LeftRight
                            },
                            session::SplitAxis::TopBottom => {
                                crate::display::SplitDirection::TopBottom
                            },
                        },
                        ratio: (f32::from(*ratio_permille) / 1000.0).clamp(0.05, 0.95),
                        preview_ratio: None,
                        dragging: false,
                        first: Box::new(first),
                        second: Box::new(second),
                    }),
                    (first, second) => first.or(second),
                }
            },
        }
    }
}
