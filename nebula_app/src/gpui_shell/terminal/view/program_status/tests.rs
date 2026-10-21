use super::*;
use crate::gpui_shell::terminal::view::{SidebarActivity, TermEvent, startup_tests::open};
use crate::runtime_api::RuntimeAgentStateSource;
use gpui::TestAppContext;
use nebula_terminal::event_loop::StreamProcessor;

struct Wire {
    stream: StreamProcessor,
    events: crate::gpui_shell::terminal::event_mailbox::EventReceiver,
    proxy: crate::gpui_shell::terminal::session::EventProxy,
}

#[gpui::test]
fn osc7501_identity_never_borrows_another_clients_session(cx: &mut TestAppContext) {
    let (view, window, _) = open(cx);
    view.update(window, |view, cx| {
        let mut wire = Wire::install(view);
        view.running_program = Some("codex".into());
        view.ai_session = Some(crate::display::AiSessionIdentity {
            source: "codex".into(),
            session_id: "old-codex-session".into(),
        });
        wire.feed(view, cx, b"\x1b]7501;state=blocked:app=claude-code\x07");
        let agent = view.runtime_agent().unwrap();
        assert_eq!(agent.kind, "claude");
        assert_eq!(agent.session_id, None);
        wire.feed(view, cx, b"\x1b]7501;state=working:app=cargo\x07");
        assert!(view.runtime_agent().is_none());
    });
}

impl Wire {
    fn install(view: &mut TerminalView) -> Self {
        let (session, _, events, proxy) =
            crate::gpui_shell::terminal::session::test_session_with_events();
        view.session = Some(session);
        Self { stream: StreamProcessor::default(), events, proxy }
    }

    fn feed(&mut self, view: &mut TerminalView, cx: &mut Context<TerminalView>, bytes: &[u8]) {
        {
            let mut term = view.session.as_ref().unwrap().term.lock();
            self.stream.feed(&mut term, &self.proxy, bytes);
        }
        while let Ok(event) = self.events.try_recv() {
            view.process_event(event, cx);
        }
    }
}

#[gpui::test]
fn osc7501_message_only_updates_do_not_refresh_workspace_status(cx: &mut TestAppContext) {
    let (view, window, _) = open(cx);
    let updates = std::rc::Rc::new(std::cell::Cell::new(0));
    let count = updates.clone();
    let _subscription = window.update(|_, cx| {
        cx.subscribe(&view, move |_, event, _| {
            if matches!(event, TerminalViewEvent::TitleChanged) {
                count.set(count.get() + 1);
            }
        })
    });
    let mut wire = view.update(window, |view, _| Wire::install(view));
    view.update(window, |view, cx| wire.feed(view, cx, b"\x1b]7501;state=working:app=codex\x07"));
    let before = updates.get();
    assert!(before > 0);
    view.update(window, |view, cx| {
        wire.feed(view, cx, b"\x1b]7501;state=working:app=codex:msg=VGVzdA==\x07");
        wire.feed(view, cx, b"\x1b]7501;state=working:app=codex:title=VGVzdA==\x07");
    });
    assert_eq!(updates.get(), before);
}

#[gpui::test]
fn osc7501_drives_agent_state_child_priority_and_progress_without_notifications(
    cx: &mut TestAppContext,
) {
    let (view, window, _) = open(cx);
    let notifications = std::rc::Rc::new(std::cell::Cell::new(0));
    let count = notifications.clone();
    let _subscription = window.update(|_, cx| {
        cx.subscribe(&view, move |_, event, _| {
            if matches!(event, TerminalViewEvent::Notification(_)) {
                count.set(count.get() + 1);
            }
        })
    });
    view.update(window, |view, cx| {
        let mut wire = Wire::install(view);
        wire.feed(view, cx, b"\x1b]7501;state=working:app=codex:progress=35\x07");
        assert_eq!(view.runtime_task_state(), RuntimeTaskState::Running);
        assert_eq!(view.sidebar_activity(), SidebarActivity::Running);
        assert_eq!(view.progress, TaskProgress::Value(35));
        let agent = view.runtime_agent().unwrap();
        assert_eq!(agent.kind, "codex");
        assert_eq!(agent.state_source, RuntimeAgentStateSource::Osc7501);
        assert!(!agent.hook_seen);
        for _ in 0..4 {
            wire.feed(view, cx, b"\x1b]7501;state=blocked:id=worker:kind=permission\x07");
            view.refresh_agent_screen_state(cx);
            assert_eq!(view.runtime_task_state(), RuntimeTaskState::Attention);
            assert_eq!(view.sidebar_activity(), SidebarActivity::Attention);
            assert_eq!(view.runtime_agent().unwrap().kind, "codex");
        }
        assert_eq!(view.progress, TaskProgress::Paused(None));
        view.process_event(TermEvent::Progress { state: 0, value: None }, cx);
        assert_eq!(view.progress, TaskProgress::Paused(None));
        wire.feed(view, cx, b"\x1b]7501;state=clear:id=worker\x07");
        assert_eq!(view.runtime_task_state(), RuntimeTaskState::Running);
        wire.feed(view, cx, b"\x1b]7501;state=done:app=codex\x07\x1b]133;A\x07");
        view.finish_foreground_command(Some(0), cx);
        assert_eq!(view.runtime_task_state(), RuntimeTaskState::Finished);
        assert_eq!(view.progress, TaskProgress::None);
        view.process_event(TermEvent::Exit, cx);
        assert_eq!(view.runtime_task_state(), RuntimeTaskState::Finished);
        view.process_event(TermEvent::PtyFailure("fixture broken pipe".into()), cx);
        assert_eq!(view.runtime_task_state(), RuntimeTaskState::Failed);
        assert_eq!(view.sidebar_activity(), SidebarActivity::Failed);
    });
    assert_eq!(notifications.get(), 0);
}

#[gpui::test]
fn osc7501_cannot_override_hooks_or_pane_failures(cx: &mut TestAppContext) {
    let (view, window, _) = open(cx);
    view.update(window, |view, cx| {
        let mut wire = Wire::install(view);
        wire.feed(view, cx, b"\x1b]7501;state=blocked:app=codex\x07");
        let event = crate::ai_hook::parse_remote_envelope(
            b"nebula-hook/1 source=claude pane=42\n{\"hook_event_name\":\"UserPromptSubmit\",\"session_id\":\"osc7501-authority\",\"bridge_sequence\":1}",
            Some(42),
        ).unwrap();
        assert!(view.handle_ai_hook(&event, cx));
        assert_eq!(view.runtime_task_state(), RuntimeTaskState::Running);
        wire.feed(view, cx, b"\x1b]7501;state=error:app=codex\x07");
        assert_eq!(view.runtime_task_state(), RuntimeTaskState::Running);
        assert_eq!(view.runtime_agent().unwrap().state_source, RuntimeAgentStateSource::Hook);
        assert_eq!(view.progress, TaskProgress::None);
        view.error = Some("fixture transport failure".into());
        assert_eq!(view.runtime_task_state(), RuntimeTaskState::Failed);
        assert_eq!(view.sidebar_activity(), SidebarActivity::Failed);
    });
}

#[gpui::test]
fn osc7501_error_and_ris_do_not_leave_stale_taskbar_state(cx: &mut TestAppContext) {
    let (view, window, _) = open(cx);
    view.update(window, |view, cx| {
        let mut wire = Wire::install(view);
        wire.feed(view, cx, b"\x1b]7501;state=error:app=cargo\x07");
        assert_eq!(view.runtime_task_state(), RuntimeTaskState::Failed);
        assert_eq!(view.sidebar_activity(), SidebarActivity::CommandFailed);
        assert_eq!(view.progress, TaskProgress::Error(None));
        assert!(view.runtime_agent().is_none());
        wire.feed(view, cx, b"\x1bc");
        assert_eq!(view.runtime_task_state(), RuntimeTaskState::Idle);
        assert_eq!(view.progress, TaskProgress::None);
        view.process_event(TermEvent::Progress { state: 1, value: Some(50) }, cx);
        assert_eq!(view.progress, TaskProgress::Value(50));
        wire.feed(view, cx, b"\x1b]7501;state=working:app=claude-code:msg=ZG9uZQ==\x07");
        assert_eq!(view.runtime_task_state(), RuntimeTaskState::Running, "msg is not evidence");
        assert_eq!(view.runtime_agent().unwrap().kind, "claude");
        wire.feed(view, cx, b"\x1b]7501;state=clear\x07");
        assert_eq!(view.runtime_task_state(), RuntimeTaskState::Idle);
        assert_eq!(view.progress, TaskProgress::None);
    });
}
