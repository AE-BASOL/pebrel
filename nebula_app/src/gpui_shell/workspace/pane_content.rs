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

impl WorkspacePane {
    pub(super) fn kind(&self) -> &'static str {
        match self.content {
            PaneContent::Terminal { .. } => "terminal",
        }
    }

    pub(super) fn file_editor(
        &self,
        _cx: &App,
    ) -> Option<Entity<crate::gpui_shell::file_editor::TextFileView>> {
        match self.content {
            PaneContent::Terminal { .. } => None,
        }
    }

    pub(super) fn runtime_file_info(
        &self,
        _cx: &App,
    ) -> Option<crate::runtime_api::tabs::FileInfo> {
        match self.content {
            PaneContent::Terminal { .. } => None,
        }
    }

    pub(super) fn title(&self, cx: &App) -> SharedString {
        match &self.content {
            PaneContent::Terminal { view, .. } => view.read(cx).tab_label().into(),
        }
    }
}

impl WorkspaceTab {
    pub(super) fn focused_pane(&self) -> Option<&WorkspacePane> {
        let Self::Split { panes, focused, .. } = self else { return None };
        panes.iter().find(|pane| pane.id == *focused).or_else(|| panes.first())
    }

    pub(super) fn file_editors(
        &self,
        cx: &App,
    ) -> Vec<Entity<crate::gpui_shell::file_editor::TextFileView>> {
        self.file_editor(cx).into_iter().collect()
    }
}
