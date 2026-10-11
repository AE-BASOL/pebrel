//! Window attachment is independent from a document operation's generation.

use super::*;
use gpui::AnyWindowHandle;

pub(super) struct Host {
    window: Option<AnyWindowHandle>,
    pub(super) generation: u64,
    load: Option<Task<()>>,
    reload: Option<Task<()>>,
    pub(super) selection_scroll: Option<Task<()>>,
}

impl Host {
    pub(super) fn new(window: &Window) -> Self {
        Self {
            window: Some(window.window_handle()),
            generation: 0,
            load: None,
            reload: None,
            selection_scroll: None,
        }
    }

    pub(super) fn is_attached(&self) -> bool {
        self.window.is_some()
    }
}

impl TextFileView {
    /// Rebinding preserves in-flight reads/writes and the same draft owner.
    pub(crate) fn attach_host(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let handle = window.window_handle();
        if self.host.window != Some(handle) {
            self.stop_preview_selection_scroll();
            self.details_resize_anchor = None;
            self.host.window = Some(handle);
            cx.notify();
        }
    }

    /// Pointer jobs and live input belong to the source window until detach.
    pub(crate) fn prepare_transfer(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.set_render_active(false, window, cx);
        self.stop_preview_selection_scroll();
        self.details_resize_anchor = None;
        self.clear_reader_focus(cx);
    }

    /// Final removal revokes late completions even when another Entity is retained.
    pub(crate) fn detach_host(&mut self, cx: &mut Context<Self>) {
        self.host.window = None;
        self.host.generation = self.host.generation.wrapping_add(1);
        self.host.load = None;
        self.host.reload = None;
        self.render_active = false;
        self.finish_live_edit(cx);
        self.stop_preview_selection_scroll();
        self.preview_task = None;
        self.inline_views.borrow_mut().clear();
        for block in self.blocks.borrow_mut().iter_mut() {
            *block = None;
        }
        self.preview_images.update(cx, |images, cx| images.retire(cx));
        self.revision = self.revision.wrapping_add(1);
        if let Some(operation) = self.operation.take() {
            operation.cancel();
        }
        self.loading = false;
        self.saving = false;
        cx.notify();
    }

    pub(super) fn load(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.attach_host(window, cx);
        self.host.generation = self.host.generation.wrapping_add(1);
        let generation = self.host.generation;
        self.loading = true;
        self.content_revision = self.content_revision.wrapping_add(1);
        self.revision = self.revision.wrapping_add(1);
        self.preview_task = None;
        self.host.reload = None;
        self.host.load = None;
        let path = self.path.clone();
        let source = self.source.clone();
        let operation = DocumentOperation::default();
        if let Some(previous) = self.operation.replace(operation.clone()) {
            previous.cancel();
        }
        let markdown = self.markdown;
        let resources = super::super::scientific_render::assets(cx);
        #[cfg(all(test, feature = "gpui-test-support"))]
        let injected = tests::take_load(&source);
        let task = cx.background_executor().spawn(async move {
            let _permit = resources.document_permit().await;
            #[cfg(all(test, feature = "gpui-test-support"))]
            let loaded = if let Some(injected) = injected {
                injected.await.map_err(|_| std::io::Error::other("Fixture read cancelled"))?
            } else {
                source.load(operation).await
            };
            #[cfg(not(all(test, feature = "gpui-test-support")))]
            let loaded = source.load(operation).await;
            loaded.map(|doc| {
                let outline = markdown.then(|| Outline::prepare(&doc.text, path.parent()));
                (doc, outline)
            })
        });
        self.host.load = Some(cx.spawn(async move |this, cx| {
            let loaded = task.await;
            let Ok(Some(handle)) = this.update(cx, |view, _| {
                (view.host.generation == generation).then_some(view.host.window).flatten()
            }) else {
                return;
            };
            let _ = handle.update(cx, |_, window, cx| {
                let _ = this.update(cx, |view, cx| {
                    if view.host.generation != generation
                        || view.host.window != Some(window.window_handle())
                    {
                        return;
                    }
                    view.loading = false;
                    view.operation = None;
                    match loaded {
                        Ok((document, outline)) => {
                            view.live_edit = None;
                            view.last_edit_cursor = None;
                            view.resume_edit = false;
                            view.history.reset(&document.text);
                            view.input.update(cx, |input, cx| {
                                input.set_value(document.text.clone(), window, cx)
                            });
                            view.document = Some(document);
                            view.dirty = false;
                            view.notice = None;
                            if let Some(outline) = outline {
                                view.apply_outline(outline, cx);
                            }
                            if view.preview && view.focus.is_focused(window) {
                                view.begin_live_edit(0, window, cx);
                            }
                        },
                        Err(error) => {
                            view.notice = Some(if error.kind() == std::io::ErrorKind::Interrupted {
                                (Message::TransferCancelled, None)
                            } else {
                                (Message::EditorReadFailed, Some(error.to_string()))
                            })
                        },
                    }
                    cx.emit(TextFileEvent::Changed);
                    cx.notify();
                });
            });
        }));
        cx.notify();
    }

    pub(super) fn request_reload(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.saving || self.loading {
            return;
        }
        if !self.dirty {
            self.load(window, cx);
            return;
        }
        let generation = self.host.generation;
        let draft = self.draft(cx);
        let language = super::super::config::ui_language(cx);
        let answer = window.prompt(
            PromptLevel::Warning,
            language.text(Message::EditorDiscardTitle),
            Some(&self.source.display()),
            &[language.text(Message::EditorCancel), language.text(Message::EditorDiscard)],
            cx,
        );
        self.host.reload = Some(cx.spawn(async move |this, cx| {
            if answer.await != Ok(1) {
                return;
            }
            let Ok(Some(handle)) = this.update(cx, |view, cx| {
                (view.host.generation == generation
                    && view.draft(cx) == draft
                    && !view.loading
                    && !view.saving)
                    .then_some(view.host.window)
                    .flatten()
            }) else {
                return;
            };
            let _ = handle.update(cx, |_, window, cx| {
                let _ = this.update(cx, |view, cx| view.load(window, cx));
            });
        }));
    }
}

#[cfg(all(test, feature = "gpui-test-support"))]
mod tests;
