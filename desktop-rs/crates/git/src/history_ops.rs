//! History rewriting and commit-level operations, used by the History tab's
//! commit context menu: push status, tags, reset, checkout, revert,
//! cherry-pick, squash, reorder, amend.
//!
//! Conventions:
//! - Everything blocks; call from a background executor.
//! - Operations that can stop on conflicts (`revert_commit`, `cherry_pick`,
//!   `move_commit`, `squash_commits` when it has to replay later commits)
//!   leave the operation **in progress** and return `Ok(OpOutcome::Conflicts)`;
//!   the generic banner (`Repository::in_progress`, continue/abort) takes over.
//! - Nothing here ever opens an editor: rebases run with
//!   `core.editor=true` / `sequence.editor=<non-interactive>`.

use std::collections::{HashMap, HashSet};

use crate::{GitError, OpOutcome, Repository, Result};

/// How `git reset` treats the index and working tree.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResetMode {
    /// Keep the index: undone changes stay staged.
    Soft,
    /// Reset the index: undone changes stay in the working tree, unstaged.
    Mixed,
}

/// A node of the first-parent history walk.
#[derive(Debug, Clone)]
struct Node {
    sha: String,
    parents: Vec<String>,
}

/// How far back squash/reorder look for the selected commits.
const WALK_LIMIT: usize = 5000;

impl Repository {
    // ---- push status ------------------------------------------------------

    /// Shas of commits reachable from `HEAD` that are not on the remote yet.
    ///
    /// With an upstream: `rev-list @{upstream}..HEAD`. Without one (an
    /// unpublished branch): every commit not reachable from any remote-tracking
    /// branch (`HEAD --not --remotes`), like Desktop. No remotes at all means
    /// nothing can be pushed, so the set is empty.
    pub fn unpushed_commits(&self) -> Result<HashSet<String>> {
        if !self.has_head() {
            return Ok(HashSet::new());
        }
        let has_upstream = self
            .git()
            .args(["rev-parse", "--verify", "-q", "@{upstream}"])
            .run()
            .is_ok();
        let out = if has_upstream {
            self.git().args(["rev-list", "@{upstream}..HEAD"]).run()?
        } else {
            if self.remotes()?.is_empty() {
                return Ok(HashSet::new());
            }
            self.git().args(["rev-list", "HEAD", "--not", "--remotes"]).run()?
        };
        Ok(out.lines().map(|l| l.trim().to_string()).filter(|l| !l.is_empty()).collect())
    }

    /// Tags by the commit they point at (annotated tags are peeled).
    pub fn tags_by_commit(&self) -> Result<HashMap<String, Vec<String>>> {
        let out = self
            .git()
            .args([
                "for-each-ref",
                "--format=%(objectname)%00%(*objectname)%00%(refname:short)",
                "refs/tags",
            ])
            .run()?;
        let mut map: HashMap<String, Vec<String>> = HashMap::new();
        for line in out.lines() {
            let f: Vec<&str> = line.split('\0').collect();
            if f.len() < 3 {
                continue;
            }
            let sha = if f[1].is_empty() { f[0] } else { f[1] };
            map.entry(sha.to_string()).or_default().push(f[2].to_string());
        }
        Ok(map)
    }

    /// Web URL of the repository on GitHub (`https://github.com/owner/repo`),
    /// from the current branch's remote (or the default remote). `None` when
    /// there is no remote or it is not a GitHub host.
    pub fn github_web_url(&self) -> Option<String> {
        let remotes = self.remotes().ok()?;
        let upstream_remote = self
            .git()
            .args(["rev-parse", "--abbrev-ref", "@{upstream}"])
            .run()
            .ok()
            .and_then(|u| u.trim().split_once('/').map(|(r, _)| r.to_string()));
        let url = upstream_remote
            .and_then(|n| remotes.iter().find(|(r, _)| *r == n))
            .or_else(|| remotes.iter().find(|(n, _)| n == "origin"))
            .or(remotes.first())
            .map(|(_, u)| u.clone())?;
        github_web_url(&url)
    }

    // ---- operation state --------------------------------------------------

    fn ensure_idle(&self, what: &str) -> Result<()> {
        if self.in_progress().is_some() {
            return Err(GitError::Invalid(format!(
                "Can't {what} while another operation (merge, rebase, cherry-pick, revert) is in progress. Continue or abort it first."
            )));
        }
        Ok(())
    }

    // ---- simple operations ------------------------------------------------

    /// Move the current branch to `sha`, keeping the undone changes.
    pub fn reset_to_commit(&self, sha: &str, mode: ResetMode) -> Result<()> {
        self.ensure_idle("reset")?;
        let flag = match mode {
            ResetMode::Soft => "--soft",
            ResetMode::Mixed => "--mixed",
        };
        self.git().args(["reset", "-q", flag, sha]).run()?;
        Ok(())
    }

    /// Detach HEAD at `sha`.
    pub fn checkout_commit(&self, sha: &str) -> Result<()> {
        self.ensure_idle("check out a commit")?;
        self.git().args(["checkout", "-q", "--detach", sha]).run()?;
        Ok(())
    }

    /// Create `name` at `sha` and switch to it.
    pub fn create_branch_at(&self, name: &str, sha: &str) -> Result<()> {
        self.git().args(["checkout", "-q", "-b", name, sha]).run()?;
        Ok(())
    }

    pub fn create_tag(&self, name: &str, sha: &str) -> Result<()> {
        self.git().args(["tag", "-a", "-m", "", name, sha]).run()?;
        Ok(())
    }

    pub fn delete_tag(&self, name: &str) -> Result<()> {
        self.git().args(["tag", "-d", name]).run()?;
        Ok(())
    }

    fn parent_count(&self, sha: &str) -> Result<usize> {
        let out = self.git().args(["rev-list", "--parents", "-n", "1", sha]).run()?;
        Ok(out.split_whitespace().count().saturating_sub(1))
    }

    /// New commit undoing `sha` (`-m 1` for merge commits). On conflicts the
    /// revert stays in progress.
    pub fn revert_commit(&self, sha: &str) -> Result<OpOutcome> {
        self.ensure_idle("revert")?;
        let mut g = self.git().args(["revert", "--no-edit"]);
        if self.parent_count(sha)? > 1 {
            g = g.args(["-m", "1"]);
        }
        let res = g.arg(sha).run().map(|_| ());
        self.outcome_of(res)
    }

    /// Switch to `target` and cherry-pick `shas` (given oldest first) onto
    /// it. Conflicts leave the cherry-pick in progress.
    pub fn cherry_pick(&self, shas: &[String], target: &str) -> Result<OpOutcome> {
        if shas.is_empty() {
            return Err(GitError::Invalid("no commits to cherry-pick".into()));
        }
        self.ensure_idle("cherry-pick")?;
        self.git().args(["checkout", "-q", target]).run()?;
        let res = self.git().arg("cherry-pick").args(shas).run().map(|_| ());
        self.outcome_of(res)
    }

    /// Replace HEAD with a new commit that has the same tree and parents
    /// but the given message and the staged `paths` added (Desktop's amend).
    /// The author is kept. Pass no paths to only reword.
    pub fn amend_commit(&self, summary: &str, description: &str, paths: &[String]) -> Result<String> {
        if summary.trim().is_empty() {
            return Err(GitError::Invalid("commit summary is empty".into()));
        }
        self.ensure_idle("amend")?;
        self.git().args(["reset", "-q", "--", "."]).run()?;
        for chunk in paths.chunks(500) {
            self.git().args(["add", "-A", "--"]).args(chunk.iter().map(String::as_str)).run()?;
        }
        let mut message = summary.trim().to_string();
        if !description.trim().is_empty() {
            message.push_str("\n\n");
            message.push_str(description.trim());
        }
        self.git()
            .args(["commit", "-q", "--amend", "--allow-empty", "-F", "-"])
            .stdin(message)
            .run()?;
        Ok(self.git().args(["rev-parse", "HEAD"]).run()?.trim().to_string())
    }

    // ---- squash / reorder -------------------------------------------------

    fn first_parent_walk(&self) -> Result<Vec<Node>> {
        let out = self
            .git()
            .args(["rev-list", "--first-parent", "--parents"])
            .arg(format!("--max-count={WALK_LIMIT}"))
            .arg("HEAD")
            .run()?;
        Ok(out
            .lines()
            .filter_map(|l| {
                let mut it = l.split_whitespace();
                Some(Node {
                    sha: it.next()?.to_string(),
                    parents: it.map(String::from).collect(),
                })
            })
            .collect())
    }

    /// Whether `shas` form one adjacent run of non-merge commits on the
    /// current branch (what [`squash_commits`](Self::squash_commits) needs).
    pub fn can_squash(&self, shas: &[String]) -> bool {
        self.squash_plan(shas).is_ok()
    }

    /// (position of newest, position of oldest, walk)
    fn squash_plan(&self, shas: &[String]) -> Result<(usize, usize, Vec<Node>)> {
        if shas.len() < 2 {
            return Err(GitError::Invalid("select at least two commits to squash".into()));
        }
        let walk = self.first_parent_walk()?;
        let mut pos = Vec::with_capacity(shas.len());
        for s in shas {
            let p = walk
                .iter()
                .position(|n| n.sha == *s)
                .ok_or_else(|| GitError::Invalid(format!("commit {s} is not on the current branch (or too far back)")))?;
            pos.push(p);
        }
        pos.sort_unstable();
        pos.dedup();
        if pos.len() != shas.len() {
            return Err(GitError::Invalid("duplicate commits selected".into()));
        }
        let (newest, oldest) = (pos[0], *pos.last().unwrap());
        if oldest - newest + 1 != pos.len() {
            return Err(GitError::Invalid(
                "Squashing needs adjacent commits: the selected commits are not next to each other.".into(),
            ));
        }
        // Merge commits can't be squashed or replayed linearly.
        if walk[..=oldest].iter().any(|n| n.parents.len() > 1) {
            return Err(GitError::Invalid(
                "Can't squash: there is a merge commit between HEAD and the selected commits.".into(),
            ));
        }
        Ok((newest, oldest, walk))
    }

    /// Squash adjacent commits into one with `message` (summary + body).
    ///
    /// Approach (no interactive rebase needed): build the squashed commit
    /// directly with `commit-tree` using the *newest* selected commit's tree
    /// and the *oldest* selected commit's parent. If the selection ends at
    /// HEAD the branch is moved with `update-ref`: the working tree and index
    /// are never touched. Otherwise the commits after the selection are
    /// replayed with `rebase --autostash --onto <squashed> <newest>`; since
    /// the squashed tree equals the old tree they apply without conflicts,
    /// but if the rebase does stop it stays in progress.
    pub fn squash_commits(&self, shas: &[String], message: &str) -> Result<OpOutcome> {
        if message.trim().is_empty() {
            return Err(GitError::Invalid("commit message is empty".into()));
        }
        self.ensure_idle("squash")?;
        let (newest, oldest, walk) = self.squash_plan(shas)?;
        let tree = format!("{}^{{tree}}", walk[newest].sha);
        let mut ct = self.git().args(["commit-tree", &tree]);
        if let Some(parent) = walk[oldest].parents.first() {
            ct = ct.args(["-p", parent.as_str()]);
        }
        let new = ct
            .args(["-F", "-"])
            .stdin(format!("{}\n", message.trim()))
            .run()?
            .trim()
            .to_string();
        if newest == 0 {
            self.git()
                .args(["update-ref", "-m", "squash", "HEAD", new.as_str(), walk[0].sha.as_str()])
                .run()?;
        } else {
            let res = self
                .git()
                .args(["-c", "core.editor=true", "rebase", "--autostash", "--onto", new.as_str(), walk[newest].sha.as_str()])
                .run()
                .map(|_| ());
            return self.outcome_of(res);
        }
        Ok(OpOutcome::Done)
    }

    /// Move `sha` one place towards the tip (`up`, newer) or towards the
    /// root (`!up`, older) with a non-interactive `rebase -i`: the todo list
    /// is written to a file inside `.git` and installed by a
    /// `sequence.editor` of `cp <file>`. Only linear history (no merges
    /// between HEAD and the pair) is supported. Conflicts stay in progress.
    pub fn move_commit(&self, sha: &str, up: bool) -> Result<OpOutcome> {
        self.ensure_idle("reorder")?;
        let walk = self.first_parent_walk()?;
        let p = walk
            .iter()
            .position(|n| n.sha == sha)
            .ok_or_else(|| GitError::Invalid("commit is not on the current branch".into()))?;
        let lo = if up {
            if p == 0 {
                return Err(GitError::Invalid("This is already the newest commit.".into()));
            }
            p - 1
        } else {
            p
        };
        if lo + 1 >= walk.len() {
            return Err(GitError::Invalid("This is already the oldest commit.".into()));
        }
        if walk[..=lo + 1].iter().any(|n| n.parents.len() > 1) {
            return Err(GitError::Invalid("Can't reorder across a merge commit.".into()));
        }
        // Oldest first, with the pair swapped.
        let mut order: Vec<&str> = walk[..=lo + 1].iter().rev().map(|n| n.sha.as_str()).collect();
        let len = order.len();
        order.swap(len - 1 - lo, len - 2 - lo);
        let todo: String = order.iter().map(|s| format!("pick {s}\n")).collect();

        let path = self.git_dir().join("gd-rebase-todo");
        std::fs::write(&path, todo).map_err(|e| GitError::Io(e.to_string()))?;
        let editor = format!("cp '{}'", path.display().to_string().replace('\'', "'\\''"));
        let mut g = self
            .git()
            .args(["-c", &format!("sequence.editor={editor}"), "-c", "core.editor=true"])
            .args(["rebase", "-i", "--autostash"]);
        g = match walk[lo + 1].parents.first() {
            Some(base) => g.arg(base),
            None => g.arg("--root"),
        };
        let res = g.run();
        let _ = std::fs::remove_file(&path);
        self.outcome_of(res.map(|_| ()))
    }
}

/// Convert a git remote URL (ssh scp-style, `ssh://`, `https://`, `git://`)
/// into the repository's GitHub web URL. Hosts must look like GitHub
/// (`github.com`, or a GitHub Enterprise host containing "github").
pub fn github_web_url(remote: &str) -> Option<String> {
    let remote = remote.trim();
    let (host, path) = if let Some((_, rest)) = remote.split_once("://") {
        let (authority, path) = rest.split_once('/')?;
        let host = authority.rsplit('@').next()?;
        (host.split(':').next()?.to_string(), path.to_string())
    } else {
        // scp-like: [user@]host:owner/repo.git
        let (authority, path) = remote.split_once(':')?;
        (authority.rsplit('@').next()?.to_string(), path.to_string())
    };
    let host = host.to_ascii_lowercase();
    if !host.contains("github") {
        return None;
    }
    let path = path.trim_matches('/');
    let path = path.strip_suffix(".git").unwrap_or(path);
    if path.split('/').count() < 2 || path.is_empty() {
        return None;
    }
    Some(format!("https://{host}/{path}"))
}
