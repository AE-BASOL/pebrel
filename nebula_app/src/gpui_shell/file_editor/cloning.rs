//! A duplicate owns its draft, history and asynchronous work independently.

use super::*;

pub(in crate::gpui_shell) struct DraftSnapshot {
    source: DocumentSource,
    document: Option<LoadedDocument>,
    draft: SharedString,
    history: edit_history::EditHistory,
    loading: bool,
    notice: Option<(Message, Option<String>)>,
    preview: bool,
    live_mode: bool,
    show_details: bool,
    info: bool,
    details_width: f32,
    revision: u64,
    content_revision: u64,
}

impl TextFileView {
    pub(in crate::gpui_shell) fn duplicate_snapshot(&self, cx: &App) -> DraftSnapshot {
        DraftSnapshot {
            source: self.source.clone(),
            document: self.document.clone(),
            draft: self.draft(cx),
            history: self.history.clone(),
            loading: self.loading,
            notice: self.notice.clone(),
            preview: self.preview,
            live_mode: self.live_mode,
            show_details: self.show_details,
            info: self.info,
            details_width: self.details_width,
            revision: self.revision,
            content_revision: self.content_revision,
        }
    }

    pub(in crate::gpui_shell) fn from_duplicate(
        snapshot: DraftSnapshot,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let mut this = Self::new_empty_with_source(snapshot.source, window, cx);
        this.document = snapshot.document;
        this.input.update(cx, |input, cx| {
            // The component's private code-input history is rebuilt as one
            // draft edit over the loaded revision, never shared with its source.
            let baseline = this.document.as_ref().map(|doc| SharedString::from(doc.text.clone()));
            input.set_value(baseline.unwrap_or_else(|| snapshot.draft.clone()), window, cx);
            if input.value() != snapshot.draft {
                input.replace_all(snapshot.draft.clone(), window, cx);
            }
        });
        this.history = snapshot.history;
        this.history.barrier();
        this.dirty = this.document.as_ref().is_some_and(|doc| snapshot.draft.as_ref() != doc.text);
        this.notice = snapshot.notice;
        this.preview = snapshot.preview;
        this.live_mode = snapshot.live_mode;
        this.show_details = snapshot.show_details;
        this.info = snapshot.info;
        this.details_width = snapshot.details_width;
        this.revision = snapshot.revision;
        this.content_revision = snapshot.content_revision;
        if snapshot.loading {
            // Each loading duplicate has its own operation and late-result owner.
            this.load(window, cx);
        } else if this.markdown && this.document.is_some() {
            this.schedule_preview(cx);
        }
        this
    }
}

#[cfg(all(test, feature = "gpui-test-support"))]
mod tests;
