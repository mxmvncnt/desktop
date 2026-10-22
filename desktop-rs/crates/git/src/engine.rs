//! In-process diff engine built on the `similar` crate.
//!
//! git is only used to *fetch* the two versions of a file (blobs via
//! `cat-file`, the working tree via the filesystem); the diff itself is
//! computed here. That gives us:
//! - word-level emphasis inside changed lines (paired delete/add rows),
//! - the full old/new texts alongside the diff, which partial (line/hunk)
//!   staging needs to build the staged blob directly,
//! - one code path for working-tree and commit diffs.
//!
//! The output keeps the flat, render-ready [`Diff`] shape the UI already
//! virtualizes: row `i` is `lines[i]`, hunk headers included.

use std::ops::Range;
use std::sync::Arc;
use std::time::{Duration, Instant};

use similar::{Algorithm, DiffOp, DiffTag, capture_diff_slices_deadline, group_diff_ops};

use crate::diff::{Diff, DiffHunk, DiffLine, DiffLineKind, DiffOptions};

/// Rows beyond this are dropped (`Diff::truncated`).
pub const MAX_LINES: usize = 50_000;
/// Files larger than this aren't diffed at all (`Diff::too_large`).
pub const MAX_BYTES: usize = 8 * 1024 * 1024;
/// Lines of context around each change, like git's default `-U3`.
pub const CONTEXT: usize = 3;
/// Give up on an optimal diff after this long (similar falls back to a
/// coarser but still correct result).
const DEADLINE: Duration = Duration::from_millis(800);
/// Word emphasis is skipped for very long lines (minified files) and when
/// most of the line changed (highlighting everything is just noise).
const EMPHASIS_MAX_LINE: usize = 1_000;
const EMPHASIS_MAX_CHANGED: f32 = 0.6;

/// The two sides a diff was computed from, kept for partial staging.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DiffSource {
    /// Old content (HEAD / parent blob), exactly as stored.
    pub old: String,
    /// New content (working tree / commit blob), exactly as stored.
    pub new: String,
}

/// One logical line of a text, without its terminator.
pub(crate) struct Line<'a> {
    pub(crate) text: &'a str,
    /// Whether a `\n` followed it (false only for a final unterminated line).
    pub(crate) terminated: bool,
}

pub(crate) fn split_lines(s: &str) -> Vec<Line<'_>> {
    let mut out = Vec::new();
    let mut rest = s;
    while !rest.is_empty() {
        match rest.find('\n') {
            Some(i) => {
                out.push(Line { text: rest[..i].strip_suffix('\r').unwrap_or(&rest[..i]), terminated: true });
                rest = &rest[i + 1..];
            }
            None => {
                out.push(Line { text: rest.strip_suffix('\r').unwrap_or(rest), terminated: false });
                rest = "";
            }
        }
    }
    out
}

/// Comparison key for a line. With `ignore_whitespace` all whitespace is
/// dropped (git's `-w`). A missing final newline is part of the key so a
/// newline-only change still shows up (git's "\ No newline" case).
fn line_key(l: &Line<'_>, ignore_whitespace: bool) -> String {
    let mut k: String = if ignore_whitespace {
        l.text.chars().filter(|c| !c.is_whitespace()).collect()
    } else {
        l.text.to_string()
    };
    if !l.terminated && !ignore_whitespace {
        k.push('\u{0}');
    }
    k
}

/// Heuristic binary check, the same one git uses: a NUL in the first 8000 bytes.
pub fn looks_binary(bytes: &[u8]) -> bool {
    bytes.iter().take(8000).any(|&b| b == 0)
}

/// Diff two texts. `old`/`new` are full file contents.
pub fn diff_texts(old: &str, new: &str, opts: DiffOptions) -> Diff {
    let start = Instant::now();
    let mut diff = Diff::default();
    if old.len() > MAX_BYTES || new.len() > MAX_BYTES {
        diff.too_large = true;
        return diff;
    }
    let old_lines = split_lines(old);
    let new_lines = split_lines(new);
    let ws = opts.ignore_whitespace;
    let old_keys: Vec<String> = old_lines.iter().map(|l| line_key(l, ws)).collect();
    let new_keys: Vec<String> = new_lines.iter().map(|l| line_key(l, ws)).collect();
    let ops = capture_diff_slices_deadline(
        Algorithm::Myers,
        &old_keys,
        &new_keys,
        Some(Instant::now() + DEADLINE),
    );

    let mut longest = 0usize;
    'groups: for group in group_diff_ops(ops, CONTEXT) {
        let (Some(first), Some(last)) = (group.first(), group.last()) else { continue };
        let old_start = first.old_range().start;
        let new_start = first.new_range().start;
        let old_len = last.old_range().end - old_start;
        let new_len = last.new_range().end - new_start;
        diff.hunks.push(DiffHunk {
            header_index: diff.lines.len(),
            old_start,
            old_len,
            new_start,
            new_len,
        });
        diff.lines.push(DiffLine {
            kind: DiffLineKind::Hunk,
            old_line: None,
            new_line: None,
            text: hunk_header(old_start, old_len, new_start, new_len),
            emphasis: Vec::new(),
        });
        for op in &group {
            let (tag, o, n) = op.as_tag_tuple();
            match tag {
                DiffTag::Equal => {
                    for (oi, ni) in o.zip(n) {
                        // Under -w "equal" lines may differ in whitespace;
                        // show the new side like git does.
                        push_line(&mut diff, &mut longest, DiffLineKind::Context, Some(oi), Some(ni), new_lines[ni].text);
                        no_newline_marker(&mut diff, &new_lines, ni);
                    }
                }
                DiffTag::Delete | DiffTag::Insert | DiffTag::Replace => {
                    let del_first = diff.lines.len();
                    for oi in o.clone() {
                        push_line(&mut diff, &mut longest, DiffLineKind::Delete, Some(oi), None, old_lines[oi].text);
                        no_newline_marker(&mut diff, &old_lines, oi);
                    }
                    let add_first = diff.lines.len();
                    for ni in n.clone() {
                        push_line(&mut diff, &mut longest, DiffLineKind::Add, None, Some(ni), new_lines[ni].text);
                        no_newline_marker(&mut diff, &new_lines, ni);
                    }
                    if tag == DiffTag::Replace {
                        emphasize_pairs(&mut diff, del_first, add_first);
                    }
                }
            }
            if diff.lines.len() >= MAX_LINES {
                diff.truncated = true;
                break 'groups;
            }
        }
    }
    diff.longest_line = longest;
    diff.source = Some(Arc::new(DiffSource { old: old.to_string(), new: new.to_string() }));
    log::debug!(
        "diff_texts: {} rows ({}+ {}-) in {:?}",
        diff.lines.len(),
        diff.added,
        diff.deleted,
        start.elapsed()
    );
    diff
}

pub(crate) fn hunk_header(old_start: usize, old_len: usize, new_start: usize, new_len: usize) -> String {
    // Same numbering as git: 1-based start, and a 0-length side starts at
    // the line *before* the change.
    let side = |start: usize, len: usize| {
        let s = if len == 0 { start } else { start + 1 };
        if len == 1 { format!("{s}") } else { format!("{s},{len}") }
    };
    format!("@@ -{} +{} @@", side(old_start, old_len), side(new_start, new_len))
}

fn push_line(
    diff: &mut Diff,
    longest: &mut usize,
    kind: DiffLineKind,
    old_ix: Option<usize>,
    new_ix: Option<usize>,
    text: &str,
) {
    match kind {
        DiffLineKind::Add => diff.added += 1,
        DiffLineKind::Delete => diff.deleted += 1,
        _ => {}
    }
    // Tabs render at inconsistent widths in proportional fallbacks; expand
    // once here instead of at every paint.
    let text = if text.contains('\t') { text.replace('\t', "    ") } else { text.to_string() };
    if text.len() > diff.lines.get(*longest).map_or(0, |l| l.text.len()) {
        *longest = diff.lines.len();
    }
    diff.lines.push(DiffLine {
        kind,
        old_line: old_ix.map(|i| i as u32 + 1),
        new_line: new_ix.map(|i| i as u32 + 1),
        text,
        emphasis: Vec::new(),
    });
}

fn no_newline_marker(diff: &mut Diff, lines: &[Line<'_>], ix: usize) {
    if !lines[ix].terminated {
        diff.lines.push(DiffLine {
            kind: DiffLineKind::NoNewline,
            old_line: None,
            new_line: None,
            text: "\\ No newline at end of file".into(),
            emphasis: Vec::new(),
        });
    }
}

/// Pair the i-th deleted row with the i-th added row of a replace block and
/// mark the words that differ, like Desktop's intra-line highlighting.
fn emphasize_pairs(diff: &mut Diff, del_first: usize, add_first: usize) {
    let dels: Vec<usize> = (del_first..add_first).filter(|&i| diff.lines[i].kind == DiffLineKind::Delete).collect();
    let adds: Vec<usize> =
        (add_first..diff.lines.len()).filter(|&i| diff.lines[i].kind == DiffLineKind::Add).collect();
    for (&d, &a) in dels.iter().zip(adds.iter()) {
        let (old_e, new_e) = word_emphasis(&diff.lines[d].text, &diff.lines[a].text);
        diff.lines[d].emphasis = old_e;
        diff.lines[a].emphasis = new_e;
    }
}

/// Split into word / whitespace / punctuation tokens (byte ranges).
fn tokenize(s: &str) -> Vec<Range<usize>> {
    #[derive(PartialEq, Clone, Copy)]
    enum K {
        Word,
        Space,
        Punct,
    }
    let class = |c: char| {
        if c.is_alphanumeric() || c == '_' {
            K::Word
        } else if c.is_whitespace() {
            K::Space
        } else {
            K::Punct
        }
    };
    let mut out = Vec::new();
    let mut start = 0;
    let mut prev: Option<K> = None;
    for (i, c) in s.char_indices() {
        let k = class(c);
        // Punctuation is one token per char so `foo(` vs `foo[` stays local.
        if prev.is_some() && (Some(k) != prev || k == K::Punct) {
            out.push(start..i);
            start = i;
        }
        prev = Some(k);
    }
    if start < s.len() {
        out.push(start..s.len());
    }
    out
}

/// Byte ranges that differ between `old` and `new`, merged when adjacent.
pub fn word_emphasis(old: &str, new: &str) -> (Vec<(usize, usize)>, Vec<(usize, usize)>) {
    if old.len() > EMPHASIS_MAX_LINE || new.len() > EMPHASIS_MAX_LINE {
        return (Vec::new(), Vec::new());
    }
    let ot = tokenize(old);
    let nt = tokenize(new);
    let ok: Vec<&str> = ot.iter().map(|r| &old[r.clone()]).collect();
    let nk: Vec<&str> = nt.iter().map(|r| &new[r.clone()]).collect();
    let ops = capture_diff_slices_deadline(Algorithm::Myers, &ok, &nk, Some(Instant::now() + Duration::from_millis(5)));
    let mut oe: Vec<(usize, usize)> = Vec::new();
    let mut ne: Vec<(usize, usize)> = Vec::new();
    let push = |v: &mut Vec<(usize, usize)>, toks: &[Range<usize>], r: Range<usize>| {
        if r.is_empty() {
            return;
        }
        let (s, e) = (toks[r.start].start, toks[r.end - 1].end);
        match v.last_mut() {
            Some(last) if last.1 == s => last.1 = e,
            _ => v.push((s, e)),
        }
    };
    for op in &ops {
        if let DiffOp::Equal { .. } = op {
            continue;
        }
        let (_, o, n) = op.as_tag_tuple();
        push(&mut oe, &ot, o);
        push(&mut ne, &nt, n);
    }
    let changed = |v: &[(usize, usize)], len: usize| {
        if len == 0 { 0.0 } else { v.iter().map(|(s, e)| e - s).sum::<usize>() as f32 / len as f32 }
    };
    // Whole-line rewrites: emphasis adds nothing.
    if changed(&oe, old.len()) > EMPHASIS_MAX_CHANGED && changed(&ne, new.len()) > EMPHASIS_MAX_CHANGED {
        return (Vec::new(), Vec::new());
    }
    (oe, ne)
}
