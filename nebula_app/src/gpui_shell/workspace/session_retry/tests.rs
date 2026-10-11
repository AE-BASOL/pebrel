use super::*;
use crate::gpui_shell::config::Settings;
use crate::platform::shell_startup_fixture as fixture;
use crate::session::{LaunchSession, LayoutSession, Session, SplitAxis, TabSession};
use gpui::{Modifiers, TestAppContext, VisualTestContext, point, size};
use gpui_component::{Root, WindowExt as _};
use std::path::PathBuf;
use std::time::{Duration, Instant};

#[test]
fn isolated_local_startup_controls() {
    let directory = tempfile::tempdir().unwrap();
    std::fs::create_dir(directory.path().join(".ssh")).unwrap();
    std::fs::create_dir_all(directory.path().join("owned-pi/agent/sessions")).unwrap();
    std::fs::write(directory.path().join(".ssh/config"), "").unwrap();
    std::fs::write(
        directory.path().join("pebrel_settings.txt"),
        "language=en-US\nfetch=0\nrestore_session=1\nai_hooks=0\nauto_check_updates=0\n",
    )
    .unwrap();
    let output = std::process::Command::new(std::env::current_exe().unwrap())
        .args([
            "local_startup_",
            "--include-ignored",
            "--skip",
            "isolated_local_startup_controls",
            "--nocapture",
            "--test-threads=1",
        ])
        .env_remove("PEBREL_CONFIG_FILE")
        .env_remove("NEBULA_CONFIG_FILE")
        .env_remove("PEBREL_GPUI_CONFIG")
        .env_remove("NEBULA_GPUI_CONFIG")
        .env_remove("NEBULA_GPUI_OPEN_DOC")
        .env_remove("SSH_AUTH_SOCK")
        .env_remove("BASH_ENV")
        .env_remove("ENV")
        .env("PI_CODING_AGENT_DIR", directory.path().join("owned-pi/agent"))
        .env("PI_CODING_AGENT_SESSION_DIR", directory.path().join("owned-pi/agent/sessions"))
        .env("PEBREL_TEST_SSH_HOME", directory.path())
        .env("PEBREL_CONFIG_DIR", directory.path())
        .env("PEBREL_LOCAL_STARTUP_QA_DIR", directory.path())
        .output()
        .unwrap();
    let evidence = format!(
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    crate::gpui_shell::try_write_stderr(format_args!("{evidence}"));
    assert!(output.status.success() && evidence.contains("6 passed;"), "{evidence}");
}

fn owned() -> PathBuf {
    let directory = PathBuf::from(
        std::env::var_os("PEBREL_LOCAL_STARTUP_QA_DIR").expect("isolated child only"),
    );
    assert!(directory.is_absolute());
    assert_eq!(std::env::var_os("PEBREL_CONFIG_DIR"), Some(directory.clone().into_os_string()));
    assert_eq!(std::env::var_os("PEBREL_TEST_SSH_HOME"), Some(directory.clone().into_os_string()));
    assert_eq!(crate::platform::dirs::data_dir(), directory);
    assert_eq!(crate::ssh::ssh_config_path(), Some(directory.join(".ssh/config")));
    assert_eq!(crate::ssh::read_ssh_config().unwrap(), "");
    assert!(crate::ssh::ssh_config_hosts().is_empty());
    let settings = nebula_settings::RawSettings::load();
    assert_eq!(settings.bool_on("ai_hooks"), Some(false));
    assert_eq!(settings.bool_on("auto_check_updates"), Some(false));
    assert_eq!(
        std::env::var_os("PI_CODING_AGENT_DIR"),
        Some(directory.join("owned-pi/agent").into_os_string())
    );
    assert_eq!(
        std::env::var_os("PI_CODING_AGENT_SESSION_DIR"),
        Some(directory.join("owned-pi/agent/sessions").into_os_string())
    );
    directory
}

fn open(cx: &mut TestAppContext) -> (Entity<NebulaWorkspace>, VisualTestContext) {
    owned(); // Assert the authority before Workspace::new loads any host sources.
    let hub = crate::runtime_api::RuntimeHub::new();
    cx.update(|cx| {
        cx.set_global(crate::gpui_shell::terminal::LocalPtyPump::default());
        gpui_component::init(cx);
        crate::gpui_shell::math_view::register(cx);
        crate::gpui_shell::file_editor::init(cx);
        super::super::init(cx);
        windowing::initialize(cx, hub.clone());
        cx.set_global(Settings::load(nebula_settings::ThemeName::Nord));
        cx.set_reduce_motion(true);
    });
    let mut result = None;
    let (_, window) = cx.add_window_view(|window, cx| {
        let workspace = cx.new(|cx| {
            NebulaWorkspace::new(
                window,
                None,
                None,
                1,
                hub,
                windowing::WorkspaceStartup::Empty,
                windowing::WindowRole::Regular,
                cx,
            )
        });
        result = Some(workspace.clone());
        Root::new(workspace, window, cx)
    });
    let mut window = window.clone();
    window.simulate_resize(size(px(1100.0), px(800.0)));
    draw(&mut window);
    (result.unwrap(), window)
}

fn draw(cx: &mut VisualTestContext) {
    cx.run_until_parked();
    cx.update(|window, cx| {
        crate::gpui_shell::terminal::LocalPtyPump::drain(cx);
        window.refresh();
        let _ = window.draw(cx);
    });
}

fn launch(program: &std::path::Path, marker: &str) -> LaunchSession {
    LaunchSession::Shell {
        name: "Synthetic shell".into(),
        program: program.to_string_lossy().into_owned(),
        args: fixture::waiting_args(marker),
    }
}

fn tab(launch: LaunchSession, cwd: &std::path::Path) -> TabSession {
    TabSession {
        cwd: cwd.to_string_lossy().into_owned(),
        custom_name: Some("Saved failing shell".into()),
        color: None,
        launch: Some(launch),
        layout: None,
        active_pane: 0,
    }
}

fn active(workspace: &Entity<NebulaWorkspace>, cx: &VisualTestContext) -> Entity<TerminalView> {
    workspace.read_with(cx, |workspace, _| {
        workspace.tabs[workspace.active].focused_view().unwrap().clone()
    })
}

fn wait(cx: &mut VisualTestContext, mut ready: impl FnMut(&mut VisualTestContext) -> bool) {
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        draw(cx);
        if ready(cx) {
            return;
        }
        assert!(Instant::now() < deadline, "synthetic PTY did not produce the expected outcome");
        std::thread::sleep(Duration::from_millis(10));
    }
}

fn text(view: &Entity<TerminalView>, cx: &VisualTestContext) -> String {
    view.read_with(cx, |view, _| {
        view.session
            .as_ref()
            .map(|session| {
                session.term.lock().grid().display_iter().map(|cell| cell.cell.c).collect()
            })
            .unwrap_or_default()
    })
}

#[gpui::test]
#[ignore = "launched by isolated_local_startup_controls with owned settings and SSH home"]
fn local_startup_retry_keeps_snapshot_metadata_and_rejects_stale_events(cx: &mut TestAppContext) {
    let directory = tempfile::tempdir_in(owned()).unwrap();
    let program = directory.path().join("repairable-shell");
    let mut saved = launch(&program, "512-READY");
    let LaunchSession::Shell { args, .. } = &mut saved else { unreachable!() };
    *args = fixture::queued_args("512-READY");
    let (workspace, mut cx) = open(cx);
    cx.update(|window, cx| {
        workspace.update(cx, |workspace, cx| {
            // A synthetic PTY snapshot exercises the production failure constructor
            // with an explicit environment override, without reading personal settings.
            let options = nebula_terminal::tty::Options {
                shell: Some(nebula_terminal::tty::Shell::new(
                    program.to_string_lossy().into_owned(),
                    fixture::queued_args("512-READY"),
                )),
                working_directory: Some(directory.path().to_path_buf()),
                env: [("PEBREL_512_FROZEN".into(), "original-value".into())].into_iter().collect(),
                ..Default::default()
            };
            let mut pane = workspace.new_pane_with(None, window, cx, |id, window, cx| {
                TerminalView::new_local_retry(id, (80, 24), saved.clone(), options, window, cx)
            });
            pane.custom_name = Some("Retained name".into());
            pane.name_history = vec![None, Some("Previous name".into())];
            let id = pane.id;
            workspace.insert_tab_at(
                0,
                WorkspaceTab::Terminal {
                    panes: vec![pane],
                    tree: SplitTree::leaf(id),
                    focused: id,
                    zoomed: true,
                    broadcast: true,
                },
                TabMeta {
                    custom_name: Some("Retained tab".into()),
                    launch: Some(saved.clone()),
                    ..Default::default()
                },
            );
            workspace.focus_active(window, cx);
        })
    });
    draw(&mut cx);
    let old = active(&workspace, &cx);
    assert!(old.read_with(&cx, |view, _| view.local_startup_failed()));
    old.update(&mut cx, |view, cx| view.run_command(fixture::queued_command().into(), cx));
    assert!(cx.debug_bounds("local-startup-error").is_some());
    cx.simulate_resize(size(px(680.0), px(600.0)));
    draw(&mut cx);
    let surface = cx.debug_bounds("local-startup-error").unwrap();
    for selector in ["nebula-btn-local-startup-retry", "nebula-btn-local-startup-choose"] {
        let bounds = cx.debug_bounds(selector).unwrap();
        assert!(bounds.size.height >= px(32.0));
        assert!(bounds.origin.x >= surface.origin.x && bounds.right() <= surface.right());
    }
    let button = cx.debug_bounds("nebula-btn-local-startup-retry").unwrap();
    let before = old.read_with(&cx, |view, _| view.failed_local_options().unwrap().clone());
    cx.simulate_mouse_move(button.center(), None, Modifiers::default());
    cx.simulate_keystrokes("tab enter");
    cx.simulate_event(gpui::KeyUpEvent { keystroke: gpui::Keystroke::parse("enter").unwrap() });
    draw(&mut cx);
    let failed_retry = active(&workspace, &cx);
    assert_ne!(old.entity_id(), failed_retry.entity_id());
    assert!(failed_retry.read_with(&cx, |view, _| view.local_startup_failed()));
    assert_eq!(
        failed_retry.read_with(&cx, |view, _| view.local_startup_requests().0).as_deref(),
        Some(fixture::queued_command())
    );
    assert!(cx.debug_bounds("nebula-btn-local-startup-retry").is_some());
    let mut before = before;
    let mut after =
        failed_retry.read_with(&cx, |view, _| view.failed_local_options().unwrap().clone());
    for key in [crate::agent_env::PANE_ENV, crate::ai_hook::LEGACY_PANE_ENV] {
        assert_ne!(
            before.env.remove(key),
            after.env.remove(key),
            "the new entity needs a new pane identity"
        );
    }
    assert!(
        before == after,
        "all remaining PTY options must stay frozen: shell={}, cwd={}, env_keys={:?}",
        before.shell == after.shell,
        before.working_directory == after.working_directory,
        before
            .env
            .keys()
            .chain(after.env.keys())
            .filter(|key| before.env.get(*key) != after.env.get(*key))
            .collect::<std::collections::BTreeSet<_>>()
    );
    fixture::install(&program);
    cx.update(|_, cx| {
        // Current defaults must not replace the failed pane's frozen command.
        cx.global_mut::<Settings>().shell_id = Some("synthetic-other-default".into());
    });
    let button = cx.debug_bounds("nebula-btn-local-startup-retry").unwrap();
    cx.simulate_click(
        point(button.origin.x + px(2.0), button.bottom() - px(2.0)),
        Modifiers::default(),
    );
    wait(&mut cx, |cx| text(&active(&workspace, cx), cx).contains("512-READY:original-value:"));
    wait(&mut cx, |cx| text(&active(&workspace, cx), cx).contains("512-QUEUED-EXECUTED"));
    let repaired = active(&workspace, &cx);
    let repaired_id = repaired.read_with(&cx, |view, _| view.pane_id);
    let actual = text(&repaired, &cx);
    assert!(
        actual.contains(&format!("512-PANE:{repaired_id}:{repaired_id}:{}", std::process::id()))
    );
    assert_eq!(actual.matches("512-QUEUED-EXECUTED").count(), 1);
    assert!(repaired.read_with(&cx, |view, _| view.local_startup_requests().0.is_none()));
    assert_ne!(failed_retry.entity_id(), repaired.entity_id());
    repaired.read_with(&cx, |view, _| {
        assert!(!view.local_startup_failed());
        assert_eq!(view.session_launch, saved);
        assert_eq!(view.cwd, directory.path().to_string_lossy());
    });
    assert!(cx.debug_bounds("local-startup-error").is_none());
    cx.update(|window, cx| {
        workspace.read_with(cx, |workspace, cx| {
            let WorkspaceTab::Terminal { panes, tree, focused, zoomed, broadcast } =
                &workspace.tabs[0]
            else {
                panic!("terminal tab")
            };
            assert_eq!(panes.len(), 1);
            assert_eq!(tree.leaves(), [panes[0].id]);
            assert_eq!(*focused, panes[0].id);
            assert!(*zoomed && *broadcast);
            assert_eq!(panes[0].custom_name.as_deref(), Some("Retained name"));
            assert_eq!(panes[0].name_history, [None, Some("Previous name".into())]);
            assert_eq!(workspace.meta(0).custom_name.as_deref(), Some("Retained tab"));
            assert!(repaired.read(cx).focus_handle.is_focused(window));
        });
        old.update(cx, |_, cx| cx.emit(TerminalViewEvent::Exited));
        failed_retry.update(cx, |_, cx| cx.emit(TerminalViewEvent::RetryLocal));
    });
    draw(&mut cx);
    assert_eq!(active(&workspace, &cx).entity_id(), repaired.entity_id());
    assert_eq!(text(&repaired, &cx).matches("512-QUEUED-EXECUTED").count(), 1);
    repaired.read_with(&cx, |view, _| view.shutdown());
}

#[gpui::test]
#[ignore = "launched by isolated_local_startup_controls with owned settings and SSH home"]
fn local_startup_picker_opens_another_shell_and_preserves_failed_diagnostics(
    cx: &mut TestAppContext,
) {
    let directory = tempfile::tempdir_in(owned()).unwrap();
    let missing = directory.path().join("missing-shell");
    let alternate = directory.path().join("alternate-shell");
    fixture::install(&alternate);
    let (workspace, mut cx) = open(cx);
    cx.update(|window, cx| {
        workspace.update(cx, |workspace, cx| {
            workspace.add_terminal_with(
                launch(&missing, "512-MISSING"),
                Some(directory.path().to_path_buf()),
                None,
                window,
                cx,
            );
        })
    });
    draw(&mut cx);
    let failed = active(&workspace, &cx);
    let choose = cx.debug_bounds("nebula-btn-local-startup-choose").unwrap();
    cx.simulate_click(choose.center(), Modifiers::default());
    draw(&mut cx);
    cx.update(|window, cx| {
        workspace.update(cx, |workspace, cx| {
            assert!(workspace.shell_picker_open && workspace.command_palette_open);
            assert_eq!(
                workspace.launcher_filter,
                crate::display::command_palette::LauncherFilter::Shell
            );
            assert!(workspace.command_palette_input.read(cx).focus_handle(cx).is_focused(window));
            assert_eq!(workspace.tabs.len(), 1);
            workspace.palette_override = Some(shell_palette_rows(
                vec![crate::shell_detect::DetectedShell {
                    id: "synthetic-512".into(),
                    name: "Synthetic alternate".into(),
                    program: alternate.to_string_lossy().into_owned(),
                    args: fixture::waiting_args("512-ALTERNATE"),
                }],
                vec![],
                &crate::gpui_shell::ssh_hosts::SshHostLists::default(),
                "",
                workspace_ui_language(),
                1.0,
            ));
            cx.notify();
        })
    });
    draw(&mut cx);
    assert!(cx.debug_bounds("command-palette-row-0").is_some());
    cx.simulate_keystrokes("enter");
    wait(&mut cx, |cx| text(&active(&workspace, cx), cx).contains("512-ALTERNATE:"));
    workspace.read_with(&cx, |workspace, cx| {
        assert_eq!(workspace.tabs.len(), 2);
        assert!(!workspace.command_palette_open);
        assert!(workspace.locate_pane(failed.entity_id()).is_some());
        assert!(failed.read(cx).local_startup_failed());
    });
    active(&workspace, &cx).read_with(&cx, |view, _| view.shutdown());
}

#[gpui::test]
#[ignore = "launched by isolated_local_startup_controls with owned settings and SSH home"]
fn local_startup_restore_counts_failed_split_leaves_and_update_warns(cx: &mut TestAppContext) {
    let directory = tempfile::tempdir_in(owned()).unwrap();
    let missing_launch = launch(&directory.path().join("missing-shell"), "512-MISSING");
    let good = directory.path().join("working-shell");
    fixture::install(&good);
    let leaf = |launch, name: &str| LayoutSession::Pane {
        cwd: directory.path().to_string_lossy().into_owned(),
        agent: None,
        launch: Some(launch),
        custom_name: Some(name.into()),
    };
    let mut saved = tab(missing_launch.clone(), directory.path());
    saved.layout = Some(LayoutSession::Split {
        axis: SplitAxis::LeftRight,
        ratio_permille: 370,
        first: Box::new(leaf(missing_launch.clone(), "Failed leaf")),
        second: Box::new(leaf(launch(&good, "512-RESTORED"), "Working leaf")),
    });
    let session =
        Session::new(0, vec![saved.clone(), tab(missing_launch.clone(), directory.path())]);
    let (workspace, mut cx) = open(cx);
    cx.update(|window, cx| {
        workspace.update(cx, |workspace, cx| {
            let summary = workspace.restore_session_tabs(&session, false, window, cx);
            assert_eq!(
                summary,
                super::super::session_recovery::RestoreSummary {
                    opened_tabs: 2,
                    failed_local_panes: 2
                }
            );
            let snapshot = workspace.snapshot_session(cx);
            assert_eq!(snapshot.tabs[0], saved);
            assert_eq!(snapshot.tabs[1].launch, Some(missing_launch.clone()));
            let all_failed = Session::new(0, vec![tab(missing_launch, directory.path())]);
            assert!(workspace.restore_update_session(&all_failed, false, window, cx));
            assert_eq!(workspace.tabs.len(), 3, "all-failed layouts retain their diagnostic panes");
        })
    });
    draw(&mut cx);
    cx.update(|window, cx| {
        assert!(
            !window.notifications(cx).is_empty(),
            "explicit update restore failure must produce feedback"
        );
    });
    workspace.read_with(&cx, |workspace, cx| {
        for tab in &workspace.tabs {
            if let WorkspaceTab::Terminal { panes, .. } = tab {
                for pane in panes {
                    pane.view.read(cx).shutdown();
                }
            }
        }
    });
}

#[gpui::test]
#[ignore = "launched by isolated_local_startup_controls with owned settings and SSH home"]
fn local_startup_cold_restore_retains_all_failed_tabs_and_reports_warning(cx: &mut TestAppContext) {
    let directory = tempfile::tempdir_in(owned()).unwrap();
    let mut saved_tab =
        tab(launch(&directory.path().join("missing-shell"), "512-MISSING"), directory.path());
    saved_tab.layout = Some(LayoutSession::Pane {
        cwd: saved_tab.cwd.clone(),
        custom_name: None,
        launch: saved_tab.launch.clone(),
        agent: Some(crate::session::AgentSession {
            source: "pi".into(),
            session_id: Some("12345678-1234-4234-8234-123456789abc".into()),
            session_file: None,
        }),
    });
    let saved = Session::new(0, vec![saved_tab]);
    let provider_calls = crate::platform::pi_session::test_resolve_calls();
    crate::session::try_save(&saved).unwrap();
    let (workspace, mut cx) = open(cx);
    cx.update(|window, cx| {
        workspace.update(cx, |workspace, cx| {
            assert!(workspace.try_restore_session(true, window, cx));
            assert_eq!(workspace.tabs.len(), 1);
            assert!(workspace.tabs[0].focused_view().unwrap().read(cx).local_startup_failed());
            assert_eq!(workspace.snapshot_session(cx).tabs, saved.tabs);
        })
    });
    draw(&mut cx);
    assert!(cx.debug_bounds("nebula-btn-local-startup-retry").is_some());
    let retry = cx.debug_bounds("nebula-btn-local-startup-retry").unwrap();
    cx.simulate_click(retry.center(), Modifiers::default());
    draw(&mut cx);
    workspace.read_with(&cx, |workspace, cx| {
        assert_eq!(
            workspace.snapshot_session(cx).tabs,
            saved.tabs,
            "failed Retry must retain the saved AI target"
        )
    });
    cx.update(|window, cx| {
        assert!(!window.notifications(cx).is_empty(), "cold restore must report the spawn failure");
    });
    // The same normalized Pi intent must stay inert through initial cold
    // restoration, failed Retry, and update restoration. Explicit owned Pi
    // directories also contain any resolver access if a regression removes the guard.
    assert_eq!(crate::platform::pi_session::test_resolve_calls(), provider_calls);
    workspace.read_with(&cx, |workspace, cx| {
        assert!(
            !workspace.tabs[0]
                .focused_view()
                .unwrap()
                .read(cx)
                .startup_recovery_resolving_for_test()
        );
    });
    cx.update(|window, cx| {
        workspace.update(cx, |workspace, cx| {
            assert!(workspace.restore_update_session(&saved, true, window, cx));
            assert_eq!(workspace.snapshot_session(cx).tabs[1], saved.tabs[0]);
            assert!(
                !workspace.tabs[1]
                    .focused_view()
                    .unwrap()
                    .read(cx)
                    .startup_recovery_resolving_for_test()
            );
        })
    });
    draw(&mut cx);
    assert_eq!(crate::platform::pi_session::test_resolve_calls(), provider_calls);
    let recorded = crate::session::load().unwrap();
    assert_eq!(recorded.boot_attempts, 1);
    assert_eq!(recorded.tabs, saved.tabs);
}

#[gpui::test]
#[ignore = "launched by isolated_local_startup_controls with owned settings and SSH home"]
fn local_startup_short_commands_keep_existing_exit_behavior(cx: &mut TestAppContext) {
    let directory = tempfile::tempdir_in(owned()).unwrap();
    let program = directory.path().join("short-command");
    fixture::install(&program);
    let (workspace, mut cx) = open(cx);
    cx.update(|window, cx| {
        workspace.update(cx, |workspace, cx| {
            workspace.add_terminal_with(
                launch(&directory.path().join("missing-anchor"), "512-ANCHOR"),
                Some(directory.path().to_path_buf()),
                None,
                window,
                cx,
            );
        })
    });
    let anchor = active(&workspace, &cx);
    for status in [0, 23] {
        cx.update(|window, cx| {
            workspace.update(cx, |workspace, cx| {
                workspace.add_terminal_with(
                    LaunchSession::Shell {
                        name: "Intentional short command".into(),
                        program: program.to_string_lossy().into_owned(),
                        args: fixture::exit_args(status),
                    },
                    Some(directory.path().to_path_buf()),
                    None,
                    window,
                    cx,
                );
            })
        });
        wait(&mut cx, |cx| workspace.read_with(cx, |workspace, _| workspace.tabs.len() == 1));
        assert_eq!(active(&workspace, &cx).entity_id(), anchor.entity_id());
    }
}
