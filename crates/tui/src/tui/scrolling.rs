//! Scroll state tracking for transcript rendering.

use std::time::{Duration, Instant};

// === Transcript Line Metadata ===

/// Metadata describing how rendered transcript lines map to history cells.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TranscriptLineMeta {
    CellLine {
        cell_index: usize,
        line_in_cell: usize,
    },
    Spacer,
}

impl TranscriptLineMeta {
    /// Return cell/line indices if this entry is a cell line.
    #[must_use]
    pub fn cell_line(&self) -> Option<(usize, usize)> {
        match *self {
            TranscriptLineMeta::CellLine {
                cell_index,
                line_in_cell,
            } => Some((cell_index, line_in_cell)),
            TranscriptLineMeta::Spacer => None,
        }
    }
}

// === Scroll Anchors ===

/// Scroll anchor for the transcript view.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TranscriptScroll {
    #[default]
    ToBottom,
    Scrolled {
        cell_index: usize,
        line_in_cell: usize,
    },
    ScrolledSpacerBeforeCell {
        cell_index: usize,
    },
}

impl TranscriptScroll {
    /// Resolve the anchor to a top line index.
    ///
    /// When the original anchor cell no longer exists (because content was
    /// rewritten — e.g. an inline RLM `repl` block expanded into
    /// `Thinking + Text`, or a tool result was replaced) we used to fall
    /// straight to `ToBottom`, which felt like the view "got stuck" because
    /// the user's next Up press would teleport-then-recompute from the
    /// bottom. Instead, clamp to the nearest surviving cell so scroll
    /// position is preserved across rewrites.
    #[must_use]
    pub fn resolve_top(self, line_meta: &[TranscriptLineMeta], max_start: usize) -> (Self, usize) {
        match self {
            TranscriptScroll::ToBottom => (TranscriptScroll::ToBottom, max_start),
            TranscriptScroll::Scrolled {
                cell_index,
                line_in_cell,
            } => {
                if let Some(idx) = anchor_index(line_meta, cell_index, line_in_cell) {
                    return (self, idx.min(max_start));
                }
                // Fallback 1: same cell, top line — handles cases where the
                // line count for a cell shrank (e.g. text was condensed).
                if let Some(idx) = anchor_index(line_meta, cell_index, 0) {
                    return (
                        TranscriptScroll::Scrolled {
                            cell_index,
                            line_in_cell: 0,
                        },
                        idx.min(max_start),
                    );
                }
                // Fallback 2: nearest surviving cell at or before the
                // requested cell index. Walks line_meta once.
                if let Some((ci, li, idx)) = nearest_cell_at_or_before(line_meta, cell_index) {
                    return (
                        TranscriptScroll::Scrolled {
                            cell_index: ci,
                            line_in_cell: li,
                        },
                        idx.min(max_start),
                    );
                }
                // Last resort — there are no cell lines at all (empty
                // transcript). ToBottom is fine here.
                (TranscriptScroll::ToBottom, max_start)
            }
            TranscriptScroll::ScrolledSpacerBeforeCell { cell_index } => {
                if let Some(idx) = spacer_before_cell_index(line_meta, cell_index) {
                    return (self, idx.min(max_start));
                }
                if let Some((ci, li, idx)) = nearest_cell_at_or_before(line_meta, cell_index) {
                    return (
                        TranscriptScroll::Scrolled {
                            cell_index: ci,
                            line_in_cell: li,
                        },
                        idx.min(max_start),
                    );
                }
                (TranscriptScroll::ToBottom, max_start)
            }
        }
    }

    /// Apply a delta scroll and return the updated anchor.
    ///
    /// When the existing anchor cell is gone (content rewrite), fall back to
    /// the nearest surviving cell instead of `max_start`. Without that, an
    /// Up press from a missing-anchor state would resolve `current_top` to
    /// the bottom and then walk up by `delta`, teleporting the user near
    /// the bottom of the transcript.
    #[must_use]
    pub fn scrolled_by(
        self,
        delta_lines: i32,
        line_meta: &[TranscriptLineMeta],
        visible_lines: usize,
    ) -> Self {
        if delta_lines == 0 {
            return self;
        }

        let total_lines = line_meta.len();
        if total_lines <= visible_lines {
            return TranscriptScroll::ToBottom;
        }

        let max_start = total_lines.saturating_sub(visible_lines);
        let current_top = match self {
            TranscriptScroll::ToBottom => max_start,
            TranscriptScroll::Scrolled {
                cell_index,
                line_in_cell,
            } => anchor_index(line_meta, cell_index, line_in_cell)
                .or_else(|| anchor_index(line_meta, cell_index, 0))
                .or_else(|| nearest_cell_at_or_before(line_meta, cell_index).map(|(_, _, idx)| idx))
                .unwrap_or(max_start)
                .min(max_start),
            TranscriptScroll::ScrolledSpacerBeforeCell { cell_index } => {
                spacer_before_cell_index(line_meta, cell_index)
                    .or_else(|| {
                        nearest_cell_at_or_before(line_meta, cell_index).map(|(_, _, idx)| idx)
                    })
                    .unwrap_or(max_start)
                    .min(max_start)
            }
        };

        let new_top = if delta_lines < 0 {
            current_top.saturating_sub(delta_lines.unsigned_abs() as usize)
        } else {
            let delta = usize::try_from(delta_lines).unwrap_or(usize::MAX);
            current_top.saturating_add(delta).min(max_start)
        };

        if new_top >= max_start {
            TranscriptScroll::ToBottom
        } else {
            TranscriptScroll::anchor_for(line_meta, new_top).unwrap_or(TranscriptScroll::ToBottom)
        }
    }

    /// Create an anchor from a top line index.
    #[must_use]
    pub fn anchor_for(line_meta: &[TranscriptLineMeta], start: usize) -> Option<Self> {
        if line_meta.is_empty() {
            return None;
        }

        let start = start.min(line_meta.len().saturating_sub(1));
        match line_meta[start] {
            TranscriptLineMeta::CellLine {
                cell_index,
                line_in_cell,
            } => Some(TranscriptScroll::Scrolled {
                cell_index,
                line_in_cell,
            }),
            TranscriptLineMeta::Spacer => {
                if let Some((cell_index, _)) = anchor_at_or_after(line_meta, start) {
                    Some(TranscriptScroll::ScrolledSpacerBeforeCell { cell_index })
                } else {
                    anchor_at_or_before(line_meta, start).map(|(cell_index, line_in_cell)| {
                        TranscriptScroll::Scrolled {
                            cell_index,
                            line_in_cell,
                        }
                    })
                }
            }
        }
    }
}

fn anchor_index(
    line_meta: &[TranscriptLineMeta],
    cell_index: usize,
    line_in_cell: usize,
) -> Option<usize> {
    line_meta
        .iter()
        .enumerate()
        .find_map(|(idx, entry)| match *entry {
            TranscriptLineMeta::CellLine {
                cell_index: ci,
                line_in_cell: li,
            } if ci == cell_index && li == line_in_cell => Some(idx),
            _ => None,
        })
}

fn spacer_before_cell_index(line_meta: &[TranscriptLineMeta], cell_index: usize) -> Option<usize> {
    line_meta.iter().enumerate().find_map(|(idx, entry)| {
        if matches!(entry, TranscriptLineMeta::Spacer)
            && line_meta
                .get(idx + 1)
                .and_then(TranscriptLineMeta::cell_line)
                .is_some_and(|(ci, _)| ci == cell_index)
        {
            Some(idx)
        } else {
            None
        }
    })
}

fn anchor_at_or_after(line_meta: &[TranscriptLineMeta], start: usize) -> Option<(usize, usize)> {
    line_meta
        .iter()
        .enumerate()
        .skip(start)
        .find_map(|(_, entry)| entry.cell_line())
}

fn anchor_at_or_before(line_meta: &[TranscriptLineMeta], start: usize) -> Option<(usize, usize)> {
    line_meta
        .iter()
        .enumerate()
        .take(start.saturating_add(1))
        .rev()
        .find_map(|(_, entry)| entry.cell_line())
}

/// Walk `line_meta` once and return the highest-priority surviving cell
/// position whose `cell_index <= target`. Used as a fallback when the
/// original anchor cell was removed by a content rewrite — keeps the user
/// near where they were instead of teleporting to the bottom.
///
/// Returns `(cell_index, line_in_cell, line_meta_index)`.
fn nearest_cell_at_or_before(
    line_meta: &[TranscriptLineMeta],
    target: usize,
) -> Option<(usize, usize, usize)> {
    let mut best: Option<(usize, usize, usize)> = None;
    for (idx, entry) in line_meta.iter().enumerate() {
        if let TranscriptLineMeta::CellLine {
            cell_index,
            line_in_cell,
        } = *entry
            && cell_index <= target
        {
            match best {
                None => best = Some((cell_index, line_in_cell, idx)),
                Some((bci, _, _)) if cell_index >= bci => {
                    best = Some((cell_index, line_in_cell, idx));
                }
                _ => {}
            }
        }
    }
    // Fall back to the very first surviving cell if nothing matched.
    if best.is_none() {
        for (idx, entry) in line_meta.iter().enumerate() {
            if let TranscriptLineMeta::CellLine {
                cell_index,
                line_in_cell,
            } = *entry
            {
                return Some((cell_index, line_in_cell, idx));
            }
        }
    }
    best
}

/// Direction for mouse scroll input.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScrollDirection {
    Up,
    Down,
}

impl ScrollDirection {
    fn sign(self) -> i32 {
        match self {
            ScrollDirection::Up => -1,
            ScrollDirection::Down => 1,
        }
    }
}

/// Stateful tracker for mouse scroll accumulation.
#[derive(Debug, Default)]
pub struct MouseScrollState {
    last_event_at: Option<Instant>,
    pending_lines: i32,
}

/// A computed scroll delta from user input.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ScrollUpdate {
    pub delta_lines: i32,
}

impl MouseScrollState {
    /// Create a new scroll state tracker.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Process a scroll event and return the resulting delta.
    pub fn on_scroll(&mut self, direction: ScrollDirection) -> ScrollUpdate {
        let now = Instant::now();
        let is_trackpad = self
            .last_event_at
            .is_some_and(|last| now.duration_since(last) < Duration::from_millis(35));
        self.last_event_at = Some(now);

        let lines_per_tick = if is_trackpad { 1 } else { 3 };
        self.pending_lines += direction.sign() * lines_per_tick;

        let delta = self.pending_lines;
        self.pending_lines = 0;
        ScrollUpdate { delta_lines: delta }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cell_line(cell_index: usize, line_in_cell: usize) -> TranscriptLineMeta {
        TranscriptLineMeta::CellLine {
            cell_index,
            line_in_cell,
        }
    }

    /// Build a synthetic line-meta array for a transcript with `cell_count`
    /// cells, each `lines_per_cell` lines tall, separated by spacers.
    fn synth_line_meta(cell_count: usize, lines_per_cell: usize) -> Vec<TranscriptLineMeta> {
        let mut meta = Vec::new();
        for cell in 0..cell_count {
            for line in 0..lines_per_cell {
                meta.push(cell_line(cell, line));
            }
            if cell + 1 < cell_count {
                meta.push(TranscriptLineMeta::Spacer);
            }
        }
        meta
    }

    /// Regression test for the "stuck after content rewrite" bug from
    /// issue #56. When the anchor cell still exists, scroll position is
    /// preserved.
    #[test]
    fn resolve_top_keeps_position_when_anchor_cell_exists() {
        let meta = synth_line_meta(5, 3); // 5 cells × 3 lines + 4 spacers = 19 entries
        let max_start = meta.len().saturating_sub(8);
        let anchor = TranscriptScroll::Scrolled {
            cell_index: 2,
            line_in_cell: 1,
        };
        let (state, top) = anchor.resolve_top(&meta, max_start);
        assert_eq!(state, anchor);
        // Cell 2, line 1 sits at: 0,1,2 (cell 0), spacer, 4,5,6 (cell 1),
        // spacer, 8,9,10 (cell 2) — line 1 of cell 2 is index 9.
        assert_eq!(top, 9);
    }

    /// Regression test for issue #56: when a content rewrite removes the
    /// anchor cell, the previous behaviour was to teleport to ToBottom.
    /// Now we clamp to the nearest surviving cell at-or-before the
    /// requested cell index.
    #[test]
    fn resolve_top_clamps_to_nearest_cell_when_anchor_cell_removed() {
        // Original transcript had cells 0..5; a rewrite shrunk it to 0..3.
        // The anchor pointed at cell 4 — that cell no longer exists.
        let meta = synth_line_meta(3, 2); // cells 0..3, 2 lines each + 2 spacers
        let max_start = meta.len();
        let stale_anchor = TranscriptScroll::Scrolled {
            cell_index: 4,
            line_in_cell: 0,
        };
        let (state, top) = stale_anchor.resolve_top(&meta, max_start);
        // Should clamp to the highest-indexed surviving cell (cell 2)
        // rather than ToBottom.
        match state {
            TranscriptScroll::Scrolled { cell_index, .. } => assert_eq!(cell_index, 2),
            other => panic!("expected Scrolled, got {other:?}"),
        }
        // top should be a valid index into meta, not max_start.
        assert!(top < meta.len());
    }

    /// Same fallback behaviour applies when scrolling further by a delta:
    /// scrolled_by computes its current_top from the (stale) anchor and
    /// the user's keypress should still move them up rather than locking
    /// them near the bottom.
    #[test]
    fn scrolled_by_does_not_teleport_on_missing_anchor() {
        let meta = synth_line_meta(3, 2);
        let visible = 4;
        let stale_anchor = TranscriptScroll::Scrolled {
            cell_index: 4,
            line_in_cell: 0,
        };
        // User presses Up (negative delta) from a stale anchor.
        let new_state = stale_anchor.scrolled_by(-1, &meta, visible);
        // Either ends up Scrolled near the top of the surviving content
        // or ToBottom if the transcript fits in one screen — but the
        // failure mode we're testing for is "ToBottom even though Up was
        // pressed and there's room to scroll," which depends on
        // total_lines > visible_lines.
        if meta.len() > visible {
            assert!(matches!(new_state, TranscriptScroll::Scrolled { .. }));
        }
    }

    /// When the transcript fits entirely in the viewport, the scroll
    /// state collapses to ToBottom regardless of where the anchor was.
    #[test]
    fn scrolled_by_collapses_to_bottom_when_view_fits() {
        let meta = synth_line_meta(2, 2);
        let visible = meta.len() + 5;
        let anchor = TranscriptScroll::Scrolled {
            cell_index: 0,
            line_in_cell: 0,
        };
        let new_state = anchor.scrolled_by(-1, &meta, visible);
        assert_eq!(new_state, TranscriptScroll::ToBottom);
    }

    /// `nearest_cell_at_or_before` returns the highest cell_index that
    /// is still ≤ the requested target.
    #[test]
    fn nearest_cell_at_or_before_picks_highest_surviving() {
        let meta = synth_line_meta(4, 1); // cells 0..4, one line each + spacers
        let result = nearest_cell_at_or_before(&meta, 10);
        let (cell_index, line_in_cell, _) = result.expect("a cell should match");
        assert_eq!(cell_index, 3);
        assert_eq!(line_in_cell, 0);
    }

    /// And falls back to the very first surviving cell when target is
    /// below all surviving cells (shouldn't happen in practice but the
    /// helper guards against it).
    #[test]
    fn nearest_cell_at_or_before_falls_back_to_first_when_target_too_low() {
        let mut meta = synth_line_meta(0, 0);
        meta.push(cell_line(5, 0));
        meta.push(cell_line(6, 0));
        let result = nearest_cell_at_or_before(&meta, 2);
        let (cell_index, _, _) = result.expect("falls back to first cell");
        assert_eq!(cell_index, 5);
    }

    /// Empty line_meta returns None — caller falls through to ToBottom.
    #[test]
    fn nearest_cell_at_or_before_empty_returns_none() {
        let meta: Vec<TranscriptLineMeta> = Vec::new();
        assert!(nearest_cell_at_or_before(&meta, 0).is_none());
    }

    #[test]
    fn to_bottom_anchor_resolves_to_max_start() {
        let meta = synth_line_meta(5, 2);
        let max_start = 7;
        let (state, top) = TranscriptScroll::ToBottom.resolve_top(&meta, max_start);
        assert_eq!(state, TranscriptScroll::ToBottom);
        assert_eq!(top, max_start);
    }
}
