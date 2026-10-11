# Terminal command capability boundary

## Status

Status: Proposed

The decision remains proposed; root-owned v2 validation is Pending.

## Context

### Trigger

RPC, AI hook, notification, clipboard paste, and send-to-chat routes currently derive a terminal from a structural leaf. Those routes must refuse nonterminal content before file variants become creatable.

### Inputs

Captured pane ID, current owning workspace, typed pane content, and existing RPC request.

### State owner

WorkspacePane owns content. `runtime_terminal_view` resolves the current owner and grants TerminalView only through terminal_view; RPC returns unsupported_pane otherwise.

### Dependency direction

Workspace runtime and AI adapters require the typed terminal capability before dispatching into TerminalView. Shared snapshots default missing old pane kinds to terminal for wire compatibility.

## Evidence

Runtime and chat adapters resolve the terminal capability through the same content owner before dispatch.

Implementation and acceptance scope: [nebula_app/src/gpui_shell/workspace/runtime_tabs.rs](../../../../../nebula_app/src/gpui_shell/workspace/runtime_tabs.rs), [nebula_app/src/gpui_shell/workspace/pane_content.rs](../../../../../nebula_app/src/gpui_shell/workspace/pane_content.rs), [nebula_app/src/gpui_shell/workspace/send_to_chat.rs](../../../../../nebula_app/src/gpui_shell/workspace/send_to_chat.rs).
Execution results remain Pending in Validation.

## Decision

Centralize strict terminal resolution across RPC, AI origins, and send-to-chat; filter terminal-only loops and add runtime pane kind/file metadata. Preserve the existing standalone document/image branches until file routing migration. This prerequisite still has only the Terminal content variant; its file metadata is truthfully absent.

## Rejected alternatives

Routing a file identity through an arbitrary sibling terminal would send user input or agent commands to an unintended process. A new leaf must never manufacture a PTY fallback.

## Consequences

### Lifecycle and failures

Closed/missing targets retain target_not_found. Unsupported content is rejected before execution, and all existing standalone document draft guards still run. Runtime file/window close examines all owned editors.

No file creation or session format is enabled here. The future file variant extends the content metadata methods and exercises every strict rejection through actual GPUI mixed leaves.

## Validation

Pending. Root-owned v2 compilation, behavior, green/red and native validation
remain outstanding. The source-handoff statements below retain their original
scope and do not establish a completed v2 run.

Pinned rustfmt, diff and architecture checks are performed before handoff. Parent runs existing runtime/workspace and native product gates. Actual nonterminal RPC/AI/clipboard regression tests are part of the later file-activation layer; they have not yet passed.

## Supersedes

None. This prerequisite makes the existing terminal authority explicit.

## Revisit when

A new RPC, AI, clipboard or chat route consumes pane content.
