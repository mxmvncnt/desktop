//! Desktop's idle Fetch caption.

use std::time::SystemTime;

use crate::shared::relative_time::{Coarsest, relative_time_since};

/// "Last fetched 5 minutes ago" / "Never fetched".
pub fn last_fetched_caption(last: Option<SystemTime>) -> String {
    let Some(last) = last else {
        return "Never fetched".into();
    };
    let ago = relative_time_since(last, Coarsest::Day);
    format!("Last fetched {ago}")
}
