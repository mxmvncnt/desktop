//! Design tokens, ported from zeron's `crates/ui/src/theme.rs` (dark variant).
//!
//! Zeron's dark palette is a near-black elevation ladder with tiny lightness
//! steps (#060606 → #0d0d0d → #0e → #10 → #16) plus translucent white "ink"
//! for hover/active washes and hairlines. Keep new colors on this ladder —
//! don't invent opaque greys, use `ink()`/`hairline()` instead.

use gd_syntax::HighlightKind;
use gpui::{App, ColorExt as _, Global, Hsla, SharedString, hsla};

#[derive(Debug, Clone)]
pub struct Theme {
    // ---- neutral surfaces ----
    /// Main content plane (diff view). Deepest.
    pub bg: Hsla,
    /// Shell / sidebar / toolbar. One step up from `bg`.
    pub surface: Hsla,
    /// Opaque raised pills/chips.
    pub surface_raised: Hsla,
    pub surface_raised_hover: Hsla,
    /// Inline cards on the main panel.
    pub surface_card: Hsla,
    /// Popovers, menus — the highest plane.
    pub surface_overlay: Hsla,
    pub element_hover: Hsla,
    pub element_active: Hsla,
    pub border: Hsla,
    pub border_strong: Hsla,

    // ---- text ----
    pub text: Hsla,
    pub text_muted: Hsla,
    pub text_faint: Hsla,
    pub text_dim: Hsla,

    // ---- high-contrast plate (primary button) ----
    pub solid: Hsla,
    pub on_solid: Hsla,

    // ---- accents ----
    pub accent: Hsla,
    pub accent_strong: Hsla,
    pub accent_wash: Hsla,
    pub on_accent: Hsla,
    pub danger: Hsla,
    pub danger_strong: Hsla,
    pub warning: Hsla,
    pub success: Hsla,

    // ---- inputs ----
    pub input_bg: Hsla,
    pub selection: Hsla,
    pub caret: Hsla,

    // ---- diff ----
    pub diff_add: Hsla,
    pub diff_del: Hsla,
    pub diff_add_bg: Hsla,
    pub diff_del_bg: Hsla,
    /// Word-level emphasis inside changed lines (stronger wash).
    pub diff_add_emph: Hsla,
    pub diff_del_emph: Hsla,
    pub diff_hunk_bg: Hsla,
    pub diff_gutter: Hsla,

    // ---- syntax ----
    pub syntax: SyntaxPalette,

    // ---- fonts ----
    pub font_sans: SharedString,
    pub font_mono: SharedString,
}

impl Global for Theme {}

// Sizes (px), from zeron's Theme consts.
pub const TOOLBAR_HEIGHT: f32 = 52.0;
pub const PANEL_RADIUS: f32 = 10.0;
pub const CONTROL_RADIUS: f32 = 6.0;
pub const BUBBLE_RADIUS: f32 = 16.0;
pub const SIDEBAR_WIDTH: f32 = 300.0;
pub const ROW_HEIGHT: f32 = 30.0;
pub const DIFF_ROW_HEIGHT: f32 = 20.0;
pub const TEXT_SM: f32 = 12.0;
pub const TEXT_BASE: f32 = 13.0;

impl Theme {
    pub fn dark() -> Self {
        let blue = oklch(0.707, 0.165, 254.624); // blue-400
        Self {
            bg: grey(6),
            surface: grey(13),
            surface_raised: neutral(0.235),
            surface_raised_hover: neutral(0.29),
            surface_card: grey(0x0e),
            surface_overlay: grey(0x16),
            element_hover: hsla(0.0, 0.0, 0.92, 0.07),
            element_active: hsla(0.0, 0.0, 0.92, 0.11),
            border: hsla(0.0, 0.0, 1.0, 0.08),
            border_strong: hsla(0.0, 0.0, 1.0, 0.14),
            text: neutral(0.922),
            text_muted: neutral(0.708),
            text_faint: neutral(0.556),
            text_dim: grey(0x98),
            solid: neutral(0.922),
            on_solid: grey(0x0e),
            accent: blue,
            accent_strong: oklch(0.546, 0.245, 262.881), // blue-600
            accent_wash: blue.opacity(0.14),
            on_accent: neutral(0.985),
            danger: oklch(0.704, 0.191, 22.216),
            danger_strong: oklch(0.58, 0.16, 25.0),
            warning: oklch(0.828, 0.189, 84.429),
            success: oklch(0.765, 0.177, 163.223),
            input_bg: hsla(0.0, 0.0, 1.0, 0.03),
            selection: blue.opacity(0.30),
            caret: blue,
            diff_add: oklch(0.765, 0.177, 163.223),
            diff_del: oklch(0.704, 0.191, 22.216),
            diff_add_bg: oklch(0.765, 0.177, 163.223).opacity(0.10),
            diff_del_bg: oklch(0.704, 0.191, 22.216).opacity(0.10),
            diff_add_emph: oklch(0.765, 0.177, 163.223).opacity(0.30),
            diff_del_emph: oklch(0.704, 0.191, 22.216).opacity(0.30),
            diff_hunk_bg: hsla(0.6, 0.35, 0.6, 0.06),
            diff_gutter: neutral(0.45),
            syntax: SyntaxPalette::dark(neutral(0.922), neutral(0.60), oklch(0.704, 0.191, 22.216)),
            font_sans: "Geist".into(),
            font_mono: "Geist Mono".into(),
        }
    }

    pub fn of(cx: &App) -> &Theme {
        cx.global::<Theme>()
    }
}

/// Paint-only syntax colors, ported from zeron's `SyntaxPalette` (dark).
/// The hues follow zeron's Git-graph lane palette (indigo, pink, emerald, amber).
#[derive(Debug, Clone, PartialEq)]
pub struct SyntaxPalette {
    pub comment: Hsla,
    pub keyword: Hsla,
    pub string: Hsla,
    pub string_special: Hsla,
    pub escape: Hsla,
    pub number: Hsla,
    pub boolean: Hsla,
    pub type_name: Hsla,
    pub type_builtin: Hsla,
    pub constructor: Hsla,
    pub function: Hsla,
    pub function_builtin: Hsla,
    pub macro_name: Hsla,
    pub property: Hsla,
    pub constant: Hsla,
    pub variable: Hsla,
    pub variable_special: Hsla,
    pub parameter: Hsla,
    pub operator: Hsla,
    pub punctuation: Hsla,
    pub tag: Hsla,
    pub attribute: Hsla,
    pub label: Hsla,
    pub markup_heading: Hsla,
    pub markup_raw: Hsla,
    pub markup_link: Hsla,
    pub markup_reference: Hsla,
    pub markup_emphasis: Hsla,
    pub markup_strong: Hsla,
    pub invalid: Hsla,
}

impl SyntaxPalette {
    /// Color for a highlight kind. Called once per span while building runs (never per frame).
    pub fn color(&self, kind: HighlightKind) -> Hsla {
        match kind {
            HighlightKind::Comment => self.comment,
            HighlightKind::Keyword => self.keyword,
            HighlightKind::String => self.string,
            HighlightKind::StringSpecial => self.string_special,
            HighlightKind::Escape => self.escape,
            HighlightKind::Number => self.number,
            HighlightKind::Boolean => self.boolean,
            HighlightKind::Type => self.type_name,
            HighlightKind::TypeBuiltin => self.type_builtin,
            HighlightKind::Constructor => self.constructor,
            HighlightKind::Function => self.function,
            HighlightKind::FunctionBuiltin => self.function_builtin,
            HighlightKind::Macro => self.macro_name,
            HighlightKind::Property => self.property,
            HighlightKind::Constant => self.constant,
            HighlightKind::Variable => self.variable,
            HighlightKind::VariableSpecial => self.variable_special,
            HighlightKind::Parameter => self.parameter,
            HighlightKind::Operator => self.operator,
            HighlightKind::Punctuation | HighlightKind::Embedded => self.punctuation,
            HighlightKind::Tag => self.tag,
            HighlightKind::Attribute => self.attribute,
            HighlightKind::Label => self.label,
            HighlightKind::MarkupHeading => self.markup_heading,
            HighlightKind::MarkupRaw => self.markup_raw,
            HighlightKind::MarkupLink => self.markup_link,
            HighlightKind::MarkupReference => self.markup_reference,
            HighlightKind::MarkupEmphasis => self.markup_emphasis,
            HighlightKind::MarkupStrong => self.markup_strong,
            HighlightKind::Invalid => self.invalid,
        }
    }

    pub fn dark(text: Hsla, comment: Hsla, danger: Hsla) -> Self {
        // Same sources and 72% saturation treatment as history::graph_color.
        let indigo = git_graph_tone(oklch(0.673, 0.182, 276.935));
        let pink = git_graph_tone(oklch(0.718, 0.202, 349.761));
        let emerald = git_graph_tone(oklch(0.765, 0.177, 163.223));
        let amber = git_graph_tone(oklch(0.828, 0.189, 84.429));
        let red = git_graph_tone(danger);
        Self {
            comment,
            keyword: indigo,
            string: emerald,
            string_special: pink,
            escape: pink,
            number: amber,
            boolean: amber,
            type_name: amber,
            type_builtin: emerald,
            constructor: amber,
            function: indigo,
            function_builtin: pink,
            macro_name: pink,
            property: amber,
            constant: emerald,
            variable: text,
            variable_special: pink,
            parameter: text,
            operator: text,
            punctuation: text,
            tag: pink,
            attribute: amber,
            label: amber,
            markup_heading: indigo,
            markup_raw: emerald,
            markup_link: pink,
            markup_reference: amber,
            markup_emphasis: pink,
            markup_strong: indigo,
            invalid: red,
        }
    }

}

fn git_graph_tone(mut color: Hsla) -> Hsla {
    color.color.saturation *= 0.72;
    color
}

/// Translucent white fill (hover/active plates). Alpha in dark-mode terms.
pub fn ink(alpha: f32) -> Hsla {
    hsla(0.0, 0.0, 1.0, alpha)
}

/// Translucent white hairline for borders/dividers.
pub fn hairline(alpha: f32) -> Hsla {
    hsla(0.0, 0.0, 1.0, alpha)
}

/// Modal backdrop.
pub fn scrim(alpha: f32) -> Hsla {
    hsla(0.0, 0.0, 0.0, alpha)
}

pub fn grey(v: u8) -> Hsla {
    hsla(0.0, 0.0, v as f32 / 255.0, 1.0)
}

/// Neutral grey at oklch lightness `l` (matches Tailwind's neutral scale).
pub fn neutral(l: f32) -> Hsla {
    let [v, _, _] = oklch_to_srgb(l, 0.0, 0.0);
    hsla(0.0, 0.0, v, 1.0)
}

/// CSS `oklch(l c h)` → Hsla. Lets us copy Tailwind/zeron values verbatim.
pub fn oklch(l: f32, c: f32, h_deg: f32) -> Hsla {
    let [r, g, b] = oklch_to_srgb(l, c, h_deg);
    gpui::rgb_to_hsla(gpui::Rgba::new(r, g, b, 1.0))
}

fn oklch_to_srgb(l: f32, c: f32, h_deg: f32) -> [f32; 3] {
    let h = h_deg.to_radians();
    let (a, b) = (c * h.cos(), c * h.sin());
    let l_ = l + 0.396_337_78 * a + 0.215_803_76 * b;
    let m_ = l - 0.105_561_346 * a - 0.063_854_17 * b;
    let s_ = l - 0.089_484_18 * a - 1.291_485_5 * b;
    let (l3, m3, s3) = (l_.powi(3), m_.powi(3), s_.powi(3));
    let r = 4.076_741_7 * l3 - 3.307_711_6 * m3 + 0.230_969_94 * s3;
    let g = -1.268_438 * l3 + 2.609_757_4 * m3 - 0.341_319_38 * s3;
    let bl = -0.004_196_086_3 * l3 - 0.703_418_6 * m3 + 1.707_614_7 * s3;
    let gamma = |x: f32| {
        let x = x.clamp(0.0, 1.0);
        if x <= 0.003_130_8 {
            12.92 * x
        } else {
            1.055 * x.powf(1.0 / 2.4) - 0.055
        }
    };
    [gamma(r), gamma(g), gamma(bl)]
}
