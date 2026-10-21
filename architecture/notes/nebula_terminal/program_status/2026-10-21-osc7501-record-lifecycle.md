# Bounded OSC 7501 records and product status projection

## Status

Implemented against Program Status Protocol revision 0.4 (2026-10-10).

## Context

Issue [#681](https://github.com/Kuddev/pebrel/issues/681) requests program-reported
states instead of deriving Agent activity from titles, progress codes or screen
words. Parsing an escape alone does not establish record lifetime, precedence or
notification ownership. Both local PTYs and built-in SSH already use the same
`StreamProcessor`; its ordered OSC boundary is the protocol entry point.

## Evidence

- [Protocol specification](https://www.superlogical.com/rex/docs/build/program-status)
  defines replacement records, hierarchical ids, limits and prompt/reset lifetime.
- [Motivation](https://mitchellh.com/writing/program-status-osc7501) describes
  experimental integrations; this does not establish support in every released CLI.
- `nebula_terminal/src/program_status.rs` owns validation and the record set.
- `nebula_terminal/tests/osc7501.rs` exercises the actual VT/OSC ingestion path.
- `nebula_app/src/ai_hook/lifecycle.rs` already owns accepted Hook facts. A generic
  program report carries neither authenticated Hook ownership nor a resumable
  Agent session id.

## Decision

Keep at most 256 records on `Term`, independent of either screen grid. Validate
all pairs before replacing a record, including overwritten text and size limits.
Reject invalid ids instead of redirecting them to root. Clear subtrees on segment
boundaries. Resolve `app` from the nearest existing ancestor when read. Keep
updates in LRU order without wall clocks, heartbeat expiry or a new worker.

OSC 133 A and process/transport end drop live records; done/error survive. RIS
clears all records and protocol preference; DECSTR and alternate-screen changes
leave them alone. Move `Term::exit` alongside this lifecycle so local and remote
successful exits use the same cleanup. SSH error handling invokes cleanup while
preserving the existing failed-pane presentation.

Reply to feature queries with a fixed constant before a subsequent DA response.
Ignore reflected replies. Suppress OSC 9;4 progress events after the first valid
7501 report until RIS, including after `clear`, so legacy progress cannot erase a
program's explicit blocked state.

The GPUI product projects the highest-priority record as blocked, error, working,
done, then idle; latest update breaks ties. This is presentation policy, not a
semantic relationship between parent and child work. Pane/transport failure and
accepted live Hook ownership take priority. Otherwise 7501 drives the existing
sidebar, taskbar and Runtime API before screen/process inference. Known `app`
identifiers can identify an Agent; they cannot create a Hook or resumable session.
The Runtime API reports this source as `osc7501`. The legacy renderer keeps the
shared parser/records but does not gain the GPUI status projection.

The event carries an invalidation rather than an allocated copy of every record.
The GPUI mailbox keeps at most one pending status invalidation; consumers read the
latest bounded set. Hook and shell edges are still delivered individually. A
selected state/progress/app projection is copied once for the view, with at most
eight ancestor lookups. Message/title-only updates do not invalidate the UI.

Do not emit additional desktop notifications or sounds. Existing Hook notification
ownership remains intact, and repeated reports cannot generate duplicate alerts.
Free text is stored as inert validated UTF-8 and is not displayed outside the
grid by this change. A future external text presentation must disarm invisible
formatting/direction overrides and retain the originating pane identity.

## Rejected alternatives

- Inferring semantic state from `msg`, title text, or inactivity: the protocol
  explicitly forbids message interpretation and heartbeat expiry.
- Storing only the last report: an idle root would hide a blocked child and
  subtree clearing/app inheritance would become impossible.
- Overwriting authenticated Hooks or importing OSC app names into Hook ownership:
  unrelated programs on the same PTY could change session/answer ownership.
- Mapping each report to a new UI event snapshot or desktop alert: redundant
  allocations and queued work grow with output volume, and Hook alerts duplicate.

## Consequences

No dependencies, settings, persistent data or threads are added. Clients must
actually emit the protocol to benefit. OSC 7501 state does not supply the session
identity, answer payload or actionable permission request provided by Hooks.
Partial/completion-only Hook integrations retain their existing precedence while
their command owns the pane; this implementation does not infer cross-channel
turn ordering. Actual native CLI adoption and multi-platform rendering require
separate acceptance evidence.

## Validation

Core tests cover every split boundary, BEL/ST, malformed and oversized reports,
UTF-8/base64/control rejection, replacement, inherited app, LRU, subtree clearing,
prompt/exit/reset lifetime, fixed query replies and OSC 9;4 precedence. GPUI
regressions cover child attention, identity/evidence, Hook priority, failure,
result retention, no extra notifications, taskbar clearing and mailbox flooding.

On Linux ARM64, `cargo test -p nebula_terminal --locked` passed 340 tests with two
pre-existing ignored tests; the scoped GPUI/Runtime API run passed 76 tests.
The real GPUI product check (including tests), architecture check against main,
format check, 55 governance tests, 99 settings tests, 24 i18n contract tests (one
ignored), and both file-budget tests passed. Windows/macOS native operation and
visual acceptance were not run on this host.

The local ARM64 `rustc -O` sniffer probe used five runs per input: 32,112,640 bytes
of plain output took 43.23–43.57 ms before and 43.32–43.53 ms after; 26,869,760
bytes of mixed ANSI/OSC 9;4 took 104.93–136.38 ms before and 102.49–140.10 ms
after. A separate 100,000-report parse/store probe at the 256-record cap took
32.16 ms and retained 256 records. These isolated measurements are local cost
evidence, not throughput or allocation guarantees for the application.

## Supersedes

None. Hook ownership and screen fallback rules remain authoritative.

## Revisit when

Released clients provide traceable protocol traffic requiring different
projection or Hook arbitration; external message presentation or notifications
are introduced; or bounded record processing is measured as a real bottleneck.
