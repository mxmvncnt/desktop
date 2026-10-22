//! Per-OS code. Each submodule exposes the same small surface, and the
//! rest of the app only talks to it through this module:
//!
//! - `Chrome`, `caption_buttons`, `titlebar_drag`, `frame`: window chrome
//! - `config_base_dir()`: where the settings directory lives
//! - `configure_window(&mut WindowOptions)`: platform window options
//! - `SECONDARY`, `REFRESH_KEY`, `QUIT_KEY`: shortcut modifier and keys

#[cfg(any(target_os = "linux", target_os = "freebsd"))]
mod linux;
#[cfg(any(target_os = "linux", target_os = "freebsd"))]
pub use linux::*;

#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "macos")]
pub use macos::*;

#[cfg(not(any(target_os = "linux", target_os = "freebsd", target_os = "macos")))]
compile_error!("unsupported platform: add a module under app/src/platform/");
