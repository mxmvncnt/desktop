/// Desktop-style branch name sanitizing: spaces → `-`, strip invalid chars.
pub fn sanitize_branch_name(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for ch in s.trim().chars() {
        match ch {
            c if c.is_whitespace() => out.push('-'),
            '~' | '^' | ':' | '?' | '*' | '[' | '\\' => {}
            c if c.is_control() => {}
            c => out.push(c),
        }
    }
    while out.contains("..") {
        out = out.replace("..", ".");
    }
    out.trim_matches(['/', '.', '-']).to_string()
}
