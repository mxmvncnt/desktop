//! Thin `std::process` wrapper around the `git` binary.

use std::ffi::{OsStr, OsString};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

#[derive(Debug, Clone, thiserror::Error)]
pub enum GitError {
    /// git exited non-zero; `stderr` is the trimmed message for display.
    #[error("{stderr}")]
    Failed { args: String, stderr: String },
    #[error("failed to run git: {0}")]
    Spawn(String),
    #[error("{0}")]
    Io(String),
    #[error("{0}")]
    Invalid(String),
    #[error("could not parse git output: {0}")]
    Parse(String),
}

pub(crate) struct Git {
    cwd: PathBuf,
    args: Vec<OsString>,
    stdin: Option<String>,
    network: bool,
    allow_exit_one: bool,
    env: Vec<(OsString, OsString)>,
}

impl Git {
    pub fn new(cwd: &Path) -> Self {
        Self {
            cwd: cwd.to_path_buf(),
            args: Vec::new(),
            stdin: None,
            network: false,
            allow_exit_one: false,
            env: Vec::new(),
        }
    }

    pub fn arg(mut self, a: impl AsRef<OsStr>) -> Self {
        self.args.push(a.as_ref().to_os_string());
        self
    }

    pub fn args<I, S>(mut self, it: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>,
    {
        self.args
            .extend(it.into_iter().map(|s| s.as_ref().to_os_string()));
        self
    }

    pub fn stdin(mut self, s: String) -> Self {
        self.stdin = Some(s);
        self
    }

    /// Marks a network operation: never prompt on the (absent) terminal.
    /// Credentials must come from a credential helper / ssh-agent.
    pub fn network(mut self) -> Self {
        self.network = true;
        self
    }

    /// Set an environment variable for this invocation only.
    pub fn env(mut self, k: impl AsRef<OsStr>, v: impl AsRef<OsStr>) -> Self {
        self.env.push((k.as_ref().to_os_string(), v.as_ref().to_os_string()));
        self
    }

    /// Never open an editor (merge/rebase/cherry-pick/revert `--continue`
    /// keep the prepared message).
    pub fn no_editor(self) -> Self {
        self.env("GIT_EDITOR", "true").env("GIT_SEQUENCE_EDITOR", "true")
    }

    /// Treat exit code 1 as success (`git diff --no-index` exits 1 when the
    /// inputs differ).
    pub fn allow_exit_one(mut self) -> Self {
        self.allow_exit_one = true;
        self
    }

    fn command(&self) -> Command {
        let mut cmd = Command::new("git");
        // Stable, parseable output regardless of user locale/config.
        cmd.current_dir(&self.cwd)
            .env("LC_ALL", "C")
            .env("GIT_OPTIONAL_LOCKS", "0")
            .args(["-c", "core.quotepath=false", "-c", "color.ui=false"])
            .args(&self.args)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        for (k, v) in &self.env {
            cmd.env(k, v);
        }
        if self.network {
            cmd.env("GIT_TERMINAL_PROMPT", "0");
        }
        cmd.stdin(if self.stdin.is_some() {
            Stdio::piped()
        } else {
            Stdio::null()
        });
        cmd
    }

    /// Run and return stdout as (lossy) UTF-8.
    pub fn run(self) -> Result<String, GitError> {
        self.run_bytes()
            .map(|b| String::from_utf8_lossy(&b).into_owned())
    }

    pub fn run_bytes(self) -> Result<Vec<u8>, GitError> {
        let start = std::time::Instant::now();
        let mut child = self
            .command()
            .spawn()
            .map_err(|e| GitError::Spawn(e.to_string()))?;
        if let Some(input) = &self.stdin {
            if let Some(mut stdin) = child.stdin.take() {
                stdin
                    .write_all(input.as_bytes())
                    .map_err(|e| GitError::Io(e.to_string()))?;
            }
        }
        let out = child
            .wait_with_output()
            .map_err(|e| GitError::Spawn(e.to_string()))?;
        let args = self
            .args
            .iter()
            .map(|a| a.to_string_lossy())
            .collect::<Vec<_>>()
            .join(" ");
        log::debug!("git {args} ({:?})", start.elapsed());
        if out.status.success() || (self.allow_exit_one && out.status.code() == Some(1)) {
            Ok(out.stdout)
        } else {
            let stderr = String::from_utf8_lossy(&out.stderr).trim().to_string();
            Err(GitError::Failed {
                args,
                stderr: if stderr.is_empty() {
                    format!("git exited with {}", out.status)
                } else {
                    stderr
                },
            })
        }
    }
}
