# Legacy restore adapter authority

## Status

Status: Proposed

The decision remains proposed; root-owned v2 validation is Pending.

## Context

### Trigger

Mixed file/terminal layouts need a typed durable format. Legacy window and startup adapters must remain inspectable when their terminal-only projection is introduced later.

### Inputs

The existing terminal session schema, startup restore preference, crash-loop counter, workspace import/export dialogs, and WindowContext pane allocation.

### State owner

`window_context/session_restore.rs` owns workspace import/export and terminal restore methods. `event/session_restore.rs` owns startup selection and its two notice channels. Session persistence remains in the session authority.

### Dependency direction

Event and window adapters call shared session rules. Session rules do not depend on either renderer.

## Evidence

The extracted legacy adapters retain the shared session authority and its existing restore contracts.

Implementation and acceptance scope: [nebula_app/src/window_context/session_restore.rs](../../../../nebula_app/src/window_context/session_restore.rs), [nebula_app/src/event/session_restore.rs](../../../../nebula_app/src/event/session_restore.rs), [nebula_app/src/session.rs](../../../../nebula_app/src/session.rs).
Execution results remain Pending in Validation.

## Decision

Move the existing implementations unchanged into their responsible child modules. Only sibling-call visibility changes. This stage preserves the existing format, spawn order, crash-loop behavior, and notices.

## Rejected alternatives

Adding mixed schema policy directly to already large window/event modules would obscure their adapter boundary. Moving unrelated handlers would enlarge this mechanical prerequisite without improving the boundary.

## Consequences

### Lifecycle and failures

The existing seed retention after spawn failure and import failure messages remain intact. No additional task, disk operation, or PTY is created by this stage.

Later format work can add safe terminal projection at one legacy authority. This extraction alone does not implement file panes or change persistence compatibility.

## Validation

Pending. Root-owned v2 compilation, behavior, green/red and native validation
remain outstanding. The source-handoff statements below retain their original
scope and do not establish a completed v2 run.

Pinned rustfmt, diff checks, and baseline architecture checks are performed before handoff. Parent executes the existing restore/session tests and both supported product builds; this note does not claim compilation or tests already passed.

## Supersedes

None. This extraction preserves the existing restore decision.

## Revisit when

Mixed session formats or startup/import failure ownership changes.
