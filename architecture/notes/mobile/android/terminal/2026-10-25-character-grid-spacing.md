# Match mobile character-art spacing to the desktop grid policy

## Status

Implemented; native Canvas acceptance pending on a supported runner.

## Context

The Android live terminal and desktop snapshot mirror use the same cell painter
but previously chose row height independently from font ascent/descent/leading.
Large font padding stretches character art, and font-rendered Braille dots have
nonuniform spacing across row boundaries. Desktop now uses a 1.2 em default row
and procedural Braille geometry.

## Evidence

Both mobile views used the full natural font height, and `TerminalGlyphs` had no
Braille path. Each text run was clipped to its row. Simply reducing row height
would therefore cut accents/descenders, and merely removing that clip would let
the following row's background erase the overhanging ink.

## Decision

One metrics helper supplies the 1.2 em default row height and centered natural
font baseline to both views. Android Paint already uses device-pixel dimensions;
row height is floored once. The existing measured column advance remains unchanged
so ASCII text batches keep their natural horizontal placement.

Render all backgrounds before foreground glyphs and restrict text horizontally,
not vertically per row. The view still clips to its own outer bounds. Procedural
Braille uses a continuous 2 by 4 grid with circular dots, the Unicode bit mapping,
and the same radius rule as desktop. Empty Braille remains empty. Text/selection
coordinates, colors, source copying, and cursor geometry retain grid ownership.

## Rejected alternatives

- Stretching glyphs or screenshots hides the row-spacing defect and distorts text.
- Only shortening the row clips text under the old painter order.
- Flooring column advance without adapting text batching accumulates horizontal
  drift; this change does not rewrite Android's existing text shaping policy.

## Consequences

More rows can fit in a given viewport, so the PTY receives new geometry through
its existing resize path. Visible foregrounds are painted after the backgrounds
with one additional bounded row traversal. Extremely tall custom glyphs can extend
outside their row; outer view clipping still applies at screen edges.

## Validation

Native Canvas tests exercise all 256 Braille patterns at three font pixel sizes,
default row dimensions, unchanged source text, and complete accent/CJK/descender
ink against an unclipped reference. Existing live/snapshot placement, block seams,
selection, cursor and keyboard-resize tests are retained.
Local Kotlin/test compilation succeeds. The local Linux AArch64 host cannot run
the pinned native Canvas runtime; its unsupported-host failure is not a rendering
pass. Native execution must succeed before merging.

## Supersedes

None; matches the desktop character-grid decision without changing material/blur.

## Revisit when

Android exposes explicit line-height/offset settings or changes its text batching
policy. Preserve geometry, input hit testing and PTY dimensions together.
