//! Shape programming ligatures without giving the shaper control of the grid.
//!
//! Only contiguous ASCII graphic cells with identical styles share shaping.
//! Wide cells, combining sequences, whitespace and presentation boundaries keep
//! the existing per-cell path. Glyph cluster indices map back to fixed columns;
//! no `force_width` tolerance or natural advances accumulate along the row.
//!
//! Typographic pairs such as Monaco's `fi` and `fl` are the exception. Their
//! ligature is one glyph with a one-cell advance, while the next cluster index
//! skips a column. Snapping that index onto the grid leaves the second letter's
//! cell empty. Those clusters are shaped one character at a time instead.
//! Programming ligatures still advance every column they cover, so they stay.

use std::sync::Arc;

use gpui::{
    FontId, LineLayout, Pixels, ShapedGlyph, ShapedLine, ShapedRun, SharedString, TextRun,
    WindowTextSystem, px,
};
use nebula_terminal::render::SnapCell;

fn ascii_graphic(cell: &SnapCell) -> bool {
    cell.text.len() == 1 && cell.text.as_bytes()[0].is_ascii_graphic()
}

/// The caller adds viewport, selection, cursor and overlay boundaries. Keeping
/// this scan separate from painting lets a singleton use the original cache.
pub(super) fn span_len(cells: &[SnapCell], allowed: impl Fn(&SnapCell, usize) -> bool) -> usize {
    let Some(first) = cells.first() else { return 0 };
    if !ascii_graphic(first) || !allowed(first, 0) {
        return 1;
    }
    cells
        .iter()
        .enumerate()
        .take_while(|(offset, cell)| {
            ascii_graphic(cell)
                && usize::from(cell.col) == usize::from(first.col) + offset
                && cell.fg == first.fg
                && cell.bg == first.bg
                && cell.bold == first.bold
                && cell.dim == first.dim
                && cell.italic == first.italic
                && cell.underline == first.underline
                && cell.strikethrough == first.strikethrough
                && allowed(cell, *offset)
        })
        .count()
}

pub(super) fn shape_ascii_span(
    text_system: &WindowTextSystem,
    text: SharedString,
    font_size: Pixels,
    mut run: TextRun,
    cell_width: Pixels,
) -> ShapedLine {
    debug_assert!(text.bytes().all(|byte| byte.is_ascii_graphic()));
    run.len = text.len();
    let mut shaped = text_system.shape_line(text.clone(), font_size, &[run.clone()], None);
    let natural =
        expand_collapsed_ligatures(&shaped, &text, text_system, font_size, &run, cell_width);
    // The cached natural layout is shared with other views. Replace only this
    // ShapedLine's Arc, preserving its text and decoration runs.
    *shaped = Arc::new(grid_layout(&natural, cell_width));
    shaped
}

/// Columns a cluster claims minus the cells its glyphs actually advance.
///
/// `0.45` leaves room for hinting and the compact cell-width rounding without
/// treating a real two-cell ligature (advance ≈ 2 cells) as collapsed.
fn cluster_collapses(columns: usize, advance: f32, cell_width: f32) -> bool {
    columns > 1 && cell_width > 0.0 && advance < cell_width * (columns as f32 - 0.45)
}

fn collapsed_cluster_ranges(natural: &LineLayout, cell_width: Pixels) -> Vec<(usize, usize)> {
    let glyphs: Vec<&ShapedGlyph> = natural.runs.iter().flat_map(|run| &run.glyphs).collect();
    let cell = cell_width.as_f32();
    let mut ranges = Vec::new();
    let mut index = 0;
    while index < glyphs.len() {
        let start = glyphs[index].index;
        let origin = glyphs[index].position.x.as_f32();
        let mut next = index + 1;
        while next < glyphs.len() && glyphs[next].index == start {
            next += 1;
        }
        let end = glyphs.get(next).map(|glyph| glyph.index).unwrap_or(natural.len);
        let next_x = glyphs
            .get(next)
            .map(|glyph| glyph.position.x.as_f32())
            .unwrap_or(natural.width.as_f32());
        if cluster_collapses(end.saturating_sub(start), next_x - origin, cell) {
            ranges.push((start, end));
        }
        index = next;
    }
    ranges
}

fn expand_collapsed_ligatures(
    natural: &LineLayout,
    text: &str,
    text_system: &WindowTextSystem,
    font_size: Pixels,
    template: &TextRun,
    cell_width: Pixels,
) -> LineLayout {
    let ranges = collapsed_cluster_ranges(natural, cell_width);
    if ranges.is_empty() {
        return LineLayout {
            font_size: natural.font_size,
            width: natural.width,
            ascent: natural.ascent,
            descent: natural.descent,
            runs: natural.runs.clone(),
            len: natural.len,
        };
    }

    let flat: Vec<(FontId, ShapedGlyph)> = natural
        .runs
        .iter()
        .flat_map(|run| run.glyphs.iter().cloned().map(|glyph| (run.font_id, glyph)))
        .collect();
    let mut placed = Vec::with_capacity(text.len());
    let mut index = 0;
    while index < flat.len() {
        let start = flat[index].1.index;
        if let Some(&(_, end)) = ranges.iter().find(|(range_start, _)| *range_start == start) {
            let end = end.min(text.len());
            for byte in start..end {
                let column = byte..byte + 1;
                if text.is_char_boundary(byte) && text.is_char_boundary(byte + 1) {
                    placed.extend(shape_one_column(
                        text_system,
                        font_size,
                        template,
                        &text[column],
                        byte,
                    ));
                }
            }
            while index < flat.len() && flat[index].1.index < end {
                index += 1;
            }
        } else {
            placed.push(flat[index].clone());
            index += 1;
        }
    }

    LineLayout {
        font_size: natural.font_size,
        width: natural.width,
        ascent: natural.ascent,
        descent: natural.descent,
        runs: coalesce_runs(placed),
        len: natural.len,
    }
}

fn shape_one_column(
    text_system: &WindowTextSystem,
    font_size: Pixels,
    template: &TextRun,
    text: &str,
    index: usize,
) -> Vec<(FontId, ShapedGlyph)> {
    let mut run = template.clone();
    run.len = text.len();
    let shaped = text_system.shape_line(text.into(), font_size, &[run], None);
    let mut glyphs = Vec::new();
    for run in &shaped.runs {
        let origin = run.glyphs.first().map(|glyph| glyph.position.x.as_f32()).unwrap_or(0.0);
        for glyph in &run.glyphs {
            let mut glyph = glyph.clone();
            glyph.position.x = px(glyph.position.x.as_f32() - origin);
            glyph.index = index;
            glyphs.push((run.font_id, glyph));
        }
    }
    glyphs
}

fn coalesce_runs(glyphs: Vec<(FontId, ShapedGlyph)>) -> Vec<ShapedRun> {
    let mut runs: Vec<ShapedRun> = Vec::new();
    for (font_id, glyph) in glyphs {
        if let Some(run) = runs.last_mut().filter(|run| run.font_id == font_id) {
            run.glyphs.push(glyph);
        } else {
            runs.push(ShapedRun { font_id, glyphs: vec![glyph] });
        }
    }
    runs
}

fn grid_layout(natural: &LineLayout, cell_width: Pixels) -> LineLayout {
    let mut runs = natural.runs.clone();
    let mut cluster = None;
    for glyph in runs.iter_mut().flat_map(|run| &mut run.glyphs) {
        let origin = match cluster {
            Some((index, origin)) if index == glyph.index => origin,
            _ => {
                cluster = Some((glyph.index, glyph.position.x));
                glyph.position.x
            },
        };
        // Multiple glyphs may share one source index. Preserve their internal
        // offsets, including zero-advance components; only the cluster moves.
        glyph.position.x = cell_width * glyph.index as f32 + (glyph.position.x - origin);
    }
    LineLayout {
        font_size: natural.font_size,
        width: cell_width * natural.len as f32,
        ascent: natural.ascent,
        descent: natural.descent,
        runs,
        len: natural.len,
    }
}

#[cfg(test)]
mod tests;
