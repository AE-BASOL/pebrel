use super::*;
use gpui::{Modifiers, TestAppContext, VisualTestContext, point};

fn open(
    path: std::path::PathBuf,
    cx: &mut TestAppContext,
) -> (Entity<NebulaWorkspace>, VisualTestContext) {
    cx.update(|cx| {
        gpui_component::init(cx);
        crate::gpui_shell::math_view::register(cx);
        crate::gpui_shell::file_editor::init(cx);
        super::super::init(cx);
        cx.set_reduce_motion(true);
        windowing::initialize(cx, crate::runtime_api::RuntimeHub::new());
    });
    let mut workspace = None;
    let (_, mut window) = cx.add_window_view(|window, cx| {
        let view = cx.new(|cx| {
            NebulaWorkspace::new(
                window,
                None,
                None,
                1,
                crate::runtime_api::RuntimeHub::new(),
                windowing::WorkspaceStartup::Empty,
                windowing::WindowRole::Regular,
                cx,
            )
        });
        view.update(cx, |view, cx| {
            view.open_document_path(path, window, cx);
            view.side_panel.open = true;
            cx.notify();
        });
        workspace = Some(view.clone());
        Root::new(view, window, cx)
    });
    window.update(|window, _| window.activate_window());
    window.run_until_parked();
    (workspace.unwrap(), window.clone())
}

#[test]
fn width_limits_keep_room_for_the_document_without_losing_preference() {
    assert_eq!(panel_width(320.0, 1400.0), 320.0);
    assert_eq!(panel_width(1000.0, 1400.0), MAX_WIDTH);
    assert_eq!(panel_width(-1.0, 1400.0), MIN_WIDTH);
    assert_eq!(panel_width(500.0, 400.0), 184.0);
    assert_eq!(panel_width(500.0, 1400.0), 500.0);
}

#[gpui::test]
fn switching_to_instant_removes_closed_panel_geometry(cx: &mut TestAppContext) {
    use crate::gpui_shell::config::Settings;
    use nebula_settings::TabRevealName;

    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("instant.md");
    std::fs::write(&path, "# Heading\n\nBody").unwrap();
    let (workspace, mut cx) = open(path, cx);
    cx.update(|_, cx| {
        cx.set_global(Settings::load(nebula_settings::ThemeName::Nord));
        // 必须保留真实动画路径，否则系统减弱动态效果会掩盖偏好切换的问题。
        cx.set_reduce_motion(false);
    });
    for close_before_switch in [false, true] {
        workspace.update(&mut cx, |view, cx| {
            cx.global_mut::<Settings>().tab_reveal = TabRevealName::Slide;
            view.toggle_side_panel(view.side_panel.view, cx);
            view.toggle_side_panel(view.side_panel.view, cx);
            if close_before_switch {
                view.toggle_side_panel(view.side_panel.view, cx);
            }
            assert!(view.side_panel_anim_armed);
        });
        cx.update(|window, cx| window.draw(cx).clear(cx));
        cx.update(|window, cx| {
            cx.global_mut::<Settings>().tab_reveal = TabRevealName::Instant;
            window.refresh();
            window.draw(cx).clear(cx);
        });
        if !close_before_switch {
            assert!(cx.debug_bounds("workspace-details-slot").unwrap().size.width > px(0.0));
            let close = cx.debug_bounds("toggle-right-sidebar").unwrap();
            cx.simulate_click(close.center(), Modifiers::default());
            cx.update(|window, cx| window.draw(cx).clear(cx));
        }
        assert!(cx.debug_bounds("workspace-details-slot").is_none());
        workspace.read_with(&cx, |view, _| {
            assert!(!view.side_panel.open);
            assert!(!view.side_panel_anim_armed);
        });
        workspace.update(&mut cx, |view, cx| view.toggle_document_details(cx));
        cx.update(|window, cx| window.draw(cx).clear(cx));
        assert!(cx.debug_bounds("workspace-details-slot").unwrap().size.width > px(0.0));
    }
    cx.update(|window, cx| {
        workspace.update(cx, |view, cx| view.open_settings(window, cx));
        window.draw(cx).clear(cx);
    });
    assert!(cx.debug_bounds("workspace-details-slot").is_none());
    cx.update(|window, cx| {
        workspace.update(cx, |view, cx| view.leave_settings(window, cx));
        window.draw(cx).clear(cx);
    });
    assert!(cx.debug_bounds("workspace-details-slot").unwrap().size.width > px(0.0));
}

#[gpui::test]
fn file_path_edit_navigates_and_keeps_the_last_directory_on_error_or_escape(
    cx: &mut TestAppContext,
) {
    let select_all = match crate::platform::Platform::current() {
        crate::platform::Platform::MacOS => "cmd-a",
        _ => "ctrl-a",
    };
    let fixture_root = tempfile::tempdir().unwrap();
    let fixture_parent = tempfile::tempdir_in(fixture_root.path()).unwrap();
    // 系统临时目录的深度因平台而异；主动嵌套才能稳定覆盖三级面包屑的祖先菜单。
    let directory = tempfile::tempdir_in(fixture_parent.path()).unwrap();
    let path = directory.path().join("sidebar.md");
    let target = directory.path().join("中文 folder");
    std::fs::write(&path, "# Heading").unwrap();
    std::fs::create_dir(&target).unwrap();
    let process_cwd = std::env::current_dir().unwrap();
    let (workspace, mut cx) = open(path, cx);
    workspace.update(&mut cx, |view, cx| {
        view.details_panel.section = None;
        view.side_panel.view = PanelView::Files;
        view.side_panel.set_custom_root(directory.path().to_path_buf());
        cx.notify();
    });
    cx.run_until_parked();
    cx.update(|window, cx| window.draw(cx).clear(cx));
    let path_button = cx.debug_bounds("file-tree-path").unwrap();
    let breadcrumbs = cx.debug_bounds("file-tree-breadcrumbs").unwrap();
    assert!(
        path_button.right() <= breadcrumbs.right() + px(0.5),
        "current directory must stay inside the clickable breadcrumb bar"
    );
    cx.simulate_click(path_button.center(), Modifiers::default());
    cx.simulate_keystrokes(select_all);
    cx.simulate_input("中文 folder");
    cx.simulate_keystrokes("enter");
    cx.run_until_parked();
    workspace.read_with(&cx, |view, _| {
        assert!(view.file_tree_path.is_none());
        assert_eq!(
            view.side_panel.root().unwrap().canonicalize().unwrap(),
            target.canonicalize().unwrap()
        );
    });
    assert_eq!(std::env::current_dir().unwrap(), process_cwd);
    cx.update(|window, cx| window.draw(cx).clear(cx));
    let empty = cx.debug_bounds("file-tree-empty").unwrap();
    let message = cx.debug_bounds("file-tree-empty-message").unwrap();
    assert!((f32::from(empty.center().x - message.center().x)).abs() <= 1.0);
    assert!((f32::from(empty.center().y - message.center().y)).abs() <= 1.0);
    let path_button = cx.debug_bounds("file-tree-path").unwrap();
    cx.simulate_click(path_button.center(), Modifiers::default());
    cx.update(|window, cx| window.draw(cx).clear(cx));
    assert!(cx.debug_bounds("file-tree-path-editor").is_some());
    cx.simulate_keystrokes("enter");
    cx.run_until_parked();
    workspace.read_with(&cx, |view, _| {
        assert!(
            view.file_tree_path.is_none(),
            "submitting the current directory must succeed: {:?}",
            view.file_tree_path_error()
        );
    });
    cx.update(|window, cx| window.draw(cx).clear(cx));
    let path_button = cx.debug_bounds("file-tree-path").unwrap();
    cx.simulate_click(path_button.center(), Modifiers::default());
    cx.simulate_keystrokes(select_all);
    cx.simulate_input("missing-directory");
    cx.simulate_keystrokes("enter");
    cx.run_until_parked();
    workspace.read_with(&cx, |view, _| {
        assert!(view.file_tree_path_error().is_some());
        assert_eq!(
            view.side_panel.root().unwrap().canonicalize().unwrap(),
            target.canonicalize().unwrap()
        );
    });
    cx.simulate_keystrokes("escape");
    cx.run_until_parked();
    workspace.read_with(&cx, |view, _| assert!(view.file_tree_path.is_none()));
    cx.update(|window, cx| window.draw(cx).clear(cx));
    let parent = cx.debug_bounds("file-tree-crumb-parent").unwrap();
    cx.simulate_click(parent.center(), Modifiers::default());
    cx.run_until_parked();
    workspace.read_with(&cx, |view, _| {
        assert_eq!(
            view.side_panel.root().unwrap().canonicalize().unwrap(),
            directory.path().canonicalize().unwrap()
        );
    });
    cx.update(|window, cx| window.draw(cx).clear(cx));
    let overflow = cx.debug_bounds("file-tree-crumb-overflow").unwrap();
    assert!(
        cx.debug_bounds("file-tree-crumb-0").is_none(),
        "earlier levels belong in the ancestor menu"
    );
    cx.simulate_click(overflow.center(), Modifiers::default());
    cx.run_until_parked();
    cx.simulate_keystrokes("down enter");
    cx.run_until_parked();
    workspace.read_with(&cx, |view, _| {
        let root = view.side_panel.root().unwrap().canonicalize().unwrap();
        let directory = directory.path().canonicalize().unwrap();
        assert_ne!(root, directory, "selecting an ancestor must navigate");
        assert!(directory.starts_with(root));
    });
}

#[gpui::test]
fn shared_sidebar_drag_changes_actual_geometry_and_escape_cancels(cx: &mut TestAppContext) {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("sidebar.md");
    std::fs::write(&path, "# Heading\n\nBody").unwrap();
    let (workspace, mut cx) = open(path, cx);
    cx.update(|window, cx| {
        let _ = window.draw(cx);
    });
    let before = cx.debug_bounds("workspace-details-slot").unwrap();
    let handle = cx.debug_bounds("workspace-details-resize").unwrap();
    assert!(handle.size.width >= px(8.0));
    assert_eq!(handle.top(), px(0.0), "resize must include the titlebar");
    let start = handle.center();
    let target = point(start.x - px(40.0), start.y);
    cx.simulate_mouse_down(start, MouseButton::Left, Modifiers::default());
    cx.simulate_mouse_move(target, Some(MouseButton::Left), Modifiers::default());
    cx.run_until_parked();
    cx.update(|window, cx| {
        let _ = window.draw(cx);
    });
    let after = cx.debug_bounds("workspace-details-slot").unwrap();
    assert!(after.size.width > before.size.width, "{before:?} -> {after:?}");
    cx.simulate_mouse_up(target, MouseButton::Left, Modifiers::default());
    cx.run_until_parked();
    assert!(workspace.read_with(&cx, |view, _| view.details_panel.resize.is_none()));
    cx.update(|window, cx| {
        assert!(window.selected_text(cx).is_empty());
        let _ = window.draw(cx);
    });
    let start = cx.debug_bounds("workspace-details-resize").unwrap().center();
    cx.simulate_mouse_down(start, MouseButton::Left, Modifiers::default());
    cx.simulate_keystrokes("escape");
    cx.run_until_parked();
    assert!(workspace.read_with(&cx, |view, _| view.details_panel.resize.is_none()));
}

#[gpui::test]
fn sidebar_tabs_keep_square_hit_targets_and_files_precedes_git(cx: &mut TestAppContext) {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("controls.md");
    std::fs::write(&path, "# Heading").unwrap();
    let (workspace, mut cx) = open(path, cx);
    cx.update(|window, cx| {
        let _ = window.draw(cx);
    });
    for id in ["details-info", "side-panel-files", "side-panel-git"] {
        let bounds = cx.debug_bounds(id).unwrap();
        assert_eq!(bounds.size.width, px(32.0), "{id}: {bounds:?}");
        assert_eq!(bounds.size.height, px(32.0), "{id}: {bounds:?}");
    }
    assert!(cx.debug_bounds("workspace-details-close").is_none());
    let files = cx.debug_bounds("side-panel-files").unwrap();
    assert!(files.right() <= cx.debug_bounds("side-panel-git").unwrap().left());
    let corner = point(files.left() + px(3.0), files.top() + px(3.0));
    cx.simulate_mouse_down(corner, MouseButton::Left, Modifiers::default());
    cx.simulate_mouse_up(corner, MouseButton::Left, Modifiers::default());
    cx.run_until_parked();
    cx.update(|window, cx| {
        let _ = window.draw(cx);
    });
    assert!(workspace.read_with(&cx, |view, cx| view.active_document_section(cx).is_none()));
    assert_eq!(cx.debug_bounds("details-outline").unwrap().size.width, px(32.0));
    assert!(cx.debug_bounds("side-panel-files").unwrap().size.width > px(32.0));
    // 真实打开、悬停和反复切换过滤按钮，覆盖组件内外重复注册 hover 的崩溃路径。
    for id in ["file-search-match-case", "file-search-whole-word", "file-search-regex"] {
        let enabled = |view: &NebulaWorkspace| {
            let options = view.side_panel.file_search_options();
            match id {
                "file-search-match-case" => options.match_case,
                "file-search-whole-word" => options.whole_word,
                _ => options.regex,
            }
        };
        let initial = workspace.read_with(&cx, |view, _| enabled(view));
        let button = cx.debug_bounds(id).unwrap();
        cx.simulate_mouse_move(button.center(), None, Modifiers::default());
        cx.update(|window, cx| window.draw(cx).clear(cx));
        cx.simulate_click(button.center(), Modifiers::default());
        cx.run_until_parked();
        assert_ne!(workspace.read_with(&cx, |view, _| enabled(view)), initial);
        cx.simulate_click(button.center(), Modifiers::default());
        cx.run_until_parked();
        assert_eq!(workspace.read_with(&cx, |view, _| enabled(view)), initial);
    }
    let selected = cx.debug_bounds("side-panel-files").unwrap().center();
    cx.simulate_mouse_down(selected, MouseButton::Left, Modifiers::default());
    cx.simulate_mouse_up(selected, MouseButton::Left, Modifiers::default());
    cx.run_until_parked();
    assert!(!workspace.read_with(&cx, |view, _| view.side_panel.open));
}

#[gpui::test]
fn right_sidebar_stops_at_minimum_then_closes_on_release_and_reopens_at_previous_width(
    cx: &mut TestAppContext,
) {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("detent.md");
    std::fs::write(&path, "# Heading").unwrap();
    let (workspace, mut cx) = open(path, cx);
    cx.update(|window, cx| {
        let _ = window.draw(cx);
    });
    let original = workspace.read_with(&cx, |view, _| view.details_panel.width);
    let start = cx.debug_bounds("workspace-details-resize").unwrap().center();
    cx.simulate_mouse_down(start, MouseButton::Left, Modifiers::default());
    let minimum = point(start.x + px(original - MIN_WIDTH + 18.0), start.y);
    cx.simulate_mouse_move(minimum, Some(MouseButton::Left), Modifiers::default());
    cx.run_until_parked();
    assert_eq!(workspace.read_with(&cx, |view, _| view.details_panel.width), MIN_WIDTH);
    assert!(workspace.read_with(&cx, |view, _| view.side_panel.open));
    let close = point(start.x + px(original - MIN_WIDTH + 100.0), start.y);
    cx.simulate_mouse_move(close, Some(MouseButton::Left), Modifiers::default());
    cx.run_until_parked();
    assert!(workspace.read_with(&cx, |view, _| view.side_panel.open));
    cx.simulate_mouse_up(close, MouseButton::Left, Modifiers::default());
    cx.run_until_parked();
    assert!(!workspace.read_with(&cx, |view, _| view.side_panel.open));
    workspace.update(&mut cx, |view, cx| view.toggle_document_details(cx));
    assert_eq!(workspace.read_with(&cx, |view, _| view.details_panel.width), original);
}

#[gpui::test]
fn left_sidebar_close_and_escape_share_the_right_sidebar_detent(cx: &mut TestAppContext) {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("left.md");
    std::fs::write(&path, "# Heading").unwrap();
    let (workspace, mut cx) = open(path, cx);
    let original = workspace.read_with(&cx, |view, _| view.sidebar_width);
    cx.update(|window, cx| {
        workspace.update(cx, |view, cx| {
            view.sidebar_collapsed = false;
            view.sidebar_resizing =
                Some(super::super::sidebar_resize::ResizeDrag::new(original, original));
            cx.notify();
        });
        let _ = window.draw(cx);
    });
    let close = point(px(nebula_settings::MIN_SIDEBAR_WIDTH - 100.0), px(200.0));
    cx.simulate_mouse_move(close, Some(MouseButton::Left), Modifiers::default());
    cx.run_until_parked();
    cx.simulate_keystrokes("escape");
    cx.run_until_parked();
    assert_eq!(workspace.read_with(&cx, |view, _| view.sidebar_width), original);
    assert!(!workspace.read_with(&cx, |view, _| view.sidebar_collapsed));
    cx.update(|window, cx| {
        workspace.update(cx, |view, cx| {
            view.sidebar_resizing =
                Some(super::super::sidebar_resize::ResizeDrag::new(original, original));
            cx.notify();
        });
        let _ = window.draw(cx);
    });
    cx.simulate_mouse_move(close, Some(MouseButton::Left), Modifiers::default());
    cx.run_until_parked();
    cx.simulate_mouse_up(close, MouseButton::Left, Modifiers::default());
    cx.run_until_parked();
    assert!(workspace.read_with(&cx, |view, _| view.sidebar_collapsed));
    assert_eq!(workspace.read_with(&cx, |view, _| view.sidebar_width), original);
}

// Use the rendered handles rather than injecting ResizeDrag: this exercises the
// press-to-capture path as well as the release that hitbox listeners cannot see.
fn prepare_sidebar_resize(
    workspace: &Entity<NebulaWorkspace>,
    cx: &mut VisualTestContext,
    left: bool,
) -> (gpui::Point<gpui::Pixels>, gpui::Point<gpui::Pixels>) {
    cx.update(|window, cx| {
        let mut settings =
            crate::gpui_shell::config::Settings::load(nebula_settings::ThemeName::Nord);
        settings.panel_resize = true;
        cx.set_global(settings);
        workspace.update(cx, |view, cx| {
            view.tabs_position = nebula_settings::TabsPositionName::Sidebar;
            view.sidebar_collapsed = false;
            view.sidebar_width = 320.0;
            view.side_panel.open = true;
            view.details_panel.width = 320.0;
            cx.notify();
        });
        window.activate_window();
        window.draw(cx).clear(cx);
    });
    let handle = if left { "sidebar-resize-handle" } else { "workspace-details-resize" };
    let start = cx.debug_bounds(handle).unwrap().center();
    let outside = cx.update(|window, _| {
        let x = if left { px(-20.0) } else { window.viewport_size().width + px(20.0) };
        point(x, start.y)
    });
    (start, outside)
}

fn sidebar_resize_state(
    workspace: &Entity<NebulaWorkspace>,
    cx: &VisualTestContext,
    left: bool,
) -> (bool, f32, Option<bool>) {
    workspace.read_with(cx, |view, _| {
        if left {
            (
                !view.sidebar_collapsed,
                view.sidebar_width,
                view.sidebar_resizing.map(|drag| drag.close),
            )
        } else {
            (
                view.side_panel.open,
                view.details_panel.width,
                view.details_panel.resize.map(|drag| drag.close),
            )
        }
    })
}

fn sidebar_drag_to_width(
    cx: &mut VisualTestContext,
    start: gpui::Point<gpui::Pixels>,
    left: bool,
    width: f32,
) {
    let delta = if left { width - 320.0 } else { 320.0 - width };
    cx.simulate_mouse_move(
        point(start.x + px(delta), start.y),
        Some(MouseButton::Left),
        Modifiers::default(),
    );
    cx.run_until_parked();
    cx.update(|window, cx| window.draw(cx).clear(cx));
}

#[gpui::test]
fn sidebar_resize_outside_release_closes_both_panels(cx: &mut TestAppContext) {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("outside-release.md");
    std::fs::write(&path, "# Heading").unwrap();
    let (workspace, mut cx) = open(path, cx);
    for left in [true, false] {
        let (start, outside) = prepare_sidebar_resize(&workspace, &mut cx, left);
        cx.simulate_mouse_down(start, MouseButton::Left, Modifiers::default());
        let minimum = if left { nebula_settings::MIN_SIDEBAR_WIDTH } else { MIN_WIDTH };
        sidebar_drag_to_width(&mut cx, start, left, minimum - 100.0);
        assert_eq!(sidebar_resize_state(&workspace, &cx, left).2, Some(true));
        cx.simulate_mouse_up(outside, MouseButton::Left, Modifiers::default());
        cx.run_until_parked();
        assert_eq!(sidebar_resize_state(&workspace, &cx, left), (false, 320.0, None));
        cx.simulate_mouse_up(outside, MouseButton::Left, Modifiers::default());
        assert!(
            !sidebar_resize_state(&workspace, &cx, left).0,
            "duplicate release must not reopen"
        );
        workspace.update(&mut cx, |view, cx| {
            if left {
                view.sidebar_collapsed = false;
                cx.notify();
            } else {
                view.toggle_document_details(cx);
            }
        });
        cx.update(|window, cx| window.draw(cx).clear(cx));
        assert_eq!(sidebar_resize_state(&workspace, &cx, left), (true, 320.0, None));
    }
}

#[gpui::test]
fn sidebar_resize_nc_move_waits_for_real_release(cx: &mut TestAppContext) {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("nc-move.md");
    std::fs::write(&path, "# Heading").unwrap();
    let (workspace, mut cx) = open(path, cx);
    for left in [true, false] {
        let (start, outside) = prepare_sidebar_resize(&workspace, &mut cx, left);
        cx.simulate_mouse_down(start, MouseButton::Left, Modifiers::default());
        let minimum = if left { nebula_settings::MIN_SIDEBAR_WIDTH } else { MIN_WIDTH };
        sidebar_drag_to_width(&mut cx, start, left, minimum - 100.0);
        let before = sidebar_resize_state(&workspace, &cx, left);
        assert_eq!(before.2, Some(true));
        // Windows WM_NCMOUSEMOVE reports no pressed button even during capture.
        cx.simulate_mouse_move(point(start.x, start.y), None, Modifiers::default());
        cx.run_until_parked();
        assert_eq!(sidebar_resize_state(&workspace, &cx, left), before);
        cx.simulate_mouse_move(outside, None, Modifiers::default());
        cx.simulate_mouse_up(outside, MouseButton::Right, Modifiers::default());
        assert_eq!(sidebar_resize_state(&workspace, &cx, left), before);
        cx.simulate_mouse_up(outside, MouseButton::Left, Modifiers::default());
        cx.run_until_parked();
        assert_eq!(sidebar_resize_state(&workspace, &cx, left), (false, 320.0, None));
    }
}

#[gpui::test]
fn sidebar_resize_outside_release_keeps_width_and_respects_reversal(cx: &mut TestAppContext) {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("outside-width.md");
    std::fs::write(&path, "# Heading").unwrap();
    let (workspace, mut cx) = open(path, cx);
    for left in [true, false] {
        let (start, outside) = prepare_sidebar_resize(&workspace, &mut cx, left);
        cx.simulate_mouse_down(start, MouseButton::Left, Modifiers::default());
        sidebar_drag_to_width(&mut cx, start, left, 360.0);
        cx.simulate_mouse_up(outside, MouseButton::Left, Modifiers::default());
        cx.run_until_parked();
        assert_eq!(sidebar_resize_state(&workspace, &cx, left), (true, 360.0, None));

        let (start, outside) = prepare_sidebar_resize(&workspace, &mut cx, left);
        cx.simulate_mouse_down(start, MouseButton::Left, Modifiers::default());
        let minimum = if left { nebula_settings::MIN_SIDEBAR_WIDTH } else { MIN_WIDTH };
        sidebar_drag_to_width(&mut cx, start, left, minimum - 100.0);
        assert_eq!(sidebar_resize_state(&workspace, &cx, left).2, Some(true));
        sidebar_drag_to_width(&mut cx, start, left, minimum + 20.0);
        assert_eq!(sidebar_resize_state(&workspace, &cx, left).2, Some(false));
        cx.simulate_mouse_up(outside, MouseButton::Left, Modifiers::default());
        cx.run_until_parked();
        assert_eq!(sidebar_resize_state(&workspace, &cx, left), (true, minimum + 20.0, None));
    }
}

#[gpui::test]
fn sidebar_resize_cancel_ignores_late_outside_release(cx: &mut TestAppContext) {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("outside-cancel.md");
    std::fs::write(&path, "# Heading").unwrap();
    let (workspace, mut cx) = open(path, cx);
    for left in [true, false] {
        for deactivate in [false, true] {
            let (start, outside) = prepare_sidebar_resize(&workspace, &mut cx, left);
            cx.simulate_mouse_down(start, MouseButton::Left, Modifiers::default());
            let minimum = if left { nebula_settings::MIN_SIDEBAR_WIDTH } else { MIN_WIDTH };
            sidebar_drag_to_width(&mut cx, start, left, minimum - 100.0);
            assert_eq!(sidebar_resize_state(&workspace, &cx, left).2, Some(true));
            if deactivate {
                cx.deactivate_window();
            } else {
                cx.simulate_keystrokes("escape");
            }
            cx.run_until_parked();
            assert_eq!(sidebar_resize_state(&workspace, &cx, left), (true, 320.0, None));
            cx.simulate_mouse_up(outside, MouseButton::Left, Modifiers::default());
            cx.run_until_parked();
            assert_eq!(sidebar_resize_state(&workspace, &cx, left), (true, 320.0, None));
        }
    }
}

#[gpui::test]
fn sidebar_resize_outside_release_before_next_paint_clears_drag(cx: &mut TestAppContext) {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("outside-before-paint.md");
    std::fs::write(&path, "# Heading").unwrap();
    let (workspace, mut cx) = open(path, cx);
    for left in [true, false] {
        let (start, outside) = prepare_sidebar_resize(&workspace, &mut cx, left);
        // Dispatch both events in the same update, with no opportunity to paint
        // an active-drag overlay or install listeners after the press.
        cx.update(|window, cx| {
            window.dispatch_event(
                gpui::PlatformInput::MouseDown(gpui::MouseDownEvent {
                    position: start,
                    button: MouseButton::Left,
                    modifiers: Modifiers::default(),
                    click_count: 1,
                    first_mouse: false,
                }),
                cx,
            );
            assert!(workspace.read(cx).sidebar_resizing.is_some() == left);
            assert!(workspace.read(cx).details_panel.resize.is_some() != left);
            window.dispatch_event(
                gpui::PlatformInput::MouseUp(gpui::MouseUpEvent {
                    position: outside,
                    button: MouseButton::Left,
                    modifiers: Modifiers::default(),
                    click_count: 1,
                }),
                cx,
            );
        });
        assert_eq!(sidebar_resize_state(&workspace, &cx, left), (true, 320.0, None));
    }
}

#[gpui::test]
fn document_outline_and_files_share_one_slot_and_one_width(cx: &mut TestAppContext) {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("tabs.md");
    std::fs::write(&path, "# Heading\n\nBody").unwrap();
    let (workspace, mut cx) = open(path, cx);
    cx.update(|window, cx| {
        workspace.update(cx, |view, cx| {
            view.details_panel.width = 380.0;
            view.select_document_section(DocumentSection::Outline, cx);
        });
        let _ = window.draw(cx);
    });
    let outline = cx.debug_bounds("markdown-outline-list").unwrap();
    let slot = cx.debug_bounds("workspace-details-slot").unwrap();
    assert!(outline.left() >= slot.left());
    let width = slot.size.width;
    cx.update(|window, cx| {
        workspace.update(cx, |view, cx| view.toggle_file_tree(cx));
        let _ = window.draw(cx);
    });
    assert!(workspace.read_with(&cx, |view, cx| view.active_document_section(cx).is_none()));
    assert!(cx.debug_bounds("markdown-outline-list").is_none());
    assert_eq!(cx.debug_bounds("workspace-details-slot").unwrap().size.width, width);
}
