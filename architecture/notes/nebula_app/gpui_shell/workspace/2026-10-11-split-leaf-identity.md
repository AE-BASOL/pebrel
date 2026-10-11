# Structural split identity

## Status

Status: Proposed

The decision remains proposed; root-owned v2 validation is Pending.

## Context

### Trigger

Terminal labels currently conflate the split container and its leaf capability. Typed file content must use the same tree without inheriting a terminal name as an implicit permission.

### Inputs

Existing terminal-only workspace trees and their runtime pane IDs.

### State owner

NebulaWorkspace owns WorkspaceTab::Split and WorkspacePane. At this stage every pane continues to hold precisely one TerminalView.

### Dependency direction

Structural identities belong to the workspace; PTY and terminal capabilities remain in TerminalView.

## Evidence

The workspace structural identities and existing split controls distinguish leaf identity from terminal capability.

Implementation and acceptance scope: [nebula_app/src/gpui_shell/workspace.rs](../../../../../nebula_app/src/gpui_shell/workspace.rs), [nebula_app/src/gpui_shell/workspace/split_layout.rs](../../../../../nebula_app/src/gpui_shell/workspace/split_layout.rs), [nebula_app/src/gpui_shell/workspace/tests.rs](../../../../../nebula_app/src/gpui_shell/workspace/tests.rs).
Execution results remain Pending in Validation.

## Decision

Rename TerminalPane to WorkspacePane and WorkspaceTab::Terminal to Split. Rename the structural predicate is_terminal to is_split. All behavior, fields, matches, and tests remain unchanged.

## Rejected alternatives

Allowing a terminal-named container to hold files obscures code review of terminal capability checks. Activating file variants during this mechanical rename would combine independent structural and behavior changes.

## Consequences

### Lifecycle and failures

No new source, task, shutdown path, or persistent format is introduced. Existing terminal ownership and failure handling remain intact.

The next stage introduces a typed Terminal content variant and explicit terminal accessors while file opening is still unavailable. Nonterminal accessors will never manufacture a PTY.

## Validation

Pending. Root-owned v2 compilation, behavior, green/red and native validation
remain outstanding. The source-handoff statements below retain their original
scope and do not establish a completed v2 run.

Pinned rustfmt, diff, and baseline architecture checks run before handoff. Parent runs existing workspace and split tests plus the normal product build; no compilation claim is made here.

## Supersedes

None. The structural rename preserves the existing ownership decision.

## Revisit when

A new structural identity or deferred pane reference is introduced.
