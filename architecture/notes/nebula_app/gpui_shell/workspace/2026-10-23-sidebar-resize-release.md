# Sidebar resize release ownership

## Status

Proposed with the fix for [#640](https://github.com/Kuddev/pebrel/issues/640).

## Context

Both sidebars show a release-to-close preview after crossing their width detent.
An outside-window release must finish that same gesture. Escape and window
deactivation must instead restore its starting width.

## Evidence

The [pinned GPUI `on_mouse_up` and `capture_any_mouse_up` implementations](https://github.com/Kuddev/zed/blob/1e716d16ec54b361499931b4278903cdcaae8949/crates/gpui/src/elements/div.rs#L206) require
a hovered hitbox, including for a full-window overlay. Outside-window releases
therefore bypass them. Windows captures the mouse on press, but its
[non-client move adapter](https://github.com/Kuddev/zed/blob/1e716d16ec54b361499931b4278903cdcaae8949/crates/gpui_windows/src/events.rs#L1010)
reports `pressed_button: None` even while the button remains held.
The old resize updaters interpret that move as cancellation.

## Decision

The workspace installs one shared pair of window-level Capture listeners for
sidebar resize from a nonvisual canvas painted before its content children.
The listeners inspect live resize state; ordinary events continue unchanged.
They are present before a press so release does not depend on another paint.

Only moves reporting Left update the preview. Other moves preserve the last
valid width and close intent. A real Left release reuses the existing `take()`
finish operations, which commit once. Inactive windows cancel instead. Existing
Escape and deactivation cancellation remain in force. Overlays only present
the resize cursor and close hint; they do not own a second event route.

## Rejected alternatives

- A larger overlay or root capture handler still depends on hover.
- Treating None as release can close a sidebar during non-client movement.
- Installing listeners only after a press leaves a gap before the next paint.
- Copying tab tear-out from #575 adds unrelated behavior; its listeners only
  cover tab state and do not finish either sidebar resize.

## Consequences

No dependency, settings format, public interface, native mouse hook or background
task is added. Window listeners are renewed each paint and hold weak references
to the owning workspace. This does not change tab, pane or split dragging.

## Validation

[The event regressions](../../../../../nebula_app/src/gpui_shell/workspace/details_panel_tests.rs) exercise the real handles for both sides: outside
release, None moves, non-Left and repeated releases, width changes and reversal,
Escape/deactivation, reopening and release before another paint. These simulated
events do not establish Windows native mouse-capture acceptance; that remains
a separate validation requirement.

## Supersedes

None.

## Revisit when

GPUI provides a lifecycle-scoped pointer-capture API that owns both client and
non-client releases. Preserve the event regressions when replacing this adapter.
