//! Error / success banner (bottom right) and the Workspace helpers that
//! raise, auto-dismiss and clear it.

mod handlers;
mod notice_banner;

pub(crate) use handlers::observe_notices;
pub use notice_banner::NoticeBanner;
