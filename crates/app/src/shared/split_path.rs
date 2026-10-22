/// Split `a/b/c.rs` into (`a/b/`, `c.rs`) for the dim-dir / bright-name rows.
pub fn split_path(path: &str) -> (&str, &str) {
    match path.rfind('/') {
        Some(i) => (&path[..=i], &path[i + 1..]),
        None => ("", path),
    }
}
