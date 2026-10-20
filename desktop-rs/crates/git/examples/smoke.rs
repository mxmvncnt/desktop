//! Manual smoke check of write operations on a scratch repo:
//!   cargo run -p gd-git --example smoke -- <scratch-dir-inside-project>
use gd_git::*;

fn main() {
    let dir = std::path::PathBuf::from(std::env::args().nth(1).expect("scratch dir"));
    let _ = std::fs::remove_dir_all(&dir);
    let repo = Repository::init(&dir).unwrap();
    std::fs::write(dir.join("a.txt"), "a\n").unwrap();
    std::fs::write(dir.join("b.txt"), "b\n").unwrap();
    let st = repo.status().unwrap();
    println!("unborn status: branch={:?} files={:?}", st.branch, st.files.iter().map(|f| (&f.path, f.status)).collect::<Vec<_>>());
    println!("untracked diff lines: {}", repo.working_diff(&st.files[0], Default::default()).unwrap().lines.len());
    // Partial commit: only a.txt.
    repo.commit("first", "body", &["a.txt".into()]).unwrap();
    let st = repo.status().unwrap();
    println!("after partial commit: {:?}", st.files.iter().map(|f| &f.path).collect::<Vec<_>>());
    repo.commit("second", "", &["b.txt".into()]).unwrap();
    std::fs::write(dir.join("a.txt"), "a\nmore\n").unwrap();
    let st = repo.status().unwrap();
    let d = repo.working_diff(&st.files[0], Default::default()).unwrap();
    println!("modified diff: +{} -{} lines={}", d.added, d.deleted, d.lines.len());
    repo.discard(&st.files).unwrap();
    println!("after discard: {} changes", repo.status().unwrap().files.len());
    // ignore: literal + extension patterns, idempotent, ends with newline.
    std::fs::write(dir.join("junk.log"), "x").unwrap();
    std::fs::write(dir.join("we[ird].tmp"), "x").unwrap();
    repo.ignore(&[escape_ignore("we[ird].tmp"), "*.log".into()]).unwrap();
    repo.ignore(&["*.log".into()]).unwrap();
    println!(".gitignore: {:?}", std::fs::read_to_string(dir.join(".gitignore")).unwrap());
    let left: Vec<_> = repo.status().unwrap().files.into_iter().map(|f| f.path).collect();
    println!("after ignore: {left:?}");
    assert!(!left.iter().any(|p| p.ends_with(".log") || p.ends_with(".tmp")));
    repo.create_branch("feature/x").unwrap();
    println!("branches: {:?}", repo.branches().unwrap().iter().map(|b| (&b.name, b.is_head)).collect::<Vec<_>>());
    let log = repo.log(0, 10).unwrap();
    println!("log: {:?}", log.iter().map(|c| (&c.summary, &c.body)).collect::<Vec<_>>());
    println!("files in HEAD: {:?}", repo.commit_files(&log[0].sha).unwrap());
    println!("root commit diff: {:?}", repo.commit_diff(&log[1].sha, &repo.commit_files(&log[1].sha).unwrap()[0], Default::default()).unwrap().lines.len());
    repo.undo_last_commit().unwrap();
    println!("after undo: {:?}", repo.status().unwrap().files.iter().map(|f| (&f.path, f.status)).collect::<Vec<_>>());
    let main = repo.branches().unwrap().into_iter().find(|b| !b.is_head).unwrap();
    repo.checkout(&main).unwrap();
    println!("head now: {:?}", repo.status().unwrap().branch);
}
