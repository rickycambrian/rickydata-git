use assert_cmd::Command;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Command as StdCommand;

pub fn fixture_path(name: &str) -> String {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures")
        .join(name)
        .to_string_lossy()
        .into_owned()
}

pub fn rickygit_json(args: &[&str]) -> serde_json::Value {
    let output = Command::cargo_bin("rickygit")
        .unwrap()
        .args(args)
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "rickygit failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).unwrap()
}

pub fn rickygit_json_env(args: &[&str], env: &[(&str, &str)]) -> serde_json::Value {
    let mut cmd = Command::cargo_bin("rickygit").unwrap();
    cmd.args(args);
    for (key, value) in env {
        cmd.env(key, value);
    }
    let output = cmd.output().unwrap();

    assert!(
        output.status.success(),
        "rickygit failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).unwrap()
}

pub fn init_git_repo(repo: &Path) {
    git_success(repo, ["init", "-b", "main"]);
}

pub fn create_initial_commit(repo: &Path) -> String {
    git_success(repo, ["config", "user.email", "agent@example.com"]);
    git_success(repo, ["config", "user.name", "Agent"]);
    std::fs::write(repo.join("README.md"), "# test repo\n").unwrap();
    git_success(repo, ["add", "README.md"]);
    git_success(repo, ["commit", "-m", "initial"]);
    git_output(repo, ["rev-parse", "HEAD"]).trim().to_string()
}

pub fn git_success<const N: usize>(repo: &Path, args: [&str; N]) {
    let output = StdCommand::new("git")
        .args(args)
        .current_dir(repo)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "git failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

pub fn git_output<const N: usize>(repo: &Path, args: [&str; N]) -> String {
    String::from_utf8(git_stdout(repo, args)).unwrap()
}

pub fn git_stdout<const N: usize>(repo: &Path, args: [&str; N]) -> Vec<u8> {
    let output = StdCommand::new("git")
        .args(args)
        .current_dir(repo)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "git failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    output.stdout
}

pub fn added_git_files(
    before: &BTreeMap<String, Vec<u8>>,
    after: &BTreeMap<String, Vec<u8>>,
) -> Vec<String> {
    after
        .keys()
        .filter(|path| !before.contains_key(*path))
        .cloned()
        .collect()
}

pub fn git_file_snapshot(repo: &Path) -> BTreeMap<String, Vec<u8>> {
    let mut snapshot = BTreeMap::new();
    collect_git_files(&repo.join(".git"), &repo.join(".git"), &mut snapshot);
    snapshot
}

fn collect_git_files(root: &Path, current: &Path, snapshot: &mut BTreeMap<String, Vec<u8>>) {
    for entry in std::fs::read_dir(current).unwrap() {
        let entry = entry.unwrap();
        let path = entry.path();
        let file_type = entry.file_type().unwrap();
        if file_type.is_dir() {
            collect_git_files(root, &path, snapshot);
        } else if file_type.is_file() {
            let relative = path
                .strip_prefix(root)
                .unwrap()
                .to_string_lossy()
                .into_owned();
            snapshot.insert(relative, std::fs::read(&path).unwrap());
        }
    }
}

pub fn assert_packed_refs_has_no_rickydata_refs(repo: &Path) {
    let packed_refs = repo.join(".git/packed-refs");
    if packed_refs.exists() {
        let contents = std::fs::read_to_string(packed_refs).unwrap();
        assert!(!contents.contains("refs/rickydata/"));
    }
}
