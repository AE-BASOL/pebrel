# Duplicate takes a live draft snapshot with independent editor ownership

## Status

Proposed. Product layout integration and execution evidence are recorded in the PR.

## Context

Reopening a file copies its saved bytes, not its live draft. Sharing an input
entity instead lets one view change another view's buffer and undo history.

## Evidence

TextFileView owns loaded byte/permission/source metadata, current input, Markdown
history, preview cache and asynchronous operations. The code component owns a
private history that its public API cannot clone.

## Decision

An editor's duplicate snapshot copies source, loaded revision and live draft.
Construction supplies fresh input/focus/cache/tasks/operation ownership. Loaded
dirty text is not reloaded. A loading copy starts its own read. A saving copy
keeps the loaded revision but does not inherit the original save operation.

Markdown copies its owned history and inserts a grouping barrier. Code rebuilds
one independent draft edit over its loaded baseline with public component APIs.
Every save retains the loaded-byte conditional-write contract, so saving one
copy cannot silently bless the other copy's stale revision.

## Rejected alternatives

- A persistent reopen record omits current drafts and history.
- Shared input/history entities allow cross-copy edits and undo.
- Copying pending tasks redirects completion or cancellation between owners.
- Reading private component fields couples the app to unexported internals.

## Consequences

The duplicate authority is independent of workspace layout and persisted format.
Markdown retains its full application-owned history. Code has a deliberately
bounded copied undo baseline; later edits and undo are independent.

## Validation

Real TestWindow regressions check dirty buffers, input/cache identity, independent
Undo/Redo, read-only/error snapshots, save conflicts and independent pending reads.
All documents and peers are isolated fixtures; no user SSH alias is resolved.

## Supersedes

Reopen-from-disk as the only way to construct a second editor for a live draft.

## Revisit when

The code component exposes a supported history snapshot, or draft ownership
moves to a document service with multiple simultaneous views.
