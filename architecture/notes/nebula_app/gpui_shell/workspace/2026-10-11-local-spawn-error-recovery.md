# Local spawn errors and restoration feedback

## Status

Proposed with the local spawn-error recovery change; runtime validation is pending.

## Context

A saved shell executable may be unavailable on the current machine. The PTY
constructor can report that failure synchronously while a restored split tree
still exists. Previously the retained pane displayed an error without a recovery
action, and rebuilding the tree counted as a successful session restoration.

## Evidence

The local branch of `terminal/view/startup.rs` receives a `session::spawn` error.
`workspace/session_recovery.rs` counts reconstructed tabs. Neither fact proves
an asynchronous SSH connection has authenticated or an intentional command has
remained alive. Short commands legitimately exit both successfully and with a
nonzero status; the shared launch schema does not describe interactive intent.

## Decision

The terminal view owns the explicit local spawn-failure state and the failed
PTY options snapshot. Manual Retry preserves that snapshot, changing only the
new pane/process identity. The workspace owns subscriptions and atomically
replaces the same split leaf, as it already does for SSH Retry. Both actions use
one replacement implementation. Old entity events cannot address the new leaf.

Opening another Shell uses the existing launcher and creates a new tab. The
failed pane remains available as diagnostic material. No default shell, profile
or session schema is rewritten and no background retry is scheduled. Pending
startup commands and saved AI targets stay under the existing startup-command
authority. Cold/update restoration and failed Retry normalize and retain the
target before a shared guard blocks provider discovery and command construction.
An explicit Retry that spawns the shell then uses the existing command/resolver
path; it does not introduce another recovery state machine.

Restoration counts rebuilt tabs and synchronous local spawn failures separately.
Known failures produce a warning and retain their launch snapshots and actions.
Successful reopening describes layout reconstruction, without asserting remote
connection readiness. Update and cold restoration share this counting authority.

## Rejected alternatives

- Classifying an early nonzero exit using a timer, first redraw or missing prompt:
  those observations cannot distinguish a failed interactive launcher from an
  intentional short command.
- Automatically falling back to another shell: it would execute a different
  configured environment and hide the saved launch that failed.
- Rereading defaults during Retry: it could replace the pane's shell, WSL
  distribution or environment and introduce a second launch authority.

## Consequences

Normal child-exit behavior is unchanged. A successfully spawned Windows WSL
launcher that immediately exits is outside this synchronous error boundary and
requires Windows evidence. Frozen PTY overrides stay in memory only while the
failed pane owns them; they are not added to persisted sessions.

## Validation

Focused regressions cover rendered controls, keyboard/padding activation, owned
synthetic missing executables, repaired Retry output and single queued-command
execution after an explicit synthetic prompt, stale events, retained leaf metadata
and focus, mixed/all-failed restoration, and normal short commands. Frozen PTY
options are compared in full, allowing only new pane identity values. A real Pi
source with a synthetic ID stays durable across cold/update restoration and failed
Retry; an observation counter verifies that its native resolver never starts.
The tests require isolated settings, SSH sources and explicit owned Pi directories
before constructing a workspace. The owned Pi roots also contain any discovery
if a regression mutation removes the guard. Runtime results must be recorded
separately from static checks.

The first isolated runtime run passed three children and failed three. Retry
incorrectly reused the full environment preparation: alias expansion runs before
identity insertion, so repeating preparation can add a previously absent legacy
process alias and regenerate runtime/hook values. Retry now refreshes only the
pane/process identity under the existing environment authority; the full frozen
options equality remains required, with diagnostics limited to changed key names.

Two real PTY children also exposed the pinned GPUI TestScheduler's rejection of
native-thread foreground wakes. These fixtures opt into an App-owned test pump
that retains the actual EventReceiver and weak terminal owner. Each test draw
polls that mailbox on the foreground and invokes the existing process_event
authority. The PTY, EventProxy, coalescing mailbox, parser and semantic events
remain real. The normal async session pump and scheduler guard are unchanged;
this fixture does not validate the normal executor's native wake integration.
The corrected runtime run is pending.

## Supersedes

None.

## Revisit when

The launch domain gains an explicit interactive-readiness protocol or verified
Windows WSL evidence establishes another distinct startup failure boundary.
