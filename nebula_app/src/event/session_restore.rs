//! Legacy startup crash-loop and restoration notice authority.

use crate::window_context::WindowBoot;

pub(super) fn legacy_startup(plain_launch: bool) -> (WindowBoot, Option<String>, Option<String>) {
    let mut restored_notice = None;
    let mut blocked_notice = None;
    let restore = if plain_launch && crate::display::restore_session_enabled() {
        match crate::session::load() {
            Some(mut session) if crate::session::should_restore(&session) => {
                if crate::session::was_crash(&session) {
                    restored_notice = Some(format!(
                        "已从上次异常退出恢复 {} 个标签（进程未正常收尾）。",
                        session.tabs.len()
                    ));
                }
                // Count this launch against the crash-loop breaker; the
                // first successful autosave (1 Hz tick) resets it.
                crate::session::mark_boot_attempt(&mut session);
                Some(session)
            },
            // 断路器跳闸：连续三次启动都没活到第一次自动保存。把这份
            // 会话隔离出去再干净启动，否则一秒后的自动保存就会盖掉这份
            // 「一恢复就崩」的唯一现场。
            Some(session) if !session.tabs.is_empty() => {
                blocked_notice = Some(match crate::session::quarantine() {
                    Some(path) => format!(
                        "连续三次启动失败，已跳过会话恢复；上次的会话保存在 {}。",
                        path.display()
                    ),
                    None => "连续三次启动失败，已跳过会话恢复。".to_owned(),
                });
                None
            },
            _ => None,
        }
    } else {
        None
    };
    (restore.map_or(WindowBoot::Fresh, WindowBoot::Restore), restored_notice, blocked_notice)
}
