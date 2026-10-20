//! Syntax highlighting for diffs (tree-sitter), ported from zeron's `zeron-syntax`
//! and trimmed to a curated grammar set. UI-agnostic: input is a path plus text,
//! output is per-line spans of `(byte_range, HighlightKind)`. Ranges are byte
//! offsets relative to one UTF-8 source line.

use std::{collections::BTreeSet, ops::Range, path::Path, sync::atomic::AtomicUsize};

use tree_sitter_highlight::{HighlightConfiguration, HighlightEvent, Highlighter};

pub const DEFAULT_MAX_SOURCE_BYTES: usize = 2 * 1024 * 1024;
pub const DEFAULT_MAX_SPANS: usize = 1_000_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HighlightLimits {
    pub max_source_bytes: usize,
    pub max_spans: usize,
}

impl Default for HighlightLimits {
    fn default() -> Self {
        Self {
            max_source_bytes: DEFAULT_MAX_SOURCE_BYTES,
            max_spans: DEFAULT_MAX_SPANS,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LanguageId {
    Rust,
    JavaScript,
    Jsx,
    TypeScript,
    Tsx,
    Python,
    Go,
    Json,
    Jsonc,
    Bash,
    Toml,
    Markdown,
    Html,
    Css,
    Yaml,
    C,
    Cpp,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum HighlightKind {
    Comment,
    Keyword,
    String,
    StringSpecial,
    Escape,
    Number,
    Boolean,
    Type,
    TypeBuiltin,
    Constructor,
    Function,
    FunctionBuiltin,
    Macro,
    Property,
    Constant,
    Variable,
    VariableSpecial,
    Parameter,
    Operator,
    Punctuation,
    Tag,
    Attribute,
    Label,
    MarkupHeading,
    MarkupRaw,
    MarkupLink,
    MarkupReference,
    MarkupEmphasis,
    MarkupStrong,
    Embedded,
    Invalid,
}

impl HighlightKind {
    /// Stable precedence used to resolve overlapping parser captures.
    pub const fn precedence(self) -> u8 {
        match self {
            Self::Invalid => 100,
            Self::Escape => 95,
            Self::Macro => 90,
            Self::Property | Self::Attribute => 85,
            Self::FunctionBuiltin | Self::TypeBuiltin | Self::VariableSpecial => 80,
            Self::StringSpecial | Self::Constructor | Self::Parameter => 75,
            Self::Function | Self::Type | Self::Constant | Self::Tag | Self::Label => 70,
            Self::Comment | Self::Keyword | Self::String | Self::Number | Self::Boolean => 60,
            Self::Variable | Self::Operator => 50,
            Self::Punctuation | Self::Embedded => 40,
            // Markdown block captures can wrap more specific inline captures,
            // and fenced-code captures can wrap an injected language. Keep
            // markup below programming-language tokens while preserving the
            // nesting order among Markdown roles.
            Self::MarkupHeading => 30,
            Self::MarkupEmphasis => 31,
            Self::MarkupStrong => 32,
            Self::MarkupLink | Self::MarkupReference => 33,
            Self::MarkupRaw => 34,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HighlightSpan {
    pub range: Range<usize>,
    pub kind: HighlightKind,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HighlightedDocument {
    pub language: LanguageId,
    pub lines: Vec<Vec<HighlightSpan>>,
}

#[derive(Debug, Clone, Copy)]
pub struct HighlightRequest<'a> {
    pub source: &'a str,
    pub path: Option<&'a str>,
    pub fence_tag: Option<&'a str>,
}

#[derive(Debug, Clone, thiserror::Error, PartialEq, Eq)]
pub enum HighlightError {
    #[error("the source language is not registered")]
    UnknownLanguage,
    #[error("highlight range {start}..{end} is invalid for a {len}-byte source")]
    InvalidRange {
        start: usize,
        end: usize,
        len: usize,
    },
    #[error("highlight range {start}..{end} is not on UTF-8 boundaries")]
    InvalidUtf8Boundary { start: usize, end: usize },
    #[error("source exceeds the configured highlighting limit")]
    SourceTooLarge,
    #[error("highlight output exceeds the configured span limit")]
    TooManySpans,
    #[error("parser failed: {0}")]
    Parser(String),
    #[error("the {0:?} grammar is not bundled")]
    GrammarUnavailable(LanguageId),
}

impl HighlightedDocument {
    /// Validate, split, and normalize absolute source spans into line-relative spans.
    pub fn from_absolute_spans(
        language: LanguageId,
        source: &str,
        spans: impl IntoIterator<Item = HighlightSpan>,
    ) -> Result<Self, HighlightError> {
        let starts = line_starts(source);
        let mut lines = vec![Vec::new(); starts.len()];
        for span in spans {
            validate_span(source, &span.range)?;
            if span.range.is_empty() {
                continue;
            }
            let first_line = starts.partition_point(|&start| start <= span.range.start) - 1;
            for (line_ix, &start) in starts.iter().enumerate().skip(first_line) {
                let raw_end = starts.get(line_ix + 1).copied().unwrap_or(source.len());
                let mut end = raw_end;
                if source.as_bytes().get(end.wrapping_sub(1)) == Some(&b'\n') {
                    end -= 1;
                    if source.as_bytes().get(end.wrapping_sub(1)) == Some(&b'\r') {
                        end -= 1;
                    }
                }
                let segment_start = span.range.start.max(start);
                let segment_end = span.range.end.min(end);
                if segment_start < segment_end {
                    lines[line_ix].push(HighlightSpan {
                        range: segment_start - start..segment_end - start,
                        kind: span.kind,
                    });
                }
                if raw_end >= span.range.end {
                    break;
                }
            }
        }
        for line in &mut lines {
            *line = normalize_line(std::mem::take(line));
        }
        Ok(Self { language, lines })
    }
}

fn validate_span(source: &str, range: &Range<usize>) -> Result<(), HighlightError> {
    if range.start > range.end || range.end > source.len() {
        return Err(HighlightError::InvalidRange {
            start: range.start,
            end: range.end,
            len: source.len(),
        });
    }
    if !source.is_char_boundary(range.start) || !source.is_char_boundary(range.end) {
        return Err(HighlightError::InvalidUtf8Boundary {
            start: range.start,
            end: range.end,
        });
    }
    Ok(())
}

fn normalize_line(spans: Vec<HighlightSpan>) -> Vec<HighlightSpan> {
    #[derive(Clone, Copy)]
    enum Edge {
        Start(usize),
        End(usize),
    }

    let mut edges = spans
        .iter()
        .enumerate()
        .flat_map(|(index, span)| {
            [
                (span.range.start, Edge::Start(index)),
                (span.range.end, Edge::End(index)),
            ]
        })
        .collect::<Vec<_>>();
    edges.sort_unstable_by_key(|(offset, _)| *offset);

    // The span index is the tie-breaker so equal-precedence overlaps retain
    // the old `Iterator::max_by_key` behavior (the later span wins).
    let mut active = BTreeSet::new();
    let mut normalized: Vec<HighlightSpan> = Vec::new();
    let mut cursor = 0;
    while cursor < edges.len() {
        let offset = edges[cursor].0;
        let group_start = cursor;
        while cursor < edges.len() && edges[cursor].0 == offset {
            if let Edge::End(index) = edges[cursor].1 {
                active.remove(&(spans[index].kind.precedence(), index));
            }
            cursor += 1;
        }
        for (_, edge) in &edges[group_start..cursor] {
            if let Edge::Start(index) = *edge {
                active.insert((spans[index].kind.precedence(), index));
            }
        }

        let Some(next_offset) = edges.get(cursor).map(|(next, _)| *next) else {
            break;
        };
        if offset == next_offset {
            continue;
        }
        if let Some((_, index)) = active.last().copied() {
            let kind = spans[index].kind;
            if let Some(previous) = normalized.last_mut()
                && previous.kind == kind
                && previous.range.end == offset
            {
                previous.range.end = next_offset;
            } else {
                normalized.push(HighlightSpan {
                    range: offset..next_offset,
                    kind,
                });
            }
        }
    }
    normalized
}

fn line_starts(source: &str) -> Vec<usize> {
    let mut starts = vec![0];
    starts.extend(
        source
            .match_indices('\n')
            .map(|(index, _)| index + 1)
            .filter(|start| *start < source.len()),
    );
    starts
}

/// Whether this build contains a parser and compatible highlight queries.
pub const fn supports_language(language: LanguageId) -> bool {
    let _ = language;
    true
}

/// Highlight a complete document with the default resource limits.
pub fn highlight(request: HighlightRequest<'_>) -> Result<HighlightedDocument, HighlightError> {
    highlight_with_limits(request, HighlightLimits::default(), None)
}

/// Highlight a complete document with explicit limits and cooperative cancellation.
pub fn highlight_with_limits(
    request: HighlightRequest<'_>,
    limits: HighlightLimits,
    cancellation_flag: Option<&AtomicUsize>,
) -> Result<HighlightedDocument, HighlightError> {
    if request.source.len() > limits.max_source_bytes {
        return Err(HighlightError::SourceTooLarge);
    }
    let language = detect_language(
        request.path,
        request.fence_tag,
        request.source.lines().next(),
    )
    .ok_or(HighlightError::UnknownLanguage)?;
    if !supports_language(language) {
        return Err(HighlightError::GrammarUnavailable(language));
    }

    let primary_configuration = cached_configuration(language)?;
    let injected = injected_languages(language);
    let markdown_inline = if language == LanguageId::Markdown {
        Some(cached_markdown_inline_configuration()?)
    } else {
        None
    };
    let mut highlighter = Highlighter::new();
    let events = highlighter
        .highlight(
            primary_configuration,
            request.source.as_bytes(),
            cancellation_flag,
            |name| {
                if name == "markdown_inline" {
                    return markdown_inline;
                }
                let language = language_for_alias(name)?;
                if !injected.contains(&language) {
                    return None;
                }
                cached_configuration(language).ok()
            },
        )
        .map_err(|error| HighlightError::Parser(error.to_string()))?;

    let mut active = Vec::new();
    let mut spans = Vec::new();
    for event in events {
        match event.map_err(|error| HighlightError::Parser(error.to_string()))? {
            HighlightEvent::HighlightStart(highlight) => active.push(CAPTURE_KINDS[highlight.0]),
            HighlightEvent::HighlightEnd => {
                active.pop();
            }
            HighlightEvent::Source { start, end } => {
                if let Some(kind) = active.iter().copied().max_by_key(|kind| kind.precedence()) {
                    spans.push(HighlightSpan {
                        range: start..end,
                        kind,
                    });
                    if spans.len() > limits.max_spans {
                        return Err(HighlightError::TooManySpans);
                    }
                }
            }
        }
    }
    HighlightedDocument::from_absolute_spans(language, request.source, spans)
}

fn injected_languages(parent: LanguageId) -> Vec<LanguageId> {
    use LanguageId::*;
    match parent {
        Html => vec![JavaScript, Css],
        Markdown => vec![
            Rust, JavaScript, Jsx, TypeScript, Tsx, Python, Go, Json, Jsonc, Bash, Toml, Html, Css,
            Yaml, C, Cpp,
        ],
        _ => Vec::new(),
    }
}

/// Compiled queries contain no document/theme state and are immutable after
/// capture configuration. Keep one per used grammar instead of recompiling
/// for every fence or eagerly compiling all 27 Markdown injection targets.
/// Each grammar has its own cell: concurrent requests compile it only once,
/// while unrelated languages never wait on a global compilation lock.
fn cached_configuration(
    language: LanguageId,
) -> Result<&'static HighlightConfiguration, HighlightError> {
    macro_rules! registry {
        ($($variant:ident),+ $(,)?) => {
            match language {
                $(LanguageId::$variant => {
                    static CONFIG: std::sync::OnceLock<Result<HighlightConfiguration, HighlightError>> =
                        std::sync::OnceLock::new();
                    CONFIG.get_or_init(|| {
                        let mut config = configuration(language)?;
                        config.configure(CAPTURE_NAMES);
                        Ok(config)
                    }).as_ref().map_err(Clone::clone)
                }),+
            }
        };
    }
    registry!(
        Rust, JavaScript, Jsx, TypeScript, Tsx, Python, Go, Json, Jsonc, Bash, Toml, Markdown,
        Html, Css, Yaml, C, Cpp,
    )
}

fn cached_markdown_inline_configuration() -> Result<&'static HighlightConfiguration, HighlightError>
{
    static CONFIG: std::sync::OnceLock<Result<HighlightConfiguration, HighlightError>> =
        std::sync::OnceLock::new();
    CONFIG
        .get_or_init(|| {
            let mut config = markdown_inline_configuration()?;
            config.configure(CAPTURE_NAMES);
            Ok(config)
        })
        .as_ref()
        .map_err(Clone::clone)
}

fn rust_configuration() -> Result<HighlightConfiguration, HighlightError> {
    // The upstream Rust query groups numbers and booleans as
    // `constant.builtin`. Zeron preserves those structural roles separately.
    let highlights = tree_sitter_rust::HIGHLIGHTS_QUERY
        .replace(
            "(boolean_literal) @constant.builtin",
            "(boolean_literal) @boolean",
        )
        .replace(
            "(integer_literal) @constant.builtin",
            "(integer_literal) @number",
        )
        .replace(
            "(float_literal) @constant.builtin",
            "(float_literal) @number",
        );
    HighlightConfiguration::new(
        tree_sitter_rust::LANGUAGE.into(),
        "rust",
        &highlights,
        tree_sitter_rust::INJECTIONS_QUERY,
        "",
    )
    .map_err(|error| HighlightError::Parser(error.to_string()))
}

fn markdown_configuration() -> Result<HighlightConfiguration, HighlightError> {
    // tree-sitter-highlight excludes child ranges from injections by default.
    // The Markdown block grammar's `inline` node owns anonymous children that
    // cover its source, so the upstream query otherwise injects an empty range.
    let injections = tree_sitter_md::INJECTION_QUERY_BLOCK.replace(
        "((inline) @injection.content\n  (#set! injection.language \"markdown_inline\"))",
        "((inline) @injection.content\n  (#set! injection.language \"markdown_inline\")\n  (#set! injection.include-children))",
    );
    make_configuration(
        tree_sitter_md::LANGUAGE.into(),
        "markdown",
        tree_sitter_md::HIGHLIGHT_QUERY_BLOCK,
        &injections,
        "",
    )
}

fn markdown_inline_configuration() -> Result<HighlightConfiguration, HighlightError> {
    make_configuration(
        tree_sitter_md::INLINE_LANGUAGE.into(),
        "markdown_inline",
        tree_sitter_md::HIGHLIGHT_QUERY_INLINE,
        tree_sitter_md::INJECTION_QUERY_INLINE,
        "",
    )
}

fn make_configuration(
    language: tree_sitter::Language,
    name: &str,
    highlights: &str,
    injections: &str,
    locals: &str,
) -> Result<HighlightConfiguration, HighlightError> {
    HighlightConfiguration::new(language, name, highlights, injections, locals)
        .map_err(|error| HighlightError::Parser(error.to_string()))
}

fn javascript_family_highlights(language: LanguageId) -> String {
    use LanguageId::*;

    let queries = match language {
        JavaScript => &[tree_sitter_javascript::HIGHLIGHT_QUERY][..],
        Jsx => &[
            tree_sitter_javascript::HIGHLIGHT_QUERY,
            tree_sitter_javascript::JSX_HIGHLIGHT_QUERY,
        ][..],
        TypeScript => &[
            tree_sitter_javascript::HIGHLIGHT_QUERY,
            tree_sitter_typescript::HIGHLIGHTS_QUERY,
        ][..],
        Tsx => &[
            tree_sitter_javascript::HIGHLIGHT_QUERY,
            tree_sitter_javascript::JSX_HIGHLIGHT_QUERY,
            tree_sitter_typescript::HIGHLIGHTS_QUERY,
        ][..],
        _ => unreachable!("JavaScript query composition requires a JavaScript-family language"),
    };
    queries.join("\n")
}

fn javascript_family_configuration(
    language: LanguageId,
) -> Result<HighlightConfiguration, HighlightError> {
    use LanguageId::*;

    let highlights = javascript_family_highlights(language);
    let (grammar, name, injections, locals) = match language {
        JavaScript => (
            tree_sitter_javascript::LANGUAGE.into(),
            "javascript",
            tree_sitter_javascript::INJECTIONS_QUERY,
            tree_sitter_javascript::LOCALS_QUERY,
        ),
        Jsx => (
            tree_sitter_javascript::LANGUAGE.into(),
            "jsx",
            tree_sitter_javascript::INJECTIONS_QUERY,
            tree_sitter_javascript::LOCALS_QUERY,
        ),
        TypeScript => (
            tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into(),
            "typescript",
            "",
            tree_sitter_typescript::LOCALS_QUERY,
        ),
        Tsx => (
            tree_sitter_typescript::LANGUAGE_TSX.into(),
            "tsx",
            "",
            tree_sitter_typescript::LOCALS_QUERY,
        ),
        _ => unreachable!("JavaScript configuration requires a JavaScript-family language"),
    };
    make_configuration(grammar, name, &highlights, injections, locals)
}

fn configuration(language: LanguageId) -> Result<HighlightConfiguration, HighlightError> {
    use LanguageId::*;
    match language {
        Rust => rust_configuration(),
        JavaScript | Jsx | TypeScript | Tsx => javascript_family_configuration(language),
        Python => make_configuration(
            tree_sitter_python::LANGUAGE.into(),
            "python",
            tree_sitter_python::HIGHLIGHTS_QUERY,
            "",
            "",
        ),
        Go => make_configuration(
            tree_sitter_go::LANGUAGE.into(),
            "go",
            tree_sitter_go::HIGHLIGHTS_QUERY,
            "",
            "",
        ),
        Json | Jsonc => make_configuration(
            tree_sitter_json::LANGUAGE.into(),
            "json",
            tree_sitter_json::HIGHLIGHTS_QUERY,
            "",
            "",
        ),
        Bash => make_configuration(
            tree_sitter_bash::LANGUAGE.into(),
            "bash",
            tree_sitter_bash::HIGHLIGHT_QUERY,
            "",
            "",
        ),
        Toml => make_configuration(
            tree_sitter_toml_ng::LANGUAGE.into(),
            "toml",
            tree_sitter_toml_ng::HIGHLIGHTS_QUERY,
            "",
            "",
        ),
        Markdown => markdown_configuration(),
        Html => make_configuration(
            tree_sitter_html::LANGUAGE.into(),
            "html",
            tree_sitter_html::HIGHLIGHTS_QUERY,
            tree_sitter_html::INJECTIONS_QUERY,
            "",
        ),
        Css => make_configuration(
            tree_sitter_css::LANGUAGE.into(),
            "css",
            tree_sitter_css::HIGHLIGHTS_QUERY,
            "",
            "",
        ),
        Yaml => make_configuration(
            tree_sitter_yaml::LANGUAGE.into(),
            "yaml",
            tree_sitter_yaml::HIGHLIGHTS_QUERY,
            "",
            "",
        ),
        C => make_configuration(
            tree_sitter_c::LANGUAGE.into(),
            "c",
            tree_sitter_c::HIGHLIGHT_QUERY,
            "",
            "",
        ),
        Cpp => make_configuration(
            tree_sitter_cpp::LANGUAGE.into(),
            "cpp",
            &format!(
                "{}\n{}",
                tree_sitter_c::HIGHLIGHT_QUERY,
                tree_sitter_cpp::HIGHLIGHT_QUERY
            ),
            "",
            "",
        ),
    }
}

// Ordered from generic to specific. `HighlightConfiguration::configure`
// resolves dotted captures to the best recognized name in this table.
const CAPTURE_NAMES: &[&str] = &[
    "comment",
    "keyword",
    "string",
    "string.special",
    "string.escape",
    "number",
    "boolean",
    "type",
    "type.builtin",
    "constructor",
    "function",
    "function.builtin",
    "function.macro",
    "property",
    "constant",
    "variable",
    "variable.builtin",
    "variable.parameter",
    "operator",
    "punctuation",
    "tag",
    "attribute",
    "label",
    "text.title",
    "text.literal",
    "text.uri",
    "text.reference",
    "text.emphasis",
    "text.strong",
    "embedded",
    "error",
];

const CAPTURE_KINDS: &[HighlightKind] = &[
    HighlightKind::Comment,
    HighlightKind::Keyword,
    HighlightKind::String,
    HighlightKind::StringSpecial,
    HighlightKind::Escape,
    HighlightKind::Number,
    HighlightKind::Boolean,
    HighlightKind::Type,
    HighlightKind::TypeBuiltin,
    HighlightKind::Constructor,
    HighlightKind::Function,
    HighlightKind::FunctionBuiltin,
    HighlightKind::Macro,
    HighlightKind::Property,
    HighlightKind::Constant,
    HighlightKind::Variable,
    HighlightKind::VariableSpecial,
    HighlightKind::Parameter,
    HighlightKind::Operator,
    HighlightKind::Punctuation,
    HighlightKind::Tag,
    HighlightKind::Attribute,
    HighlightKind::Label,
    HighlightKind::MarkupHeading,
    HighlightKind::MarkupRaw,
    HighlightKind::MarkupLink,
    HighlightKind::MarkupReference,
    HighlightKind::MarkupEmphasis,
    HighlightKind::MarkupStrong,
    HighlightKind::Embedded,
    HighlightKind::Invalid,
];

pub fn detect_language(
    path: Option<&str>,
    fence_tag: Option<&str>,
    first_line: Option<&str>,
) -> Option<LanguageId> {
    fence_tag
        .and_then(language_for_alias)
        .or_else(|| path.and_then(language_for_path))
        .or_else(|| first_line.and_then(language_for_shebang))
}

pub fn language_for_alias(alias: &str) -> Option<LanguageId> {
    let alias = alias
        .trim()
        .split_ascii_whitespace()
        .next()?
        .to_ascii_lowercase();
    Some(match alias.as_str() {
        "rust" | "rs" => LanguageId::Rust,
        "javascript" | "js" | "mjs" | "cjs" => LanguageId::JavaScript,
        "jsx" => LanguageId::Jsx,
        "typescript" | "ts" | "mts" | "cts" => LanguageId::TypeScript,
        "tsx" => LanguageId::Tsx,
        "python" | "py" | "python3" => LanguageId::Python,
        "go" | "golang" => LanguageId::Go,
        "json" => LanguageId::Json,
        "jsonc" => LanguageId::Jsonc,
        "bash" | "sh" | "shell" | "zsh" | "console" => LanguageId::Bash,
        "toml" => LanguageId::Toml,
        "markdown" | "md" => LanguageId::Markdown,
        "html" | "htm" => LanguageId::Html,
        "css" => LanguageId::Css,
        "yaml" | "yml" => LanguageId::Yaml,
        "c" => LanguageId::C,
        "cpp" | "c++" | "cc" | "cxx" | "hpp" => LanguageId::Cpp,
        _ => return None,
    })
}

pub fn language_for_path(path: &str) -> Option<LanguageId> {
    let path = Path::new(path);
    let name = path.file_name()?.to_str()?;
    match name.to_ascii_lowercase().as_str() {
        "cargo.lock" | "cargo.toml" | "pyproject.toml" => return Some(LanguageId::Toml),
        _ => {}
    }
    language_for_alias(path.extension()?.to_str()?)
}

fn language_for_shebang(line: &str) -> Option<LanguageId> {
    let line = line.strip_prefix("#!")?.to_ascii_lowercase();
    if line.contains("python") {
        Some(LanguageId::Python)
    } else if line.contains("node") {
        Some(LanguageId::JavaScript)
    } else if ["bash", "zsh", "/sh", " sh"]
        .iter()
        .any(|name| line.contains(name))
    {
        Some(LanguageId::Bash)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(path: &str, source: &str) -> HighlightedDocument {
        highlight(HighlightRequest {
            source,
            path: Some(path),
            fence_tag: None,
        })
        .unwrap()
    }

    #[test]
    fn every_grammar_loads_and_highlights() {
        for (path, src) in [
            ("a.rs", "fn main() { let x = 1; } // hi"),
            ("a.js", "const a = () => 'x';"),
            ("a.jsx", "const a = <div>hi</div>;"),
            ("a.ts", "let a: number = 1;"),
            ("a.tsx", "const a: number = 1; <b/>"),
            ("a.py", "def f(x):\n    return 'a'"),
            ("a.go", "package main\nfunc main() {}"),
            ("a.json", "{\"a\": 1}"),
            ("a.sh", "echo \"hi\" | cat"),
            ("Cargo.toml", "[a]\nb = 1"),
            (
                "a.md",
                "# Title\n\n`code` and *em*\n\n```rust\nfn x() {}\n```\n",
            ),
            (
                "a.html",
                "<div class=\"a\"><script>let a = 1;</script><style>a { color: red }</style></div>",
            ),
            ("a.css", "a { color: red; }"),
            ("a.yml", "a: 1\nb: [x]"),
            ("a.c", "int main() { return 0; }"),
            ("a.cpp", "class A { public: int x; };"),
        ] {
            let doc = run(path, src);
            assert!(
                doc.lines.iter().any(|l| !l.is_empty()),
                "{path} produced no spans"
            );
        }
    }

    #[test]
    fn unknown_and_oversized_are_errors() {
        let req = |p| HighlightRequest {
            source: "x",
            path: Some(p),
            fence_tag: None,
        };
        assert_eq!(
            highlight(req("a.unknownext")).unwrap_err(),
            HighlightError::UnknownLanguage
        );
        let big = "a".repeat(DEFAULT_MAX_SOURCE_BYTES + 1);
        let r = HighlightRequest {
            source: &big,
            path: Some("a.rs"),
            fence_tag: None,
        };
        assert_eq!(highlight(r).unwrap_err(), HighlightError::SourceTooLarge);
    }

    #[test]
    fn spans_are_line_relative() {
        let doc = run("a.rs", "let a = 1;\nlet b = \"é\";\n");
        for (line, text) in doc.lines.iter().zip(["let a = 1;", "let b = \"é\";"]) {
            for s in line {
                assert!(s.range.end <= text.len() && text.is_char_boundary(s.range.start));
            }
        }
    }
}
