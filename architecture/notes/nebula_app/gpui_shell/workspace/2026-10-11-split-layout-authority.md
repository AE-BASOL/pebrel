# Split layout adapter authority

## Status

Status: Proposed

The decision remains proposed; root-owned v2 validation is Pending.

## Context

### Trigger

File leaves will share the existing split tree and its real pointer/keyboard geometry. Keeping that authority inspectable is a prerequisite for typed content.

### Inputs

The current terminal-only WorkspaceTab, SplitTree, pane bounds, drag preview, resize contract, and GPUI focus.

### State owner

NebulaWorkspace still owns every pane and all geometry observations. Its `split_layout` child owns operations which interpret that structure.

### Dependency direction

The adapter calls the shared `nebula_split` geometry. Terminal resizing remains in TerminalView. No domain rule moves into the view.

## Evidence

The workspace adapter delegates split geometry to the shared split owner; workspace controls protect existing layout behavior.

Implementation and acceptance scope: [nebula_app/src/gpui_shell/workspace/split_layout.rs](../../../../../nebula_app/src/gpui_shell/workspace/split_layout.rs), [nebula_split/src/lib.rs](../../../../../nebula_split/src/lib.rs), [nebula_app/src/gpui_shell/workspace/tests.rs](../../../../../nebula_app/src/gpui_shell/workspace/tests.rs).
Execution results remain Pending in Validation.

## Decision

Extract the existing split create/close/focus/navigation/zoom/render/divider implementations unchanged. Private methods become visible to their parent and sibling adapters only.

## Rejected alternatives

Typing leaf content inside a large mixed parent obscures terminal-specific assumptions. Moving palette or sidebar handlers would be unrelated to the split structure boundary.

## Consequences

### Lifecycle and failures

Existing failed-attachment shutdown, sibling collapse, committed resize, and busy drag-close behavior remain intact. No extra task or allocation is introduced.

The next typed-content stage can visibly replace each terminal-only assumption in one structure authority. This stage alone adds no file opening or new persisted format.

## Validation

Pending. Root-owned v2 compilation, behavior, green/red and native validation
remain outstanding. The source-handoff statements below retain their original
scope and do not establish a completed v2 run.

Pinned rustfmt, diff checks, and baseline architecture checks are performed before handoff. Parent executes existing split/pane-header/layout tests and product builds. Compilation and test success are not yet claimed.

## Supersedes

None. The existing geometry and lifecycle decision is retained.

## Revisit when

New pane content, split geometry rules or attachment lifecycle changes.
