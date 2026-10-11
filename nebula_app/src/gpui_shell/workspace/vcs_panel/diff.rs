//! Git row activation captures the repository; a patch is never a file editor.
use super::*;
use crate::display::side_panel::diff::DiffKey;

impl NebulaWorkspace {
    pub(super) fn open_git_diff_tab(
        &mut self,
        location: crate::display::side_panel::GitLocation,
        relative_path: String,
        staged: bool,
        focus: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let key = DiffKey { location, path: relative_path, staged };
        let existing = self.tabs.iter().position(|tab| {
            matches!(tab, WorkspaceTab::Code { view, .. } if view.read(cx).matches_git_diff(&key, cx))
        });
        if let Some(index) = existing {
            self.active = index;
            self.reveal_active_tab();
        } else {
            let view = cx
                .new(|cx| crate::gpui_shell::code_tab::CodeTabView::new_git_diff(key, window, cx));
            let subscription = cx.subscribe(&view, Self::on_code_tab_event);
            self.insert_new_tab(WorkspaceTab::Code { view, _subscription: subscription });
        }
        // Mouse activation leaves the changed-file row focused so the native second click
        // still opens its real file. Enter explicitly transfers focus into the patch.
        if focus {
            self.focus_active(window, cx);
        }
        cx.notify();
    }
}
