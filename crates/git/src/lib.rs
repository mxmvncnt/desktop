//! `gd-git` — the git layer for desktop-rs.
//!
//! Design (see `docs/ARCHITECTURE.md` §Git layer):
//! - Shells out to the user's `git` binary, the same strategy as the Electron
//!   app's `dugite`. Hooks, credential helpers, config and LFS behave exactly
//!   like the CLI, and we avoid libgit2's partial feature coverage.
//! - Every function is **blocking**. The UI runs them on GPUI's background
//!   executor (`cx.background_spawn`) and never on the main thread.
//! - Machine-readable formats only (`status --porcelain=v2 -z`, NUL-separated
//!   `log` formats) so parsing is cheap and locale-independent.

mod credentials;
mod diff;
pub mod engine;
mod exec;
mod expand;
mod history_ops;
mod identity;
mod log;
mod merge;
mod partial;
mod pr_preview;
mod progress;
mod refs;
mod stash;
mod status;

pub use credentials::{HostCredential, set_credentials};
pub use identity::{EffectiveIdentity, Identity, IdentitySource, global_identity, set_global_identity};

#[doc(hidden)]
pub fn credential_env_for_tests() -> Vec<(std::ffi::OsString, std::ffi::OsString)> {
    credentials::env()
}

pub use diff::{Side, Diff, DiffHunk, DiffLine, DiffLineKind, DiffOptions};
pub use exec::GitError;
pub use expand::{EXPANSION_STEP, Expansion, expand, expandable, gap_above, gap_below};
pub use history_ops::{ResetMode, github_web_url};
pub use log::{CoAuthor, Commit, CommitFile, parse_co_authors};
pub use merge::{MergePreview, default_squash_summary};
pub use partial::{PartialFile, selectable, staged_text};
pub use pr_preview::{Comparison, Host, compare_url, host_of};
pub use progress::{InProgress, OpOutcome};
pub use refs::{Branch, BranchKind};
pub use stash::Stash;
pub use status::{AheadBehind, FileChange, FileStatus, WorkingStatus};

use std::path::{Path, PathBuf};

use exec::Git;

pub type Result<T> = std::result::Result<T, GitError>;

/// A handle to a repository on disk. Cheap to clone; holds no open resources.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Repository {
    root: PathBuf,
}

impl Repository {
    /// Open the repository containing `path` (any subdirectory works).
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        let out = Git::new(path.as_ref())
            .args(["rev-parse", "--show-toplevel"])
            .run()?;
        let root = PathBuf::from(out.trim_end_matches(['\n', '\r']));
        Ok(Self { root })
    }

    /// Initialize a new repository at `path` (creating the directory).
    pub fn init(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref();
        std::fs::create_dir_all(path).map_err(|e| GitError::Io(e.to_string()))?;
        Git::new(path).args(["init", "-q"]).run()?;
        Self::open(path)
    }

    /// Clone `url` into `dest`. Blocks for the whole transfer.
    pub fn clone(url: &str, dest: impl AsRef<Path>) -> Result<Self> {
        let dest = dest.as_ref();
        let parent = dest.parent().unwrap_or(Path::new("."));
        std::fs::create_dir_all(parent).map_err(|e| GitError::Io(e.to_string()))?;
        Git::new(parent)
            .args(["clone", "--progress", "--", url])
            .arg(dest.as_os_str())
            .network()
            .run()?;
        Self::open(dest)
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Directory name, used as the display name in the repository list.
    pub fn name(&self) -> String {
        self.root
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| self.root.display().to_string())
    }

    pub(crate) fn git(&self) -> Git {
        Git::new(&self.root)
    }

    /// Whether `HEAD` resolves to a commit (false in a freshly-initialized repo).
    pub fn has_head(&self) -> bool {
        self.git()
            .args(["rev-parse", "--verify", "-q", "HEAD"])
            .run()
            .is_ok()
    }

    // ---- write operations -------------------------------------------------

    /// Commit exactly `paths` with the given message, mirroring Desktop's
    /// "checkbox" model: the index is reset to HEAD and only the selected
    /// paths are staged, so whatever the user ticked is what gets committed.
    pub fn commit(&self, summary: &str, description: &str, paths: &[String]) -> Result<String> {
        self.commit_partial(summary, description, paths, &[])
    }

    /// [`Self::commit`] plus files of which only some lines are committed
    /// (`partial`, not listed in `paths`).
    pub fn commit_partial(
        &self,
        summary: &str,
        description: &str,
        paths: &[String],
        partial: &[PartialFile],
    ) -> Result<String> {
        if summary.trim().is_empty() {
            return Err(GitError::Invalid("commit summary is empty".into()));
        }
        if paths.is_empty() && partial.is_empty() {
            return Err(GitError::Invalid("no files selected".into()));
        }
        if self.has_head() {
            self.git().args(["reset", "-q", "--", "."]).run()?;
        } else {
            // No HEAD yet: unstage everything by removing it from the index only.
            let _ = self
                .git()
                .args(["rm", "-q", "-r", "--cached", "--ignore-unmatch", "--", "."])
                .run();
        }
        for chunk in paths.chunks(500) {
            self.git()
                .args(["add", "-A", "--"])
                .args(chunk.iter().map(String::as_str))
                .run()?;
        }
        self.stage_partial(partial)?;
        let mut message = summary.trim().to_string();
        if !description.trim().is_empty() {
            message.push_str("\n\n");
            message.push_str(description.trim());
        }
        self.git()
            .args(["commit", "-q", "-F", "-"])
            .stdin(message)
            .run()?;
        Ok(self
            .git()
            .args(["rev-parse", "HEAD"])
            .run()?
            .trim()
            .to_string())
    }

    /// Discard working-tree changes for `paths`. Untracked files are deleted.
    pub fn discard(&self, changes: &[FileChange]) -> Result<()> {
        let (untracked, tracked): (Vec<_>, Vec<_>) = changes
            .iter()
            .partition(|c| c.status == FileStatus::Untracked);
        if !tracked.is_empty() {
            let paths: Vec<&str> = tracked.iter().map(|c| c.path.as_str()).collect();
            if self.has_head() {
                self.git()
                    .args(["checkout", "-q", "HEAD", "--"])
                    .args(paths.iter().copied())
                    .run()?;
            } else {
                self.git()
                    .args(["rm", "-q", "--cached", "--"])
                    .args(paths.iter().copied())
                    .run()?;
            }
        }
        for c in untracked {
            let p = self.root.join(&c.path);
            let res = if p.is_dir() {
                std::fs::remove_dir_all(&p)
            } else {
                std::fs::remove_file(&p)
            };
            res.map_err(|e| GitError::Io(format!("{}: {e}", c.path)))?;
        }
        Ok(())
    }

    /// Switch to an existing local branch, or create a tracking branch for a
    /// remote one (`origin/foo` → `foo`).
    pub fn checkout(&self, branch: &Branch) -> Result<()> {
        match branch.kind {
            BranchKind::Local => self.git().args(["checkout", "-q", &branch.name]).run()?,
            BranchKind::Remote => {
                let local = branch
                    .name
                    .split_once('/')
                    .map(|(_, rest)| rest)
                    .unwrap_or(&branch.name);
                self.git()
                    .args(["checkout", "-q", "-b", local, "--track", &branch.name])
                    .run()?
            }
        };
        Ok(())
    }

    /// Create a branch from HEAD and switch to it.
    pub fn create_branch(&self, name: &str) -> Result<()> {
        self.git().args(["checkout", "-q", "-b", name]).run()?;
        Ok(())
    }

    pub fn delete_branch(&self, name: &str, force: bool) -> Result<()> {
        let flag = if force { "-D" } else { "-d" };
        self.git().args(["branch", flag, name]).run()?;
        Ok(())
    }

    pub fn fetch(&self) -> Result<()> {
        // Fetch the default remote strictly, then the others best-effort, so
        // a broken secondary remote (e.g. a mistyped `upstream` URL) doesn't
        // fail the whole fetch like `git fetch --all` would.
        let default = self.default_remote()?;
        let Some(default) = default else { return Ok(()) };
        self.git()
            .args(["fetch", "--prune", &default])
            .network()
            .run()?;
        for (name, _) in self.remotes()?.into_iter().filter(|(n, _)| *n != default) {
            if let Err(e) = self.git().args(["fetch", "--prune", &name]).network().run() {
                eprintln!("fetch of remote '{name}' failed: {e}");
            }
        }
        Ok(())
    }

    pub fn pull(&self) -> Result<()> {
        self.git().args(["pull", "--ff"]).network().run()?;
        Ok(())
    }

    /// Push the current branch, setting upstream on first push like Desktop's
    /// "Publish branch".
    pub fn push(&self, branch: &str, has_upstream: bool) -> Result<()> {
        if has_upstream {
            self.git().args(["push"]).network().run()?;
        } else {
            let remote = self.default_remote()?.unwrap_or_else(|| "origin".into());
            self.git()
                .args(["push", "--set-upstream", &remote, branch])
                .network()
                .run()?;
        }
        Ok(())
    }

    /// Append `patterns` to the repository's root `.gitignore` (created if
    /// missing). Patterns already present are skipped. Use [`escape_ignore`]
    /// for literal paths.
    pub fn ignore(&self, patterns: &[String]) -> Result<()> {
        use std::io::Write;
        let file = self.root.join(".gitignore");
        let existing = std::fs::read_to_string(&file).unwrap_or_default();
        let mut out = String::new();
        if !existing.is_empty() && !existing.ends_with('\n') {
            out.push('\n');
        }
        for p in patterns {
            if !existing.lines().any(|l| l == p) && !out.lines().any(|l| l == p) {
                out.push_str(p);
                out.push('\n');
            }
        }
        if out.trim().is_empty() {
            return Ok(());
        }
        std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&file)
            .and_then(|mut f| f.write_all(out.as_bytes()))
            .map_err(|e| GitError::Io(format!(".gitignore: {e}")))
    }

    /// Undo the last commit, keeping its changes in the working tree.
    pub fn undo_last_commit(&self) -> Result<()> {
        self.git().args(["reset", "-q", "--soft", "HEAD~1"]).run()?;
        Ok(())
    }

    pub fn remotes(&self) -> Result<Vec<(String, String)>> {
        let out = self.git().args(["remote", "-v"]).run()?;
        let mut v: Vec<(String, String)> = out
            .lines()
            .filter(|l| l.ends_with("(fetch)"))
            .filter_map(|l| {
                let mut it = l.split_whitespace();
                Some((it.next()?.to_string(), it.next()?.to_string()))
            })
            .collect();
        v.dedup();
        Ok(v)
    }

    fn default_remote(&self) -> Result<Option<String>> {
        let remotes = self.remotes()?;
        Ok(remotes
            .iter()
            .find(|(n, _)| n == "origin")
            .or(remotes.first())
            .map(|(n, _)| n.clone()))
    }
}

// ---- filesystem-level helpers (no process spawn) ----------------------------

impl Repository {
    /// The repository's git directory (`<root>/.git`, or the target of a
    /// `.git` *file* as used by linked worktrees and submodules). Reads the
    /// filesystem only, so it is cheap and never spawns git.
    pub fn git_dir(&self) -> PathBuf {
        let dot_git = self.root.join(".git");
        if dot_git.is_file() {
            if let Ok(s) = std::fs::read_to_string(&dot_git) {
                if let Some(p) = s.trim().strip_prefix("gitdir:") {
                    let p = PathBuf::from(p.trim());
                    return if p.is_absolute() { p } else { self.root.join(p) };
                }
            }
        }
        dot_git
    }

    /// The directory shared between linked worktrees (holds `refs`,
    /// `FETCH_HEAD`, `config`). Equal to [`git_dir`](Self::git_dir) for a
    /// normal repository.
    pub fn common_git_dir(&self) -> PathBuf {
        let dir = self.git_dir();
        match std::fs::read_to_string(dir.join("commondir")) {
            Ok(s) => {
                let p = PathBuf::from(s.trim());
                if p.is_absolute() { p } else { dir.join(p) }
            }
            Err(_) => dir,
        }
    }

    /// When the repository last fetched, from the mtime of `FETCH_HEAD`
    /// (what Desktop does too). `None` if it never fetched. Rewritten by every
    /// `git fetch`, even when nothing new arrived.
    pub fn last_fetch_time(&self) -> Option<std::time::SystemTime> {
        std::fs::metadata(self.common_git_dir().join("FETCH_HEAD"))
            .and_then(|m| m.modified())
            .ok()
    }
}

/// Escape gitignore metacharacters so `path` matches literally, anchored at
/// the repository root (`/dir/file name.txt`).
pub fn escape_ignore(path: &str) -> String {
    let mut out = String::from("/");
    for (i, c) in path.chars().enumerate() {
        if matches!(c, '*' | '?' | '[' | ']' | '\\') || (i == 0 && matches!(c, '#' | '!')) {
            out.push('\\');
        }
        out.push(c);
    }
    out
}
