//! Pane creation, subscriptions and atomic manual retry replacement.

use super::*;
use crate::gpui_shell::terminal::view::TerminalLaunch;

impl NebulaWorkspace {
    pub(super) fn new_pane(
        &mut self,
        grid: (u16, u16),
        launch: TerminalLaunch,
        command: Option<String>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> TerminalPane {
        self.new_pane_with(command, window, cx, |id, window, cx| {
            TerminalView::new(id, grid, launch, window, cx)
        })
    }

    fn new_pane_with(
        &mut self,
        command: Option<String>,
        window: &mut Window,
        cx: &mut Context<Self>,
        create: impl FnOnce(u64, &mut Window, &mut Context<TerminalView>) -> TerminalView,
    ) -> TerminalPane {
        let pane_id = self.next_pane_id;
        self.next_pane_id = self.next_pane_id.saturating_add(1);
        let view = cx.new(|cx| create(pane_id, window, cx));
        let subscription = cx.subscribe_in(&view, window, Self::on_terminal_event);
        if let Some(command) = command {
            view.update(cx, |view, cx| view.run_command(command, cx));
        }
        TerminalPane {
            id: pane_id,
            custom_name: None,
            name_history: Vec::new(),
            view,
            _subscription: subscription,
        }
    }

    pub(super) fn retry_ssh_pane(
        &mut self,
        tab_ix: usize,
        pane_id: u64,
        destination: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(WorkspaceTab::Terminal { panes, .. }) = self.tabs.get(tab_ix) else { return };
        let Some(old) = panes.iter().find(|pane| pane.id == pane_id) else { return };
        let view = old.view.read(cx);
        if view.ssh_destination.as_deref() != Some(destination.as_str()) {
            return;
        }
        let grid = (view.grid_cols() as u16, view.grid_rows() as u16);
        let cwd = (!view.cwd.is_empty()).then(|| view.cwd.clone());
        let replacement =
            self.new_pane(grid, TerminalLaunch::Ssh { destination, cwd }, None, window, cx);
        self.replace_retry_pane(tab_ix, pane_id, replacement, window, cx);
    }

    pub(super) fn retry_local_pane(
        &mut self,
        tab_ix: usize,
        pane_id: u64,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(WorkspaceTab::Terminal { panes, .. }) = self.tabs.get(tab_ix) else { return };
        let Some(old) = panes.iter().find(|pane| pane.id == pane_id) else { return };
        let view = old.view.read(cx);
        let Some(options) = view.failed_local_options().cloned() else { return };
        let launch = view.session_launch.clone();
        let requests = view.local_startup_requests();
        let grid = (view.grid_cols() as u16, view.grid_rows() as u16);
        let replacement = self.new_pane_with(None, window, cx, |id, window, cx| {
            let mut view = TerminalView::new_local_retry(id, grid, launch, options, window, cx);
            view.resume_local_startup_requests(requests, cx);
            view
        });
        self.replace_retry_pane(tab_ix, pane_id, replacement, window, cx);
    }

    /// The tree, metadata and subscription move together. Old asynchronous
    /// results are scoped to the old entity and cannot address its replacement.
    fn replace_retry_pane(
        &mut self,
        tab_ix: usize,
        pane_id: u64,
        mut replacement: TerminalPane,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let replacement_id = replacement.id;
        let old = {
            let Some(WorkspaceTab::Terminal { panes, tree, focused, .. }) =
                self.tabs.get_mut(tab_ix)
            else {
                replacement.view.read(cx).shutdown();
                return;
            };
            let Some(index) = panes.iter().position(|pane| pane.id == pane_id) else {
                replacement.view.read(cx).shutdown();
                return;
            };
            if !tree.replace_leaf(pane_id, replacement_id) {
                replacement.view.read(cx).shutdown();
                return;
            }
            replacement.custom_name = panes[index].custom_name.clone();
            replacement.name_history = panes[index].name_history.clone();
            if *focused == pane_id {
                *focused = replacement_id;
            }
            std::mem::replace(&mut panes[index], replacement)
        };
        self.forget_pane_rename(pane_id);
        self.runtime_hub.record_pane_closed(self.runtime_window_id, pane_id);
        self.pane_bounds.borrow_mut().remove(&pane_id);
        old.view.read(cx).shutdown();
        if tab_ix == self.active {
            self.focus_active(window, cx);
            self.sync_side_panel_to_active(true, cx);
        }
        cx.notify();
    }
}

#[cfg(all(test, feature = "gpui-test-support"))]
mod tests;
