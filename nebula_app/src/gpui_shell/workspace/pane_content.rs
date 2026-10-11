//! Structural pane ownership and its explicit terminal content capability.

use super::*;

pub(super) struct WorkspacePane {
    pub(super) id: u64,
    pub(super) custom_name: Option<String>,
    pub(super) name_history: Vec<Option<String>>,
    pub(super) content: PaneContent,
}

pub(super) enum PaneContent {
    Terminal { view: Entity<TerminalView>, _subscription: Subscription },
}

impl WorkspacePane {
    pub(super) fn terminal_view(&self) -> Option<&Entity<TerminalView>> {
        match &self.content {
            PaneContent::Terminal { view, .. } => Some(view),
        }
    }

    pub(super) fn rebind(&mut self, window: &mut Window, cx: &mut Context<NebulaWorkspace>) {
        match &mut self.content {
            PaneContent::Terminal { view, _subscription } => {
                *_subscription = cx.subscribe_in(view, window, NebulaWorkspace::on_terminal_event);
            },
        }
    }
}
