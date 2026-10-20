//! Embedded assets: Geist fonts (OFL) and Solar icons (CC BY 4.0,
//! "Solar Icons by 480 Design"), copied from zeron's `crates/ui/assets`.
//!
//! Everything is `include_bytes!`'d so the binary is self-contained and icon
//! loads never touch the filesystem.

use std::borrow::Cow;

use gpui::{App, AssetSource, Result, SharedString};

macro_rules! icons {
    ($($name:ident => $file:literal),+ $(,)?) => {
        /// Icon paths, for `icon(Icon::Check)` / `svg().path(...)`.
        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub enum Icon { $($name),+ }

        impl Icon {
            pub const ALL: &[Icon] = &[$(Self::$name),+];

            pub fn path(self) -> &'static str {
                match self { $(Self::$name => concat!("icons/", $file, ".svg")),+ }
            }
        }

        fn load_icon(path: &str) -> Option<&'static [u8]> {
            match path {
                $(concat!("icons/", $file, ".svg") => Some(include_bytes!(concat!("../assets/icons/", $file, ".svg")).as_slice()),)+
                _ => None,
            }
        }
    };
}

icons! {
    ChevronDown => "alt-arrow-down",
    ChevronRight => "alt-arrow-right",
    ChevronUp => "alt-arrow-up",
    ArrowDown => "arrow-down",
    ArrowUp => "arrow-up",
    ArrowLeft => "arrow-left",
    ArrowRight => "arrow-right",
    Check => "check",
    Close => "close",
    CloseCircle => "close-circle",
    Clock => "clock-circle",
    Cloud => "cloud",
    Copy => "copy",
    Document => "document",
    DocumentAdd => "document-add",
    Eye => "eye",
    EyeClosed => "eye-closed",
    Folder => "folder",
    FolderWithFiles => "folder-with-files",
    GitBranch => "git-branch",
    Globe => "global",
    Info => "info-circle",
    Search => "magnifer",
    More => "more-horizontal",
    Plus => "plus",
    PullRequest => "pull-request",
    Refresh => "refresh",
    Restart => "restart",
    Settings => "settings",
    Sort => "sort",
    Tag => "tag",
    Terminal => "terminal",
    Trash => "trash-bin-minimalistic",
    List => "list",
    Warning => "danger-triangle",
    Fork => "fork",
    Pen => "pen",
    WindowMinimize => "window-minimize",
    WindowMaximize => "window-maximize",
    WindowRestore => "window-restore",
    Tuning => "tuning",
}

pub struct Assets;

impl AssetSource for Assets {
    fn load(&self, path: &str) -> Result<Option<Cow<'static, [u8]>>> {
        Ok(load_icon(path).map(Cow::Borrowed))
    }

    /// GPUI-CE builds its `AssetRegistry` by enumerating `list("")` once at
    /// startup — anything not listed here is unloadable, so list every icon.
    fn list(&self, path: &str) -> Result<Vec<SharedString>> {
        Ok(Icon::ALL
            .iter()
            .map(|i| i.path())
            .filter(|p| p.starts_with(path))
            .map(SharedString::from)
            .collect())
    }
}

pub fn load_fonts(cx: &App) {
    let fonts: Vec<Cow<'static, [u8]>> = vec![
        Cow::Borrowed(include_bytes!("../assets/fonts/Geist.ttf")),
        Cow::Borrowed(include_bytes!("../assets/fonts/Geist-Medium.ttf")),
        Cow::Borrowed(include_bytes!("../assets/fonts/Geist-SemiBold.ttf")),
        Cow::Borrowed(include_bytes!("../assets/fonts/Geist-Bold.ttf")),
        Cow::Borrowed(include_bytes!("../assets/fonts/GeistMono.ttf")),
        Cow::Borrowed(include_bytes!("../assets/fonts/GeistMono-Medium.ttf")),
    ];
    if let Err(e) = cx.text_system().add_fonts(fonts) {
        log::error!("failed to load embedded fonts: {e:#}");
    }
}
