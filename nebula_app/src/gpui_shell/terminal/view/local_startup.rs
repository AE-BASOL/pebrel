//! Presentation and snapshots for an explicit local PTY creation error.

use super::*;
use crate::gpui_shell::widgets::NebulaButton;
use crate::i18n::Message;

impl TerminalView {
    pub(crate) fn local_startup_failed(&self) -> bool {
        self.ssh_destination.is_none() && self.session.is_none() && self.error.is_some()
    }

    pub(crate) fn failed_local_options(&self) -> Option<&nebula_terminal::tty::Options> {
        self.local_startup_failed().then_some(self.failed_local_options.as_ref()).flatten()
    }

    pub(crate) fn new_local_retry(
        pane_id: u64,
        grid: (u16, u16),
        launch: crate::session::LaunchSession,
        mut options: nebula_terminal::tty::Options,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        // A new entity/process receives a new pane identity, while every other
        // PTY override stays frozen. Do not refresh settings or re-resolve WSL.
        crate::agent_env::refresh_identity(&mut options.env, pane_id);
        let terminal_launch = TerminalLaunch::Local {
            cwd: options.working_directory.clone(),
            shell: options.shell.clone(),
            shell_name: match &launch {
                crate::session::LaunchSession::Shell { name, .. }
                | crate::session::LaunchSession::Profile { name, .. } => Some(name.clone()),
                _ => None,
            },
        };
        let mut view =
            Self::new_with_local_options(pane_id, grid, terminal_launch, Some(options), window, cx);
        view.session_launch = launch;
        view
    }

    pub(super) fn local_startup_error(
        &self,
        error: &str,
        cx: &mut Context<Self>,
    ) -> gpui::AnyElement {
        if !self.local_startup_failed() {
            return div().p_4().text_color(gpui::red()).child(error.to_owned()).into_any_element();
        }
        let language = crate::gpui_shell::config::ui_language(cx);
        let retry_view = cx.entity().downgrade();
        let choose_view = retry_view.clone();
        div()
            .id("local-startup-error")
            .debug_selector(|| "local-startup-error".into())
            .size_full()
            .overflow_y_scroll()
            .flex()
            .flex_col()
            .gap_3()
            .p_4()
            .child(
                div()
                    .text_color(cx.theme().danger)
                    .child(language.text(Message::TerminalStartupFailed)),
            )
            .child(div().text_color(cx.theme().muted_foreground).child(error.to_owned()))
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .gap_2()
                    .child(
                        NebulaButton::new("local-startup-retry")
                            .label(language.text(Message::TerminalStartupRetry))
                            .primary()
                            .on_click(move |_, _, cx| {
                                let _ = retry_view.update(cx, |view, cx| {
                                    if view.local_startup_failed() {
                                        cx.emit(TerminalViewEvent::RetryLocal);
                                    }
                                });
                            }),
                    )
                    .child(
                        NebulaButton::new("local-startup-choose")
                            .label(language.text(Message::TerminalStartupChoose))
                            .outline()
                            .on_click(move |_, _, cx| {
                                let _ = choose_view.update(cx, |view, cx| {
                                    if view.local_startup_failed() {
                                        cx.emit(TerminalViewEvent::ChooseLocalShell);
                                    }
                                });
                            }),
                    ),
            )
            .into_any_element()
    }
}
