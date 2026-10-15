//! 输入区参与正文布局；行内对象的输入放在所在段落下方，保留段落的基线和换行。

use super::object_source::ObjectSource;
use super::*;
use gpui::ImageCache;

impl TextFileView {
    pub(super) fn render_object_editor(
        &self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> gpui::AnyElement {
        let edit = self.object_edit.as_ref().expect("active object");
        let language = super::super::config::ui_language(cx);
        let image_result = edit.image.as_ref().and_then(|resource| {
            self.preview_images.update(cx, |cache, cx| cache.load(resource, window, cx))
        });
        let error = edit
            .error
            .or_else(|| {
                image_result.as_ref().and_then(|result| {
                    result.as_ref().err().map(|_| Message::EditorObjectImageFailed)
                })
            })
            .or(edit.warning);
        let ready = !edit.changed
            || (!edit.pending
                && edit.error.is_none()
                && (edit.alt.is_none()
                    || image_result.as_ref().is_some_and(|result| result.is_ok())));
        let title = match edit.syntax {
            ObjectSource::Math { .. } => Message::EditorObjectLatexSource,
            ObjectSource::Image { .. } => Message::EditorObjectImage,
        };
        v_flex()
            .id("markdown-object-editor")
            .debug_selector(|| "markdown-object-editor".to_owned())
            .key_context("MarkdownObject")
            .w_full()
            .max_w(px(456.0))
            .min_w_0()
            .mt(px(12.0))
            .mb(px(8.0))
            .p(px(16.0))
            .rounded(px(10.0))
            .border_1()
            .border_color(cx.theme().border)
            .bg(super::super::theme::code_block_background(cx))
            .gap(px(12.0))
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .on_click(|_, _, cx| cx.stop_propagation())
            .child(
                h_flex()
                    .w_full()
                    .min_w_0()
                    .gap(px(10.0))
                    .child(div().text_size(px(13.0)).font_semibold().child(language.text(title)))
                    .when(matches!(edit.syntax, ObjectSource::Math { .. }), |row| {
                        row.child(
                            div()
                                .text_size(px(12.0))
                                .text_color(cx.theme().muted_foreground)
                                .child(language.text(
                                    if matches!(
                                        edit.syntax,
                                        ObjectSource::Math { display: true, .. }
                                    ) {
                                        Message::EditorObjectDisplayMath
                                    } else {
                                        Message::EditorObjectInlineMath
                                    },
                                )),
                        )
                    })
                    .child(div().flex_1())
                    .child(
                        Button::new("object-close")
                            .ghost()
                            .size(px(32.0))
                            .icon(Icon::new(IconName::Close).size(px(16.0)))
                            .tooltip(language.text(Message::EditorCancel))
                            .on_click(cx.listener(|view, _, window, cx| {
                                view.cancel_object_edit(cx);
                                view.focus.focus(window, cx);
                            })),
                    ),
            )
            .when_some(edit.alt.as_ref(), |column, alt| {
                let status = match &image_result {
                    Some(Ok(image)) => {
                        let size = image.size(0);
                        language.format(
                            Message::EditorObjectImageDimensions,
                            &[
                                ("width", &size.width.0.to_string()),
                                ("height", &size.height.0.to_string()),
                            ],
                        )
                    },
                    Some(Err(_)) => language.text(Message::EditorObjectImageFailed).to_owned(),
                    None => language.text(Message::EditorObjectImageLoading).to_owned(),
                };
                column
                    .child(
                        h_flex()
                            .w_full()
                            .min_w_0()
                            .gap(px(12.0))
                            .when_some(edit.image.clone(), |row, resource| {
                                row.child(
                                    gpui::img(gpui::ImageSource::Resource(resource))
                                        .w(px(76.0))
                                        .h(px(49.0))
                                        .object_fit(gpui::ObjectFit::Contain)
                                        .rounded(px(5.0)),
                                )
                            })
                            .child(
                                div()
                                    .flex_1()
                                    .min_w_0()
                                    .text_size(px(12.0))
                                    .text_color(cx.theme().muted_foreground)
                                    .child(status),
                            ),
                    )
                    .child(self.object_field(
                        "markdown-object-address",
                        Message::EditorObjectImageAddress,
                        &edit.input,
                        false,
                        window,
                        cx,
                    ))
                    .child(self.object_field(
                        "markdown-object-alt",
                        Message::EditorObjectImageAlt,
                        alt,
                        false,
                        window,
                        cx,
                    ))
                    .child(
                        Button::new("object-choose-image")
                            .debug_selector(|| "markdown-object-choose-image".to_owned())
                            .outline()
                            .h(px(32.0))
                            .px(px(12.0))
                            .text_size(px(13.0))
                            .label(language.text(Message::EditorObjectChooseImage))
                            .on_click(cx.listener(|view, _, window, cx| {
                                view.choose_object_image(window, cx)
                            })),
                    )
            })
            .when(edit.alt.is_none(), |column| {
                column.child(self.object_field(
                    "markdown-object-latex",
                    Message::EditorObjectLatexSource,
                    &edit.input,
                    true,
                    window,
                    cx,
                ))
            })
            .when_some(error, |column, error| {
                column.child(
                    div()
                        .debug_selector(|| "markdown-object-error".to_owned())
                        .text_size(px(12.0))
                        .text_color(cx.theme().danger)
                        .child(language.text(error)),
                )
            })
            .child(
                h_flex()
                    .w_full()
                    .justify_end()
                    .gap(px(6.0))
                    .child(
                        Button::new("object-cancel")
                            .ghost()
                            .h(px(32.0))
                            .px(px(12.0))
                            .label(language.text(Message::EditorCancel))
                            .on_click(cx.listener(|view, _, window, cx| {
                                view.cancel_object_edit(cx);
                                view.focus.focus(window, cx);
                            })),
                    )
                    .child(
                        Button::new("object-apply")
                            .debug_selector(|| "markdown-object-apply".to_owned())
                            .ghost()
                            .h(px(32.0))
                            .px(px(12.0))
                            .bg(cx.theme().primary.opacity(0.12))
                            .text_color(cx.theme().foreground)
                            .label(language.text(Message::EditorFinishEditing))
                            .disabled(!ready)
                            .on_click(cx.listener(|view, _, window, cx| {
                                view.apply_object_edit(window, cx);
                            })),
                    ),
            )
            .into_any_element()
    }

    fn object_field(
        &self,
        selector: &'static str,
        label: Message,
        input: &Entity<InputState>,
        math: bool,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> gpui::AnyElement {
        let language = super::super::config::ui_language(cx);
        let focused = input.read(cx).focus_handle(cx).is_focused(window);
        let edit = self.object_edit.as_ref().expect("active object");
        let mut lines = edit.focus_lines.borrow_mut();
        let line = &mut lines[usize::from(*input != edit.input)];
        line.set_focused(focused);
        let underline = line.underline(cx.theme().ring);
        v_flex()
            .w_full()
            .min_w_0()
            .gap(px(6.0))
            .when(!math, |column| {
                column.child(
                    div()
                        .text_size(px(12.0))
                        .text_color(cx.theme().muted_foreground)
                        .child(language.text(label)),
                )
            })
            .child(
                div()
                    .id(selector)
                    .debug_selector(move || selector.to_owned())
                    .relative()
                    .w_full()
                    .min_w_0()
                    .rounded_t(px(6.0))
                    .border_b_1()
                    .border_color(cx.theme().border)
                    .bg(cx.theme().background.opacity(0.4))
                    .child(
                        Input::new(input)
                            .appearance(false)
                            .bordered(false)
                            .focus_bordered(false)
                            .w_full()
                            .px(px(10.0))
                            .py(px(8.0))
                            .text_size(px(13.0))
                            .when(math, |input| {
                                input
                                    .font_family(cx.theme().mono_font_family.clone())
                                    .line_height(gpui::relative(1.75))
                            }),
                    )
                    .child(underline),
            )
            .into_any_element()
    }
}
