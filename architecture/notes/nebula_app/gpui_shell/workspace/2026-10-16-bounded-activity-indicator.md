# Bounded Agent activity animation

## Status

Proposed for review.

## Context

A visible running Agent kept a display-frame callback pending. Each callback
advanced an eleven-pixel badge and notified `NebulaWorkspace`, rebuilding the
workspace even when terminal contents had not changed. Multiple running tabs
shared that callback, but its frequency still followed the display frame source.

## Evidence

`workspace/agents.rs::arm_activity_spinner_frame` notified the workspace from
`Context::on_next_frame`. The former `sidebar.rs::spinner` generated roughly
36 rounded quads for each badge on every frame. GPUI already submits those quads
through its native GPU renderer; changing the graphics backend would not remove
the application-side scheduling and layout work.

## Decision

Use three fixed-position dots with one highlighted dot. Advance at most once
after a 250 ms timer, shared by the workspace. A subsequent render must explicitly
renew the timer. Keep the existing visibility, hidden-window and minimization
checks, and respect GPUI's reduced-motion preference. Reset visibility at each
workspace render; modes that omit the tabs must not retain the preceding frame's
visible flag. Collapsed sidebar sections do not renew the timer. A one-shot task
holds only the weak workspace handle supplied by `spawn_in`; it cannot keep a
closed workspace alive or renew itself without a render.

## Rejected alternatives

- A simpler circular shape with the same per-frame callback retains the main
  scheduling cost.
- Skipping phase changes inside a display-frame callback still wakes the window
  on every frame.
- A separate entity is not sufficient by itself: the pinned GPUI implementation
  marks ancestors dirty when a descendant is invalidated. A broader caching or
  compositor change needs separate profiling and lifecycle evidence.

## Consequences

The indicator remains animated when the window is visible but unfocused, and
keeps its original layout slot and semantic colors. Actual Agent state updates
still arrive immediately through their existing owners; the timer only controls
decorative motion. One outstanding timer can finish after the indicator becomes
invisible, but checks visibility again before notifying the workspace.

Four animation updates per second is a scheduling bound, not a measured CPU or
battery guarantee. Other terminal output, transitions and UI events can still
cause additional rendering.

## Validation

The existing GPUI test suite contains a controlled-clock regression for the
250 ms deadline, duplicate scheduling, entering Settings through an actual root
render, hidden windows and reduced motion. On Windows with pinned Rust 1.97.1,
the targeted GPUI test passed (one test, no ignored tests). The default GPUI
product built with `--locked --offline --features gpui-shell`. Architecture
validation against `2bd95f36` and formatting checks passed.

Native visual acceptance and a controlled CPU comparison remain pending. A
compiled executable and a virtual-window timing test do not establish the CPU
reduction of an installed application.

## Supersedes

None.

## Revisit when

Profiling still attributes material idle CPU to the bounded indicator, or GPUI
provides a verified independently composited animation path.
