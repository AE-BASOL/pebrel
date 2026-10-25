# Retract stale screen-owned attention

## Status

Implemented for review against issue #662.

## Context

A transient permission form could set screen-derived Blocked status indefinitely.
Later unmatched snapshots reset only the idle counter. A newer CLI spinner or a
custom multi-line footer could prevent both working and idle rules from matching,
so the tray and agent list continued asking for input after the form disappeared.

## Evidence

The shared lifecycle previously returned immediately for `observe_screen(None)`.
Its regression now runs real Claude screen detection over a permission form and
a spinner/prompt with four footer lines. The latter has no matching rule, exposing
the same persistent-attention path described by the report.

## Decision

Only screen-owned Blocked status expires after three consecutive observations
without blocked evidence. A renewed form resets that count. Preserve the status
source and rule of a preceding Working state and restore that evidence; otherwise
retract to Unknown. Neither absence nor retraction fabricates task completion.
Usable positive working evidence still takes precedence over retraction.

Accepted hook events clear the saved screen fallback and counters. Full lifecycle
hooks retain their exclusive state authority. Turn-only hooks can temporarily
borrow attention from screen rules and recover their prior Working state when the
form disappears, while their completion event remains authoritative. Explicit
submissions and command boundaries discard old recovery state.

## Rejected alternatives

- Keeping every unmatched state forever preserves expired screen evidence.
- A blanket timeout for Blocked would also retract real hook-owned permissions.
- Inferring Done or a new Working turn from absence would create false completion
  notifications or restart a completed hook-owned task.
- Matching generic spinner glyphs alone confuses live work with completed summary
  lines. This fix addresses lifecycle expiry without broadening those heuristics.

## Consequences

Both desktop shells use the same lifecycle authority on all supported platforms.
One bounded counter and a saved source/rule apply only to screen attention; there
is no new timer, thread, I/O, dependency or persisted format. Rule text is cloned
once on entry from Working, not on every screen poll. Three observations tolerate
transient redraws; they are not a universal wall-clock timeout.

## Validation

Regressions cover the reported unmatched layout, disappearing and renewed forms,
positive work at the expiry boundary, unknown/prior-completed states, full hook
authority, turn-only hook recovery and absence of duplicate completion notices.
The 19 lifecycle tests and all 130 executed hook tests pass locally (one opt-in
native test remains ignored). Production GPUI compilation with test targets,
formatting, architecture and platform-cfg checks also pass.
The report's suspected hook-to-pane association gap is not established by these
screen-state tests and requires its own identity evidence.

## Supersedes

None; narrows the previous retain-on-no-match policy to evidence that still has
authority, preserving the existing hook and completion contracts.

## Revisit when

Screen observations expose a typed distinction between a temporarily incomplete
redraw and a complete frame without the prior form.
