//! Syntax highlights for a loaded [`Diff`]. Computed off-thread after the diff
//! loads (see `RepoModel::start_highlight`); the view renders plain text until
//! the result arrives, then swaps colors in without changing layout.
//!
//! Strategy: reconstruct each side of the diff and highlight it separately.
//! Context + deletions form the old side, context + additions the new side.
//! Context/added rows take their spans from the new side, deleted rows from
//! the old side. Hunks are stitched together (separated by a blank line), so a
//! construct that straddles an unchanged gap (a block comment or string that
//! opens in one hunk and closes in the next) can be mis-colored. Highlighting
//! the full files would fix that but needs `git show` per side; not worth it yet.

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::sync::{Arc, Mutex};
use std::time::Instant;

use gd_git::{Diff, DiffLineKind};
use gd_syntax::{HighlightKind, HighlightRequest, language_for_path};
use gd_ui::theme::SyntaxPalette;
use gpui::{Font, Hsla, TextRun};

/// Above this many rows or bytes of text we skip highlighting entirely.
pub const MAX_LINES: usize = 20_000;
pub const MAX_BYTES: usize = 2 * 1024 * 1024;
/// Entries in the per-repository result cache.
const CACHE_CAP: usize = 8;

#[derive(Debug, Clone, Copy)]
pub struct Span {
    pub start: u32,
    pub end: u32,
    pub kind: HighlightKind,
}

/// Flat per-row span storage: row `i` owns `spans[offsets[i]..offsets[i + 1]]`.
#[derive(Debug, Default)]
pub struct DiffHighlight {
    spans: Vec<Span>,
    offsets: Vec<u32>,
}

impl DiffHighlight {
    pub fn row(&self, ix: usize) -> &[Span] {
        match (self.offsets.get(ix), self.offsets.get(ix + 1)) {
            (Some(&a), Some(&b)) => &self.spans[a as usize..b as usize],
            _ => &[],
        }
    }
}

/// Kinds drawn in the row's own text color: coloring every identifier and
/// punctuation mark only adds noise (zeron's palette maps them to `text`).
fn is_plain(kind: HighlightKind) -> bool {
    use HighlightKind::*;
    matches!(kind, Variable | Parameter | Operator | Punctuation | Embedded)
}

/// Build text runs for one row. Called only for visible rows.
pub fn runs_for(
    text: &str,
    spans: &[Span],
    base: Hsla,
    font: &Font,
    palette: &SyntaxPalette,
) -> Vec<TextRun> {
    let run = |len: usize, color: Hsla| TextRun {
        len,
        font: font.clone(),
        color,
        background_color: None,
        underline: None,
        strikethrough: None,
        letter_spacing: None,
    };
    let mut runs = Vec::with_capacity(spans.len() * 2 + 1);
    let mut pos = 0usize;
    for s in spans {
        let (start, end) = (s.start as usize, (s.end as usize).min(text.len()));
        if is_plain(s.kind)
            || start < pos
            || start >= end
            || !text.is_char_boundary(start)
            || !text.is_char_boundary(end)
        {
            continue;
        }
        if start > pos {
            runs.push(run(start - pos, base));
        }
        runs.push(run(end - start, palette.color(s.kind)));
        pos = end;
    }
    if pos < text.len() {
        runs.push(run(text.len() - pos, base));
    }
    runs
}

/// The text element for one diff row: syntax colors (`spans`, may be empty)
/// plus a background wash over the word-level `emphasis` ranges. Plain rows
/// (no spans, no emphasis) stay a cheap single-run text.
pub fn line_text(
    text: &str,
    spans: &[Span],
    emphasis: &[(usize, usize)],
    fg: Hsla,
    emph_bg: Hsla,
    font: &Font,
    palette: &SyntaxPalette,
) -> gpui::AnyElement {
    use gpui::{IntoElement, ParentElement, Styled};
    if spans.is_empty() && emphasis.is_empty() {
        return gpui::div().text_color(fg).child(text.to_string()).into_any_element();
    }
    let mut runs = runs_for(text, spans, fg, font, palette);
    if !emphasis.is_empty() {
        runs = overlay_background(text, runs, emphasis, emph_bg);
    }
    gpui::StyledText::new(text.to_string()).with_runs(runs).into_any_element()
}

/// Split `runs` at emphasis edges and set `background_color` inside them.
fn overlay_background(text: &str, runs: Vec<TextRun>, ranges: &[(usize, usize)], bg: Hsla) -> Vec<TextRun> {
    let ranges: Vec<(usize, usize)> = ranges
        .iter()
        .map(|&(s, e)| (s.min(text.len()), e.min(text.len())))
        .filter(|&(s, e)| s < e && text.is_char_boundary(s) && text.is_char_boundary(e))
        .collect();
    let mut out = Vec::with_capacity(runs.len() + ranges.len() * 2);
    let mut pos = 0usize;
    for run in runs {
        let (rs, re) = (pos, pos + run.len);
        let mut cuts = vec![rs, re];
        for &(s, e) in &ranges {
            for c in [s, e] {
                if c > rs && c < re {
                    cuts.push(c);
                }
            }
        }
        cuts.sort_unstable();
        cuts.dedup();
        for w in cuts.windows(2) {
            let (a, b) = (w[0], w[1]);
            let inside = ranges.iter().any(|&(s, e)| a >= s && b <= e);
            let mut piece = run.clone();
            piece.len = b - a;
            if inside {
                piece.background_color = Some(bg);
            }
            out.push(piece);
        }
        pos = re;
    }
    out
}

/// Small LRU keyed by a hash of (path, diff content). Shared with the
/// background task so re-selecting a file is a lookup, not a re-parse.
#[derive(Clone, Default)]
pub struct HighlightCache(Arc<Mutex<Vec<(u64, Arc<DiffHighlight>)>>>);

impl HighlightCache {
    fn get(&self, key: u64) -> Option<Arc<DiffHighlight>> {
        let mut v = self.0.lock().ok()?;
        let ix = v.iter().position(|(k, _)| *k == key)?;
        let hit = v.remove(ix);
        let out = hit.1.clone();
        v.push(hit);
        Some(out)
    }

    fn put(&self, key: u64, value: Arc<DiffHighlight>) {
        if let Ok(mut v) = self.0.lock() {
            v.retain(|(k, _)| *k != key);
            if v.len() >= CACHE_CAP {
                v.remove(0);
            }
            v.push((key, value));
        }
    }
}

fn cache_key(diff: &Diff, path: &str) -> u64 {
    let mut h = DefaultHasher::new();
    path.hash(&mut h);
    diff.lines.len().hash(&mut h);
    for l in &diff.lines {
        (l.kind as u8).hash(&mut h);
        l.text.hash(&mut h);
    }
    h.finish()
}

/// Highlight `diff` (blocking; call from a background executor). Returns
/// `None` when the file type is unknown or the diff is over the caps.
pub fn highlight_diff(diff: &Diff, path: &str, cache: &HighlightCache) -> Option<Arc<DiffHighlight>> {
    if diff.is_binary || diff.lines.is_empty() || diff.lines.len() > MAX_LINES {
        return None;
    }
    language_for_path(path)?;
    let bytes: usize = diff.lines.iter().map(|l| l.text.len() + 1).sum();
    if bytes > MAX_BYTES {
        return None;
    }
    let key = cache_key(diff, path);
    if let Some(hit) = cache.get(key) {
        return Some(hit);
    }
    let started = Instant::now();
    let n = diff.lines.len();
    let mut per_row: Vec<Vec<Span>> = vec![Vec::new(); n];
    for new_side in [true, false] {
        let mut source = String::new();
        let mut rows: Vec<Option<usize>> = Vec::new(); // source line -> diff row
        let mut wanted = false;
        for (ix, line) in diff.lines.iter().enumerate() {
            let on_side = match line.kind {
                DiffLineKind::Context => true,
                DiffLineKind::Add => new_side,
                DiffLineKind::Delete => !new_side,
                DiffLineKind::Hunk => {
                    // Blank separator so tokens can't join across hunks.
                    if !rows.is_empty() {
                        source.push('\n');
                        rows.push(None);
                    }
                    continue;
                }
                DiffLineKind::NoNewline => continue,
            };
            if !on_side {
                continue;
            }
            // Take spans from this side only for rows it owns.
            let owns = match line.kind {
                DiffLineKind::Delete => !new_side,
                _ => new_side,
            };
            wanted |= owns;
            source.push_str(&line.text);
            source.push('\n');
            rows.push(owns.then_some(ix));
        }
        if !wanted {
            continue; // e.g. no deletions: the old side adds nothing
        }
        let doc = match gd_syntax::highlight(HighlightRequest {
            source: &source,
            path: Some(path),
            fence_tag: None,
        }) {
            Ok(doc) => doc,
            Err(e) => {
                log::debug!("syntax: skipped {path}: {e}");
                return None;
            }
        };
        for (line_spans, row) in doc.lines.iter().zip(&rows) {
            if let Some(row) = *row {
                per_row[row] = line_spans
                    .iter()
                    .map(|s| Span { start: s.range.start as u32, end: s.range.end as u32, kind: s.kind })
                    .collect();
            }
        }
    }
    let mut out = DiffHighlight { spans: Vec::new(), offsets: Vec::with_capacity(n + 1) };
    for row in per_row {
        out.offsets.push(out.spans.len() as u32);
        out.spans.extend(row);
    }
    out.offsets.push(out.spans.len() as u32);
    let out = Arc::new(out);
    log::info!(
        "syntax: highlighted {path} ({n} rows, {} spans) in {:?}",
        out.spans.len(),
        started.elapsed()
    );
    cache.put(key, out.clone());
    Some(out)
}
