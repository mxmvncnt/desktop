//! Data-driven color themes. Each [`ColorTheme`] is a family with a dark and a
//! light [`Palette`] (a handful of hex values); [`Theme::from_palette`] expands a
//! palette into the full token set. To add a theme, append one entry to
//! [`COLOR_THEMES`]; the settings page and persistence pick it up by `id`.
//!
//! Mode rule: every family has both variants, so System/Light/Dark simply picks
//! the variant. The built-in `zeron` family (the default) uses the hand-tuned
//! `Theme::dark()` / `Theme::light()` instead of a palette.

use gpui::{ColorExt as _, Hsla, Rgba};

use crate::theme::{SyntaxPalette, Theme, set_light};

/// Source colors of one theme variant, as `0xRRGGBB`.
#[derive(Debug, Clone, Copy)]
pub struct Palette {
    /// Diff plane (the deepest surface in dark variants).
    pub bg: u32,
    /// Toolbar / sidebar.
    pub surface: u32,
    pub card: u32,
    /// Popovers and menus.
    pub overlay: u32,
    /// Opaque chips and pills.
    pub raised: u32,
    pub text: u32,
    pub muted: u32,
    pub faint: u32,
    pub accent: u32,
    pub accent_strong: u32,
    pub danger: u32,
    pub warning: u32,
    pub success: u32,
    pub comment: u32,
    pub keyword: u32,
    pub string: u32,
    pub number: u32,
    pub type_name: u32,
    pub function: u32,
    pub macro_name: u32,
    pub property: u32,
    pub special: u32,
}

pub struct ColorTheme {
    pub id: &'static str,
    pub label: &'static str,
    /// `(dark, light)`; `None` for the built-in zeron theme.
    pub palettes: Option<(Palette, Palette)>,
}

impl ColorTheme {
    pub fn palette(&self, light: bool) -> Option<&Palette> {
        self.palettes.as_ref().map(|(d, l)| if light { l } else { d })
    }
}

pub const DEFAULT_THEME: &str = "zeron";

pub fn find(id: &str) -> Option<&'static ColorTheme> {
    COLOR_THEMES.iter().find(|t| t.id == id)
}

pub static COLOR_THEMES: &[ColorTheme] = &[
    ColorTheme { id: DEFAULT_THEME, label: "Zeron", palettes: None },
    ColorTheme { id: "nord", label: "Nord", palettes: Some((NORD_DARK, NORD_LIGHT)) },
    ColorTheme { id: "tokyo-night", label: "Tokyo Night", palettes: Some((TOKYO_NIGHT, TOKYO_DAY)) },
    ColorTheme { id: "vscode", label: "VS Code", palettes: Some((VSCODE_DARK, VSCODE_LIGHT)) },
    ColorTheme { id: "catppuccin", label: "Catppuccin", palettes: Some((MOCHA, LATTE)) },
    ColorTheme { id: "solarized", label: "Solarized", palettes: Some((SOLARIZED_DARK, SOLARIZED_LIGHT)) },
];

impl Theme {
    /// Theme for a family id (unknown ids fall back to zeron) and resolved appearance.
    /// Also flips `ink`/`hairline`.
    pub fn for_family(id: &str, light: bool) -> Self {
        set_light(light);
        Self::preview(id, light)
    }

    /// Like `for_family` but without touching the global `ink` flag (for swatches).
    pub fn preview(id: &str, light: bool) -> Self {
        match find(id).and_then(|t| t.palette(light)) {
            Some(p) => Self::from_palette(p, light),
            None if light => Self::light(),
            None => Self::dark(),
        }
    }

    fn from_palette(p: &Palette, light: bool) -> Self {
        let mut t = if light { Self::light() } else { Self::dark() };
        let c = hex;
        let (accent, danger, success) = (c(p.accent), c(p.danger), c(p.success));
        t.bg = c(p.bg);
        t.surface = c(p.surface);
        t.surface_card = c(p.card);
        t.surface_overlay = c(p.overlay);
        t.surface_raised = c(p.raised);
        t.surface_raised_hover = mix(p.raised, p.text, 0.10);
        t.text = c(p.text);
        t.text_muted = c(p.muted);
        t.text_faint = c(p.faint);
        t.text_dim = c(p.muted);
        t.solid = c(p.text);
        t.on_solid = c(p.bg);
        t.accent = accent;
        t.accent_strong = c(p.accent_strong);
        t.accent_wash = with_alpha(accent, 0.14);
        t.on_accent = if light { hex(0xffffff) } else { c(p.bg) };
        t.danger = danger;
        t.danger_strong = c(p.danger);
        t.warning = c(p.warning);
        t.success = success;
        t.selection = with_alpha(accent, 0.30);
        t.caret = accent;
        t.diff_add = success;
        t.diff_del = danger;
        let (wash, emph) = if light { (0.14, 0.28) } else { (0.10, 0.30) };
        t.diff_add_bg = with_alpha(success, wash);
        t.diff_del_bg = with_alpha(danger, wash);
        t.diff_add_emph = with_alpha(success, emph);
        t.diff_del_emph = with_alpha(danger, emph);
        t.diff_hunk_bg = with_alpha(accent, 0.08);
        t.diff_gutter = c(p.faint);
        t.syntax = SyntaxPalette::from_palette(p);
        t
    }
}

impl SyntaxPalette {
    fn from_palette(p: &Palette) -> Self {
        let c = hex;
        Self {
            comment: c(p.comment),
            keyword: c(p.keyword),
            string: c(p.string),
            string_special: c(p.special),
            escape: c(p.special),
            number: c(p.number),
            boolean: c(p.number),
            type_name: c(p.type_name),
            type_builtin: c(p.type_name),
            constructor: c(p.type_name),
            function: c(p.function),
            function_builtin: c(p.function),
            macro_name: c(p.macro_name),
            property: c(p.property),
            constant: c(p.number),
            variable: c(p.text),
            variable_special: c(p.special),
            parameter: c(p.text),
            operator: c(p.text),
            punctuation: c(p.text),
            tag: c(p.keyword),
            attribute: c(p.property),
            label: c(p.special),
            markup_heading: c(p.function),
            markup_raw: c(p.string),
            markup_link: c(p.accent),
            markup_reference: c(p.special),
            markup_emphasis: c(p.macro_name),
            markup_strong: c(p.keyword),
            invalid: c(p.danger),
        }
    }
}

pub fn hex(v: u32) -> Hsla {
    gpui::rgb_to_hsla(Rgba::new(((v >> 16) & 0xff) as f32 / 255.0, ((v >> 8) & 0xff) as f32 / 255.0, (v & 0xff) as f32 / 255.0, 1.0))
}

fn with_alpha(c: Hsla, a: f32) -> Hsla {
    c.opacity(a)
}

/// `a` blended toward `b` by `t` (sRGB space, fine for small steps).
fn mix(a: u32, b: u32, t: f32) -> Hsla {
    let ch = |v: u32, shift: u32| ((v >> shift) & 0xff) as f32 / 255.0;
    let l = |shift: u32| ch(a, shift) + (ch(b, shift) - ch(a, shift)) * t;
    gpui::rgb_to_hsla(Rgba::new(l(16), l(8), l(0), 1.0))
}

const NORD_DARK: Palette = Palette {
    bg: 0x2e3440, surface: 0x272c36, card: 0x2b303b, overlay: 0x3b4252, raised: 0x434c5e,
    text: 0xd8dee9, muted: 0xaab2c0, faint: 0x7b88a1,
    accent: 0x88c0d0, accent_strong: 0x5e81ac, danger: 0xd0707a, warning: 0xebcb8b, success: 0xa3be8c,
    comment: 0x616e88, keyword: 0x81a1c1, string: 0xa3be8c, number: 0xb48ead, type_name: 0x8fbcbb,
    function: 0x88c0d0, macro_name: 0xd08770, property: 0xd8dee9, special: 0xebcb8b,
};

const NORD_LIGHT: Palette = Palette {
    bg: 0xeceff4, surface: 0xe5e9f0, card: 0xe9edf3, overlay: 0xf6f8fb, raised: 0xd8dee9,
    text: 0x2e3440, muted: 0x4c566a, faint: 0x6b7690,
    accent: 0x5e81ac, accent_strong: 0x4c6f9a, danger: 0xbf616a, warning: 0xb5832a, success: 0x5f8a4a,
    comment: 0x8a93a6, keyword: 0x5e81ac, string: 0x5f8a4a, number: 0x9b6a95, type_name: 0x3f8b8b,
    function: 0x4c7a99, macro_name: 0xc0643f, property: 0x2e3440, special: 0xb5832a,
};

const TOKYO_NIGHT: Palette = Palette {
    bg: 0x1a1b26, surface: 0x16161e, card: 0x1c1d29, overlay: 0x24283b, raised: 0x292e42,
    text: 0xc0caf5, muted: 0x9aa5ce, faint: 0x565f89,
    accent: 0x7aa2f7, accent_strong: 0x3d59a1, danger: 0xf7768e, warning: 0xe0af68, success: 0x9ece6a,
    comment: 0x565f89, keyword: 0xbb9af7, string: 0x9ece6a, number: 0xff9e64, type_name: 0x2ac3de,
    function: 0x7aa2f7, macro_name: 0x7dcfff, property: 0x73daca, special: 0xe0af68,
};

const TOKYO_DAY: Palette = Palette {
    bg: 0xe1e2e7, surface: 0xd5d6db, card: 0xdcdde3, overlay: 0xf0f0f5, raised: 0xc8cad3,
    text: 0x343b58, muted: 0x4c505e, faint: 0x6172b0,
    accent: 0x2e7de9, accent_strong: 0x3760bf, danger: 0xf52a65, warning: 0x8c6c3e, success: 0x587539,
    comment: 0x848cb5, keyword: 0x9854f1, string: 0x587539, number: 0xb15c00, type_name: 0x007197,
    function: 0x2e7de9, macro_name: 0x118c74, property: 0x387068, special: 0x8c6c3e,
};

const VSCODE_DARK: Palette = Palette {
    bg: 0x1e1e1e, surface: 0x181818, card: 0x202020, overlay: 0x252526, raised: 0x37373d,
    text: 0xd4d4d4, muted: 0x9d9d9d, faint: 0x6e7681,
    accent: 0x3794ff, accent_strong: 0x0e639c, danger: 0xf48771, warning: 0xcca700, success: 0x89d185,
    comment: 0x6a9955, keyword: 0x569cd6, string: 0xce9178, number: 0xb5cea8, type_name: 0x4ec9b0,
    function: 0xdcdcaa, macro_name: 0xc586c0, property: 0x9cdcfe, special: 0xd7ba7d,
};

const VSCODE_LIGHT: Palette = Palette {
    bg: 0xffffff, surface: 0xf3f3f3, card: 0xf8f8f8, overlay: 0xffffff, raised: 0xe8e8e8,
    text: 0x3b3b3b, muted: 0x616161, faint: 0x8a8a8a,
    accent: 0x0066bf, accent_strong: 0x005fb8, danger: 0xcd3131, warning: 0xbf8803, success: 0x388a34,
    comment: 0x008000, keyword: 0x0000ff, string: 0xa31515, number: 0x098658, type_name: 0x267f99,
    function: 0x795e26, macro_name: 0xaf00db, property: 0x001080, special: 0xe50000,
};

const MOCHA: Palette = Palette {
    bg: 0x1e1e2e, surface: 0x181825, card: 0x1a1a29, overlay: 0x313244, raised: 0x313244,
    text: 0xcdd6f4, muted: 0xa6adc8, faint: 0x7f849c,
    accent: 0x89b4fa, accent_strong: 0x5b8bdc, danger: 0xf38ba8, warning: 0xf9e2af, success: 0xa6e3a1,
    comment: 0x6c7086, keyword: 0xcba6f7, string: 0xa6e3a1, number: 0xfab387, type_name: 0xf9e2af,
    function: 0x89b4fa, macro_name: 0xf5c2e7, property: 0x89dceb, special: 0xf2cdcd,
};

const LATTE: Palette = Palette {
    bg: 0xeff1f5, surface: 0xe6e9ef, card: 0xebedf2, overlay: 0xfcfcfd, raised: 0xccd0da,
    text: 0x4c4f69, muted: 0x5c5f77, faint: 0x8c8fa1,
    accent: 0x1e66f5, accent_strong: 0x1a56d6, danger: 0xd20f39, warning: 0xdf8e1d, success: 0x40a02b,
    comment: 0x9ca0b0, keyword: 0x8839ef, string: 0x40a02b, number: 0xfe640b, type_name: 0xdf8e1d,
    function: 0x1e66f5, macro_name: 0xea76cb, property: 0x209fb5, special: 0xdd7878,
};

const SOLARIZED_DARK: Palette = Palette {
    bg: 0x002b36, surface: 0x00212b, card: 0x00252f, overlay: 0x073642, raised: 0x0b3f4c,
    text: 0x93a1a1, muted: 0x839496, faint: 0x657b83,
    accent: 0x268bd2, accent_strong: 0x1e6fa8, danger: 0xdc322f, warning: 0xb58900, success: 0x859900,
    comment: 0x586e75, keyword: 0x859900, string: 0x2aa198, number: 0xd33682, type_name: 0xb58900,
    function: 0x268bd2, macro_name: 0xcb4b16, property: 0x6c71c4, special: 0xd33682,
};

const SOLARIZED_LIGHT: Palette = Palette {
    bg: 0xfdf6e3, surface: 0xeee8d5, card: 0xf5eed9, overlay: 0xfffaf0, raised: 0xe4ddc8,
    text: 0x586e75, muted: 0x657b83, faint: 0x93a1a1,
    accent: 0x268bd2, accent_strong: 0x1e6fa8, danger: 0xdc322f, warning: 0xb58900, success: 0x859900,
    comment: 0x93a1a1, keyword: 0x859900, string: 0x2aa198, number: 0xd33682, type_name: 0xb58900,
    function: 0x268bd2, macro_name: 0xcb4b16, property: 0x6c71c4, special: 0xd33682,
};

