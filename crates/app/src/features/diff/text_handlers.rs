//! `impl Workspace` handlers of the diff text selection: press, drag, select
//! all and copy.

use gpui::{ClipboardItem, Context, Window};

use super::text_selection::{TextPos, TextSel, TextSide, word_at};
use crate::workspace::Workspace;

impl Workspace {
    /// Mouse down on diff text. Click starts a drag selection, shift extends,
    /// double-click selects a word, triple-click the line.
    pub fn diff_text_down(
        &mut self,
        pos: TextPos,
        side: TextSide,
        clicks: usize,
        shift: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        // Take focus away from text inputs so Ctrl/Cmd+C and +A reach the diff.
        self.focus(window, cx);
        let Some(text) = self.diff_view.line_text(pos.line) else { return };
        let len = text.len();
        let word = (clicks == 2).then(|| word_at(text, pos.col));
        let prev = self.diff_view.text_sel.filter(|s| s.side == side);
        let sel = match (clicks, prev) {
            (1, Some(prev)) if shift => TextSel { head: pos, ..prev },
            (2, _) => {
                let w = word.unwrap_or(pos.col..pos.col);
                TextSel { anchor: TextPos { col: w.start, ..pos }, head: TextPos { col: w.end, ..pos }, side }
            }
            (n, _) if n >= 3 => {
                TextSel { anchor: TextPos { col: 0, ..pos }, head: TextPos { col: len, ..pos }, side }
            }
            _ => TextSel { anchor: pos, head: pos, side },
        };
        self.diff_view.text_sel = Some(sel);
        self.diff_view.text_drag = clicks == 1;
        cx.notify();
    }

    /// The pointer moved to `pos` while the button is down.
    pub fn diff_text_drag(&mut self, pos: TextPos, cx: &mut Context<Self>) {
        let Some(sel) = self.diff_view.text_sel.as_mut().filter(|s| s.head != pos) else { return };
        sel.head = pos;
        cx.notify();
    }

    pub fn diff_text_drag_end(&mut self) {
        self.diff_view.text_drag = false;
    }

    /// Select all text of the shown diff (of the side last used in split view).
    pub fn select_all_diff_text(&mut self, cx: &mut Context<Self>) {
        let Some(diff) = self.diff_view.shown_diff() else { return };
        let side = match (&self.diff_view.split_cache, self.settings.diff_split) {
            (Some(_), true) => self.diff_view.text_sel.map_or(TextSide::Right, |s| s.side),
            _ => TextSide::Unified,
        };
        self.diff_view.text_sel = TextSel::all(&diff, side);
        cx.notify();
    }

    /// Copy the selected diff text (code only, no gutters) to the clipboard.
    pub fn copy_diff_text(&mut self, cx: &mut Context<Self>) {
        let (Some(sel), Some(diff)) = (self.diff_view.text_sel, self.diff_view.shown_diff()) else { return };
        let split = self.diff_view.split_cache.as_deref().filter(|_| sel.side != TextSide::Unified);
        let text = sel.text(&diff, split);
        if !text.is_empty() {
            cx.write_to_clipboard(ClipboardItem::new_string(text));
        }
    }
}
