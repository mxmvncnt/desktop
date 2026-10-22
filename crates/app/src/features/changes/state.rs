//! Values derived from the repo model for the sidebar, so the components
//! take typed props instead of a dozen locals.

use gd_ui::CheckState;
use gpui::SharedString;

use crate::repo::RepoModel;
use crate::workspace::plural;

/// State of the "select all" checkbox and the count above the list.
#[derive(Clone, Copy)]
pub struct SelectionSummary {
    pub total: usize,
    /// Files not unchecked in the list.
    pub included: usize,
    pub all: CheckState,
}

impl SelectionSummary {
    pub fn of(r: &RepoModel) -> Self {
        let total = r.changes().len();
        let included = total - r.changes().iter().filter(|c| r.excluded.contains(&c.path)).count();
        let all = match included {
            0 => CheckState::Unchecked,
            i if i == total && r.partial.is_empty() => CheckState::Checked,
            _ => CheckState::Mixed,
        };
        Self { total, included, all }
    }
}

/// What the commit button shows and whether it is enabled.
pub struct CommitButton {
    pub label: SharedString,
    pub enabled: bool,
    pub amending: bool,
    /// The commit being amended was already pushed (force push needed).
    pub head_pushed: bool,
}

impl CommitButton {
    pub fn of(r: &RepoModel, included: usize, amending: bool, summary_ok: bool) -> Self {
        let committing = r.committing;
        let conflicts = r.conflict_count();
        let merging = r.merge.in_progress.as_ref().is_some_and(|p| p.commit_finishes());
        // Amending may change only the message, so no files are needed.
        let enabled = (included > 0 || amending) && summary_ok && !committing && conflicts == 0 && r.merge.busy.is_none();
        let label: SharedString = if amending {
            if committing { "Amending…".into() } else { "Amend last commit".into() }
        } else if committing {
            "Committing…".into()
        } else if conflicts > 0 {
            format!("Resolve {conflicts} conflict{} to commit", plural(conflicts as u32)).into()
        } else if merging {
            "Commit merge".into()
        } else {
            format!("Commit to {}", r.branch_name()).into()
        };
        let head_pushed = r.history.push_state_loaded && r.commits.first().is_some_and(|c| !r.is_unpushed(&c.sha));
        Self { label, enabled, amending, head_pushed }
    }
}
