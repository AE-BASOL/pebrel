//! Virtual, read-only patch rows; no filesystem editor or save/dirty authority.
use super::*;
use crate::display::side_panel::diff::{DiffKey, DiffPatch};
use crate::gpui_shell::copy_feedback::CopyFeedback;
use crate::i18n::Message;
use gpui::{
    FocusHandle, Focusable, InteractiveElement as _, KeyDownEvent, StatefulInteractiveElement as _,
    Subscription, Task,
};

pub(super) struct GitDiffView {
    pub(super) key: DiffKey,
    focus: FocusHandle,
    scroll: gpui::UniformListScrollHandle,
    patch: Option<DiffPatch>,
    rows: Vec<String>,
    longest: usize,
    wrapped: bool,
    error: Option<String>,
    generation: u64,
    loading: Option<Task<()>>,
    copy_feedback: Entity<CopyFeedback>,
    _copy_feedback_subscription: Subscription,
    #[cfg(test)]
    rendered: usize,
}

impl GitDiffView {
    fn new(key: DiffKey, cx: &mut Context<Self>) -> Self {
        let copy_feedback = cx.new(|_| CopyFeedback::new());
        let copy_feedback_subscription = cx.observe(&copy_feedback, |_, _, cx| cx.notify());
        let mut view = Self {
            key,
            focus: cx.focus_handle(),
            scroll: gpui::UniformListScrollHandle::new(),
            patch: None,
            rows: Vec::new(),
            longest: 0,
            wrapped: false,
            error: None,
            generation: 0,
            loading: None,
            copy_feedback,
            _copy_feedback_subscription: copy_feedback_subscription,
            #[cfg(test)]
            rendered: 0,
        };
        view.refresh(cx);
        view
    }

    pub(super) fn refresh(&mut self, cx: &mut Context<Self>) {
        self.copy_feedback.update(cx, |feedback, cx| feedback.clear(cx));
        self.generation = self.generation.wrapping_add(1);
        let generation = self.generation;
        let key = self.key.clone();
        self.patch = None;
        self.rows.clear();
        self.longest = 0;
        self.wrapped = false;
        self.error = None;
        let worker = cx
            .background_executor()
            .spawn(async move { crate::display::side_panel::diff::load(&key) });
        self.loading = Some(cx.spawn(async move |this, cx| {
            let result = worker.await;
            let _ = this.update(cx, |view, cx| {
                if view.generation != generation {
                    return;
                }
                match result {
                    Ok(patch) => {
                        // Bound shaping too: a single generated/minified line can fill the
                        // entire capture. Preserve exact bytes in Copy, wrap preview chunks.
                        for line in patch.text.lines() {
                            let chars: Vec<_> = line.chars().collect();
                            view.wrapped |= chars.len() > 512;
                            if chars.is_empty() {
                                view.rows.push(String::new());
                            }
                            for (index, chunk) in chars.chunks(512).enumerate() {
                                let mut row =
                                    if index == 0 { String::new() } else { "↪ ".into() };
                                row.extend(chunk);
                                view.rows.push(row);
                            }
                        }
                        view.longest =
                            view.rows.iter().map(|row| row.chars().count()).max().unwrap_or(0);
                        view.patch = Some(patch);
                    },
                    Err(error) => view.error = Some(error),
                }
                view.loading = None;
                cx.notify();
            });
        }));
        cx.notify();
    }
}

impl Focusable for GitDiffView {
    fn focus_handle(&self, _: &gpui::App) -> FocusHandle {
        self.focus.clone()
    }
}

impl Render for GitDiffView {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let language = crate::gpui_shell::config::ui_language(cx);
        let theme = cx.theme().clone();
        let copied = self.copy_feedback.read(cx).is_copied();
        let status = if let Some(error) = &self.error {
            format!("{}: {error}", language.text(Message::VcsDiffFailed))
        } else if self.loading.is_some() {
            language.text(Message::VcsDiffLoading).into()
        } else if self.rows.is_empty() {
            language.text(Message::VcsDiffEmpty).into()
        } else {
            language.text(Message::VcsDiffReadonly).into()
        };
        let source =
            if self.key.staged { Message::VcsDiffStaged } else { Message::VcsDiffWorktree };
        let provenance = self.patch.as_ref().map_or_else(
            || self.key.label(),
            |patch| {
                let root = match &self.key.location {
                    GitLocation::Wsl { distro, .. } => format!("{distro}:{}", patch.root),
                    _ => patch.root.clone(),
                };
                format!("{root} / {}", self.key.path)
            },
        );
        let warning = self.patch.as_ref().is_some_and(|patch| patch.truncated || patch.lossy);
        let row_width =
            px((self.longest as f32 * f32::from(theme.mono_font_size) * 0.65 + 60.0).max(400.0));
        v_flex()
            .id("git-diff-view")
            .debug_selector(|| "git-diff-view".into())
            .track_focus(&self.focus)
            .size_full()
            .overflow_hidden()
            .on_key_down(cx.listener(|this, event: &KeyDownEvent, window, cx| {
                if this.focus.contains_focused(window, cx) && !event.keystroke.modifiers.modified()
                {
                    let next = match event.keystroke.key.as_str() {
                        "home" => Some(0),
                        "end" => Some(this.rows.len().saturating_sub(1)),
                        _ => None,
                    };
                    if let Some(row) = next {
                        this.scroll.scroll_to_item(row, gpui::ScrollStrategy::Top);
                        cx.stop_propagation();
                        cx.notify();
                    }
                }
            }))
            .child(
                h_flex()
                    .h(px(38.0))
                    .px_3()
                    .gap_2()
                    .items_center()
                    .flex_shrink_0()
                    .border_b_1()
                    .border_color(theme.border)
                    .child(div().min_w_0().flex_1().truncate().child(language.text(source)))
                    .child(
                        Button::new("git-diff-refresh")
                            .debug_selector(|| "git-diff-refresh".into())
                            .label(language.text(Message::VcsDiffRefresh))
                            .small()
                            .on_click(cx.listener(|this, _, _, cx| this.refresh(cx))),
                    )
                    .child(
                        div()
                            .w(px(120.0))
                            .flex_shrink_0()
                            .debug_selector(move || {
                                if copied { "git-diff-copy-success" } else { "git-diff-copy-ready" }
                                    .into()
                            })
                            .child(
                                Button::new("git-diff-copy")
                                    .debug_selector(|| "git-diff-copy".into())
                                    .w_full()
                                    .icon(if copied { IconName::Check } else { IconName::Copy })
                                    .label(language.text(if copied {
                                        Message::VcsDiffCopied
                                    } else {
                                        Message::VcsDiffCopy
                                    }))
                                    .small()
                                    .disabled(self.patch.is_none())
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        if let Some(patch) = &this.patch {
                                            cx.write_to_clipboard(ClipboardItem::new_string(
                                                patch.text.clone(),
                                            ));
                                            this.copy_feedback.update(cx, |feedback, cx| {
                                                feedback.mark_copied(cx)
                                            });
                                        }
                                    })),
                            ),
                    ),
            )
            .child(div().px_3().py_1().text_xs().child(provenance))
            .child(
                div()
                    .debug_selector(|| "git-diff-status".into())
                    .px_3()
                    .py_1()
                    .text_xs()
                    .text_color(if self.error.is_some() {
                        theme.danger
                    } else {
                        theme.muted_foreground
                    })
                    .child(status),
            )
            .when(warning, |view| {
                view.child(
                    div()
                        .px_3()
                        .py_1()
                        .text_xs()
                        .text_color(theme.warning)
                        .child(language.text(Message::VcsDiffCaptureWarning)),
                )
            })
            .when(self.wrapped, |view| {
                view.child(
                    div()
                        .px_3()
                        .py_1()
                        .text_xs()
                        .text_color(theme.muted_foreground)
                        .child(language.text(Message::VcsDiffWrapped)),
                )
            })
            .child(
                div().id("git-diff-scroll").flex_1().min_h_0().overflow_x_scroll().child(
                    gpui::uniform_list(
                        "git-diff-lines",
                        self.rows.len(),
                        cx.processor(move |this, range: std::ops::Range<usize>, _, cx| {
                            #[cfg(test)]
                            {
                                this.rendered = range.len();
                            }
                            let theme = cx.theme();
                            range
                                .map(|index| {
                                    let text = &this.rows[index];
                                    let color = match text.as_bytes().first() {
                                        Some(b'+') => theme.success,
                                        Some(b'-') => theme.danger,
                                        Some(b'@') => theme.primary,
                                        _ => theme.foreground,
                                    };
                                    h_flex()
                                        .h(px(24.0))
                                        .gap_2()
                                        .font_family(theme.mono_font_family.clone())
                                        .text_size(theme.mono_font_size)
                                        .text_color(color)
                                        .child(
                                            div()
                                                .w(px(48.0))
                                                .text_color(theme.muted_foreground)
                                                .child((index + 1).to_string()),
                                        )
                                        .child(text.clone())
                                        .into_any_element()
                                })
                                .collect()
                        }),
                    )
                    .w(row_width)
                    .h_full()
                    .track_scroll(&self.scroll),
                ),
            )
    }
}

impl CodeTabView {
    pub(crate) fn new_git_diff(key: DiffKey, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let language = crate::gpui_shell::config::ui_language(cx);
        let source =
            language.text(if key.staged { Message::VcsStaged } else { Message::VcsChanges });
        let title =
            language.format(Message::VcsDiffTitle, &[("path", &key.path), ("source", source)]);
        let path = PathBuf::from(&key.path);
        let input = code_input("text", window, cx);
        let diff = cx.new(|cx| GitDiffView::new(key, cx));
        Self {
            path,
            title,
            input,
            notice: None,
            lines: 0,
            merge: None,
            file: None,
            diff: Some(diff),
            _file_subscription: None,
        }
    }

    pub(crate) fn matches_git_diff(&self, key: &DiffKey, cx: &gpui::App) -> bool {
        self.diff.as_ref().is_some_and(|diff| diff.read(cx).key == *key)
    }

    #[cfg(test)]
    pub(crate) fn diff_text(&self, cx: &gpui::App) -> Option<String> {
        self.diff.as_ref()?.read(cx).patch.as_ref().map(|patch| patch.text.clone())
    }

    #[cfg(test)]
    pub(crate) fn diff_error(&self, cx: &gpui::App) -> Option<String> {
        self.diff.as_ref()?.read(cx).error.clone()
    }

    #[cfg(test)]
    pub(crate) fn diff_preview_stats(&self, cx: &gpui::App) -> (usize, usize, bool, usize) {
        let diff = self.diff.as_ref().unwrap().read(cx);
        (diff.rows.len(), diff.longest, diff.wrapped, diff.rendered)
    }

    #[cfg(test)]
    pub(crate) fn diff_feedback(&self, cx: &gpui::App) -> gpui::WeakEntity<CopyFeedback> {
        self.diff.as_ref().unwrap().read(cx).copy_feedback.downgrade()
    }
}
