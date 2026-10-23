//! "Alice and Bob" style author text for a commit with `Co-authored-by` trailers.

use gd_git::Commit;

/// Author plus co-authors: "A", "A and B", "A, B and C", "A, B and 2 others".
pub fn authors_text(c: &Commit) -> String {
    let mut names: Vec<&str> = vec![c.author_name.as_str()];
    names.extend(c.co_authors.iter().map(|a| a.name.as_str()));
    match names.as_slice() {
        [one] => (*one).into(),
        [a, b] => format!("{a} and {b}"),
        [a, b, c] => format!("{a}, {b} and {c}"),
        [a, b, rest @ ..] => format!("{a}, {b} and {} others", rest.len()),
        [] => String::new(),
    }
}

/// The commit body without `Co-authored-by:` trailer lines (shown as authors instead).
pub fn body_without_trailers(body: &str) -> String {
    let kept: Vec<&str> = body
        .lines()
        .filter(|l| !l.split_once(':').is_some_and(|(k, _)| k.trim().eq_ignore_ascii_case("co-authored-by")))
        .collect();
    kept.join("\n").trim().to_string()
}
