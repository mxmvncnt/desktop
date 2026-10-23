//! Identity + credential smoke test against scratch files only:
//! `cargo run -p gd-git --example identity_smoke -- /tmp/desktop-rs/identity`

use std::io::Write;

use gd_git::{Identity, IdentitySource, Repository};

fn main() {
    let dir = std::path::PathBuf::from(std::env::args().nth(1).expect("scratch dir"));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    // Never touch the real global config.
    // SAFETY: single-threaded at this point.
    unsafe { std::env::set_var("GIT_CONFIG_GLOBAL", dir.join("gitconfig")) };
    let repo = Repository::init(dir.join("repo")).unwrap();

    assert_eq!(gd_git::global_identity().unwrap(), Identity::default());
    assert_eq!(repo.effective_identity().unwrap().source, IdentitySource::Unset);

    let g = Identity { name: "Global Gal".into(), email: "gal@example.com".into() };
    gd_git::set_global_identity(&g).unwrap();
    assert_eq!(gd_git::global_identity().unwrap(), g);
    let eff = repo.effective_identity().unwrap();
    assert_eq!((eff.identity.clone(), eff.source), (g.clone(), IdentitySource::Global));

    let l = Identity { name: "Local Lou".into(), email: String::new() };
    repo.set_local_identity(&l).unwrap();
    let eff = repo.effective_identity().unwrap();
    assert_eq!(eff.source, IdentitySource::Repository);
    assert_eq!((eff.identity.name.as_str(), eff.identity.email.as_str()), ("Local Lou", "gal@example.com"));

    repo.set_local_identity(&Identity::default()).unwrap();
    assert_eq!(repo.effective_identity().unwrap().source, IdentitySource::Global);
    gd_git::set_global_identity(&Identity::default()).unwrap();
    assert_eq!(gd_git::global_identity().unwrap(), Identity::default());

    // Credential helper: ask git for the credential with the env a network op gets.
    gd_git::set_credentials(vec![gd_git::HostCredential { host: "git.example.com".into(), username: "me".into(), token: "s3cret".into() }]);
    let mut cmd = std::process::Command::new("git");
    cmd.current_dir(dir.join("repo")).args(["credential", "fill"]).env("GIT_TERMINAL_PROMPT", "0");
    for (k, v) in gd_git::credential_env_for_tests() {
        cmd.env(k, v);
    }
    let mut child = cmd.stdin(std::process::Stdio::piped()).stdout(std::process::Stdio::piped()).spawn().unwrap();
    child.stdin.take().unwrap().write_all(b"protocol=https\nhost=git.example.com\npath=o/r.git\n\n").unwrap();
    let out = String::from_utf8(child.wait_with_output().unwrap().stdout).unwrap();
    assert!(out.contains("username=me") && out.contains("password=s3cret"), "got {out:?}");
    println!("identity + credential smoke ok");
}
