//! Provider accounts and their persistence.
//!
//! Metadata (everything but the token) is in `<config dir>/accounts`, one
//! account per line: `id<TAB>provider<TAB>name<TAB>host<TAB>username`.
//! Tokens are in `<config dir>/tokens` (`id<TAB>token`), written with mode
//! 0600 on Unix. No keyring crate: its Secret Service / D-Bus dependency tree
//! is heavy and fails on headless boxes, so the token file is the documented
//! trade-off (tokens are plain text, protected by file permissions only).

use std::path::Path;

use gd_git::HostCredential;

use crate::features::settings::config_dir;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Provider {
    GitHub,
    GitHubEnterprise,
    GitLab,
    Bitbucket,
    Generic,
}

impl Provider {
    pub const ALL: [Provider; 5] =
        [Self::GitHub, Self::GitHubEnterprise, Self::GitLab, Self::Bitbucket, Self::Generic];

    pub fn label(self) -> &'static str {
        match self {
            Self::GitHub => "GitHub",
            Self::GitHubEnterprise => "Enterprise",
            Self::GitLab => "GitLab",
            Self::Bitbucket => "Bitbucket",
            Self::Generic => "Generic",
        }
    }

    pub fn full_label(self) -> &'static str {
        match self {
            Self::GitHubEnterprise => "GitHub Enterprise",
            p => p.label(),
        }
    }

    /// Persistence key.
    fn key(self) -> &'static str {
        match self {
            Self::GitHub => "github",
            Self::GitHubEnterprise => "github-enterprise",
            Self::GitLab => "gitlab",
            Self::Bitbucket => "bitbucket",
            Self::Generic => "generic",
        }
    }

    fn from_key(k: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|p| p.key() == k)
    }

    /// Fixed host of hosted providers; `None` = the user types it.
    pub fn fixed_host(self) -> Option<&'static str> {
        match self {
            Self::GitHub => Some("github.com"),
            Self::GitLab => Some("gitlab.com"),
            Self::Bitbucket => Some("bitbucket.org"),
            Self::GitHubEnterprise | Self::Generic => None,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Account {
    pub id: String,
    pub provider: Provider,
    pub name: String,
    pub host: String,
    pub username: String,
    /// Kept in memory for the credential helper; never in the `accounts` file.
    pub token: String,
}

impl Account {
    /// Credential for git network ops (`None` without a token or host).
    pub fn credential(&self) -> Option<HostCredential> {
        (!self.token.is_empty() && !self.host.is_empty()).then(|| HostCredential {
            host: self.host.clone(),
            username: if self.username.is_empty() { "git".into() } else { self.username.clone() },
            token: self.token.clone(),
        })
    }
}

/// `https://Git.Corp/path/` -> `git.corp`; keeps an explicit port.
pub fn normalize_host(input: &str) -> String {
    let s = input.trim();
    let s = s.split_once("://").map_or(s, |(_, rest)| rest);
    let s = s.split('/').next().unwrap_or("");
    let s = s.rsplit_once('@').map_or(s, |(_, h)| h);
    s.trim().to_lowercase()
}

/// A fresh unique id (time + process-local counter).
pub fn new_id() -> String {
    use std::sync::atomic::{AtomicU32, Ordering};
    static N: AtomicU32 = AtomicU32::new(0);
    let nanos = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_nanos());
    format!("{nanos:x}{:x}", N.fetch_add(1, Ordering::Relaxed))
}

fn clean(s: &str) -> String {
    s.chars().filter(|c| !c.is_control()).collect::<String>().trim().to_string()
}

pub fn accounts_text(list: &[Account]) -> String {
    list.iter()
        .map(|a| format!("{}\t{}\t{}\t{}\t{}\n", a.id, a.provider.key(), clean(&a.name), a.host, clean(&a.username)))
        .collect()
}

pub fn tokens_text(list: &[Account]) -> String {
    list.iter().filter(|a| !a.token.is_empty()).map(|a| format!("{}\t{}\n", a.id, clean(&a.token))).collect()
}

pub fn parse(accounts: &str, tokens: &str) -> Vec<Account> {
    let tokens: Vec<(&str, &str)> = tokens.lines().filter_map(|l| l.split_once('\t')).collect();
    accounts
        .lines()
        .filter_map(|l| {
            let mut f = l.split('\t');
            let (id, provider, name, host, username) = (f.next()?, f.next()?, f.next()?, f.next()?, f.next()?);
            Some(Account {
                id: id.to_string(),
                provider: Provider::from_key(provider)?,
                name: name.to_string(),
                host: host.to_string(),
                username: username.to_string(),
                token: tokens.iter().find(|(i, _)| *i == id).map_or_else(String::new, |(_, t)| t.to_string()),
            })
        })
        .collect()
}

pub fn load() -> Vec<Account> {
    let Some(dir) = config_dir() else { return Vec::new() };
    let read = |n: &str| std::fs::read_to_string(dir.join(n)).unwrap_or_default();
    parse(&read("accounts"), &read("tokens"))
}

pub fn save(list: &[Account]) {
    let Some(dir) = config_dir() else { return };
    if let Err(e) = std::fs::create_dir_all(&dir) {
        log::error!("cannot create config dir: {e}");
        return;
    }
    if let Err(e) = std::fs::write(dir.join("accounts"), accounts_text(list)) {
        log::error!("cannot save accounts: {e}");
    }
    if let Err(e) = write_private(&dir.join("tokens"), &tokens_text(list)) {
        log::error!("cannot save tokens: {e}");
    }
}

/// Write `text` readable by the owner only (created 0600, never briefly wider).
fn write_private(path: &Path, text: &str) -> std::io::Result<()> {
    use std::io::Write;
    let mut opts = std::fs::OpenOptions::new();
    opts.write(true).create(true).truncate(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
        opts.mode(0o600);
        let mut f = opts.open(path)?;
        // An older file may have wider permissions.
        f.set_permissions(std::fs::Permissions::from_mode(0o600))?;
        f.write_all(text.as_bytes())
    }
    #[cfg(not(unix))]
    {
        opts.open(path)?.write_all(text.as_bytes())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn acct(id: &str, provider: Provider, token: &str) -> Account {
        Account { id: id.into(), provider, name: "Work".into(), host: "github.com".into(), username: "me".into(), token: token.into() }
    }

    #[test]
    fn round_trips_and_keeps_tokens_out_of_metadata() {
        let list = vec![acct("a1", Provider::GitHub, "ghp_x"), acct("b2", Provider::Generic, "")];
        let meta = accounts_text(&list);
        assert!(!meta.contains("ghp_x"));
        assert_eq!(parse(&meta, &tokens_text(&list)), list);
        assert_eq!(parse("junk\nid\tbogus\ta\tb\tc\n", ""), vec![]);
    }

    #[test]
    fn normalizes_hosts() {
        assert_eq!(normalize_host(" https://User@Git.Corp:8443/x/y "), "git.corp:8443");
        assert_eq!(normalize_host("github.com"), "github.com");
    }

    #[test]
    fn credential_needs_token_and_defaults_username() {
        let mut a = acct("a", Provider::GitHub, "");
        assert!(a.credential().is_none());
        a.token = "t".into();
        a.username.clear();
        assert_eq!(a.credential().unwrap().username, "git");
    }
}
