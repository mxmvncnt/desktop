//! Settings modal (appearance and diff pages) and the persisted `Settings`
//! model (`model.rs`). Shown while `Workspace::settings_open`.

mod appearance_page;
mod diff_page;
mod handlers;
mod model;
mod settings_modal;
mod settings_nav;
mod state;

pub use model::{Settings, ThemeMode};
pub use settings_modal::SettingsModal;
pub use state::SettingsPage;
