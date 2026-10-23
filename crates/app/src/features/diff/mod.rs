//! Diff viewer: unified and split lists, line-selection gutter, hunk
//! expansion, syntax highlight, options popover. Git side is in
//! `gd-git/src/{engine,partial,expand}.rs`; see docs/ARCHITECTURE.md.

mod check_option_row;
mod diff_frame;
mod diff_options_button;
mod diff_options_menu;
mod diff_pane;
mod expand_button;
mod expander;
mod gutter_style;
mod handlers;
mod highlight;
mod line_selection;
mod number_cell;
mod render_diff;
mod row_select;
mod rows_list;
mod scroll_x;
mod split_diff;
mod split_list;
mod split_row;
mod state;
mod text_flow;
mod text_handlers;
mod text_select;
mod text_selection;
mod unified_list;
mod unified_row;
mod whitespace_note;

pub use check_option_row::CheckOptionRow;
pub use expander::ExpandTarget;
pub use diff_options_button::DiffOptionsButton;
pub use diff_options_menu::DiffOptionsMenu;
pub use diff_pane::DiffPane;
pub use highlight::{DiffHighlight, HighlightCache, highlight_diff};
pub use line_selection::LineSelection;
pub use state::{DiffSelectUi, DiffView};
#[cfg(feature = "snapshot")]
pub use text_selection::{TextPos, TextSel, TextSide};
