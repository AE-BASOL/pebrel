# Explicit terminal content ownership

## Status

Status: Proposed

The decision remains proposed; root-owned v2 validation is Pending.

## Context

### Trigger

A structural pane ID must not itself grant a TerminalView. The current terminal field must move behind a typed content owner before any file leaf can be allocated.

### Inputs

WorkspacePane identity, TerminalView Entity and its owning event subscription.

### State owner

WorkspacePane owns PaneContent. The Terminal variant owns its Entity and subscription; rebind updates that subscription for the current workspace.

### Dependency direction

Workspace structure asks typed content for a terminal capability. TerminalView continues to own every PTY and its protocol state.

## Evidence

PaneContent and WorkspacePane make terminal access explicit while existing workspace controls retain terminal behavior.

Implementation and acceptance scope: [nebula_app/src/gpui_shell/workspace/pane_content.rs](../../../../../nebula_app/src/gpui_shell/workspace/pane_content.rs), [nebula_app/src/gpui_shell/workspace.rs](../../../../../nebula_app/src/gpui_shell/workspace.rs), [nebula_app/src/gpui_shell/workspace/tests.rs](../../../../../nebula_app/src/gpui_shell/workspace/tests.rs).
Execution results remain Pending in Validation.

## Decision

Introduce PaneContent with only its Terminal variant, replace direct field access with terminal_view, and move subscription rebinding into the content owner. The explicit expectations are valid only while this stage has no file variant or creator; subsequent capability guards replace them before file activation.

## Rejected alternatives

Retaining a second unconditional terminal field alongside file content could accidentally route PTY/AI events to a fabricated terminal. Activating file creation before every access is audited would break capability ownership.

## Consequences

### Lifecycle and failures

Existing subscription replacement, shutdown, and terminal session behavior are unchanged. No file variant, file creation, persistent format, or fallback PTY is introduced here.

The next capability stage must eliminate terminal-only expectations from general routes before enabling file variants. It may retain them only directly after a constructor whose contract is explicitly terminal-only.

## Validation

Pending. Root-owned v2 compilation, behavior, green/red and native validation
remain outstanding. The source-handoff statements below retain their original
scope and do not establish a completed v2 run.

Pinned rustfmt, diff and architecture checks run before handoff. Parent runs the existing workspace, duplication, transfer, and terminal activity tests plus the product build. Test success is not yet claimed.

## Supersedes

None. This prerequisite preserves terminal behavior before file variants exist.

## Revisit when

A new content variant or unconditional terminal accessor is introduced.
