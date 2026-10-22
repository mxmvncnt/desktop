//! Hunk expansion: show more unchanged lines around a hunk, like Desktop's
//! expand buttons in the hunk header gutter (`text-diff-expansion.ts`).
//!
//! The diff keeps both full texts ([`Diff::source`]), so expansion is a pure
//! function: it splits the flat rows back into hunks, pulls context lines
//! from the new text into the requested gap, merges hunks whose gap closed,
//! and re-flattens. Added/deleted rows (and their word emphasis) are copied
//! unchanged; only context rows and headers are new.

use crate::diff::{Diff, DiffHunk, DiffLine, DiffLineKind};
use crate::engine::{hunk_header, split_lines};

/// Lines revealed per click, as in Desktop (`DefaultDiffExpansionStep`).
pub const EXPANSION_STEP: usize = 20;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Expansion {
    /// Reveal up to `EXPANSION_STEP` lines above hunk `h`.
    Up(usize),
    /// Reveal up to `EXPANSION_STEP` lines below hunk `h`.
    Down(usize),
    /// Reveal the whole gap above hunk `h` (merging it with the previous one).
    All(usize),
}

/// Unchanged lines between hunk `h` and the previous hunk (or the file start).
pub fn gap_above(diff: &Diff, h: usize) -> usize {
    let Some(hunk) = diff.hunks.get(h) else { return 0 };
    let prev_end = h.checked_sub(1).map_or(0, |p| diff.hunks[p].new_start + diff.hunks[p].new_len);
    hunk.new_start.saturating_sub(prev_end)
}

/// Unchanged lines between hunk `h` and the next hunk (or the file end).
pub fn gap_below(diff: &Diff, h: usize) -> usize {
    let Some(hunk) = diff.hunks.get(h) else { return 0 };
    let end = hunk.new_start + hunk.new_len;
    match diff.hunks.get(h + 1) {
        Some(next) => next.new_start.saturating_sub(end),
        None => diff.source.as_ref().map_or(0, |s| split_lines(&s.new).len()).saturating_sub(end),
    }
}

/// Whether `diff` can be expanded at all (full texts known, not truncated).
pub fn expandable(diff: &Diff) -> bool {
    diff.source.is_some() && !diff.truncated && !diff.hunks.is_empty()
}

struct Hunk {
    old_start: usize,
    old_len: usize,
    new_start: usize,
    new_len: usize,
    rows: Vec<DiffLine>,
}

fn context_row(text: &str, old_ix: usize, new_ix: usize) -> DiffLine {
    DiffLine {
        kind: DiffLineKind::Context,
        old_line: Some(old_ix as u32 + 1),
        new_line: Some(new_ix as u32 + 1),
        text: if text.contains('\t') { text.replace('\t', "    ") } else { text.to_string() },
        emphasis: Vec::new(),
    }
}

/// `diff` with `e` applied, or `None` if there is nothing to reveal.
pub fn expand(diff: &Diff, e: Expansion) -> Option<Diff> {
    if !expandable(diff) {
        return None;
    }
    let src = diff.source.as_ref()?;
    let new_lines = split_lines(&src.new);
    // Flat rows -> hunks.
    let mut hunks: Vec<Hunk> = diff
        .hunks
        .iter()
        .enumerate()
        .map(|(i, h)| {
            let end = diff.hunks.get(i + 1).map_or(diff.lines.len(), |n| n.header_index);
            Hunk {
                old_start: h.old_start,
                old_len: h.old_len,
                new_start: h.new_start,
                new_len: h.new_len,
                rows: diff.lines[h.header_index + 1..end].to_vec(),
            }
        })
        .collect();
    let (h, up, count) = match e {
        Expansion::Up(h) => (h, true, gap_above(diff, h).min(EXPANSION_STEP)),
        Expansion::All(h) => (h, true, gap_above(diff, h)),
        Expansion::Down(h) => (h, false, gap_below(diff, h).min(EXPANSION_STEP)),
    };
    if count == 0 || h >= hunks.len() {
        return None;
    }
    let hunk = &mut hunks[h];
    if up {
        let (os, ns) = (hunk.old_start - count, hunk.new_start - count);
        let rows: Vec<DiffLine> = (0..count).map(|i| context_row(new_lines[ns + i].text, os + i, ns + i)).collect();
        hunk.rows.splice(0..0, rows);
        hunk.old_start = os;
        hunk.new_start = ns;
    } else {
        let (os, ns) = (hunk.old_start + hunk.old_len, hunk.new_start + hunk.new_len);
        for i in 0..count {
            let line = &new_lines[ns + i];
            hunk.rows.push(context_row(line.text, os + i, ns + i));
            if !line.terminated {
                hunk.rows.push(DiffLine {
                    kind: DiffLineKind::NoNewline,
                    old_line: None,
                    new_line: None,
                    text: "\\ No newline at end of file".into(),
                    emphasis: Vec::new(),
                });
            }
        }
    }
    hunk.old_len += count;
    hunk.new_len += count;
    // Merge hunks that now touch.
    let mut merged: Vec<Hunk> = Vec::with_capacity(hunks.len());
    for hunk in hunks {
        match merged.last_mut() {
            Some(prev) if prev.new_start + prev.new_len >= hunk.new_start => {
                prev.old_len = hunk.old_start + hunk.old_len - prev.old_start;
                prev.new_len = hunk.new_start + hunk.new_len - prev.new_start;
                prev.rows.extend(hunk.rows);
            }
            _ => merged.push(hunk),
        }
    }
    // Re-flatten.
    let mut out = Diff { source: diff.source.clone(), added: diff.added, deleted: diff.deleted, ..Default::default() };
    for hunk in merged {
        out.hunks.push(DiffHunk {
            header_index: out.lines.len(),
            old_start: hunk.old_start,
            old_len: hunk.old_len,
            new_start: hunk.new_start,
            new_len: hunk.new_len,
        });
        out.lines.push(DiffLine {
            kind: DiffLineKind::Hunk,
            old_line: None,
            new_line: None,
            text: hunk_header(hunk.old_start, hunk.old_len, hunk.new_start, hunk.new_len),
            emphasis: Vec::new(),
        });
        out.lines.extend(hunk.rows);
    }
    out.longest_line = (0..out.lines.len()).max_by_key(|&i| out.lines[i].text.len()).unwrap_or(0);
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::DiffOptions;
    use crate::engine::diff_texts;

    fn file(n: usize) -> String {
        (1..=n).map(|i| format!("{i}\n")).collect()
    }

    /// Context rows must always show the line they claim to be.
    fn check_rows(d: &Diff, new: &str) {
        let lines: Vec<&str> = new.lines().collect();
        for l in &d.lines {
            if l.kind == DiffLineKind::Context {
                assert_eq!(l.text, lines[l.new_line.unwrap() as usize - 1]);
            }
        }
    }

    #[test]
    fn up_down_and_merge() {
        let old = file(100);
        let new = old.replace("10\n", "ten\n").replace("60\n", "sixty\n");
        let d = diff_texts(&old, &new, DiffOptions::default());
        assert_eq!(d.hunks.len(), 2);
        assert_eq!(gap_above(&d, 0), 6); // lines 1..=6 hidden
        assert_eq!(gap_above(&d, 1), 43); // lines 14..=56 hidden
        assert_eq!(gap_below(&d, 1), 100 - 63);

        let up = expand(&d, Expansion::Up(0)).unwrap();
        assert_eq!(gap_above(&up, 0), 0);
        assert_eq!(up.lines[0].text, "@@ -1,13 +1,13 @@");
        check_rows(&up, &new);

        let down = expand(&d, Expansion::Down(1)).unwrap();
        assert_eq!(gap_below(&down, 1), 37 - 20);
        check_rows(&down, &new);

        let all = expand(&d, Expansion::All(1)).unwrap();
        assert_eq!(all.hunks.len(), 1);
        assert_eq!(all.lines.iter().filter(|l| l.kind == DiffLineKind::Hunk).count(), 1);
        check_rows(&all, &new);
        // Changes and staging are unaffected by expansion.
        assert_eq!((all.added, all.deleted), (d.added, d.deleted));
        let pick = |d: &Diff| crate::staged_text(d, |ix| d.lines[ix].text == "ten" || d.lines[ix].text == "10").unwrap();
        assert_eq!(pick(&all), pick(&d));
    }

    #[test]
    fn down_to_eof_without_newline() {
        let old = "a\nb\nc\nd\ne\nf\ng";
        let new = "A\nb\nc\nd\ne\nf\ng";
        let d = diff_texts(old, new, DiffOptions::default());
        let down = expand(&d, Expansion::Down(0)).unwrap();
        assert_eq!(down.lines.last().unwrap().kind, DiffLineKind::NoNewline);
        assert_eq!(gap_below(&down, 0), 0);
        assert!(expand(&down, Expansion::Down(0)).is_none());
    }
}
