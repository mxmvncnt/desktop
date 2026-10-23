//! Hunk expansion of the displayed diff.

use std::sync::Arc;

use gpui::Context;

use super::state::RepoModel;
use crate::features::diff::ExpandTarget;

impl RepoModel {
    /// Reveal more unchanged lines around a hunk of the displayed working or
    /// commit diff. Pure and cheap (the diff holds both texts), so it runs
    /// inline; highlighting is redone in the background.
    pub fn expand_diff(&mut self, target: ExpandTarget, e: gd_git::Expansion, cx: &mut Context<Self>) {
        let (current, path) = match target {
            ExpandTarget::Commit => {
                let path = self.selected_commit_file.and_then(|i| self.commit_files.get(i)).map(|f| f.path.clone());
                (self.commit_diff.clone(), path)
            }
            ExpandTarget::Changes => (self.change_diff.clone(), self.selected_path()),
        };
        let (Some(current), Some(path)) = (current, path) else { return };
        let Some(expanded) = gd_git::expand(&current, e) else { return };
        let d = Arc::new(expanded);
        match target {
            ExpandTarget::Commit => {
                self.commit_diff = Some(d.clone());
                self.commit_hl = None;
                self._commit_hl_task = Some(self.spawn_highlight(d, path, target, cx));
            }
            ExpandTarget::Changes => {
                self.change_diff = Some(d.clone());
                self.change_hl = None;
                self._change_hl_task = Some(self.spawn_highlight(d, path, target, cx));
            }
        }
        cx.notify();
    }
}
