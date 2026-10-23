//! Per-host HTTPS credentials for network operations.
//!
//! The app registers the token of each configured account here. Every
//! network git command then gets, via `GIT_CONFIG_COUNT`/`KEY`/`VALUE`
//! environment variables (nothing is written to any config file), a
//! host-scoped `credential.https://<host>.helper` that answers with the
//! account's username and token. Git itself matches the remote URL against
//! the scope, so no remote parsing happens here. The secret travels only in
//! the child's environment, never on a command line.

use std::ffi::OsString;
use std::sync::RwLock;

#[derive(Clone, PartialEq, Eq)]
pub struct HostCredential {
    /// Bare host, optionally with a port (`github.com`, `git.corp:8443`).
    pub host: String,
    pub username: String,
    pub token: String,
}

impl std::fmt::Debug for HostCredential {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "HostCredential({}, {}, <redacted>)", self.host, self.username)
    }
}

static CREDENTIALS: RwLock<Vec<HostCredential>> = RwLock::new(Vec::new());

/// Replace the registered credentials (entries with an empty host or token are dropped).
pub fn set_credentials(list: Vec<HostCredential>) {
    let list = list.into_iter().filter(|c| !c.host.trim().is_empty() && !c.token.is_empty()).collect();
    *CREDENTIALS.write().unwrap_or_else(|e| e.into_inner()) = list;
}

/// Shell snippet git runs as `<snippet> get`; `U`/`T` are replaced by the env var names.
const HELPER: &str = r#"!f() { test "$1" = get && printf 'username=%s\npassword=%s\n' "$U" "$T"; }; f"#;

/// Environment for a network command. Each credential gets its own helper
/// reading `GD_CRED_USER_<i>`/`GD_CRED_TOKEN_<i>`.
pub(crate) fn env() -> Vec<(OsString, OsString)> {
    let creds = CREDENTIALS.read().unwrap_or_else(|e| e.into_inner());
    if creds.is_empty() {
        return Vec::new();
    }
    let mut n: usize = std::env::var("GIT_CONFIG_COUNT").ok().and_then(|v| v.parse().ok()).unwrap_or(0);
    let mut out: Vec<(OsString, OsString)> = Vec::new();
    for (i, c) in creds.iter().enumerate() {
        let key = format!("credential.https://{}.helper", c.host.trim());
        let helper = HELPER.replace("$U", &format!("${{GD_CRED_USER_{i}}}")).replace("$T", &format!("${{GD_CRED_TOKEN_{i}}}"));
        // The empty value resets the helper list, so the account wins over
        // other helpers for this host.
        for value in [String::new(), helper] {
            out.push((format!("GIT_CONFIG_KEY_{n}").into(), key.clone().into()));
            out.push((format!("GIT_CONFIG_VALUE_{n}").into(), value.into()));
            n += 1;
        }
        out.push((format!("GD_CRED_USER_{i}").into(), c.username.clone().into()));
        out.push((format!("GD_CRED_TOKEN_{i}").into(), c.token.clone().into()));
    }
    out.push(("GIT_CONFIG_COUNT".into(), n.to_string().into()));
    out
}
