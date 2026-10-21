//! Program records share the PTY lifecycle, never the active grid's lifetime.
use super::Term;
use crate::event::{Event, EventListener};
use crate::program_status::{FEATURE_REPLY, Records, Report};

impl<T> Term<T> {
    pub fn program_statuses(&self) -> &Records {
        &self.program_statuses
    }
}

impl<T: EventListener> Term<T> {
    pub(crate) fn report_program_status(&mut self, report: Report) {
        if report == Report::Query {
            self.event_proxy.send_event(Event::PtyWrite(FEATURE_REPLY.into()));
        } else if self.program_statuses.apply(report) {
            self.event_proxy.send_event(Event::ProgramStatusChanged);
        }
    }

    /// Drop live records at a shell prompt or transport end, retaining results.
    pub fn end_program_status_activity(&mut self) {
        if self.program_statuses.command_ended() {
            self.event_proxy.send_event(Event::ProgramStatusChanged);
        }
    }

    pub(super) fn reset_program_status(&mut self) {
        if self.program_statuses.reset() {
            self.event_proxy.send_event(Event::ProgramStatusChanged);
        }
    }

    pub fn exit(&mut self) {
        self.end_program_status_activity();
        self.event_proxy.send_event(Event::Exit);
    }
}
