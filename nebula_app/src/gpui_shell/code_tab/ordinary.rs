//! Ordinary code tabs own the shared editor; merge tools keep a separate model.

use super::*;
use crate::gpui_shell::file_editor;

impl CodeTabView {
    pub fn new(path: PathBuf, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let file = cx.new(|cx| file_editor::TextFileView::new(path, window, cx));
        Self::with_file(file, window, cx)
    }

    pub(super) fn from_duplicate(
        snapshot: file_editor::DraftSnapshot,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let file = cx.new(|cx| file_editor::TextFileView::from_duplicate(snapshot, window, cx));
        Self::with_file(file, window, cx)
    }

    fn with_file(
        file: Entity<file_editor::TextFileView>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let path = file.read(cx).path.clone();
        let title = file.read(cx).title.clone();
        let language = language_for_path(&path.to_string_lossy());
        let input = code_input(language, window, cx);
        let subscription = cx.subscribe(&file, |_, _, _, cx| {
            cx.emit(CodeTabViewEvent::Changed);
            cx.notify();
        });
        Self {
            path,
            title,
            input,
            notice: None,
            lines: 0,
            merge: None,
            file: Some(file),
            _file_subscription: Some(subscription),
        }
    }
}
