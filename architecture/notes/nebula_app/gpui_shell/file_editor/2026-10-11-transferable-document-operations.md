# Document operations follow their current pane host

## Status

Proposed with #53. Runtime validation outcomes are recorded in the PR.

## Context

A file pane can retain its editor entity while moving to another window. Its
draft, undo history, source identity and in-flight operation belong to that
entity. Pointer selection and window-bound input belong to its current host.

## Evidence

The original load and dirty-reload callbacks captured their construction
WindowHandle. After the source window closes, that handle cannot apply a result
to the live transferred editor. Markdown selection scrolling also captured the
old window and detached its timer task.

GPUI dynamically routes entity subscriptions to the most recently rendered
window. That mapping does not establish a destination before its first draw.
The explicit attachment and completion checks live in
[`file_editor/host.rs`](../../../../../nebula_app/src/gpui_shell/file_editor/host.rs).

## Decision

The editor owns one current host attachment, operation generation and cancellable
load, reload-prompt and selection-scroll tasks. Construction supplies an initial
host; pane rebinding supplies the destination before rendering. Read completion
resolves the latest attachment and checks its operation generation before
changing InputState, document metadata, loading status or notices.

Transfer commits live input and suspends source-window pointer/preview jobs,
then attaches the same editor to the destination. It preserves a pending read or
save and does not increment its I/O generation. Final retirement revokes the
attachment, advances the generation and cancels outstanding work, including when
another owner still retains an Entity reference.

Dirty reload approval also matches the exact draft it presented. Editing while
the prompt waits cannot silently authorize discarding the newer draft. A save
keeps its existing exact-snapshot clean/dirty rule across a transfer; a retired
owner cannot accept its completion.

## Rejected alternatives

- Reopen on transfer duplicates buffer/history ownership and loses dirty edits.
- A captured source handle leaves a live destination editor loading indefinitely.
- Relying only on GPUI's rendering map leaves an unrendered destination gap.
- Detached pointer tasks can dispatch stale selection events in the old window.
- Cancelling every write on movement changes the established save contract.

## Consequences

Workspace pane lifecycle calls prepare_transfer with the source window, then
attach_host in the destination and restores activity for visible leaves. Only
final removal calls detach_host. Source identity and remote authenticated SFTP
snapshot ownership remain unchanged; no local fallback or saved schema is added.

## Validation

GPUI host tests exercise the actual constructors and load/prompt callbacks while
closing the source window. They cover local read completion before the first
destination draw, remote source preservation, dirty reload approval/newer-draft
rejection, superseded/retired reads and pointer/preview cleanup.

Snapshots come from isolated real local files or the existing synthetic SFTP peer
through its real codec. Only application result delivery is gated; these tests
do not claim encrypted SSH packet timing or native drag/IME acceptance. They
never resolve user SSH profiles or read credentials.

## Supersedes

The captured source-window assumption for file-editor asynchronous work.

## Revisit when

Documents acquire multiple simultaneous render owners, live source retargeting
or a new platform prompt lifetime contract.
