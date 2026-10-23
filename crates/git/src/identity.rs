//! Git identity (`user.name` / `user.email`): global and per-repository.
//!
//! Global reads/writes use `git config --global`, so `GIT_CONFIG_GLOBAL`
//! (tests) is honored. Blocking, like everything in this crate.

use std::path::Path;

use crate::exec::Git;
use crate::{Repository, Result};

/// A `user.name` / `user.email` pair; empty string = unset.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Identity {
    pub name: String,
    pub email: String,
}

impl Identity {
    pub fn is_empty(&self) -> bool {
        self.name.is_empty() && self.email.is_empty()
    }
}

/// Where the effective identity of a repository comes from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IdentitySource {
    Repository,
    Global,
    /// Neither set (git may still derive one from the system).
    Unset,
}

/// What a commit in the repository would use.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EffectiveIdentity {
    pub identity: Identity,
    pub source: IdentitySource,
}

#[derive(Clone, Copy)]
enum Scope {
    Global,
    Local,
}

impl Scope {
    fn flag(self) -> &'static str {
        match self {
            Self::Global => "--global",
            Self::Local => "--local",
        }
    }
}

fn read(cwd: &Path, scope: Scope) -> Result<Identity> {
    let get = |key: &str| -> Result<String> {
        let out = Git::new(cwd).args(["config", scope.flag(), "--get", key]).allow_exit_one().run()?;
        Ok(out.trim().to_string())
    };
    Ok(Identity { name: get("user.name")?, email: get("user.email")? })
}

/// Set both keys; an empty value unsets the key.
fn write(cwd: &Path, scope: Scope, id: &Identity) -> Result<()> {
    for (key, value) in [("user.name", &id.name), ("user.email", &id.email)] {
        let value = value.trim();
        if value.is_empty() {
            // Fails (exit 5) when the key isn't present: nothing to unset.
            let _ = Git::new(cwd).args(["config", scope.flag(), "--unset-all", key]).run();
        } else {
            Git::new(cwd).args(["config", scope.flag(), key, value]).run()?;
        }
    }
    Ok(())
}

/// The global identity (`~/.gitconfig` or `$GIT_CONFIG_GLOBAL`).
pub fn global_identity() -> Result<Identity> {
    read(&std::env::temp_dir(), Scope::Global)
}

pub fn set_global_identity(id: &Identity) -> Result<()> {
    write(&std::env::temp_dir(), Scope::Global, id)
}

impl Repository {
    /// The repository-local override (empty fields = not overridden).
    pub fn local_identity(&self) -> Result<Identity> {
        read(self.root(), Scope::Local)
    }

    pub fn set_local_identity(&self, id: &Identity) -> Result<()> {
        write(self.root(), Scope::Local, id)
    }

    /// Which identity a commit here would use, and where it comes from.
    pub fn effective_identity(&self) -> Result<EffectiveIdentity> {
        let local = self.local_identity()?;
        let global = global_identity()?;
        let pick = |l: &str, g: &str| if l.is_empty() { g.to_string() } else { l.to_string() };
        let identity = Identity { name: pick(&local.name, &global.name), email: pick(&local.email, &global.email) };
        let source = if !local.is_empty() {
            IdentitySource::Repository
        } else if !global.is_empty() {
            IdentitySource::Global
        } else {
            IdentitySource::Unset
        };
        Ok(EffectiveIdentity { identity, source })
    }
}
