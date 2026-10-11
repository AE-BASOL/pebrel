use super::*;
use crate::display::side_panel::{
    GitLocation,
    diff::{
        DiffKey,
        tests::{commit, repository},
    },
};
use crate::gpui_shell::code_tab::CodeTabView;
use crate::gpui_shell::copy_feedback::COPY_FEEDBACK_TTL;
use std::io::Write as _;

const DISABLED_SETTINGS: &str = "ai_hooks ai_hooks_claude ai_hooks_codex ai_hooks_opencode ai_hooks_pi \
     ai_hooks_copilot ai_hooks_grok ai_hooks_omp ai_hooks_cursor ai_hooks_kimi \
     auto_check_updates auto_download_updates restore_session resume_ai refresh_environment";

#[test]
fn isolated_git_diff_controls() {
    for test in [
        "git_patch_click_enter_copy_refresh_and_double_click_keep_real_file_identity",
        "git_patch_row_uses_its_rendered_repository_even_if_followed_cwd_changes",
        "git_patch_empty_binary_and_failure_have_visible_readonly_controls",
        "git_patch_large_content_virtualizes_and_bounds_single_line_shaping",
        "git_patch_full_workspace_save_route_preserves_the_neighbor_draft_and_releases_feedback",
    ] {
        let directory = tempfile::tempdir().unwrap();
        std::fs::create_dir(directory.path().join(".ssh")).unwrap();
        std::fs::write(directory.path().join(".ssh/config"), "Host git-diff-fixture\n").unwrap();
        let settings = DISABLED_SETTINGS
            .split_whitespace()
            .map(|key| format!("{key}=0\n"))
            .collect::<String>();
        std::fs::write(directory.path().join("pebrel_settings.txt"), settings).unwrap();
        let git_configuration =
            crate::display::side_panel::diff::test_fixture::GitTestConfig::new();
        let mut child = std::process::Command::new(std::env::current_exe().unwrap());
        git_configuration.configure(&mut child);
        let output = child
            .arg("--exact")
            .arg(format!("gpui_shell::workspace::vcs_panel::tree_tests::diff_tests::{test}"))
            .args(["--ignored", "--nocapture"])
            .env_remove("PEBREL_CONFIG_FILE")
            .env_remove("NEBULA_CONFIG_FILE")
            .env_remove("PEBREL_GPUI_CONFIG")
            .env_remove("NEBULA_GPUI_CONFIG")
            .env_remove("SSH_AUTH_SOCK")
            .env("PEBREL_TEST_SSH_HOME", directory.path())
            .env("PEBREL_CONFIG_DIR", directory.path())
            .env("PEBREL_GIT_DIFF_QA_DIR", directory.path())
            .output()
            .unwrap();
        let evidence = format!(
            "{}\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        writeln!(std::io::stdout(), "{test}: {evidence}").unwrap();
        assert!(
            output.status.success()
                && evidence.contains("running 1 test")
                && evidence.contains("1 passed;"),
            "{test}: {evidence}"
        );
    }
}

fn assert_isolation() {
    let directory = PathBuf::from(std::env::var_os("PEBREL_GIT_DIFF_QA_DIR").unwrap());
    assert_eq!(Some(directory.as_os_str()), std::env::var_os("PEBREL_CONFIG_DIR").as_deref());
    assert_eq!(crate::ssh::ssh_config_path(), Some(directory.join(".ssh/config")));
    assert_eq!(crate::ssh::ssh_config_hosts(), ["git-diff-fixture"]);
    let raw = nebula_settings::RawSettings::try_load().unwrap();
    assert!(DISABLED_SETTINGS.split_whitespace().all(|key| raw.bool_on(key) == Some(false)));
}

struct DiffProbe {
    workspace: Entity<NebulaWorkspace>,
}

impl Render for DiffProbe {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.workspace.update(cx, |workspace, cx| {
            let content = match workspace.tabs.get(workspace.active) {
                Some(WorkspaceTab::Code { view, .. }) => div().size_full().child(view.clone()),
                _ => div().size_full(),
            };
            h_flex()
                .w(px(900.0))
                .h(px(420.0))
                .overflow_hidden()
                .child(
                    v_flex()
                        .w(px(300.0))
                        .h_full()
                        .flex_shrink_0()
                        .child(workspace.render_vcs_layout_toggle(cx))
                        .child(workspace.render_vcs_list(cx)),
                )
                .child(div().flex_1().min_w_0().h_full().child(content))
        })
    }
}

fn initialize(cx: &mut TestAppContext, configure: impl FnOnce(&mut gpui::App)) {
    cx.update(|cx| {
        gpui_component::init(cx);
        crate::gpui_shell::math_view::register(cx);
        crate::gpui_shell::file_editor::init(cx);
        crate::gpui_shell::init(cx, None);
        configure(cx);
        windowing::initialize(cx, crate::runtime_api::RuntimeHub::new());
    });
}

fn new_workspace(window: &mut Window, cx: &mut Context<NebulaWorkspace>) -> NebulaWorkspace {
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
}

fn window<'a>(
    cx: &'a mut TestAppContext,
    root: &Path,
    data: GitInfo,
) -> (Entity<NebulaWorkspace>, &'a mut VisualTestContext) {
    assert_isolation();
    initialize(cx, |_| {});
    let mut owner = None;
    let (_, cx) = cx.add_window_view(|window, cx| {
        let workspace = cx.new(|cx| new_workspace(window, cx));
        workspace.update(cx, |workspace, _| {
            workspace.side_panel.toggle(PanelView::Git);
            workspace.vcs_list.sync(Some(Arc::new(data)), GitPanelView::Changes, Some(root.into()));
        });
        owner = Some(workspace.clone());
        let probe = cx.new(|_| DiffProbe { workspace });
        Root::new(probe, window, cx)
    });
    let owner = owner.unwrap();
    settle_panel(&owner, root, cx);
    cx.update(|window, cx| {
        let _ = window.draw(cx);
    });
    (owner, cx)
}

fn diff(
    owner: &Entity<NebulaWorkspace>,
    root: &Path,
    path: &str,
    staged: bool,
    cx: &mut VisualTestContext,
) -> Entity<CodeTabView> {
    let key =
        DiffKey { location: GitLocation::Local { root: root.into() }, path: path.into(), staged };
    owner
        .read_with(cx, |workspace, cx| {
            workspace.tabs.iter().find_map(|tab| match tab {
                WorkspaceTab::Code { view, .. } if view.read(cx).matches_git_diff(&key, cx) => {
                    Some(view.clone())
                },
                _ => None,
            })
        })
        .expect("the real Git row must create the captured patch tab")
}

fn settle(view: &Entity<CodeTabView>, cx: &mut VisualTestContext) -> String {
    for _ in 0..1000 {
        cx.run_until_parked();
        if let Some(text) = view.read_with(cx, |view, cx| view.diff_text(cx)) {
            return text;
        }
        if let Some(error) = view.read_with(cx, |view, cx| view.diff_error(cx)) {
            panic!("{error}");
        }
        std::thread::sleep(Duration::from_millis(2));
    }
    panic!("synthetic patch did not settle");
}

fn draw(cx: &mut VisualTestContext) {
    cx.update(|window, cx| {
        let _ = window.draw(cx);
    });
}

#[gpui::test]
#[ignore = "launched by isolated_git_diff_controls with owned configuration and SSH home"]
fn git_patch_click_enter_copy_refresh_and_double_click_keep_real_file_identity(
    cx: &mut TestAppContext,
) {
    let directory = repository();
    let root = directory.path();
    std::fs::create_dir(root.join("module")).unwrap();
    let path = "module/文件.rs";
    std::fs::write(root.join(path), "initial\n").unwrap();
    commit(root);
    std::fs::write(root.join(path), "index\n").unwrap();
    git(root, &["add", "--", path]);
    std::fs::write(root.join(path), "working tree\n").unwrap();
    let data = GitInfo {
        staged: vec![('M', path.into())],
        unstaged: vec![('M', path.into())],
        ..GitInfo::default()
    };
    let (owner, cx) = window(cx, root, data);
    let row = cx.debug_bounds("git-tree-row-changes-0-module/文件.rs").unwrap();
    cx.simulate_mouse_move(row.center(), None, Modifiers::default());
    cx.run_until_parked();
    cx.executor().advance_clock(Duration::from_secs(1));
    cx.run_until_parked();
    draw(cx);
    assert!(cx.debug_bounds("git-diff-row-hint").is_some());
    cx.simulate_click(row.center(), Modifiers::default());
    let worktree = diff(&owner, root, path, false, cx);
    assert!(settle(&worktree, cx).contains("+working tree"));
    assert!(
        cx.update(|window, cx| !worktree.read(cx).focus_handle(cx).contains_focused(window, cx))
    );
    press("enter", cx);
    assert!(
        cx.update(|window, cx| worktree.read(cx).focus_handle(cx).contains_focused(window, cx))
    );
    draw(cx);
    let copy = cx.debug_bounds("git-diff-copy").unwrap();
    cx.simulate_click(copy.center(), Modifiers::default());
    let captured = worktree.read_with(cx, |view, cx| view.diff_text(cx)).unwrap();
    assert_eq!(cx.read(|cx| cx.read_from_clipboard().unwrap().text().unwrap()), captured);
    cx.run_until_parked();
    cx.simulate_mouse_move(gpui::point(px(0.0), px(0.0)), None, Modifiers::default());
    draw(cx);
    assert!(cx.debug_bounds("git-diff-copy-success").is_some());
    assert_eq!(cx.debug_bounds("git-diff-copy").unwrap(), copy);
    cx.executor().advance_clock(COPY_FEEDBACK_TTL - Duration::from_millis(1));
    cx.run_until_parked();
    cx.simulate_click(copy.center(), Modifiers::default());
    cx.run_until_parked();
    cx.executor().advance_clock(Duration::from_millis(1));
    cx.run_until_parked();
    draw(cx);
    assert!(cx.debug_bounds("git-diff-copy-success").is_some());
    cx.executor().advance_clock(COPY_FEEDBACK_TTL);
    cx.run_until_parked();
    draw(cx);
    assert!(cx.debug_bounds("git-diff-copy-ready").is_some());
    let original = std::fs::read(root.join(path)).unwrap();
    cx.update(|window, cx| worktree.read(cx).focus_handle(cx).focus(window, cx));
    cx.simulate_keystrokes("x");
    press("cmd-s", cx);
    assert_eq!(std::fs::read(root.join(path)).unwrap(), original);
    assert!(worktree.read_with(cx, |view, _| view.file_editor().is_none()));
    std::fs::write(root.join(path), "after refresh\n").unwrap();
    draw(cx);
    click("git-diff-refresh", cx);
    assert!(settle(&worktree, cx).contains("+after refresh"));
    draw(cx);
    let staged = cx.debug_bounds("git-tree-row-staged-0-module/文件.rs").unwrap();
    cx.simulate_click(staged.center(), Modifiers::default());
    let index = diff(&owner, root, path, true, cx);
    assert!(settle(&index, cx).contains("+index"));
    assert_ne!(worktree.entity_id(), index.entity_id());
    draw(cx);
    let row = cx.debug_bounds("git-tree-row-changes-0-module/文件.rs").unwrap();
    cx.simulate_click(row.center(), Modifiers::default());
    // Native double-click is the first normal click followed by click_count=2.
    cx.simulate_event(gpui::MouseDownEvent {
        position: row.center(),
        button: gpui::MouseButton::Left,
        click_count: 2,
        ..Default::default()
    });
    cx.simulate_event(gpui::MouseUpEvent {
        position: row.center(),
        button: gpui::MouseButton::Left,
        click_count: 2,
        ..Default::default()
    });
    assert!(owner.read_with(cx, |workspace, cx| {
        let key = DiffKey {
            location: GitLocation::Local { root: root.into() },
            path: path.into(),
            staged: false,
        };
        workspace
            .tabs
            .iter()
            .filter(|tab| {
                matches!(tab, WorkspaceTab::Code { view, .. }
            if view.read(cx).matches_git_diff(&key, cx))
            })
            .count()
            == 1
    }));
    assert!(owner.read_with(cx, |workspace, cx| matches!(&workspace.tabs[workspace.active],
        WorkspaceTab::Code { view, .. } if view.read(cx).is_regular_path(&root.join(path)))));
    assert!(cx.update(|window, cx| {
        let workspace = owner.read(cx);
        let WorkspaceTab::Code { view, .. } = &workspace.tabs[workspace.active] else {
            return false;
        };
        view.read(cx).focus_handle(cx).contains_focused(window, cx)
    }));
}

#[gpui::test]
#[ignore = "launched by isolated_git_diff_controls with owned configuration and SSH home"]
fn git_patch_row_uses_its_rendered_repository_even_if_followed_cwd_changes(
    cx: &mut TestAppContext,
) {
    let first = repository();
    let second = repository();
    let path = "same.rs";
    for (root, value) in [(first.path(), "first"), (second.path(), "second")] {
        std::fs::write(root.join(path), "base\n").unwrap();
        commit(root);
        std::fs::write(root.join(path), format!("{value}\n")).unwrap();
    }
    let (owner, cx) = window(
        cx,
        first.path(),
        GitInfo { unstaged: vec![('M', path.into())], ..GitInfo::default() },
    );
    let bounds = cx.debug_bounds("git-tree-row-changes-0-same.rs").unwrap();
    // The old snapshot remains visible during refresh, including mouse-down redraws.
    cx.update(|_, cx| {
        owner.update(cx, |workspace, _| {
            workspace.side_panel.sync_at(Some(second.path().into()), None);
            workspace.vcs_list.sync(
                workspace.side_panel.git_snapshot(),
                GitPanelView::Changes,
                Some(second.path().into()),
            );
        })
    });
    cx.simulate_click(bounds.center(), Modifiers::default());
    let view = diff(&owner, first.path(), path, false, cx);
    let patch = settle(&view, cx);
    assert!(patch.contains("+first") && !patch.contains("+second"));
}

#[gpui::test]
#[ignore = "launched by isolated_git_diff_controls with owned configuration and SSH home"]
fn git_patch_empty_binary_and_failure_have_visible_readonly_controls(cx: &mut TestAppContext) {
    let directory = repository();
    let root = directory.path();
    std::fs::write(root.join("binary.dat"), [0, 1, 2]).unwrap();
    std::fs::write(root.join("empty.rs"), "").unwrap();
    commit(root);
    std::fs::write(root.join("binary.dat"), [0, 2, 3]).unwrap();
    // A stale changed-file row must still present the now-clean comparison.
    let data = GitInfo {
        unstaged: vec![('M', "empty.rs".into()), ('M', "binary.dat".into())],
        ..GitInfo::default()
    };
    let (owner, cx) = window(cx, root, data);
    click("git-tree-row-changes-0-empty.rs", cx);
    let empty = diff(&owner, root, "empty.rs", false, cx);
    assert!(settle(&empty, cx).is_empty());
    draw(cx);
    assert!(cx.debug_bounds("git-diff-status").is_some());
    click("git-tree-row-changes-1-binary.dat", cx);
    let binary = diff(&owner, root, "binary.dat", false, cx);
    assert!(settle(&binary, cx).contains("Binary files"));
    std::fs::remove_dir_all(root.join(".git")).unwrap();
    draw(cx);
    click("git-diff-refresh", cx);
    for _ in 0..1000 {
        cx.run_until_parked();
        if binary.read_with(cx, |view, cx| view.diff_error(cx)).is_some() {
            break;
        }
        std::thread::sleep(Duration::from_millis(2));
    }
    assert!(binary.read_with(cx, |view, cx| view.diff_error(cx)).is_some());
    draw(cx);
    assert!(cx.debug_bounds("git-diff-status").is_some());
    cx.update(|_, cx| cx.write_to_clipboard(ClipboardItem::new_string("sentinel".into())));
    click("git-diff-copy", cx);
    assert_eq!(cx.read(|cx| cx.read_from_clipboard().unwrap().text().unwrap()), "sentinel");
    assert!(cx.debug_bounds("git-diff-copy-success").is_none());
    assert!(binary.read_with(cx, |view, _| view.file_editor().is_none()));
}

#[gpui::test]
#[ignore = "launched by isolated_git_diff_controls with owned configuration and SSH home"]
fn git_patch_large_content_virtualizes_and_bounds_single_line_shaping(cx: &mut TestAppContext) {
    let directory = repository();
    let root = directory.path();
    std::fs::write(
        root.join("large.rs"),
        format!("{}\n{}", "x".repeat(40_000), "line\n".repeat(40_000)),
    )
    .unwrap();
    let (owner, cx) = window(
        cx,
        root,
        GitInfo { unstaged: vec![('?', "large.rs".into())], ..GitInfo::default() },
    );
    click("git-tree-row-changes-0-large.rs", cx);
    let view = diff(&owner, root, "large.rs", false, cx);
    let patch = settle(&view, cx);
    assert!(patch.contains(&"x".repeat(40_000)));
    draw(cx);
    let (rows, longest, wrapped, rendered) =
        view.read_with(cx, |view, cx| view.diff_preview_stats(cx));
    assert!(rows > 40_000 && longest <= 514 && wrapped);
    assert!(rendered > 0 && rendered < 40, "virtual viewport rendered {rendered}/{rows}");
    cx.update(|window, cx| view.read(cx).focus_handle(cx).focus(window, cx));
    press("end", cx);
    draw(cx);
    let (_, _, _, rendered) = view.read_with(cx, |view, cx| view.diff_preview_stats(cx));
    assert!(rendered > 0 && rendered < 40);
    click("git-diff-copy", cx);
    assert_eq!(cx.read(|cx| cx.read_from_clipboard().unwrap().text().unwrap()), patch);
}

#[gpui::test]
#[ignore = "launched by isolated_git_diff_controls with owned configuration and SSH home"]
fn git_patch_full_workspace_save_route_preserves_the_neighbor_draft_and_releases_feedback(
    cx: &mut TestAppContext,
) {
    assert_isolation();
    let directory = repository();
    let root = directory.path();
    let path = root.join("safe.rs");
    std::fs::write(&path, "base\n").unwrap();
    commit(root);
    initialize(cx, |cx| cx.set_reduce_motion(true));
    let mut owner = None;
    let (_, cx) = cx.add_window_view(|window, cx| {
        let workspace = cx.new(|cx| new_workspace(window, cx));
        workspace.update(cx, |workspace, cx| {
            workspace.open_document_path(path.clone(), window, cx);
        });
        owner = Some(workspace.clone());
        Root::new(workspace, window, cx)
    });
    let owner = owner.unwrap();
    let file = owner.read_with(cx, |workspace, cx| workspace.tabs[0].file_editor(cx)).unwrap();
    for _ in 0..1000 {
        cx.run_until_parked();
        if file.read_with(cx, |file, cx| file.draft(cx).as_ref() == "base\n") {
            break;
        }
        std::thread::sleep(Duration::from_millis(2));
    }
    assert_eq!(file.read_with(cx, |file, cx| file.draft(cx).to_string()), "base\n");
    draw(cx);
    cx.update(|window, cx| file.read(cx).focus_handle(cx).focus(window, cx));
    let modifier = if crate::platform::Platform::current() == crate::platform::Platform::MacOS {
        "cmd"
    } else {
        "ctrl"
    };
    press(&format!("{modifier}-a"), cx);
    cx.simulate_input("saved fixture\n");
    assert!(file.read_with(cx, |file, _| file.is_dirty()));
    press(&format!("{modifier}-s"), cx);
    for _ in 0..1000 {
        cx.run_until_parked();
        if !file.read_with(cx, |file, _| file.is_dirty()) {
            break;
        }
        std::thread::sleep(Duration::from_millis(2));
    }
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "saved fixture\n");
    press(&format!("{modifier}-a"), cx);
    cx.simulate_input("unsaved neighbor\n");
    assert!(file.read_with(cx, |file, _| file.is_dirty()));
    cx.update(|window, cx| {
        owner.update(cx, |workspace, cx| {
            workspace.open_git_diff_tab(
                GitLocation::Local { root: root.into() },
                "safe.rs".into(),
                false,
                true,
                window,
                cx,
            );
        })
    });
    let view = diff(&owner, root, "safe.rs", false, cx);
    assert!(settle(&view, cx).contains("+saved fixture"));
    draw(cx);
    assert!(cx.debug_bounds("git-diff-view").is_some());
    assert!(cx.debug_bounds("file-source-content").is_none());
    cx.simulate_input("must not reach the hidden draft");
    press(&format!("{modifier}-s"), cx);
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "saved fixture\n");
    assert_eq!(file.read_with(cx, |file, cx| file.draft(cx).to_string()), "unsaved neighbor\n");
    assert!(file.read_with(cx, |file, _| file.is_dirty()));
    let feedback = view.read_with(cx, |view, cx| view.diff_feedback(cx));
    click("git-diff-copy", cx);
    draw(cx);
    assert!(cx.debug_bounds("git-diff-copy-success").is_some());
    press("ctrl-shift-w", cx);
    drop(view);
    draw(cx);
    cx.run_until_parked();
    assert_eq!(owner.read_with(cx, |workspace, _| workspace.tabs.len()), 1);
    assert!(feedback.upgrade().is_none());
    cx.executor().advance_clock(COPY_FEEDBACK_TTL);
    cx.run_until_parked();
    assert!(file.read_with(cx, |file, _| file.is_dirty()));
}
