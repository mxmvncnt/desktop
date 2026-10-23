//! English plural suffix for counts.

pub fn plural(n: u32) -> &'static str {
    if n == 1 { "" } else { "s" }
}
