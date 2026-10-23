//! Filtering of the repository and branch lists by the popover's filter input.

use std::collections::HashSet;

use gd_git::BranchKind;

use crate::workspace::Workspace;

impl Workspace {
    fn filter_text(&self, cx: &gpui::App) -> String {
        self.filter.read(cx).as_str().trim().to_lowercase()
    }

    /// Indices into `settings.repositories` matching the filter.
    pub(super) fn filtered_repos(&self, cx: &gpui::App) -> Vec<usize> {
        let f = self.filter_text(cx);
        self.settings
            .repositories
            .iter()
            .enumerate()
            .filter(|(_, p)| f.is_empty() || p.display().to_string().to_lowercase().contains(&f))
            .map(|(i, _)| i)
            .collect()
    }

    /// Indices into `repo.branches` matching the filter.
    pub(super) fn filtered_branches(&self, cx: &gpui::App) -> Vec<usize> {
        let f = self.filter_text(cx);
        let Some(repo) = &self.repo else { return Vec::new() };
        let r = repo.read(cx);
        let local: HashSet<&str> =
            r.branches.iter().filter(|b| b.kind == BranchKind::Local).map(|b| b.name.as_str()).collect();
        r.branches
            .iter()
            .enumerate()
            .filter(|(_, b)| f.is_empty() || b.name.to_lowercase().contains(&f))
            // Hide remote branches that already have a local counterpart.
            .filter(|(_, b)| {
                b.kind == BranchKind::Local || !b.name.split_once('/').is_some_and(|(_, n)| local.contains(n))
            })
            .map(|(i, _)| i)
            .collect()
    }
}
