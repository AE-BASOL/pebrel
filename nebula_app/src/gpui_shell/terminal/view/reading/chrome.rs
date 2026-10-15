//! 小浮层只覆盖网格；复制是主动作，执行列表仅保存核心给出的有界元数据。
use super::*;
use gpui::prelude::FluentBuilder as _;
use gpui_component::{
    Disableable as _, IconName, Selectable as _,
    button::{Button, ButtonVariants as _},
    input::Input,
};

impl TerminalView {
    pub(in crate::gpui_shell::terminal) fn reading_chrome(
        &mut self,
        cx: &mut Context<Self>,
    ) -> gpui::AnyElement {
        let language = crate::gpui_shell::config::ui_language(cx);
        let t = |message| language.text(message);
        let Some(panel) = &self.reading.panel else {
            let mut entry = div().absolute().top_1().right_3().occlude().child(
                Button::new("terminal-find")
                    .debug_selector(|| "terminal-find".to_owned())
                    .ghost()
                    .size(px(32.0))
                    .icon(IconName::Search)
                    .aria_label(t(Message::TerminalReadingOpen))
                    .tooltip(t(Message::TerminalReadingShortcut))
                    .on_click(
                        cx.listener(|view, _, window, cx| view.open_reading(false, window, cx)),
                    ),
            );
            // 搜索入口只消费自己的命中区域，不把点击继续交给终端鼠标协议。
            for button in [MouseButton::Left, MouseButton::Right, MouseButton::Middle] {
                entry = entry
                    .on_mouse_down(button, |_, _, cx| cx.stop_propagation())
                    .on_mouse_up(button, |_, _, cx| cx.stop_propagation());
            }
            return div()
                .absolute()
                .inset_0()
                .child(self.output_hover_chrome(cx))
                .child(entry)
                .into_any_element();
        };
        let copied = panel.feedback.read(cx).is_copied();
        let pending = panel.feedback.read(cx).is_pending();
        let unavailable = panel.scope.is_none() || panel.source.is_none();
        let selected = panel.selected.map_or(0, |i| i + 1);
        let count = language.format(
            Message::TerminalReadingCount,
            &[("current", &selected.to_string()), ("total", &panel.hits.len().to_string())],
        );
        let title = panel.scope.map_or_else(
            || t(Message::TerminalReadingAll).to_owned(),
            |id| {
                panel
                    .commands
                    .iter()
                    .find(|command| command.id == id)
                    .map(|command| command.command.clone())
                    .filter(|label| !label.is_empty())
                    .unwrap_or_else(|| {
                        language
                            .format(Message::TerminalReadingExecution, &[("id", &id.to_string())])
                    })
            },
        );
        let mut overlay = div()
            .id("terminal-reading-overlay")
            .debug_selector(|| "terminal-reading-overlay".to_owned())
            .w_full()
            .max_w(px(490.0))
            .occlude()
            .flex()
            .flex_col()
            .gap_2()
            .p_2()
            .rounded_md()
            .border_1()
            .border_color(cx.theme().border)
            .bg(cx.theme().popover)
            .text_color(cx.theme().foreground)
            .text_sm()
            .shadow_sm()
            .on_key_down(cx.listener(Self::reading_key))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_1()
                    .child(div().flex_1().min_w_0().child(t(Message::TerminalReadingOpen)))
                    .child(
                        Button::new("reading-close")
                            .icon(IconName::Close)
                            .small()
                            .aria_label(t(Message::TerminalReadingClose))
                            .tooltip(t(Message::TerminalReadingClose))
                            .on_click(
                                cx.listener(|view, _, window, cx| view.close_reading(window, cx)),
                            ),
                    ),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_1()
                    .child(
                        div()
                            .id("reading-input-wrap")
                            .debug_selector(|| "reading-input".to_owned())
                            .relative()
                            .flex_1()
                            .min_w_0()
                            .border_b_1()
                            .border_color(cx.theme().border)
                            .child(
                                Input::new(&panel.input)
                                    .small()
                                    .appearance(false)
                                    .bordered(false)
                                    .focus_bordered(false),
                            )
                            .child(panel.focus_line.underline(cx.theme().ring)),
                    )
                    .child(
                        Button::new("reading-previous")
                            .icon(IconName::ChevronUp)
                            .small()
                            .aria_label(t(Message::TerminalReadingPrevious))
                            .tooltip(t(Message::TerminalReadingPrevious))
                            .disabled(panel.hits.is_empty())
                            .on_click(
                                cx.listener(|view, _, _, cx| view.navigate_reading_match(true, cx)),
                            ),
                    )
                    .child(
                        Button::new("reading-next")
                            .icon(IconName::ChevronDown)
                            .small()
                            .aria_label(t(Message::TerminalReadingNext))
                            .tooltip(t(Message::TerminalReadingNext))
                            .disabled(panel.hits.is_empty())
                            .on_click(
                                cx.listener(|view, _, _, cx| {
                                    view.navigate_reading_match(false, cx)
                                }),
                            ),
                    )
                    .child(div().child(count)),
            )
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .items_center()
                    .gap_1()
                    .child(
                        Button::new("reading-match-case")
                            .ghost()
                            .small()
                            .label("Aa")
                            .selected(panel.match_case)
                            .aria_label(t(Message::TerminalReadingMatchCase))
                            .tooltip(t(Message::TerminalReadingMatchCase))
                            .on_click(cx.listener(|view, _, _, cx| {
                                if let Some(panel) = view.reading.panel.as_mut() {
                                    panel.match_case = !panel.match_case;
                                }
                                view.restart_reading_search(cx);
                            })),
                    )
                    .child(
                        Button::new("reading-refresh")
                            .ghost()
                            .small()
                            .icon(IconName::Redo)
                            .aria_label(t(Message::TerminalReadingRefresh))
                            .tooltip(t(Message::TerminalReadingRefresh))
                            .on_click(
                                cx.listener(|view, _, _, cx| view.restart_reading_search(cx)),
                            ),
                    )
                    .child(
                        Button::new("reading-scope")
                            .icon(IconName::Menu)
                            .small()
                            .label(t(Message::TerminalReadingChoose))
                            .on_click(cx.listener(|view, _, _, cx| {
                                view.refresh_reading_commands(cx);
                                if let Some(panel) = view.reading.panel.as_mut() {
                                    panel.commands_open = !panel.commands_open;
                                }
                                cx.notify();
                            })),
                    )
                    .child(
                        Button::new("reading-copy")
                            .small()
                            .primary()
                            .icon(if copied { IconName::Check } else { IconName::Copy })
                            .label(t(if copied {
                                Message::TerminalReadingCopied
                            } else if pending {
                                Message::TerminalReadingCopying
                            } else {
                                Message::TerminalReadingCopy
                            }))
                            .disabled(unavailable || pending)
                            .on_click(cx.listener(|view, _, _, cx| view.copy_reading_command(cx))),
                    )
                    .child(
                        Button::new("reading-start")
                            .icon(IconName::ChevronUp)
                            .small()
                            .aria_label(t(Message::TerminalReadingStart))
                            .tooltip(t(Message::TerminalReadingStart))
                            .disabled(unavailable)
                            .on_click(
                                cx.listener(|view, _, _, cx| view.jump_reading_command(false, cx)),
                            ),
                    )
                    .child(
                        Button::new("reading-end")
                            .icon(IconName::ChevronDown)
                            .small()
                            .aria_label(t(Message::TerminalReadingEnd))
                            .tooltip(t(Message::TerminalReadingEnd))
                            .disabled(unavailable)
                            .on_click(
                                cx.listener(|view, _, _, cx| view.jump_reading_command(true, cx)),
                            ),
                    ),
            )
            .child(div().min_w_0().truncate().child(title));
        if panel.commands_open {
            let mut list = div()
                .id("reading-command-list")
                .flex()
                .flex_col()
                .gap_1()
                .max_h(px(180.0))
                .overflow_y_scroll()
                .child(
                    Button::new("reading-all")
                        .small()
                        .label(t(Message::TerminalReadingAll))
                        .on_click(
                            cx.listener(|view, _, _, cx| view.choose_reading_scope(None, cx)),
                        ),
                );
            if panel.commands.is_empty() {
                list = list.child(div().child(t(Message::TerminalReadingNoCommands)));
            }
            for command in &panel.commands {
                let id = command.id;
                let label = if command.command.is_empty() {
                    language.format(Message::TerminalReadingExecution, &[("id", &id.to_string())])
                } else {
                    command.command.clone()
                };
                list = list.child(
                    div()
                        .flex()
                        .items_center()
                        .gap_1()
                        .child(
                            Button::new(("reading-command", id))
                                .small()
                                .label(label.clone())
                                .tooltip(label)
                                .on_click(cx.listener(move |view, _, _, cx| {
                                    view.choose_reading_scope(Some(id), cx)
                                })),
                        )
                        .child(
                            Button::new(("reading-command-copy", id))
                                .small()
                                .icon(IconName::Copy)
                                .aria_label(t(Message::TerminalReadingCopy))
                                .tooltip(t(Message::TerminalReadingCopy))
                                .on_click(cx.listener(move |view, _, _, cx| {
                                    view.choose_reading_scope(Some(id), cx);
                                    view.copy_reading_command(cx);
                                })),
                        ),
                );
            }
            overlay = overlay.child(list);
        }
        let notice = panel
            .notice
            .map(|issue| match issue {
                ReadingIssue::Changed => Message::TerminalReadingChanged,
                ReadingIssue::Unavailable => Message::TerminalReadingUnavailable,
                ReadingIssue::QueryTooLong => Message::TerminalReadingQueryLimit,
                ReadingIssue::CellTooLarge => Message::TerminalReadingCellLimit,
                ReadingIssue::CopyTooLarge => Message::TerminalReadingCopyLimit,
                ReadingIssue::EmptyOutput => Message::TerminalReadingEmpty,
            })
            .or_else(|| {
                if panel.running {
                    Some(Message::TerminalReadingSearching)
                } else if panel.truncated {
                    Some(Message::TerminalReadingTruncated)
                } else if panel.hits.is_empty() && !panel.input.read(cx).value().is_empty() {
                    Some(Message::TerminalReadingNoMatches)
                } else {
                    None
                }
            });
        overlay =
            overlay.when_some(notice, |overlay, notice| overlay.child(div().child(t(notice))));
        // Bubble listeners let the Input/Button consume its own event first; occlude alone
        // does not prevent ancestor terminal handlers from reporting mouse input to a TUI.
        for button in [MouseButton::Left, MouseButton::Right, MouseButton::Middle] {
            overlay = overlay
                .on_mouse_down(button, |_, _, cx| cx.stop_propagation())
                .on_mouse_up(button, |_, _, cx| cx.stop_propagation());
        }
        let overlay = overlay
            .on_mouse_move(|_, _, cx| cx.stop_propagation())
            .on_scroll_wheel(|_, _, cx| cx.stop_propagation());
        // 外层负责可用宽度，避免 max-width:100% 再加 right inset 后越过左边界。
        div()
            .absolute()
            .top_1()
            .left_3()
            .right_3()
            .flex()
            .justify_end()
            .child(overlay)
            .into_any_element()
    }
}
