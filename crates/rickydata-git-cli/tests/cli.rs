mod support;

use assert_cmd::Command;
use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;
#[cfg(feature = "tee")]
use axum::routing::get;
use axum::routing::post;
use predicates::prelude::*;
use serde_json::Value;
use std::net::TcpListener;
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::Duration;
use support::{
    added_git_files, assert_packed_refs_has_no_rickydata_refs, create_initial_commit, fixture_path,
    git_file_snapshot, git_output, git_stdout, git_success, init_git_repo, rickygit_json,
    rickygit_json_env,
};

#[test]
fn doctor_emits_structured_success() {
    let mut cmd = Command::cargo_bin("rickygit").unwrap();
    cmd.args(["doctor", "--json"])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"status\": \"ok\""));
}

#[test]
fn manifest_emits_initial_contracts() {
    let mut cmd = Command::cargo_bin("rickygit").unwrap();
    cmd.args(["manifest", "--json"])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"name\": \"attempt_abandon\""))
        .stdout(predicate::str::contains("\"name\": \"attempt_list\""))
        .stdout(predicate::str::contains("\"name\": \"attempt_show\""))
        .stdout(predicate::str::contains("\"name\": \"attempt_start\""))
        .stdout(predicate::str::contains("\"name\": \"attempt_status\""))
        .stdout(predicate::str::contains("\"name\": \"attempt_submit\""))
        .stdout(predicate::str::contains("\"name\": \"change_detect\""))
        .stdout(predicate::str::contains("\"name\": \"change_list\""))
        .stdout(predicate::str::contains("\"name\": \"change_show\""))
        .stdout(predicate::str::contains("\"name\": \"discovery_emit\""))
        .stdout(predicate::str::contains("\"name\": \"intent_list\""))
        .stdout(predicate::str::contains("\"name\": \"intent_show\""))
        .stdout(predicate::str::contains("\"name\": \"intent_write\""))
        .stdout(predicate::str::contains("\"name\": \"issue_import\""))
        .stdout(predicate::str::contains("\"name\": \"key_generate\""))
        .stdout(predicate::str::contains("\"name\": \"key_show\""))
        .stdout(predicate::str::contains("\"name\": \"repo_init\""))
        .stdout(predicate::str::contains("\"name\": \"repo_inspect\""))
        .stdout(predicate::str::contains("\"name\": \"object_write\""))
        .stdout(predicate::str::contains("\"name\": \"patch_apply\""))
        .stdout(predicate::str::contains("\"name\": \"patch_checkout\""))
        .stdout(predicate::str::contains("\"name\": \"patch_export\""))
        .stdout(predicate::str::contains("\"name\": \"patch_list\""))
        .stdout(predicate::str::contains("\"name\": \"patch_prepare\""))
        .stdout(predicate::str::contains("\"name\": \"patch_retire\""))
        .stdout(predicate::str::contains("\"name\": \"patch_review_queue\""))
        .stdout(predicate::str::contains("\"name\": \"patch_show\""))
        .stdout(predicate::str::contains("\"name\": \"proof\""))
        .stdout(predicate::str::contains("\"name\": \"relay_pull\""))
        .stdout(predicate::str::contains("\"name\": \"relay_push\""))
        .stdout(predicate::str::contains("\"name\": \"relay_status\""))
        .stdout(predicate::str::contains("\"name\": \"run_exec\""))
        .stdout(predicate::str::contains("\"name\": \"run_list\""))
        .stdout(predicate::str::contains("\"name\": \"run_show\""))
        .stdout(predicate::str::contains("\"name\": \"schema_emit\""))
        .stdout(predicate::str::contains("\"name\": \"repo_status\""))
        .stdout(predicate::str::contains("\"name\": \"sync_pull\""))
        .stdout(predicate::str::contains("\"name\": \"sync_push\""))
        .stdout(predicate::str::contains("\"name\": \"sync_status\""))
        .stdout(predicate::str::contains("\"name\": \"sync_verify\""))
        .stdout(predicate::str::contains("\"name\": \"work_start\""))
        .stdout(predicate::str::contains(
            "\"output_schema_hash\": \"sha256:",
        ))
        .stdout(predicate::str::contains("\"stable_hash\": \"sha256:"));
}

#[test]
fn repo_status_reports_uninitialized_repo_without_writing_metadata() {
    let repo = tempfile::tempdir().unwrap();
    init_git_repo(repo.path());
    create_initial_commit(repo.path());
    let repo_arg = repo.path().to_str().unwrap();
    let before = git_file_snapshot(repo.path());

    let status = rickygit_json(&["status", "--repo", repo_arg, "--json"]);

    assert_eq!(status["status"], "not_initialized");
    assert_eq!(status["inspection"]["is_git_repo"], true);
    assert_eq!(status["store"]["initialized"], false);
    assert!(
        status["store"]["diagnostics"][0]
            .as_str()
            .unwrap()
            .contains("not initialized")
    );
    assert_eq!(status["verify"], serde_json::Value::Null);
    assert_eq!(status["sync"], serde_json::Value::Null);
    assert!(
        status["data_locations"]["metadata_dir"]
            .as_str()
            .unwrap()
            .ends_with(".git/rickydata")
    );
    assert_eq!(
        status["data_locations"]["refspec"],
        "refs/rickydata/*:refs/rickydata/*"
    );
    assert!(!repo.path().join(".git/rickydata").exists());
    assert!(!repo.path().join(".git/refs/rickydata").exists());
    assert_eq!(before, git_file_snapshot(repo.path()));
}

#[test]
fn repo_status_reports_initialized_synced_repo_read_only() {
    let remote = tempfile::tempdir().unwrap();
    git_success(remote.path(), ["init", "--bare"]);

    let repo = tempfile::tempdir().unwrap();
    init_git_repo(repo.path());
    create_initial_commit(repo.path());
    let repo_arg = repo.path().to_str().unwrap();
    rickygit_json(&["init", "--repo", repo_arg, "--json"]);
    rickygit_json(&[
        "object",
        "write",
        "--repo",
        repo_arg,
        "--kind",
        "agent.intent",
        "--body-file",
        &fixture_path("work-intent.valid.json"),
        "--json",
    ]);
    git_success(
        repo.path(),
        ["remote", "add", "origin", remote.path().to_str().unwrap()],
    );
    rickygit_json(&[
        "sync", "push", "--repo", repo_arg, "--remote", "origin", "--json",
    ]);
    let before = git_file_snapshot(repo.path());

    let status = rickygit_json(&["status", "--repo", repo_arg, "--remote", "origin", "--json"]);

    assert_eq!(status["status"], "ok");
    assert_eq!(status["store"]["initialized"], true);
    assert_eq!(status["store"]["store_version"], "rickydata.git.store.v1");
    assert!(
        status["store"]["diagnostics"]
            .as_array()
            .unwrap()
            .is_empty()
    );
    assert_eq!(status["verify"]["status"], "ok");
    assert_eq!(status["verify"]["object_count"], 1);
    assert_eq!(status["verify"]["valid_object_count"], 1);
    assert_eq!(status["verify"]["patch_count"], 0);
    assert_eq!(status["sync"]["status"], "ok");
    assert_eq!(status["sync"]["local_ref_count"], 1);
    assert_eq!(status["sync"]["remote_ref_count"], 1);
    assert_eq!(status["sync"]["matching_ref_count"], 1);
    assert!(
        status["sync"]["local_only_refs"]
            .as_array()
            .unwrap()
            .is_empty()
    );
    assert!(
        status["sync"]["remote_only_refs"]
            .as_array()
            .unwrap()
            .is_empty()
    );
    assert_eq!(before, git_file_snapshot(repo.path()));
    assert_eq!(git_output(repo.path(), ["status", "--short"]), "");
}

#[test]
fn repo_status_surfaces_invalid_metadata_diagnostics() {
    let repo = tempfile::tempdir().unwrap();
    init_git_repo(repo.path());
    create_initial_commit(repo.path());
    let repo_arg = repo.path().to_str().unwrap();
    rickygit_json(&["init", "--repo", repo_arg, "--json"]);
    let write = rickygit_json(&[
        "object",
        "write",
        "--repo",
        repo_arg,
        "--kind",
        "agent.intent",
        "--body-file",
        &fixture_path("work-intent.valid.json"),
        "--json",
    ]);
    std::fs::write(write["cache_path"].as_str().unwrap(), b"{}").unwrap();

    let status = rickygit_json(&["status", "--repo", repo_arg, "--json"]);

    assert_eq!(status["status"], "failed");
    assert_eq!(status["verify"]["status"], "failed");
    assert_eq!(
        status["verify"]["invalid_objects"][0]["object_id"],
        write["object_id"]
    );
    assert!(
        status["verify"]["invalid_objects"][0]["diagnostics"][0]
            .as_str()
            .unwrap()
            .contains("object")
    );
}

#[test]
fn schema_emits_foundational_objects() {
    let mut cmd = Command::cargo_bin("rickygit").unwrap();
    cmd.args(["schema", "--json"])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"WorkIntent\""))
        .stdout(predicate::str::contains("\"schema_hashes\""))
        .stdout(predicate::str::contains("\"DiscoveryObject\""));
}

#[test]
fn schema_hashes_are_stable() {
    let first = rickygit_json(&["schema", "--json"]);
    let second = rickygit_json(&["schema", "--json"]);

    assert_eq!(first["schema_hashes"], second["schema_hashes"]);
    assert!(
        first["schema_hashes"]["WorkIntent"]
            .as_str()
            .unwrap()
            .starts_with("sha256:")
    );
}

#[test]
fn discovery_emits_compiled_rdl_adapter_contracts() {
    let mut cmd = Command::cargo_bin("rickygit").unwrap();
    cmd.args(["discovery", "--repo", env!("CARGO_MANIFEST_DIR"), "--json"])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"object_id\": \"sha256:"))
        .stdout(predicate::str::contains("\"discovery\""))
        .stdout(predicate::str::contains("\"adapter_name\": \"rust-rdl\""))
        .stdout(predicate::str::contains("\"name\": \"repo_inspect\""))
        .stdout(predicate::str::contains("\"name\": \"discovery_emit\""));
}

#[test]
fn init_creates_local_rickydata_store_idempotently() {
    let repo = tempfile::tempdir().unwrap();
    init_git_repo(repo.path());
    let initial_snapshot = git_file_snapshot(repo.path());
    let initial_branch = git_output(repo.path(), ["branch", "--show-current"]);

    let repo_arg = repo.path().to_str().unwrap();
    let first = rickygit_json(&["init", "--repo", repo_arg, "--json"]);
    let second = rickygit_json(&["init", "--repo", repo_arg, "--json"]);

    assert_eq!(first["status"], "created");
    assert_eq!(second["status"], "already_initialized");
    assert_eq!(first["store_version"], "rickydata.git.store.v1");
    assert!(repo.path().join(".git/rickydata/VERSION").exists());
    assert!(
        repo.path()
            .join(".git/rickydata/cache/objects/sha256")
            .is_dir()
    );
    assert!(repo.path().join(".git/rickydata/cache/bundles").is_dir());
    assert!(repo.path().join(".git/refs/rickydata/intents").is_dir());
    assert_eq!(
        added_git_files(&initial_snapshot, &git_file_snapshot(repo.path())),
        vec!["rickydata/VERSION".to_string()]
    );
    assert_eq!(
        git_output(repo.path(), ["branch", "--show-current"]),
        initial_branch
    );
    assert_eq!(git_output(repo.path(), ["status", "--short"]), "");
    git_success(repo.path(), ["fsck"]);
}

#[test]
fn init_rejects_non_git_directory_with_structured_json() {
    let directory = tempfile::tempdir().unwrap();

    let mut cmd = Command::cargo_bin("rickygit").unwrap();
    cmd.args([
        "init",
        "--repo",
        directory.path().to_str().unwrap(),
        "--json",
    ])
    .assert()
    .failure()
    .stdout(predicate::str::contains("\"kind\": \"command\""))
    .stdout(predicate::str::contains("not inside a Git repository"));
}

#[test]
fn object_write_read_verify_round_trip() {
    let repo = tempfile::tempdir().unwrap();
    init_git_repo(repo.path());
    let repo_arg = repo.path().to_str().unwrap();
    rickygit_json(&["init", "--repo", repo_arg, "--json"]);
    let body_file = fixture_path("work-intent.valid.json");

    let write = rickygit_json(&[
        "object",
        "write",
        "--repo",
        repo_arg,
        "--kind",
        "agent.intent",
        "--body-file",
        &body_file,
        "--json",
    ]);
    let duplicate = rickygit_json(&[
        "object",
        "write",
        "--repo",
        repo_arg,
        "--kind",
        "agent.intent",
        "--body-file",
        &body_file,
        "--json",
    ]);
    let object_id = write["object_id"].as_str().unwrap();
    let read = rickygit_json(&[
        "object",
        "read",
        "--repo",
        repo_arg,
        "--object-id",
        object_id,
        "--json",
    ]);
    let verify = rickygit_json(&[
        "object",
        "verify",
        "--repo",
        repo_arg,
        "--object-id",
        object_id,
        "--json",
    ]);

    assert_eq!(write["status"], "written");
    assert_eq!(duplicate["status"], "already_exists");
    assert_eq!(
        object_id,
        "sha256:a0c9cd2e0309cc8d965aebc13c2d8acf09b4b1df507b8e83374f0e1a538ff071"
    );
    assert_eq!(read["object"]["object_id"], object_id);
    assert_eq!(read["source"], "cache");
    assert_eq!(verify["valid"], true);
    assert_eq!(verify["source"], "cache");
    let ref_name = write["ref_name"].as_str().unwrap();
    let git_object_id = write["git_object_id"].as_str().unwrap();
    let cache_path = write["cache_path"].as_str().unwrap();
    assert!(ref_name.starts_with("refs/rickydata/objects/sha256/"));
    assert!(git_output(repo.path(), ["show-ref", "--verify", ref_name]).contains(git_object_id));
    assert_eq!(
        git_stdout(repo.path(), ["cat-file", "-p", ref_name]),
        std::fs::read(cache_path).unwrap()
    );
    assert_eq!(git_output(repo.path(), ["status", "--short"]), "");
    git_success(repo.path(), ["fsck"]);
}

#[test]
fn object_read_recovers_after_fetching_rickydata_refs_without_cache() {
    let remote = tempfile::tempdir().unwrap();
    git_success(remote.path(), ["init", "--bare"]);

    let repo_a = tempfile::tempdir().unwrap();
    init_git_repo(repo_a.path());
    let repo_a_arg = repo_a.path().to_str().unwrap();
    rickygit_json(&["init", "--repo", repo_a_arg, "--json"]);
    let write = rickygit_json(&[
        "object",
        "write",
        "--repo",
        repo_a_arg,
        "--kind",
        "agent.intent",
        "--body-file",
        &fixture_path("work-intent.valid.json"),
        "--json",
    ]);
    let object_id = write["object_id"].as_str().unwrap();
    git_success(
        repo_a.path(),
        ["remote", "add", "origin", remote.path().to_str().unwrap()],
    );
    git_success(
        repo_a.path(),
        ["push", "origin", "refs/rickydata/*:refs/rickydata/*"],
    );
    git_success(repo_a.path(), ["fsck"]);
    git_success(remote.path(), ["fsck"]);

    let checkout = tempfile::tempdir().unwrap();
    git_success(
        checkout.path(),
        [
            "clone",
            remote.path().to_str().unwrap(),
            checkout.path().join("repo-b").to_str().unwrap(),
        ],
    );
    let repo_b = checkout.path().join("repo-b");
    git_success(
        &repo_b,
        ["fetch", "origin", "refs/rickydata/*:refs/rickydata/*"],
    );
    let repo_b_arg = repo_b.to_str().unwrap();
    rickygit_json(&["init", "--repo", repo_b_arg, "--json"]);
    std::fs::remove_dir_all(repo_b.join(".git/rickydata/cache/objects")).unwrap();

    let read = rickygit_json(&[
        "object",
        "read",
        "--repo",
        repo_b_arg,
        "--object-id",
        object_id,
        "--json",
    ]);
    let verify = rickygit_json(&[
        "object",
        "verify",
        "--repo",
        repo_b_arg,
        "--object-id",
        object_id,
        "--json",
    ]);

    assert_eq!(read["source"], "git_ref");
    assert_eq!(read["object"]["object_id"], object_id);
    assert_eq!(verify["valid"], true);
    git_success(&repo_b, ["fsck"]);
}

#[test]
fn intent_list_and_show_work_after_pack_refs_without_cache() {
    let repo = tempfile::tempdir().unwrap();
    init_git_repo(repo.path());
    let repo_arg = repo.path().to_str().unwrap();
    rickygit_json(&["init", "--repo", repo_arg, "--json"]);
    let write = rickygit_json(&[
        "intent",
        "write",
        "--repo",
        repo_arg,
        &fixture_path("work-intent.valid.json"),
        "--json",
    ]);
    let object_id = write["object"]["object_id"].as_str().unwrap();

    git_success(repo.path(), ["pack-refs", "--all", "--prune"]);
    std::fs::remove_dir_all(repo.path().join(".git/rickydata/cache/objects")).unwrap();
    let list = rickygit_json(&["intent", "list", "--repo", repo_arg, "--json"]);
    let show = rickygit_json(&[
        "intent",
        "show",
        "--repo",
        repo_arg,
        "--object-id",
        object_id,
        "--json",
    ]);

    assert_eq!(list["intents"].as_array().unwrap().len(), 1);
    assert_eq!(list["intents"][0]["object_id"], object_id);
    assert_eq!(show["source"], "git_ref");
    assert_eq!(show["valid"], true);
    git_success(repo.path(), ["fsck"]);
}

#[test]
fn intent_workflow_recovers_across_two_clones_without_cache() {
    let remote = tempfile::tempdir().unwrap();
    git_success(remote.path(), ["init", "--bare"]);
    let repo_a = tempfile::tempdir().unwrap();
    init_git_repo(repo_a.path());
    let repo_a_arg = repo_a.path().to_str().unwrap();
    rickygit_json(&["init", "--repo", repo_a_arg, "--json"]);
    let write = rickygit_json(&[
        "intent",
        "write",
        "--repo",
        repo_a_arg,
        &fixture_path("work-intent.valid.json"),
        "--json",
    ]);
    let object_id = write["object"]["object_id"].as_str().unwrap();
    git_success(
        repo_a.path(),
        ["remote", "add", "origin", remote.path().to_str().unwrap()],
    );
    git_success(
        repo_a.path(),
        ["push", "origin", "refs/rickydata/*:refs/rickydata/*"],
    );

    let checkout = tempfile::tempdir().unwrap();
    git_success(
        checkout.path(),
        [
            "clone",
            remote.path().to_str().unwrap(),
            checkout.path().join("repo-b").to_str().unwrap(),
        ],
    );
    let repo_b = checkout.path().join("repo-b");
    git_success(
        &repo_b,
        ["fetch", "origin", "refs/rickydata/*:refs/rickydata/*"],
    );
    let repo_b_arg = repo_b.to_str().unwrap();
    rickygit_json(&["init", "--repo", repo_b_arg, "--json"]);
    std::fs::remove_dir_all(repo_b.join(".git/rickydata/cache/objects")).unwrap();

    let list = rickygit_json(&["intent", "list", "--repo", repo_b_arg, "--json"]);
    let show = rickygit_json(&[
        "intent",
        "show",
        "--repo",
        repo_b_arg,
        "--object-id",
        object_id,
        "--json",
    ]);

    assert_eq!(list["intents"].as_array().unwrap().len(), 1);
    assert_eq!(list["intents"][0]["object_id"], object_id);
    assert_eq!(show["source"], "git_ref");
    assert_eq!(show["valid"], true);
    git_success(&repo_b, ["fsck"]);
}

#[test]
fn intent_workflow_survives_git_gc_and_recovers_without_cache() {
    let remote = tempfile::tempdir().unwrap();
    git_success(remote.path(), ["init", "--bare"]);
    let repo_a = tempfile::tempdir().unwrap();
    init_git_repo(repo_a.path());
    let repo_a_arg = repo_a.path().to_str().unwrap();
    rickygit_json(&["init", "--repo", repo_a_arg, "--json"]);
    let write = rickygit_json(&[
        "intent",
        "write",
        "--repo",
        repo_a_arg,
        &fixture_path("work-intent.valid.json"),
        "--json",
    ]);
    let object_id = write["object"]["object_id"].as_str().unwrap();
    git_success(
        repo_a.path(),
        ["remote", "add", "origin", remote.path().to_str().unwrap()],
    );
    git_success(
        repo_a.path(),
        ["push", "origin", "refs/rickydata/*:refs/rickydata/*"],
    );
    git_success(repo_a.path(), ["gc", "--prune=now"]);
    git_success(remote.path(), ["gc", "--prune=now"]);

    let checkout = tempfile::tempdir().unwrap();
    git_success(
        checkout.path(),
        [
            "clone",
            remote.path().to_str().unwrap(),
            checkout.path().join("repo-b").to_str().unwrap(),
        ],
    );
    let repo_b = checkout.path().join("repo-b");
    git_success(
        &repo_b,
        ["fetch", "origin", "refs/rickydata/*:refs/rickydata/*"],
    );
    let repo_b_arg = repo_b.to_str().unwrap();
    rickygit_json(&["init", "--repo", repo_b_arg, "--json"]);
    std::fs::remove_dir_all(repo_b.join(".git/rickydata/cache/objects")).unwrap();

    let show = rickygit_json(&[
        "intent",
        "show",
        "--repo",
        repo_b_arg,
        "--object-id",
        object_id,
        "--json",
    ]);

    assert_eq!(show["source"], "git_ref");
    assert_eq!(show["valid"], true);
    git_success(&repo_b, ["fsck"]);
}

#[test]
fn object_verify_detects_cache_ref_mismatch() {
    let repo = tempfile::tempdir().unwrap();
    init_git_repo(repo.path());
    let repo_arg = repo.path().to_str().unwrap();
    rickygit_json(&["init", "--repo", repo_arg, "--json"]);
    let body_file = fixture_path("work-intent.valid.json");
    let original = rickygit_json(&[
        "object",
        "write",
        "--repo",
        repo_arg,
        "--kind",
        "agent.intent",
        "--body-file",
        &body_file,
        "--json",
    ]);
    let alternate = rickygit_json(&[
        "object",
        "write",
        "--repo",
        repo_arg,
        "--kind",
        "agent.run",
        "--body-file",
        &body_file,
        "--json",
    ]);
    git_success(
        repo.path(),
        [
            "update-ref",
            original["ref_name"].as_str().unwrap(),
            alternate["git_object_id"].as_str().unwrap(),
        ],
    );

    let verify = rickygit_json(&[
        "object",
        "verify",
        "--repo",
        repo_arg,
        "--object-id",
        original["object_id"].as_str().unwrap(),
        "--json",
    ]);

    assert_eq!(verify["valid"], false);
    assert!(
        verify["diagnostics"]
            .as_array()
            .unwrap()
            .iter()
            .any(|diagnostic| diagnostic["code"] == "OBJECT006")
    );
}

#[test]
fn object_read_rejects_ref_body_mismatch_without_cache() {
    let repo = tempfile::tempdir().unwrap();
    init_git_repo(repo.path());
    let repo_arg = repo.path().to_str().unwrap();
    rickygit_json(&["init", "--repo", repo_arg, "--json"]);
    let body_file = fixture_path("work-intent.valid.json");
    let original = rickygit_json(&[
        "object",
        "write",
        "--repo",
        repo_arg,
        "--kind",
        "agent.intent",
        "--body-file",
        &body_file,
        "--json",
    ]);
    let alternate = rickygit_json(&[
        "object",
        "write",
        "--repo",
        repo_arg,
        "--kind",
        "agent.run",
        "--body-file",
        &body_file,
        "--json",
    ]);
    git_success(
        repo.path(),
        [
            "update-ref",
            original["ref_name"].as_str().unwrap(),
            alternate["git_object_id"].as_str().unwrap(),
        ],
    );
    std::fs::remove_file(original["cache_path"].as_str().unwrap()).unwrap();

    let mut cmd = Command::cargo_bin("rickygit").unwrap();
    cmd.args([
        "object",
        "read",
        "--repo",
        repo_arg,
        "--object-id",
        original["object_id"].as_str().unwrap(),
        "--json",
    ])
    .assert()
    .failure()
    .stdout(predicate::str::contains("ref-backed object"))
    .stderr(predicate::str::is_empty());
}

#[test]
fn object_write_requires_init() {
    let repo = tempfile::tempdir().unwrap();
    init_git_repo(repo.path());
    let body_file = fixture_path("work-intent.valid.json");

    let mut cmd = Command::cargo_bin("rickygit").unwrap();
    cmd.args([
        "object",
        "write",
        "--repo",
        repo.path().to_str().unwrap(),
        "--kind",
        "agent.intent",
        "--body-file",
        &body_file,
        "--json",
    ])
    .assert()
    .failure()
    .stdout(predicate::str::contains("not initialized"));
}

#[test]
fn object_verify_reports_tampered_cache() {
    let repo = tempfile::tempdir().unwrap();
    init_git_repo(repo.path());
    let repo_arg = repo.path().to_str().unwrap();
    rickygit_json(&["init", "--repo", repo_arg, "--json"]);
    let body_file = fixture_path("work-intent.valid.json");
    let write = rickygit_json(&[
        "object",
        "write",
        "--repo",
        repo_arg,
        "--kind",
        "agent.intent",
        "--body-file",
        &body_file,
        "--json",
    ]);
    let object_id = write["object_id"].as_str().unwrap();
    let cache_path = write["cache_path"].as_str().unwrap();

    std::fs::write(cache_path, br#"{"bad":true}"#).unwrap();
    let verify = rickygit_json(&[
        "object",
        "verify",
        "--repo",
        repo_arg,
        "--object-id",
        object_id,
        "--json",
    ]);

    assert_eq!(verify["valid"], false);
    assert_eq!(verify["diagnostics"][0]["code"], "OBJECT002");
}

#[test]
fn argument_errors_are_structured_json() {
    let mut cmd = Command::cargo_bin("rickygit").unwrap();
    cmd.args(["not-a-command", "--json"])
        .assert()
        .failure()
        .stdout(predicate::str::contains("\"kind\": \"argument_parse\""))
        .stdout(predicate::str::contains("\"status\": \"error\""))
        .stderr(predicate::str::is_empty());
}

#[test]
fn invalid_intent_fixture_emits_diagnostics() {
    let intent_path = fixture_path("work-intent.invalid.json");

    let mut cmd = Command::cargo_bin("rickygit").unwrap();
    cmd.args(["intent", "validate", &intent_path, "--json"])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"valid\": false"))
        .stdout(predicate::str::contains("\"code\": \"INTENT001\""));
}

#[test]
fn valid_intent_fixture_hash_is_stable() {
    let intent_path = fixture_path("work-intent.valid.json");

    let first = rickygit_json(&["intent", "hash", &intent_path, "--json"]);
    let second = rickygit_json(&["intent", "hash", &intent_path, "--json"]);

    assert_eq!(first, second);
    assert!(first["object_id"].as_str().unwrap().starts_with("sha256:"));
    assert!(first["body_hash"].as_str().unwrap().starts_with("sha256:"));
    assert_eq!(first["valid"], true);
    assert_eq!(first["diagnostics"].as_array().unwrap().len(), 0);
}

#[test]
fn invalid_intent_hash_includes_diagnostics() {
    let intent_path = fixture_path("work-intent.invalid.json");
    let output = rickygit_json(&["intent", "hash", &intent_path, "--json"]);

    assert_eq!(output["valid"], false);
    assert!(output["diagnostics"].as_array().unwrap().len() >= 3);
}

#[test]
fn intent_write_persists_valid_intent_as_agent_intent_object() {
    let repo = tempfile::tempdir().unwrap();
    init_git_repo(repo.path());
    let repo_arg = repo.path().to_str().unwrap();
    rickygit_json(&["init", "--repo", repo_arg, "--json"]);
    let intent_path = fixture_path("work-intent.valid.json");

    let write = rickygit_json(&[
        "intent",
        "write",
        "--repo",
        repo_arg,
        &intent_path,
        "--json",
    ]);
    let object = &write["object"];
    let object_id = object["object_id"].as_str().unwrap();
    let read = rickygit_json(&[
        "object",
        "read",
        "--repo",
        repo_arg,
        "--object-id",
        object_id,
        "--json",
    ]);

    assert_eq!(write["valid"], true);
    assert_eq!(write["diagnostics"].as_array().unwrap().len(), 0);
    assert_eq!(object["kind"], "agent.intent");
    assert_eq!(
        object_id,
        "sha256:a0c9cd2e0309cc8d965aebc13c2d8acf09b4b1df507b8e83374f0e1a538ff071"
    );
    assert_eq!(
        read["object"]["body"]["objective"],
        "Initialize rickydata_git with repo-native agent collaboration schemas."
    );
    assert_eq!(git_output(repo.path(), ["status", "--short"]), "");
}

#[test]
fn invalid_intent_write_is_diagnostic_only_and_does_not_initialize_store() {
    let repo = tempfile::tempdir().unwrap();
    init_git_repo(repo.path());
    let repo_arg = repo.path().to_str().unwrap();
    let intent_path = fixture_path("work-intent.invalid.json");

    let write = rickygit_json(&[
        "intent",
        "write",
        "--repo",
        repo_arg,
        &intent_path,
        "--json",
    ]);

    assert_eq!(write["valid"], false);
    assert!(write["object"].is_null());
    assert!(write["diagnostics"].as_array().unwrap().len() >= 3);
    assert!(!repo.path().join(".git/rickydata").exists());
    assert_eq!(git_output(repo.path(), ["status", "--short"]), "");
}

#[test]
fn valid_intent_write_requires_initialized_store() {
    let repo = tempfile::tempdir().unwrap();
    init_git_repo(repo.path());
    let intent_path = fixture_path("work-intent.valid.json");

    let mut cmd = Command::cargo_bin("rickygit").unwrap();
    cmd.args([
        "intent",
        "write",
        "--repo",
        repo.path().to_str().unwrap(),
        &intent_path,
        "--json",
    ])
    .assert()
    .failure()
    .stdout(predicate::str::contains("not initialized"));
}

#[test]
fn intent_list_and_show_return_repo_native_intents() {
    let repo = tempfile::tempdir().unwrap();
    init_git_repo(repo.path());
    let repo_arg = repo.path().to_str().unwrap();
    rickygit_json(&["init", "--repo", repo_arg, "--json"]);
    let intent_path = fixture_path("work-intent.valid.json");
    let write = rickygit_json(&[
        "intent",
        "write",
        "--repo",
        repo_arg,
        &intent_path,
        "--json",
    ]);
    let object_id = write["object"]["object_id"].as_str().unwrap();

    let list = rickygit_json(&["intent", "list", "--repo", repo_arg, "--json"]);
    let show = rickygit_json(&[
        "intent",
        "show",
        "--repo",
        repo_arg,
        "--object-id",
        object_id,
        "--json",
    ]);

    assert_eq!(list["intents"].as_array().unwrap().len(), 1);
    assert_eq!(list["intents"][0]["object_id"], object_id);
    assert_eq!(list["intents"][0]["kind"], "agent.intent");
    assert_eq!(show["object_id"], object_id);
    assert_eq!(show["source"], "cache");
    assert_eq!(show["valid"], true);
    assert_eq!(
        show["intent"]["issue_refs"][0]["repository"],
        "rickycambrian/rickydata-git"
    );
}

#[test]
fn issue_import_creates_repo_native_work_intent() {
    let repo = tempfile::tempdir().unwrap();
    init_git_repo(repo.path());
    create_initial_commit(repo.path());
    let repo_arg = repo.path().to_str().unwrap();
    rickygit_json(&["init", "--repo", repo_arg, "--json"]);

    let imported = rickygit_json(&[
        "issue",
        "import",
        "--repo",
        repo_arg,
        "--issue-repository",
        "rickycambrian/rickydata_code",
        "--issue-id",
        "42",
        "--objective",
        "Make issue import usable",
        "--url",
        "https://github.com/rickycambrian/rickydata_code/issues/42",
        "--created-by",
        "agent:test",
        "--json",
    ]);
    let object_id = imported["object"]["object_id"].as_str().unwrap();
    let shown = rickygit_json(&[
        "intent",
        "show",
        "--repo",
        repo_arg,
        "--object-id",
        object_id,
        "--json",
    ]);

    assert_eq!(imported["status"], "ok");
    assert_eq!(imported["valid"], true);
    assert_eq!(imported["intent"]["issue_refs"][0]["platform"], "github");
    assert_eq!(
        imported["intent"]["issue_refs"][0]["repository"],
        "rickycambrian/rickydata_code"
    );
    assert_eq!(imported["intent"]["issue_refs"][0]["id"], "42");
    assert_eq!(shown["valid"], true);
    assert_eq!(shown["intent"]["objective"], "Make issue import usable");
}

#[test]
fn work_start_imports_issue_and_starts_isolated_attempt() {
    let repo = tempfile::tempdir().unwrap();
    init_git_repo(repo.path());
    create_initial_commit(repo.path());
    let repo_arg = repo.path().to_str().unwrap();
    rickygit_json(&["init", "--repo", repo_arg, "--json"]);

    let started = rickygit_json(&[
        "work",
        "start",
        "--repo",
        repo_arg,
        "--issue-repository",
        "rickycambrian/rickydata_code",
        "--issue-id",
        "43",
        "--objective",
        "Start work from issue",
        "--agent-id",
        "agent:test",
        "--idempotency-key",
        "issue-43",
        "--json",
    ]);
    let intent_id = started["intent_object"]["object_id"].as_str().unwrap();
    let attempt_id = started["attempt"]["attempt_id"].as_str().unwrap();
    let intent_list = rickygit_json(&["intent", "list", "--repo", repo_arg, "--json"]);
    let attempt_status = rickygit_json(&[
        "attempt",
        "status",
        "--repo",
        repo_arg,
        "--attempt-id",
        attempt_id,
        "--json",
    ]);

    assert_eq!(started["status"], "ok");
    assert_eq!(started["intent"]["issue_refs"][0]["id"], "43");
    assert_eq!(started["attempt"]["intent_id"], intent_id);
    assert_eq!(started["attempt"]["status"], "running");
    assert_eq!(started["worktree_created"], true);
    assert_eq!(intent_list["intents"].as_array().unwrap().len(), 1);
    assert_eq!(attempt_status["status"], "running");
    assert_eq!(attempt_status["worktree_exists"], true);
}

#[test]
fn graph_scan_projects_repo_native_agent_objects() {
    let repo = tempfile::tempdir().unwrap();
    init_git_repo(repo.path());
    create_initial_commit(repo.path());
    let repo_arg = repo.path().to_str().unwrap();
    rickygit_json(&["init", "--repo", repo_arg, "--json"]);
    let started = rickygit_json(&[
        "work",
        "start",
        "--repo",
        repo_arg,
        "--objective",
        "Graph scan test",
        "--agent-id",
        "agent:test",
        "--idempotency-key",
        "graph-scan-test",
        "--json",
    ]);

    let scan = rickygit_json(&["graph", "scan", "--repo", repo_arg, "--json"]);

    assert_eq!(scan["status"], "ok");
    assert_eq!(scan["schema_version"], "rickydata.repo_execution_graph.v1");
    assert!(scan["node_count"].as_u64().unwrap() >= 3);
    assert!(scan["edge_count"].as_u64().unwrap() >= 2);
    assert!(scan["graph_hash"].as_str().unwrap().starts_with("sha256:"));
    let nodes = scan["nodes"].as_array().unwrap();
    assert!(nodes.iter().any(|node| node["label"] == "Repository"));
    assert!(
        nodes
            .iter()
            .any(|node| node["label"] == "RickydataWorkIntent")
    );
    assert!(nodes.iter().any(|node| node["label"] == "RickydataAttempt"));
    assert!(
        scan.to_string()
            .contains(started["attempt"]["attempt_id"].as_str().unwrap())
    );
}

#[test]
fn graph_scan_includes_lightweight_rust_code_structure_when_requested() {
    let repo = tempfile::tempdir().unwrap();
    init_git_repo(repo.path());
    std::fs::create_dir_all(repo.path().join("src")).unwrap();
    std::fs::write(
        repo.path().join("src/lib.rs"),
        r#"
pub struct Widget {
    name: String,
}

pub enum WidgetMode {
    Fast,
}

pub fn build_widget(name: String) -> Widget {
    Widget { name }
}

#[test]
fn builds_widget() {
    let _ = build_widget(String::from("demo"));
}
"#,
    )
    .unwrap();
    create_initial_commit(repo.path());
    let repo_arg = repo.path().to_str().unwrap();
    rickygit_json(&["init", "--repo", repo_arg, "--json"]);

    let scan = rickygit_json(&[
        "graph",
        "scan",
        "--repo",
        repo_arg,
        "--include-code-structure",
        "--json",
    ]);
    let projection = rickygit_json(&[
        "project-kfdb",
        "--repo",
        repo_arg,
        "--include-code-structure",
        "--dry-run",
        "--json",
    ]);

    assert_eq!(scan["status"], "ok");
    assert_eq!(scan["include_code_structure"], true);
    let nodes = scan["nodes"].as_array().unwrap();
    assert!(nodes.iter().any(|node| {
        node["label"] == "Function" && node["properties"]["name"] == "build_widget"
    }));
    assert!(nodes.iter().any(|node| {
        node["label"] == "TypeDefinition" && node["properties"]["name"] == "Widget"
    }));
    assert!(nodes.iter().any(|node| {
        node["label"] == "TypeDefinition" && node["properties"]["name"] == "WidgetMode"
    }));
    assert!(nodes.iter().any(|node| {
        node["label"] == "TestCase" && node["properties"]["name"] == "builds_widget"
    }));
    let edges = scan["edges"].as_array().unwrap();
    assert!(edges.iter().any(|edge| edge["edge_type"] == "DEFINES"));
    assert!(edges.iter().any(|edge| edge["edge_type"] == "TESTS"));
    assert_eq!(projection["status"], "ok");
    assert_eq!(projection["dry_run"], true);
    assert_eq!(projection["include_code_structure"], true);
    assert_eq!(
        projection["nodes_written"].as_u64().unwrap(),
        scan["node_count"].as_u64().unwrap()
    );
}

#[test]
fn impact_context_and_kfdb_projection_are_structured_json() {
    let repo = tempfile::tempdir().unwrap();
    init_git_repo(repo.path());
    create_initial_commit(repo.path());
    let repo_arg = repo.path().to_str().unwrap();
    rickygit_json(&["init", "--repo", repo_arg, "--json"]);
    let started = rickygit_json(&[
        "work",
        "start",
        "--repo",
        repo_arg,
        "--objective",
        "Assess src/main.rs impact",
        "--agent-id",
        "agent:test",
        "--idempotency-key",
        "graph-impact-test",
        "--json",
    ]);
    let attempt_id = started["attempt"]["attempt_id"].as_str().unwrap();

    let impact = rickygit_json(&[
        "impact",
        "--repo",
        repo_arg,
        "--attempt-id",
        attempt_id,
        "--changed-file",
        "src/main.rs",
        "--json",
    ]);
    let context = rickygit_json(&[
        "context", "--repo", repo_arg, "--query", "impact", "--limit", "5", "--json",
    ]);
    let projection = rickygit_json(&[
        "project-kfdb",
        "--repo",
        repo_arg,
        "--scope",
        "test-scope",
        "--dry-run",
        "--json",
    ]);

    assert_eq!(impact["status"], "ok");
    assert_eq!(
        impact["changed_files"].as_array().unwrap()[0],
        "src/main.rs"
    );
    assert!(
        impact["suggested_tests"]
            .as_array()
            .unwrap()
            .iter()
            .any(|value| value == "cargo test")
    );
    assert_eq!(context["status"], "ok");
    assert!(
        context["command_suggestions"]
            .as_array()
            .unwrap()
            .iter()
            .any(|value| value.as_str().unwrap().contains("graph scan"))
    );
    assert_eq!(projection["status"], "ok");
    assert_eq!(projection["dry_run"], true);
    assert_eq!(projection["scope"], "test-scope");
    assert!(projection["nodes_written"].as_u64().unwrap() >= 3);
    assert!(projection["edges_written"].as_u64().unwrap() >= 2);
    assert!(
        projection["projection_hash"]
            .as_str()
            .unwrap()
            .starts_with("sha256:")
    );
}

#[test]
fn live_kfdb_projection_requires_private_derive_headers_by_default() {
    let repo = tempfile::tempdir().unwrap();
    init_git_repo(repo.path());
    create_initial_commit(repo.path());
    let repo_arg = repo.path().to_str().unwrap();
    rickygit_json(&["init", "--repo", repo_arg, "--json"]);

    let mut cmd = Command::cargo_bin("rickygit").unwrap();
    cmd.args([
        "project-kfdb",
        "--repo",
        repo_arg,
        "--kfdb-url",
        "http://127.0.0.1:9",
        "--derive-session-id-env",
        "RICKYGIT_TEST_MISSING_DERIVE_SESSION",
        "--derive-key-env",
        "RICKYGIT_TEST_MISSING_DERIVE_KEY",
        "--json",
    ])
    .env_remove("RICKYGIT_TEST_MISSING_DERIVE_SESSION")
    .env_remove("RICKYGIT_TEST_MISSING_DERIVE_KEY")
    .assert()
    .failure()
    .stdout(predicate::str::contains(
        "live KFDB projection is private by default",
    ));
}

#[test]
fn live_kfdb_projection_chunks_payloads_and_uses_lowercase_rickydata_labels() {
    let repo = tempfile::tempdir().unwrap();
    init_git_repo(repo.path());
    std::fs::create_dir_all(repo.path().join("src")).unwrap();
    std::fs::write(
        repo.path().join("src/lib.rs"),
        "pub fn alpha() {}\npub fn beta() {}\n",
    )
    .unwrap();
    create_initial_commit(repo.path());
    let repo_arg = repo.path().to_str().unwrap();
    rickygit_json(&["init", "--repo", repo_arg, "--json"]);
    let kfdb = start_project_kfdb(StatusCode::OK, None);

    let projection = rickygit_json_env(
        &[
            "project-kfdb",
            "--repo",
            repo_arg,
            "--kfdb-url",
            &kfdb.url,
            "--api-key-env",
            "RICKYGIT_TEST_KFDB_TOKEN",
            "--derive-session-id-env",
            "RICKYGIT_TEST_DERIVE_SESSION",
            "--derive-key-env",
            "RICKYGIT_TEST_DERIVE_KEY",
            "--wallet-address-env",
            "RICKYGIT_TEST_WALLET",
            "--include-code-structure",
            "--batch-size",
            "3",
            "--json",
        ],
        &[
            ("RICKYGIT_TEST_KFDB_TOKEN", "test-token"),
            ("RICKYGIT_TEST_DERIVE_SESSION", "session-1"),
            ("RICKYGIT_TEST_DERIVE_KEY", "derive-key-1"),
            ("RICKYGIT_TEST_WALLET", "0xabc"),
        ],
    );

    assert_eq!(projection["status"], "ok");
    assert!(projection["batches_written"].as_u64().unwrap() > 1);
    assert_eq!(projection["batch_size"], 3);
    let requests = kfdb.requests.lock().unwrap();
    assert!(requests.len() > 1);
    for request in requests.iter() {
        assert_eq!(request["derive_session_id"], "session-1");
        assert_eq!(request["derive_key"], "derive-key-1");
        assert_eq!(request["wallet_address"], "0xabc");
        let operations = request["body"]["operations"].as_array().unwrap();
        assert!(operations.len() <= 3);
        for operation in operations {
            if operation["operation"] == "create_node" {
                let label = operation["label"].as_str().unwrap();
                assert!(
                    label.starts_with("rickydata"),
                    "private projection label should use lowercase rickydata prefix: {label}"
                );
                assert!(!label.starts_with("Rickydata"));
            }
        }
    }
}

#[test]
fn live_kfdb_projection_error_includes_response_body() {
    let repo = tempfile::tempdir().unwrap();
    init_git_repo(repo.path());
    create_initial_commit(repo.path());
    let repo_arg = repo.path().to_str().unwrap();
    rickygit_json(&["init", "--repo", repo_arg, "--json"]);
    let kfdb = start_project_kfdb(
        StatusCode::BAD_REQUEST,
        Some(serde_json::json!({"message":"schema rejected lowercase projection"})),
    );

    let mut cmd = Command::cargo_bin("rickygit").unwrap();
    cmd.args([
        "project-kfdb",
        "--repo",
        repo_arg,
        "--kfdb-url",
        &kfdb.url,
        "--derive-session-id-env",
        "RICKYGIT_TEST_DERIVE_SESSION",
        "--derive-key-env",
        "RICKYGIT_TEST_DERIVE_KEY",
        "--json",
    ])
    .env("RICKYGIT_TEST_DERIVE_SESSION", "session-1")
    .env("RICKYGIT_TEST_DERIVE_KEY", "derive-key-1")
    .assert()
    .failure()
    .stdout(predicate::str::contains(
        "schema rejected lowercase projection",
    ));
}

#[test]
fn intent_show_rejects_non_intent_object() {
    let repo = tempfile::tempdir().unwrap();
    init_git_repo(repo.path());
    let repo_arg = repo.path().to_str().unwrap();
    rickygit_json(&["init", "--repo", repo_arg, "--json"]);
    let body_file = fixture_path("work-intent.valid.json");
    let object = rickygit_json(&[
        "object",
        "write",
        "--repo",
        repo_arg,
        "--kind",
        "agent.run",
        "--body-file",
        &body_file,
        "--json",
    ]);

    let mut cmd = Command::cargo_bin("rickygit").unwrap();
    cmd.args([
        "intent",
        "show",
        "--repo",
        repo_arg,
        "--object-id",
        object["object_id"].as_str().unwrap(),
        "--json",
    ])
    .assert()
    .failure()
    .stdout(predicate::str::contains("expected `agent.intent`"));
}

#[test]
fn attempt_start_requires_existing_intent() {
    let repo = tempfile::tempdir().unwrap();
    init_git_repo(repo.path());
    create_initial_commit(repo.path());
    let repo_arg = repo.path().to_str().unwrap();
    rickygit_json(&["init", "--repo", repo_arg, "--json"]);

    let mut cmd = Command::cargo_bin("rickygit").unwrap();
    cmd.args([
        "attempt",
        "start",
        "--repo",
        repo_arg,
        "--intent-id",
        "sha256:0000000000000000000000000000000000000000000000000000000000000000",
        "--agent-id",
        "agent:test",
        "--idempotency-key",
        "first",
        "--json",
    ])
    .assert()
    .failure()
    .stdout(predicate::str::contains("cached object does not exist"));
}

#[test]
fn attempt_start_creates_hidden_worktree_and_is_idempotent() {
    let repo = tempfile::tempdir().unwrap();
    init_git_repo(repo.path());
    let base_commit = create_initial_commit(repo.path());
    let repo_arg = repo.path().to_str().unwrap();
    rickygit_json(&["init", "--repo", repo_arg, "--json"]);
    let intent = rickygit_json(&[
        "intent",
        "write",
        "--repo",
        repo_arg,
        &fixture_path("work-intent.valid.json"),
        "--json",
    ]);
    let intent_id = intent["object"]["object_id"].as_str().unwrap();

    let first = rickygit_json(&[
        "attempt",
        "start",
        "--repo",
        repo_arg,
        "--intent-id",
        intent_id,
        "--agent-id",
        "agent:test",
        "--idempotency-key",
        "first",
        "--json",
    ]);
    let duplicate = rickygit_json(&[
        "attempt",
        "start",
        "--repo",
        repo_arg,
        "--intent-id",
        intent_id,
        "--agent-id",
        "agent:test",
        "--idempotency-key",
        "first",
        "--json",
    ]);
    let second = rickygit_json(&[
        "attempt",
        "start",
        "--repo",
        repo_arg,
        "--intent-id",
        intent_id,
        "--agent-id",
        "agent:test",
        "--idempotency-key",
        "second",
        "--json",
    ]);

    assert_eq!(first["attempt"]["intent_id"], intent_id);
    assert_eq!(first["attempt"]["agent_id"], "agent:test");
    assert_eq!(first["attempt"]["base_commit"], base_commit);
    assert_eq!(first["attempt"]["status"], "running");
    assert_eq!(first["object"]["kind"], "agent.attempt");
    assert_eq!(first["worktree_created"], true);
    assert!(std::path::Path::new(first["local_worktree_path"].as_str().unwrap()).is_dir());
    assert_eq!(
        duplicate["attempt"]["attempt_id"],
        first["attempt"]["attempt_id"]
    );
    assert_eq!(duplicate["object"]["status"], "already_exists");
    assert_eq!(duplicate["worktree_created"], false);
    assert_ne!(
        second["attempt"]["attempt_id"],
        first["attempt"]["attempt_id"]
    );
    assert!(std::path::Path::new(second["local_worktree_path"].as_str().unwrap()).is_dir());
    assert_eq!(git_output(repo.path(), ["status", "--short"]), "");
    git_success(repo.path(), ["fsck"]);
}

#[test]
fn attempt_list_and_show_return_repo_native_attempts() {
    let repo = tempfile::tempdir().unwrap();
    init_git_repo(repo.path());
    create_initial_commit(repo.path());
    let repo_arg = repo.path().to_str().unwrap();
    rickygit_json(&["init", "--repo", repo_arg, "--json"]);
    let intent = rickygit_json(&[
        "intent",
        "write",
        "--repo",
        repo_arg,
        &fixture_path("work-intent.valid.json"),
        "--json",
    ]);
    let intent_id = intent["object"]["object_id"].as_str().unwrap();
    let attempt = rickygit_json(&[
        "attempt",
        "start",
        "--repo",
        repo_arg,
        "--intent-id",
        intent_id,
        "--agent-id",
        "agent:test",
        "--idempotency-key",
        "first",
        "--json",
    ]);
    let attempt_id = attempt["attempt"]["attempt_id"].as_str().unwrap();

    let list = rickygit_json(&["attempt", "list", "--repo", repo_arg, "--json"]);
    let show = rickygit_json(&[
        "attempt",
        "show",
        "--repo",
        repo_arg,
        "--attempt-id",
        attempt_id,
        "--json",
    ]);

    assert_eq!(list["attempts"].as_array().unwrap().len(), 1);
    assert_eq!(list["attempts"][0]["attempt"]["attempt_id"], attempt_id);
    assert_eq!(list["attempts"][0]["attempt"]["intent_id"], intent_id);
    assert_eq!(show["attempt"]["attempt_id"], attempt_id);
    assert_eq!(show["attempt"]["status"], "running");
    assert!(show["local_worktree_path"].is_null());
}

#[test]
fn attempt_show_reports_missing_attempt_id() {
    let repo = tempfile::tempdir().unwrap();
    init_git_repo(repo.path());
    create_initial_commit(repo.path());
    let repo_arg = repo.path().to_str().unwrap();
    rickygit_json(&["init", "--repo", repo_arg, "--json"]);

    let mut cmd = Command::cargo_bin("rickygit").unwrap();
    cmd.args([
        "attempt",
        "show",
        "--repo",
        repo_arg,
        "--attempt-id",
        "sha256:0000000000000000000000000000000000000000000000000000000000000000",
        "--json",
    ])
    .assert()
    .failure()
    .stdout(predicate::str::contains("was not found"));
}

#[test]
fn run_exec_runs_inside_attempt_worktree_and_writes_agent_run() {
    let repo = tempfile::tempdir().unwrap();
    init_git_repo(repo.path());
    create_initial_commit(repo.path());
    let repo_arg = repo.path().to_str().unwrap();
    rickygit_json(&["init", "--repo", repo_arg, "--json"]);
    let intent = rickygit_json(&[
        "intent",
        "write",
        "--repo",
        repo_arg,
        &fixture_path("work-intent.valid.json"),
        "--json",
    ]);
    let attempt = rickygit_json(&[
        "attempt",
        "start",
        "--repo",
        repo_arg,
        "--intent-id",
        intent["object"]["object_id"].as_str().unwrap(),
        "--agent-id",
        "agent:test",
        "--idempotency-key",
        "first",
        "--json",
    ]);
    let attempt_id = attempt["attempt"]["attempt_id"].as_str().unwrap();
    let worktree_path = attempt["local_worktree_path"].as_str().unwrap();

    let run = rickygit_json(&[
        "run",
        "exec",
        "--repo",
        repo_arg,
        "--attempt-id",
        attempt_id,
        "--json",
        "--",
        "sh",
        "-c",
        "printf private-output && printf private-error >&2 && echo changed > run-output.txt",
    ]);

    assert_eq!(run["exit_code"], 0);
    assert_eq!(run["run"]["attempt_id"], attempt_id);
    assert_eq!(run["run"]["result"], "succeeded");
    assert_eq!(run["object"]["kind"], "agent.run");
    assert_eq!(run["trace_object"]["kind"], "agent.run_trace");
    assert!(run["command_hash"].as_str().unwrap().starts_with("sha256:"));
    assert!(run["stdout_hash"].as_str().unwrap().starts_with("sha256:"));
    assert!(run["stderr_hash"].as_str().unwrap().starts_with("sha256:"));
    assert_eq!(run["stdout_bytes"], 14);
    assert_eq!(run["stderr_bytes"], 13);
    assert!(
        std::path::Path::new(worktree_path)
            .join("run-output.txt")
            .exists()
    );
    assert!(!repo.path().join("run-output.txt").exists());
    let trace = rickygit_json(&[
        "object",
        "read",
        "--repo",
        repo_arg,
        "--object-id",
        run["trace_object"]["object_id"].as_str().unwrap(),
        "--json",
    ]);
    let trace_body = &trace["object"]["body"];
    assert_eq!(trace_body["trace_id"], run["run"]["trace_hash"]);
    assert_eq!(trace_body["attempt_id"], attempt_id);
    assert!(trace_body["command_argv"].is_null());
    assert_eq!(trace_body["executable"], "sh");
    assert_eq!(trace_body["arg_count"], 3);
    assert_eq!(trace_body["privacy"], "public_metadata");
    assert!(
        !serde_json::to_string(trace_body)
            .unwrap()
            .contains("private-output")
    );
    assert_eq!(git_output(repo.path(), ["status", "--short"]), "");
    git_success(repo.path(), ["fsck"]);
}

#[test]
fn run_exec_records_failed_command_as_structured_run() {
    let repo = tempfile::tempdir().unwrap();
    init_git_repo(repo.path());
    create_initial_commit(repo.path());
    let repo_arg = repo.path().to_str().unwrap();
    rickygit_json(&["init", "--repo", repo_arg, "--json"]);
    let intent = rickygit_json(&[
        "intent",
        "write",
        "--repo",
        repo_arg,
        &fixture_path("work-intent.valid.json"),
        "--json",
    ]);
    let attempt = rickygit_json(&[
        "attempt",
        "start",
        "--repo",
        repo_arg,
        "--intent-id",
        intent["object"]["object_id"].as_str().unwrap(),
        "--agent-id",
        "agent:test",
        "--idempotency-key",
        "first",
        "--json",
    ]);
    let attempt_id = attempt["attempt"]["attempt_id"].as_str().unwrap();

    let mut cmd = Command::cargo_bin("rickygit").unwrap();
    let output = cmd
        .args([
            "run",
            "exec",
            "--repo",
            repo_arg,
            "--attempt-id",
            attempt_id,
            "--json",
            "--",
            "sh",
            "-c",
            "printf raw-secret; exit 7",
        ])
        .output()
        .unwrap();
    assert!(output.status.success());
    assert!(!String::from_utf8_lossy(&output.stdout).contains("raw-secret"));
    let run: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();

    assert_eq!(run["exit_code"], 7);
    assert_eq!(run["run"]["result"], "failed");
    assert_eq!(run["stdout_bytes"], 10);
    assert_eq!(run["stderr_bytes"], 0);
    assert_eq!(run["object"]["kind"], "agent.run");
}

#[test]
fn run_exec_requires_existing_attempt() {
    let repo = tempfile::tempdir().unwrap();
    init_git_repo(repo.path());
    create_initial_commit(repo.path());
    let repo_arg = repo.path().to_str().unwrap();
    rickygit_json(&["init", "--repo", repo_arg, "--json"]);

    let mut cmd = Command::cargo_bin("rickygit").unwrap();
    cmd.args([
        "run",
        "exec",
        "--repo",
        repo_arg,
        "--attempt-id",
        "sha256:0000000000000000000000000000000000000000000000000000000000000000",
        "--json",
        "--",
        "sh",
        "-c",
        "true",
    ])
    .assert()
    .failure()
    .stdout(predicate::str::contains("was not found"));
}

#[test]
fn run_list_and_show_return_repo_native_runs() {
    let repo = tempfile::tempdir().unwrap();
    init_git_repo(repo.path());
    create_initial_commit(repo.path());
    let repo_arg = repo.path().to_str().unwrap();
    rickygit_json(&["init", "--repo", repo_arg, "--json"]);
    let intent = rickygit_json(&[
        "intent",
        "write",
        "--repo",
        repo_arg,
        &fixture_path("work-intent.valid.json"),
        "--json",
    ]);
    let attempt = rickygit_json(&[
        "attempt",
        "start",
        "--repo",
        repo_arg,
        "--intent-id",
        intent["object"]["object_id"].as_str().unwrap(),
        "--agent-id",
        "agent:test",
        "--idempotency-key",
        "first",
        "--json",
    ]);
    let attempt_id = attempt["attempt"]["attempt_id"].as_str().unwrap();
    let run = rickygit_json(&[
        "run",
        "exec",
        "--repo",
        repo_arg,
        "--attempt-id",
        attempt_id,
        "--json",
        "--",
        "sh",
        "-c",
        "true",
    ]);
    let run_id = run["run"]["run_id"].as_str().unwrap();

    let list = rickygit_json(&["run", "list", "--repo", repo_arg, "--json"]);
    let show = rickygit_json(&[
        "run", "show", "--repo", repo_arg, "--run-id", run_id, "--json",
    ]);

    assert_eq!(list["runs"].as_array().unwrap().len(), 1);
    assert_eq!(list["runs"][0]["run"]["run_id"], run_id);
    assert_eq!(list["runs"][0]["run"]["attempt_id"], attempt_id);
    assert_eq!(show["run"]["run_id"], run_id);
    assert_eq!(show["run"]["result"], "succeeded");
    assert!(show["stdout_hash"].is_null());
}

#[test]
fn run_show_reports_missing_run_id() {
    let repo = tempfile::tempdir().unwrap();
    init_git_repo(repo.path());
    create_initial_commit(repo.path());
    let repo_arg = repo.path().to_str().unwrap();
    rickygit_json(&["init", "--repo", repo_arg, "--json"]);

    let mut cmd = Command::cargo_bin("rickygit").unwrap();
    cmd.args([
        "run",
        "show",
        "--repo",
        repo_arg,
        "--run-id",
        "sha256:0000000000000000000000000000000000000000000000000000000000000000",
        "--json",
    ])
    .assert()
    .failure()
    .stdout(predicate::str::contains("was not found"));
}

#[test]
fn change_detect_writes_evidence_for_attempt_worktree_changes() {
    let repo = tempfile::tempdir().unwrap();
    init_git_repo(repo.path());
    create_initial_commit(repo.path());
    let repo_arg = repo.path().to_str().unwrap();
    rickygit_json(&["init", "--repo", repo_arg, "--json"]);
    let intent = rickygit_json(&[
        "intent",
        "write",
        "--repo",
        repo_arg,
        &fixture_path("work-intent.valid.json"),
        "--json",
    ]);
    let intent_id = intent["object"]["object_id"].as_str().unwrap();
    let attempt = rickygit_json(&[
        "attempt",
        "start",
        "--repo",
        repo_arg,
        "--intent-id",
        intent_id,
        "--agent-id",
        "agent:test",
        "--idempotency-key",
        "first",
        "--json",
    ]);
    let attempt_id = attempt["attempt"]["attempt_id"].as_str().unwrap();
    let run = rickygit_json(&[
        "run",
        "exec",
        "--repo",
        repo_arg,
        "--attempt-id",
        attempt_id,
        "--json",
        "--",
        "sh",
        "-c",
        "printf '\\nchanged\\n' >> README.md && printf 'new\\n' > generated.txt",
    ]);
    let run_id = run["run"]["run_id"].as_str().unwrap();

    let change = rickygit_json(&[
        "change",
        "detect",
        "--repo",
        repo_arg,
        "--attempt-id",
        attempt_id,
        "--json",
    ]);
    let change_object = rickygit_json(&[
        "object",
        "read",
        "--repo",
        repo_arg,
        "--object-id",
        change["object"]["object_id"].as_str().unwrap(),
        "--json",
    ]);

    assert_eq!(change["change"]["intent_id"], intent_id);
    assert_eq!(change["change"]["attempt_id"], attempt_id);
    assert_eq!(change["change"]["run_ids"][0], run_id);
    assert!(
        change["change"]["change_id"]
            .as_str()
            .unwrap()
            .starts_with("sha256:")
    );
    assert!(
        change["change"]["diff_hash"]
            .as_str()
            .unwrap()
            .starts_with("sha256:")
    );
    assert_eq!(change["changed"], true);
    assert!(change["diff_bytes"].as_u64().unwrap() > 0);
    assert!(
        change["change"]["file_paths"]
            .as_array()
            .unwrap()
            .iter()
            .any(|path| path == "README.md")
    );
    assert!(
        change["change"]["file_paths"]
            .as_array()
            .unwrap()
            .iter()
            .any(|path| path == "generated.txt")
    );
    assert_eq!(change["object"]["kind"], "agent.change");
    assert_eq!(change_object["object"]["kind"], "agent.change");
    assert!(change["raw_diff"].is_null());
    assert_eq!(git_output(repo.path(), ["status", "--short"]), "");
    git_success(repo.path(), ["fsck"]);
}

#[test]
fn change_detect_is_deterministic_for_same_attempt_diff() {
    let repo = tempfile::tempdir().unwrap();
    init_git_repo(repo.path());
    create_initial_commit(repo.path());
    let repo_arg = repo.path().to_str().unwrap();
    rickygit_json(&["init", "--repo", repo_arg, "--json"]);
    let intent = rickygit_json(&[
        "intent",
        "write",
        "--repo",
        repo_arg,
        &fixture_path("work-intent.valid.json"),
        "--json",
    ]);
    let attempt = rickygit_json(&[
        "attempt",
        "start",
        "--repo",
        repo_arg,
        "--intent-id",
        intent["object"]["object_id"].as_str().unwrap(),
        "--agent-id",
        "agent:test",
        "--idempotency-key",
        "first",
        "--json",
    ]);
    let attempt_id = attempt["attempt"]["attempt_id"].as_str().unwrap();
    rickygit_json(&[
        "run",
        "exec",
        "--repo",
        repo_arg,
        "--attempt-id",
        attempt_id,
        "--json",
        "--",
        "sh",
        "-c",
        "printf 'new\\n' > generated.txt",
    ]);

    let first = rickygit_json(&[
        "change",
        "detect",
        "--repo",
        repo_arg,
        "--attempt-id",
        attempt_id,
        "--json",
    ]);
    let second = rickygit_json(&[
        "change",
        "detect",
        "--repo",
        repo_arg,
        "--attempt-id",
        attempt_id,
        "--json",
    ]);

    assert_eq!(first["change"]["change_id"], second["change"]["change_id"]);
    assert_eq!(first["change"]["diff_hash"], second["change"]["diff_hash"]);
    assert_eq!(first["object"]["object_id"], second["object"]["object_id"]);
    assert_eq!(second["object"]["status"], "already_exists");
}

#[test]
fn change_detect_requires_existing_attempt() {
    let repo = tempfile::tempdir().unwrap();
    init_git_repo(repo.path());
    create_initial_commit(repo.path());
    let repo_arg = repo.path().to_str().unwrap();
    rickygit_json(&["init", "--repo", repo_arg, "--json"]);

    let mut cmd = Command::cargo_bin("rickygit").unwrap();
    cmd.args([
        "change",
        "detect",
        "--repo",
        repo_arg,
        "--attempt-id",
        "sha256:0000000000000000000000000000000000000000000000000000000000000000",
        "--json",
    ])
    .assert()
    .failure()
    .stdout(predicate::str::contains("was not found"));
}

#[test]
fn change_detect_rejects_noop_attempt_without_writing_change() {
    let repo = tempfile::tempdir().unwrap();
    init_git_repo(repo.path());
    create_initial_commit(repo.path());
    let repo_arg = repo.path().to_str().unwrap();
    rickygit_json(&["init", "--repo", repo_arg, "--json"]);
    let intent = rickygit_json(&[
        "intent",
        "write",
        "--repo",
        repo_arg,
        &fixture_path("work-intent.valid.json"),
        "--json",
    ]);
    let attempt = rickygit_json(&[
        "attempt",
        "start",
        "--repo",
        repo_arg,
        "--intent-id",
        intent["object"]["object_id"].as_str().unwrap(),
        "--agent-id",
        "agent:test",
        "--idempotency-key",
        "noop",
        "--json",
    ]);
    let attempt_id = attempt["attempt"]["attempt_id"].as_str().unwrap();

    let before = rickygit_json(&["change", "list", "--repo", repo_arg, "--json"]);
    assert_eq!(before["changes"].as_array().unwrap().len(), 0);

    let mut cmd = Command::cargo_bin("rickygit").unwrap();
    cmd.args([
        "change",
        "detect",
        "--repo",
        repo_arg,
        "--attempt-id",
        attempt_id,
        "--json",
    ])
    .assert()
    .failure()
    .stdout(predicate::str::contains(
        "refusing to write empty agent.change evidence",
    ));

    let after = rickygit_json(&["change", "list", "--repo", repo_arg, "--json"]);
    assert_eq!(after["changes"].as_array().unwrap().len(), 0);
}

#[test]
fn change_list_and_show_return_repo_native_changes() {
    let repo = tempfile::tempdir().unwrap();
    init_git_repo(repo.path());
    create_initial_commit(repo.path());
    let repo_arg = repo.path().to_str().unwrap();
    rickygit_json(&["init", "--repo", repo_arg, "--json"]);
    let intent = rickygit_json(&[
        "intent",
        "write",
        "--repo",
        repo_arg,
        &fixture_path("work-intent.valid.json"),
        "--json",
    ]);
    let intent_id = intent["object"]["object_id"].as_str().unwrap();
    let attempt = rickygit_json(&[
        "attempt",
        "start",
        "--repo",
        repo_arg,
        "--intent-id",
        intent_id,
        "--agent-id",
        "agent:test",
        "--idempotency-key",
        "first",
        "--json",
    ]);
    let attempt_id = attempt["attempt"]["attempt_id"].as_str().unwrap();
    rickygit_json(&[
        "run",
        "exec",
        "--repo",
        repo_arg,
        "--attempt-id",
        attempt_id,
        "--json",
        "--",
        "sh",
        "-c",
        "printf 'new\\n' > generated.txt",
    ]);
    let change = rickygit_json(&[
        "change",
        "detect",
        "--repo",
        repo_arg,
        "--attempt-id",
        attempt_id,
        "--json",
    ]);
    let change_id = change["change"]["change_id"].as_str().unwrap();

    let list = rickygit_json(&["change", "list", "--repo", repo_arg, "--json"]);
    let show = rickygit_json(&[
        "change",
        "show",
        "--repo",
        repo_arg,
        "--change-id",
        change_id,
        "--json",
    ]);

    assert_eq!(list["changes"].as_array().unwrap().len(), 1);
    assert_eq!(list["changes"][0]["change"]["change_id"], change_id);
    assert_eq!(list["changes"][0]["change"]["intent_id"], intent_id);
    assert_eq!(list["changes"][0]["change"]["attempt_id"], attempt_id);
    assert_eq!(show["change"]["change_id"], change_id);
    assert_eq!(show["change"]["file_paths"][0], "generated.txt");
    assert!(show["raw_diff"].is_null());
}

#[test]
fn change_show_reports_missing_change_id() {
    let repo = tempfile::tempdir().unwrap();
    init_git_repo(repo.path());
    create_initial_commit(repo.path());
    let repo_arg = repo.path().to_str().unwrap();
    rickygit_json(&["init", "--repo", repo_arg, "--json"]);

    let mut cmd = Command::cargo_bin("rickygit").unwrap();
    cmd.args([
        "change",
        "show",
        "--repo",
        repo_arg,
        "--change-id",
        "sha256:0000000000000000000000000000000000000000000000000000000000000000",
        "--json",
    ])
    .assert()
    .failure()
    .stdout(predicate::str::contains("was not found"));
}

#[test]
fn change_list_and_show_work_after_pack_refs_without_cache() {
    let repo = tempfile::tempdir().unwrap();
    init_git_repo(repo.path());
    create_initial_commit(repo.path());
    let repo_arg = repo.path().to_str().unwrap();
    rickygit_json(&["init", "--repo", repo_arg, "--json"]);
    let intent = rickygit_json(&[
        "intent",
        "write",
        "--repo",
        repo_arg,
        &fixture_path("work-intent.valid.json"),
        "--json",
    ]);
    let attempt = rickygit_json(&[
        "attempt",
        "start",
        "--repo",
        repo_arg,
        "--intent-id",
        intent["object"]["object_id"].as_str().unwrap(),
        "--agent-id",
        "agent:test",
        "--idempotency-key",
        "first",
        "--json",
    ]);
    let attempt_id = attempt["attempt"]["attempt_id"].as_str().unwrap();
    rickygit_json(&[
        "run",
        "exec",
        "--repo",
        repo_arg,
        "--attempt-id",
        attempt_id,
        "--json",
        "--",
        "sh",
        "-c",
        "printf 'new\\n' > generated.txt",
    ]);
    let change = rickygit_json(&[
        "change",
        "detect",
        "--repo",
        repo_arg,
        "--attempt-id",
        attempt_id,
        "--json",
    ]);
    let change_id = change["change"]["change_id"].as_str().unwrap();

    git_success(repo.path(), ["pack-refs", "--all", "--prune"]);
    std::fs::remove_dir_all(repo.path().join(".git/rickydata/cache/objects")).unwrap();
    let list = rickygit_json(&["change", "list", "--repo", repo_arg, "--json"]);
    let show = rickygit_json(&[
        "change",
        "show",
        "--repo",
        repo_arg,
        "--change-id",
        change_id,
        "--json",
    ]);

    assert_eq!(list["changes"].as_array().unwrap().len(), 1);
    assert_eq!(list["changes"][0]["change"]["change_id"], change_id);
    assert_eq!(show["change"]["change_id"], change_id);
    assert_eq!(show["change"]["file_paths"][0], "generated.txt");
    git_success(repo.path(), ["fsck"]);
}

#[test]
fn patch_prepare_writes_repo_native_patch_summary() {
    let repo = tempfile::tempdir().unwrap();
    init_git_repo(repo.path());
    create_initial_commit(repo.path());
    let repo_arg = repo.path().to_str().unwrap();
    rickygit_json(&["init", "--repo", repo_arg, "--json"]);
    let intent = rickygit_json(&[
        "intent",
        "write",
        "--repo",
        repo_arg,
        &fixture_path("work-intent.valid.json"),
        "--json",
    ]);
    let intent_id = intent["object"]["object_id"].as_str().unwrap();
    let attempt = rickygit_json(&[
        "attempt",
        "start",
        "--repo",
        repo_arg,
        "--intent-id",
        intent_id,
        "--agent-id",
        "agent:test",
        "--idempotency-key",
        "first",
        "--json",
    ]);
    let attempt_id = attempt["attempt"]["attempt_id"].as_str().unwrap();
    let run = rickygit_json(&[
        "run",
        "exec",
        "--repo",
        repo_arg,
        "--attempt-id",
        attempt_id,
        "--json",
        "--",
        "sh",
        "-c",
        "printf 'new\\n' > generated.txt",
    ]);
    let run_id = run["run"]["run_id"].as_str().unwrap();
    let change = rickygit_json(&[
        "change",
        "detect",
        "--repo",
        repo_arg,
        "--attempt-id",
        attempt_id,
        "--json",
    ]);
    let change_id = change["change"]["change_id"].as_str().unwrap();

    let patch = rickygit_json(&[
        "patch",
        "prepare",
        "--repo",
        repo_arg,
        "--attempt-id",
        attempt_id,
        "--json",
    ]);
    let duplicate = rickygit_json(&[
        "patch",
        "prepare",
        "--repo",
        repo_arg,
        "--attempt-id",
        attempt_id,
        "--json",
    ]);
    let patch_object = rickygit_json(&[
        "object",
        "read",
        "--repo",
        repo_arg,
        "--object-id",
        patch["object"]["object_id"].as_str().unwrap(),
        "--json",
    ]);

    assert_eq!(patch["patch"]["intent_id"], intent_id);
    assert_eq!(patch["patch"]["attempt_id"], attempt_id);
    assert_eq!(patch["patch"]["change_ids"][0], change_id);
    assert_eq!(patch["patch"]["run_ids"][0], run_id);
    assert_eq!(patch["patch"]["file_paths"][0], "generated.txt");
    assert_eq!(change["change"]["diff_summary"]["file_count"], 1);
    assert_eq!(change["change"]["diff_summary"]["files_added"], 1);
    assert_eq!(change["change"]["diff_summary"]["insertions"], 1);
    assert_eq!(change["change"]["diff_summary"]["deletions"], 0);
    assert!(
        patch["patch"]["diff_hashes"][0]
            .as_str()
            .unwrap()
            .starts_with("sha256:")
    );
    assert_eq!(patch["object"]["kind"], "agent.patch");
    assert_eq!(patch["change_count"], 1);
    assert_eq!(patch["file_count"], 1);
    assert_eq!(duplicate["patch"]["patch_id"], patch["patch"]["patch_id"]);
    assert_eq!(duplicate["object"]["status"], "already_exists");
    assert_eq!(patch_object["object"]["kind"], "agent.patch");
    assert!(patch["raw_diff"].is_null());
    assert_eq!(git_output(repo.path(), ["status", "--short"]), "");
    git_success(repo.path(), ["fsck"]);
}

#[test]
fn patch_prepare_requires_existing_change_evidence() {
    let repo = tempfile::tempdir().unwrap();
    init_git_repo(repo.path());
    create_initial_commit(repo.path());
    let repo_arg = repo.path().to_str().unwrap();
    rickygit_json(&["init", "--repo", repo_arg, "--json"]);
    let intent = rickygit_json(&[
        "intent",
        "write",
        "--repo",
        repo_arg,
        &fixture_path("work-intent.valid.json"),
        "--json",
    ]);
    let attempt = rickygit_json(&[
        "attempt",
        "start",
        "--repo",
        repo_arg,
        "--intent-id",
        intent["object"]["object_id"].as_str().unwrap(),
        "--agent-id",
        "agent:test",
        "--idempotency-key",
        "first",
        "--json",
    ]);

    let mut cmd = Command::cargo_bin("rickygit").unwrap();
    cmd.args([
        "patch",
        "prepare",
        "--repo",
        repo_arg,
        "--attempt-id",
        attempt["attempt"]["attempt_id"].as_str().unwrap(),
        "--json",
    ])
    .assert()
    .failure()
    .stdout(predicate::str::contains("no change evidence"));
}

#[test]
fn patch_list_and_show_return_repo_native_patches() {
    let repo = tempfile::tempdir().unwrap();
    init_git_repo(repo.path());
    create_initial_commit(repo.path());
    let repo_arg = repo.path().to_str().unwrap();
    rickygit_json(&["init", "--repo", repo_arg, "--json"]);
    let intent = rickygit_json(&[
        "intent",
        "write",
        "--repo",
        repo_arg,
        &fixture_path("work-intent.valid.json"),
        "--json",
    ]);
    let attempt = rickygit_json(&[
        "attempt",
        "start",
        "--repo",
        repo_arg,
        "--intent-id",
        intent["object"]["object_id"].as_str().unwrap(),
        "--agent-id",
        "agent:test",
        "--idempotency-key",
        "first",
        "--json",
    ]);
    let attempt_id = attempt["attempt"]["attempt_id"].as_str().unwrap();
    rickygit_json(&[
        "run",
        "exec",
        "--repo",
        repo_arg,
        "--attempt-id",
        attempt_id,
        "--json",
        "--",
        "sh",
        "-c",
        "printf 'new\\n' > generated.txt",
    ]);
    rickygit_json(&[
        "change",
        "detect",
        "--repo",
        repo_arg,
        "--attempt-id",
        attempt_id,
        "--json",
    ]);
    let patch = rickygit_json(&[
        "patch",
        "prepare",
        "--repo",
        repo_arg,
        "--attempt-id",
        attempt_id,
        "--json",
    ]);
    let patch_id = patch["patch"]["patch_id"].as_str().unwrap();

    let list = rickygit_json(&["patch", "list", "--repo", repo_arg, "--json"]);
    let show = rickygit_json(&[
        "patch",
        "show",
        "--repo",
        repo_arg,
        "--patch-id",
        patch_id,
        "--json",
    ]);

    assert_eq!(list["patches"].as_array().unwrap().len(), 1);
    assert_eq!(list["patches"][0]["patch"]["patch_id"], patch_id);
    assert_eq!(list["patches"][0]["patch"]["attempt_id"], attempt_id);
    assert_eq!(show["patch"]["patch_id"], patch_id);
    assert_eq!(show["patch"]["file_paths"][0], "generated.txt");
    assert!(show["raw_diff"].is_null());
}

struct PreparedPatchFixture {
    repo: tempfile::TempDir,
    repo_arg: String,
    attempt_id: String,
    patch_id: String,
    base_commit: String,
    diff_hash: String,
}

fn prepare_generated_patch_fixture() -> PreparedPatchFixture {
    let repo = tempfile::tempdir().unwrap();
    init_git_repo(repo.path());
    create_initial_commit(repo.path());
    let base_commit = git_output(repo.path(), ["rev-parse", "HEAD"])
        .trim()
        .to_string();
    let repo_arg = repo.path().to_str().unwrap().to_string();
    rickygit_json(&["init", "--repo", &repo_arg, "--json"]);
    let intent = rickygit_json(&[
        "intent",
        "write",
        "--repo",
        &repo_arg,
        &fixture_path("work-intent.valid.json"),
        "--json",
    ]);
    let attempt = rickygit_json(&[
        "attempt",
        "start",
        "--repo",
        &repo_arg,
        "--intent-id",
        intent["object"]["object_id"].as_str().unwrap(),
        "--agent-id",
        "agent:test",
        "--idempotency-key",
        "first",
        "--json",
    ]);
    let attempt_id = attempt["attempt"]["attempt_id"]
        .as_str()
        .unwrap()
        .to_string();
    rickygit_json(&[
        "run",
        "exec",
        "--repo",
        &repo_arg,
        "--attempt-id",
        &attempt_id,
        "--json",
        "--",
        "sh",
        "-c",
        "printf 'new\\n' > generated.txt",
    ]);
    rickygit_json(&[
        "change",
        "detect",
        "--repo",
        &repo_arg,
        "--attempt-id",
        &attempt_id,
        "--json",
    ]);
    let patch = rickygit_json(&[
        "patch",
        "prepare",
        "--repo",
        &repo_arg,
        "--attempt-id",
        &attempt_id,
        "--json",
    ]);
    let patch_id = patch["patch"]["patch_id"].as_str().unwrap().to_string();
    let diff_hash = patch["patch"]["diff_hashes"][0]
        .as_str()
        .unwrap()
        .to_string();

    PreparedPatchFixture {
        repo,
        repo_arg,
        attempt_id,
        patch_id,
        base_commit,
        diff_hash,
    }
}

#[test]
fn attempt_status_submit_and_review_queue_report_lifecycle() {
    let fixture = prepare_generated_patch_fixture();
    let key_dir = tempfile::tempdir().unwrap();
    let key_path = key_dir.path().join("submitter.key");
    let key_arg = key_path.to_str().unwrap();
    rickygit_json(&["key", "generate", "--output", key_arg, "--json"]);

    let status = rickygit_json(&[
        "attempt",
        "status",
        "--repo",
        &fixture.repo_arg,
        "--attempt-id",
        &fixture.attempt_id,
        "--json",
    ]);
    let queue_before = rickygit_json(&[
        "patch",
        "review-queue",
        "--repo",
        &fixture.repo_arg,
        "--json",
    ]);
    let submit = rickygit_json(&[
        "attempt",
        "submit",
        "--repo",
        &fixture.repo_arg,
        "--attempt-id",
        &fixture.attempt_id,
        "--reason",
        "ready for review",
        "--by",
        "agent:test",
        "--signing-key-file",
        key_arg,
        "--json",
    ]);
    let status_after = rickygit_json(&[
        "attempt",
        "status",
        "--repo",
        &fixture.repo_arg,
        "--attempt-id",
        &fixture.attempt_id,
        "--json",
    ]);
    let queue_after = rickygit_json(&[
        "patch",
        "review-queue",
        "--repo",
        &fixture.repo_arg,
        "--json",
    ]);

    assert_eq!(status["status"], "running");
    assert_eq!(status["changed"], true);
    assert_eq!(status["diff_hash"], fixture.diff_hash);
    assert_eq!(queue_before["patch_count"], 1);
    assert_eq!(queue_before["ready_count"], 1);
    assert_eq!(queue_before["patches"][0]["patch_id"], fixture.patch_id);
    assert_eq!(queue_before["patches"][0]["apply_ready"], true);
    assert_eq!(submit["status"], "ok");
    assert_eq!(submit["effective_status"], "submitted");
    assert_eq!(status_after["status"], "submitted");
    assert!(
        status_after["status_object_id"]
            .as_str()
            .unwrap()
            .starts_with("sha256:")
    );
    assert_eq!(submit["object"]["status"], "written");
    let verify = rickygit_json(&[
        "object",
        "verify",
        "--repo",
        &fixture.repo_arg,
        "--object-id",
        status_after["status_object_id"].as_str().unwrap(),
        "--json",
    ]);
    assert_eq!(verify["signature_count"], 1);
    assert_eq!(verify["valid_signature_count"], 1);
    assert_eq!(queue_after["patches"][0]["attempt_status"], "submitted");

    let mut cmd = Command::cargo_bin("rickygit").unwrap();
    cmd.args([
        "run",
        "exec",
        "--repo",
        &fixture.repo_arg,
        "--attempt-id",
        &fixture.attempt_id,
        "--json",
        "--",
        "sh",
        "-c",
        "printf 'late\\n' > late.txt",
    ])
    .assert()
    .failure()
    .stdout(predicate::str::contains("expected running"));
}

#[test]
fn attempt_abandon_blocks_later_runs_without_deleting_worktree() {
    let repo = tempfile::tempdir().unwrap();
    init_git_repo(repo.path());
    create_initial_commit(repo.path());
    let repo_arg = repo.path().to_str().unwrap();
    rickygit_json(&["init", "--repo", repo_arg, "--json"]);
    let intent = rickygit_json(&[
        "intent",
        "write",
        "--repo",
        repo_arg,
        &fixture_path("work-intent.valid.json"),
        "--json",
    ]);
    let attempt = rickygit_json(&[
        "attempt",
        "start",
        "--repo",
        repo_arg,
        "--intent-id",
        intent["object"]["object_id"].as_str().unwrap(),
        "--agent-id",
        "agent:test",
        "--json",
    ]);
    let attempt_id = attempt["attempt"]["attempt_id"].as_str().unwrap();
    let worktree_path = attempt["local_worktree_path"].as_str().unwrap();

    let abandon = rickygit_json(&[
        "attempt",
        "abandon",
        "--repo",
        repo_arg,
        "--attempt-id",
        attempt_id,
        "--reason",
        "stale",
        "--json",
    ]);
    let status = rickygit_json(&[
        "attempt",
        "status",
        "--repo",
        repo_arg,
        "--attempt-id",
        attempt_id,
        "--json",
    ]);

    assert_eq!(abandon["effective_status"], "abandoned");
    assert_eq!(status["status"], "abandoned");
    assert_eq!(status["worktree_exists"], true);
    assert_eq!(status["worktree_path"], worktree_path);

    let mut cmd = Command::cargo_bin("rickygit").unwrap();
    cmd.args([
        "run",
        "exec",
        "--repo",
        repo_arg,
        "--attempt-id",
        attempt_id,
        "--json",
        "--",
        "sh",
        "-c",
        "printf 'late\\n' > late.txt",
    ])
    .assert()
    .failure()
    .stdout(predicate::str::contains("expected running"));
}

#[test]
fn patch_checkout_creates_isolated_review_worktree() {
    let fixture = prepare_generated_patch_fixture();

    let checkout = rickygit_json(&[
        "patch",
        "checkout",
        "--repo",
        &fixture.repo_arg,
        "--patch-id",
        &fixture.patch_id,
        "--json",
    ]);
    let checkout_path = std::path::PathBuf::from(checkout["checkout_path"].as_str().unwrap());

    assert_eq!(checkout["patch_id"], fixture.patch_id);
    assert_eq!(checkout["attempt_id"], fixture.attempt_id);
    assert_eq!(checkout["base_commit"], fixture.base_commit);
    assert_eq!(checkout["head_commit"], fixture.base_commit);
    assert_eq!(checkout["applied"], true);
    assert_eq!(checkout["diff_hash"], fixture.diff_hash);
    assert_eq!(checkout["file_paths"][0], "generated.txt");
    assert_eq!(
        std::fs::read_to_string(checkout_path.join("generated.txt")).unwrap(),
        "new\n"
    );
    assert!(!fixture.repo.path().join("generated.txt").exists());
    assert_eq!(git_output(fixture.repo.path(), ["status", "--short"]), "");
}

#[test]
fn patch_checkout_works_when_main_worktree_is_dirty() {
    let fixture = prepare_generated_patch_fixture();
    std::fs::write(fixture.repo.path().join("local.txt"), "local\n").unwrap();

    let checkout = rickygit_json(&[
        "patch",
        "checkout",
        "--repo",
        &fixture.repo_arg,
        "--patch-id",
        &fixture.patch_id,
        "--json",
    ]);
    let checkout_path = std::path::PathBuf::from(checkout["checkout_path"].as_str().unwrap());

    assert_eq!(checkout["applied"], true);
    assert_eq!(
        std::fs::read_to_string(checkout_path.join("generated.txt")).unwrap(),
        "new\n"
    );
    assert_eq!(
        std::fs::read_to_string(fixture.repo.path().join("local.txt")).unwrap(),
        "local\n"
    );
    assert!(!fixture.repo.path().join("generated.txt").exists());
    assert!(git_output(fixture.repo.path(), ["status", "--short"]).contains("local.txt"));
}

#[test]
fn patch_checkout_refuses_base_drift_by_default() {
    let fixture = prepare_generated_patch_fixture();
    std::fs::write(
        fixture.repo.path().join("README.md"),
        "# test repo\nupdated\n",
    )
    .unwrap();
    git_success(fixture.repo.path(), ["add", "README.md"]);
    git_success(fixture.repo.path(), ["commit", "-m", "advance main"]);

    let mut cmd = Command::cargo_bin("rickygit").unwrap();
    cmd.args([
        "patch",
        "checkout",
        "--repo",
        &fixture.repo_arg,
        "--patch-id",
        &fixture.patch_id,
        "--json",
    ])
    .assert()
    .failure()
    .stdout(predicate::str::contains(
        "does not match prepared patch base commit",
    ));
    assert!(!fixture.repo.path().join(".git/rickydata/reviews").exists());
}

#[test]
fn patch_checkout_refuses_existing_checkout_path_without_force() {
    let fixture = prepare_generated_patch_fixture();
    let checkout_path = fixture.repo.path().join("review-checkout");
    std::fs::create_dir(&checkout_path).unwrap();

    let mut cmd = Command::cargo_bin("rickygit").unwrap();
    cmd.args([
        "patch",
        "checkout",
        "--repo",
        &fixture.repo_arg,
        "--patch-id",
        &fixture.patch_id,
        "--path",
        checkout_path.to_str().unwrap(),
        "--json",
    ])
    .assert()
    .failure()
    .stdout(predicate::str::contains("already exists"));
    assert!(checkout_path.is_dir());
    assert!(!checkout_path.join("generated.txt").exists());

    let mut force_cmd = Command::cargo_bin("rickygit").unwrap();
    force_cmd
        .args([
            "patch",
            "checkout",
            "--repo",
            &fixture.repo_arg,
            "--patch-id",
            &fixture.patch_id,
            "--path",
            checkout_path.to_str().unwrap(),
            "--force",
            "--json",
        ])
        .assert()
        .failure()
        .stdout(predicate::str::contains("not marked as Rickydata-owned"));
    assert!(checkout_path.is_dir());
    assert!(!checkout_path.join("generated.txt").exists());
}

#[test]
fn patch_checkout_force_replaces_owned_checkout() {
    let fixture = prepare_generated_patch_fixture();
    let checkout_path = fixture.repo.path().join("review-checkout");
    let first = rickygit_json(&[
        "patch",
        "checkout",
        "--repo",
        &fixture.repo_arg,
        "--patch-id",
        &fixture.patch_id,
        "--path",
        checkout_path.to_str().unwrap(),
        "--json",
    ]);
    std::fs::write(checkout_path.join("generated.txt"), "stale\n").unwrap();

    let second = rickygit_json(&[
        "patch",
        "checkout",
        "--repo",
        &fixture.repo_arg,
        "--patch-id",
        &fixture.patch_id,
        "--path",
        checkout_path.to_str().unwrap(),
        "--force",
        "--json",
    ]);

    assert_eq!(first["replaced"], false);
    assert_eq!(second["replaced"], true);
    assert_eq!(
        std::fs::read_to_string(checkout_path.join("generated.txt")).unwrap(),
        "new\n"
    );
    assert!(!fixture.repo.path().join("generated.txt").exists());
}

#[test]
fn sync_verify_validates_patch_evidence_after_cache_deletion() {
    let fixture = prepare_generated_patch_fixture();
    std::fs::remove_dir_all(fixture.repo.path().join(".git/rickydata/cache/objects")).unwrap();

    let verify = rickygit_json(&["sync", "verify", "--repo", &fixture.repo_arg, "--json"]);

    assert_eq!(verify["status"], "ok");
    assert!(verify["object_count"].as_u64().unwrap() >= 6);
    assert_eq!(verify["object_count"], verify["valid_object_count"]);
    assert_eq!(verify["object_count"], verify["recoverable_object_count"]);
    assert_eq!(verify["patch_count"], 1);
    assert_eq!(verify["valid_patch_count"], 1);
    assert_eq!(verify["retired_patch_count"], 0);
    assert!(verify["invalid_objects"].as_array().unwrap().is_empty());
    assert!(verify["invalid_patches"].as_array().unwrap().is_empty());
}

#[test]
fn sync_verify_ignores_retired_legacy_patch_evidence() {
    let fixture = prepare_generated_patch_fixture();
    let legacy_patch_id = format!("sha256:{}", "1".repeat(64));
    let legacy_patch = serde_json::json!({
        "patch_id": legacy_patch_id,
        "intent_id": format!("sha256:{}", "2".repeat(64)),
        "attempt_id": fixture.attempt_id,
        "base_commit": fixture.base_commit,
        "change_ids": [],
        "run_ids": [],
        "file_paths": [],
        "diff_hashes": [],
        "diff_object_ids": [],
        "related_contract_hashes": [],
        "diagnostics": []
    });
    let legacy_patch_file = fixture
        .repo
        .path()
        .join(".git/rickydata/tmp/legacy-patch.json");
    std::fs::write(
        &legacy_patch_file,
        serde_json::to_string_pretty(&legacy_patch).unwrap(),
    )
    .unwrap();
    rickygit_json(&[
        "object",
        "write",
        "--repo",
        &fixture.repo_arg,
        "--kind",
        "agent.patch",
        "--body-file",
        legacy_patch_file.to_str().unwrap(),
        "--json",
    ]);

    let before = rickygit_json(&["sync", "verify", "--repo", &fixture.repo_arg, "--json"]);
    assert_eq!(before["status"], "failed");
    assert_eq!(before["patch_count"], 2);
    assert_eq!(before["valid_patch_count"], 1);
    assert_eq!(before["retired_patch_count"], 0);
    assert_eq!(
        before["invalid_patches"][0]["diagnostics"][0],
        "prepared patch has no diff_object_ids"
    );

    let retirement = rickygit_json(&[
        "patch",
        "retire",
        "--repo",
        &fixture.repo_arg,
        "--patch-id",
        &legacy_patch_id,
        "--reason",
        "legacy patch before diff_object_ids were stored",
        "--retired-by",
        "agent:test",
        "--idempotency-key",
        "retire-once",
        "--json",
    ]);
    assert_eq!(retirement["retirement"]["patch_id"], legacy_patch_id);
    assert_eq!(
        retirement["retirement"]["reason"],
        "legacy patch before diff_object_ids were stored"
    );
    assert_eq!(retirement["retirement"]["idempotency_key"], "retire-once");
    assert_eq!(retirement["object"]["kind"], "agent.patch_retirement");

    let after = rickygit_json(&["sync", "verify", "--repo", &fixture.repo_arg, "--json"]);
    assert_eq!(after["status"], "ok");
    assert_eq!(after["patch_count"], 2);
    assert_eq!(after["valid_patch_count"], 1);
    assert_eq!(after["retired_patch_count"], 1);
    assert!(after["invalid_patches"].as_array().unwrap().is_empty());
}

#[test]
fn patch_export_writes_git_apply_compatible_diff() {
    let repo = tempfile::tempdir().unwrap();
    init_git_repo(repo.path());
    create_initial_commit(repo.path());
    let repo_arg = repo.path().to_str().unwrap();
    rickygit_json(&["init", "--repo", repo_arg, "--json"]);
    let intent = rickygit_json(&[
        "intent",
        "write",
        "--repo",
        repo_arg,
        &fixture_path("work-intent.valid.json"),
        "--json",
    ]);
    let attempt = rickygit_json(&[
        "attempt",
        "start",
        "--repo",
        repo_arg,
        "--intent-id",
        intent["object"]["object_id"].as_str().unwrap(),
        "--agent-id",
        "agent:test",
        "--idempotency-key",
        "first",
        "--json",
    ]);
    let attempt_id = attempt["attempt"]["attempt_id"].as_str().unwrap();
    rickygit_json(&[
        "run",
        "exec",
        "--repo",
        repo_arg,
        "--attempt-id",
        attempt_id,
        "--json",
        "--",
        "sh",
        "-c",
        "printf 'new\\n' > generated.txt",
    ]);
    rickygit_json(&[
        "change",
        "detect",
        "--repo",
        repo_arg,
        "--attempt-id",
        attempt_id,
        "--json",
    ]);
    let patch = rickygit_json(&[
        "patch",
        "prepare",
        "--repo",
        repo_arg,
        "--attempt-id",
        attempt_id,
        "--json",
    ]);
    let patch_id = patch["patch"]["patch_id"].as_str().unwrap();
    let output = repo.path().with_extension("patch");

    let export = rickygit_json(&[
        "patch",
        "export",
        "--repo",
        repo_arg,
        "--patch-id",
        patch_id,
        "--output",
        output.to_str().unwrap(),
        "--json",
    ]);

    assert_eq!(export["patch_id"], patch_id);
    assert_eq!(export["attempt_id"], attempt_id);
    assert_eq!(export["file_count"], 1);
    assert_eq!(export["file_paths"][0], "generated.txt");
    assert_eq!(export["diff_hash"], patch["patch"]["diff_hashes"][0]);
    assert!(export["diff_bytes"].as_u64().unwrap() > 0);
    assert!(output.exists());
    git_success(repo.path(), ["apply", "--check", output.to_str().unwrap()]);
}

#[test]
fn patch_export_uses_stored_diff_when_attempt_worktree_drifts() {
    let repo = tempfile::tempdir().unwrap();
    init_git_repo(repo.path());
    create_initial_commit(repo.path());
    let repo_arg = repo.path().to_str().unwrap();
    rickygit_json(&["init", "--repo", repo_arg, "--json"]);
    let intent = rickygit_json(&[
        "intent",
        "write",
        "--repo",
        repo_arg,
        &fixture_path("work-intent.valid.json"),
        "--json",
    ]);
    let attempt = rickygit_json(&[
        "attempt",
        "start",
        "--repo",
        repo_arg,
        "--intent-id",
        intent["object"]["object_id"].as_str().unwrap(),
        "--agent-id",
        "agent:test",
        "--idempotency-key",
        "first",
        "--json",
    ]);
    let attempt_id = attempt["attempt"]["attempt_id"].as_str().unwrap();
    let attempt_worktree = attempt["local_worktree_path"].as_str().unwrap();
    rickygit_json(&[
        "run",
        "exec",
        "--repo",
        repo_arg,
        "--attempt-id",
        attempt_id,
        "--json",
        "--",
        "sh",
        "-c",
        "printf 'new\\n' > generated.txt",
    ]);
    rickygit_json(&[
        "change",
        "detect",
        "--repo",
        repo_arg,
        "--attempt-id",
        attempt_id,
        "--json",
    ]);
    let patch = rickygit_json(&[
        "patch",
        "prepare",
        "--repo",
        repo_arg,
        "--attempt-id",
        attempt_id,
        "--json",
    ]);
    std::fs::write(
        std::path::Path::new(attempt_worktree).join("generated.txt"),
        "drift\n",
    )
    .unwrap();
    let output = repo.path().with_extension("patch");

    let export = rickygit_json(&[
        "patch",
        "export",
        "--repo",
        repo_arg,
        "--patch-id",
        patch["patch"]["patch_id"].as_str().unwrap(),
        "--output",
        output.to_str().unwrap(),
        "--json",
    ]);
    let exported = std::fs::read_to_string(output).unwrap();

    assert_eq!(export["diff_hash"], patch["patch"]["diff_hashes"][0]);
    assert!(exported.contains("+new"));
    assert!(!exported.contains("+drift"));
}

#[test]
fn patch_export_recovers_after_sync_pull_without_cache_or_attempt_worktree() {
    let remote = tempfile::tempdir().unwrap();
    git_success(remote.path(), ["init", "--bare"]);

    let repo_a = tempfile::tempdir().unwrap();
    init_git_repo(repo_a.path());
    create_initial_commit(repo_a.path());
    git_success(
        repo_a.path(),
        ["remote", "add", "origin", remote.path().to_str().unwrap()],
    );
    git_success(repo_a.path(), ["push", "-u", "origin", "main"]);
    git_success(remote.path(), ["symbolic-ref", "HEAD", "refs/heads/main"]);
    let repo_a_arg = repo_a.path().to_str().unwrap();
    rickygit_json(&["init", "--repo", repo_a_arg, "--json"]);
    let intent = rickygit_json(&[
        "intent",
        "write",
        "--repo",
        repo_a_arg,
        &fixture_path("work-intent.valid.json"),
        "--json",
    ]);
    let attempt = rickygit_json(&[
        "attempt",
        "start",
        "--repo",
        repo_a_arg,
        "--intent-id",
        intent["object"]["object_id"].as_str().unwrap(),
        "--agent-id",
        "agent:test",
        "--idempotency-key",
        "first",
        "--json",
    ]);
    let attempt_id = attempt["attempt"]["attempt_id"].as_str().unwrap();
    rickygit_json(&[
        "run",
        "exec",
        "--repo",
        repo_a_arg,
        "--attempt-id",
        attempt_id,
        "--json",
        "--",
        "sh",
        "-c",
        "printf 'new\\n' > generated.txt",
    ]);
    rickygit_json(&[
        "change",
        "detect",
        "--repo",
        repo_a_arg,
        "--attempt-id",
        attempt_id,
        "--json",
    ]);
    let patch = rickygit_json(&[
        "patch",
        "prepare",
        "--repo",
        repo_a_arg,
        "--attempt-id",
        attempt_id,
        "--json",
    ]);
    let patch_id = patch["patch"]["patch_id"].as_str().unwrap();
    assert_eq!(
        patch["patch"]["diff_object_ids"].as_array().unwrap().len(),
        1
    );
    rickygit_json(&[
        "sync", "push", "--repo", repo_a_arg, "--remote", "origin", "--json",
    ]);

    let checkout = tempfile::tempdir().unwrap();
    git_success(
        checkout.path(),
        [
            "clone",
            remote.path().to_str().unwrap(),
            checkout.path().join("repo-b").to_str().unwrap(),
        ],
    );
    let repo_b = checkout.path().join("repo-b");
    let repo_b_arg = repo_b.to_str().unwrap();
    rickygit_json(&["init", "--repo", repo_b_arg, "--json"]);
    rickygit_json(&[
        "sync", "pull", "--repo", repo_b_arg, "--remote", "origin", "--json",
    ]);
    std::fs::remove_dir_all(repo_b.join(".git/rickydata/cache/objects")).unwrap();
    let output = repo_b.with_extension("patch");

    let export = rickygit_json(&[
        "patch",
        "export",
        "--repo",
        repo_b_arg,
        "--patch-id",
        patch_id,
        "--output",
        output.to_str().unwrap(),
        "--json",
    ]);

    assert_eq!(export["patch_id"], patch_id);
    assert_eq!(export["diff_hash"], patch["patch"]["diff_hashes"][0]);
    git_success(&repo_b, ["apply", "--check", output.to_str().unwrap()]);
    let checkout = rickygit_json(&[
        "patch",
        "checkout",
        "--repo",
        repo_b_arg,
        "--patch-id",
        patch_id,
        "--json",
    ]);
    let checkout_path = std::path::PathBuf::from(checkout["checkout_path"].as_str().unwrap());
    assert_eq!(checkout["patch_id"], patch_id);
    assert_eq!(checkout["diff_hash"], patch["patch"]["diff_hashes"][0]);
    assert_eq!(
        std::fs::read_to_string(checkout_path.join("generated.txt")).unwrap(),
        "new\n"
    );
    assert!(!repo_b.join("generated.txt").exists());
    assert_eq!(git_output(&repo_b, ["status", "--short"]), "");
    let apply = rickygit_json(&[
        "patch",
        "apply",
        "--repo",
        repo_b_arg,
        "--patch-id",
        patch_id,
        "--json",
    ]);
    assert_eq!(apply["applied"], true);
    assert_eq!(
        std::fs::read_to_string(repo_b.join("generated.txt")).unwrap(),
        "new\n"
    );
}

#[test]
fn patch_apply_applies_stored_diff_to_clean_worktree() {
    let repo = tempfile::tempdir().unwrap();
    init_git_repo(repo.path());
    create_initial_commit(repo.path());
    let repo_arg = repo.path().to_str().unwrap();
    rickygit_json(&["init", "--repo", repo_arg, "--json"]);
    let intent = rickygit_json(&[
        "intent",
        "write",
        "--repo",
        repo_arg,
        &fixture_path("work-intent.valid.json"),
        "--json",
    ]);
    let attempt = rickygit_json(&[
        "attempt",
        "start",
        "--repo",
        repo_arg,
        "--intent-id",
        intent["object"]["object_id"].as_str().unwrap(),
        "--agent-id",
        "agent:test",
        "--idempotency-key",
        "first",
        "--json",
    ]);
    let attempt_id = attempt["attempt"]["attempt_id"].as_str().unwrap();
    rickygit_json(&[
        "run",
        "exec",
        "--repo",
        repo_arg,
        "--attempt-id",
        attempt_id,
        "--json",
        "--",
        "sh",
        "-c",
        "printf 'new\\n' > generated.txt",
    ]);
    rickygit_json(&[
        "change",
        "detect",
        "--repo",
        repo_arg,
        "--attempt-id",
        attempt_id,
        "--json",
    ]);
    let patch = rickygit_json(&[
        "patch",
        "prepare",
        "--repo",
        repo_arg,
        "--attempt-id",
        attempt_id,
        "--json",
    ]);
    let patch_id = patch["patch"]["patch_id"].as_str().unwrap();

    let apply = rickygit_json(&[
        "patch",
        "apply",
        "--repo",
        repo_arg,
        "--patch-id",
        patch_id,
        "--applied-by",
        "agent:test",
        "--reason",
        "cli regression test",
        "--idempotency-key",
        "apply-once",
        "--json",
    ]);

    assert_eq!(apply["patch_id"], patch_id);
    assert_eq!(apply["applied"], true);
    assert_eq!(apply["replayed"], false);
    assert_eq!(apply["application"]["patch_id"], patch_id);
    assert_eq!(apply["application"]["applied_by"], "agent:test");
    assert_eq!(apply["application"]["idempotency_key"], "apply-once");
    assert_eq!(apply["object"]["kind"], "agent.patch_application");
    assert_eq!(apply["file_paths"][0], "generated.txt");
    let replay = rickygit_json(&[
        "patch",
        "apply",
        "--repo",
        repo_arg,
        "--patch-id",
        patch_id,
        "--idempotency-key",
        "apply-once",
        "--json",
    ]);
    assert_eq!(replay["applied"], false);
    assert_eq!(replay["replayed"], true);
    assert_eq!(replay["object"]["object_id"], apply["object"]["object_id"]);
    assert_eq!(
        std::fs::read_to_string(repo.path().join("generated.txt")).unwrap(),
        "new\n"
    );
}

#[test]
fn patch_apply_refuses_dirty_worktree_before_mutation() {
    let repo = tempfile::tempdir().unwrap();
    init_git_repo(repo.path());
    create_initial_commit(repo.path());
    let repo_arg = repo.path().to_str().unwrap();
    rickygit_json(&["init", "--repo", repo_arg, "--json"]);
    let intent = rickygit_json(&[
        "intent",
        "write",
        "--repo",
        repo_arg,
        &fixture_path("work-intent.valid.json"),
        "--json",
    ]);
    let attempt = rickygit_json(&[
        "attempt",
        "start",
        "--repo",
        repo_arg,
        "--intent-id",
        intent["object"]["object_id"].as_str().unwrap(),
        "--agent-id",
        "agent:test",
        "--idempotency-key",
        "first",
        "--json",
    ]);
    let attempt_id = attempt["attempt"]["attempt_id"].as_str().unwrap();
    rickygit_json(&[
        "run",
        "exec",
        "--repo",
        repo_arg,
        "--attempt-id",
        attempt_id,
        "--json",
        "--",
        "sh",
        "-c",
        "printf 'new\\n' > generated.txt",
    ]);
    rickygit_json(&[
        "change",
        "detect",
        "--repo",
        repo_arg,
        "--attempt-id",
        attempt_id,
        "--json",
    ]);
    let patch = rickygit_json(&[
        "patch",
        "prepare",
        "--repo",
        repo_arg,
        "--attempt-id",
        attempt_id,
        "--json",
    ]);
    std::fs::write(repo.path().join("local.txt"), "local\n").unwrap();

    let mut cmd = Command::cargo_bin("rickygit").unwrap();
    cmd.args([
        "patch",
        "apply",
        "--repo",
        repo_arg,
        "--patch-id",
        patch["patch"]["patch_id"].as_str().unwrap(),
        "--json",
    ])
    .assert()
    .failure()
    .stdout(predicate::str::contains("worktree is dirty"));
    assert!(!repo.path().join("generated.txt").exists());
}

#[test]
fn patch_show_reports_missing_patch_id() {
    let repo = tempfile::tempdir().unwrap();
    init_git_repo(repo.path());
    create_initial_commit(repo.path());
    let repo_arg = repo.path().to_str().unwrap();
    rickygit_json(&["init", "--repo", repo_arg, "--json"]);

    let mut cmd = Command::cargo_bin("rickygit").unwrap();
    cmd.args([
        "patch",
        "show",
        "--repo",
        repo_arg,
        "--patch-id",
        "sha256:0000000000000000000000000000000000000000000000000000000000000000",
        "--json",
    ])
    .assert()
    .failure()
    .stdout(predicate::str::contains("was not found"));
}

#[test]
fn sync_push_and_pull_replicate_rickydata_refs_between_clones() {
    let remote = tempfile::tempdir().unwrap();
    git_success(remote.path(), ["init", "--bare"]);

    let repo_a = tempfile::tempdir().unwrap();
    init_git_repo(repo_a.path());
    let repo_a_arg = repo_a.path().to_str().unwrap();
    rickygit_json(&["init", "--repo", repo_a_arg, "--json"]);
    let intent = rickygit_json(&[
        "intent",
        "write",
        "--repo",
        repo_a_arg,
        &fixture_path("work-intent.valid.json"),
        "--json",
    ]);
    let intent_id = intent["object"]["object_id"].as_str().unwrap();
    git_success(
        repo_a.path(),
        ["remote", "add", "origin", remote.path().to_str().unwrap()],
    );

    let before_push_status = rickygit_json(&[
        "sync", "status", "--repo", repo_a_arg, "--remote", "origin", "--json",
    ]);
    let push = rickygit_json(&[
        "sync", "push", "--repo", repo_a_arg, "--remote", "origin", "--json",
    ]);
    let after_push_status = rickygit_json(&[
        "sync", "status", "--repo", repo_a_arg, "--remote", "origin", "--json",
    ]);

    let checkout = tempfile::tempdir().unwrap();
    git_success(
        checkout.path(),
        [
            "clone",
            remote.path().to_str().unwrap(),
            checkout.path().join("repo-b").to_str().unwrap(),
        ],
    );
    let repo_b = checkout.path().join("repo-b");
    let repo_b_arg = repo_b.to_str().unwrap();
    rickygit_json(&["init", "--repo", repo_b_arg, "--json"]);
    let before_pull_status = rickygit_json(&[
        "sync", "status", "--repo", repo_b_arg, "--remote", "origin", "--json",
    ]);

    let pull = rickygit_json(&[
        "sync", "pull", "--repo", repo_b_arg, "--remote", "origin", "--json",
    ]);
    let after_pull_status = rickygit_json(&[
        "sync", "status", "--repo", repo_b_arg, "--remote", "origin", "--json",
    ]);
    let list = rickygit_json(&["intent", "list", "--repo", repo_b_arg, "--json"]);

    assert_eq!(before_push_status["local_ref_count"], 1);
    assert_eq!(before_push_status["remote_ref_count"], 0);
    assert_eq!(
        before_push_status["local_only_refs"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
    assert_eq!(after_push_status["matching_ref_count"], 1);
    assert_eq!(
        after_push_status["divergent_refs"]
            .as_array()
            .unwrap()
            .len(),
        0
    );
    assert_eq!(
        before_pull_status["remote_only_refs"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
    assert_eq!(after_pull_status["matching_ref_count"], 1);
    assert_eq!(push["direction"], "push");
    assert_eq!(pull["direction"], "pull");
    assert_eq!(push["remote"], "origin");
    assert_eq!(pull["remote"], "origin");
    assert_eq!(push["refspec"], "refs/rickydata/*:refs/rickydata/*");
    assert_eq!(pull["refspec"], "refs/rickydata/*:refs/rickydata/*");
    assert!(push["stdout_hash"].as_str().unwrap().starts_with("sha256:"));
    assert!(pull["stderr_hash"].as_str().unwrap().starts_with("sha256:"));
    assert!(push["stdout"].is_null());
    assert_eq!(list["intents"].as_array().unwrap().len(), 1);
    assert_eq!(list["intents"][0]["object_id"], intent_id);
    git_success(repo_a.path(), ["fsck"]);
    git_success(remote.path(), ["fsck"]);
    git_success(&repo_b, ["fsck"]);
}

#[test]
fn relay_push_status_and_pull_replicate_ref_backed_objects() {
    let relay = start_relay();
    let repo_id = "relay-test";

    let repo_a = tempfile::tempdir().unwrap();
    init_git_repo(repo_a.path());
    create_initial_commit(repo_a.path());
    let repo_a_arg = repo_a.path().to_str().unwrap();
    rickygit_json(&["init", "--repo", repo_a_arg, "--json"]);
    let intent = rickygit_json(&[
        "intent",
        "write",
        "--repo",
        repo_a_arg,
        &fixture_path("work-intent.valid.json"),
        "--json",
    ]);
    let intent_id = intent["object"]["object_id"].as_str().unwrap();
    let attempt = rickygit_json(&[
        "attempt",
        "start",
        "--repo",
        repo_a_arg,
        "--intent-id",
        intent_id,
        "--agent-id",
        "agent:test",
        "--idempotency-key",
        "relay-chunk-test",
        "--json",
    ]);
    assert_eq!(attempt["attempt"]["status"], "running");

    let push = rickygit_json(&[
        "relay",
        "push",
        "--repo",
        repo_a_arg,
        "--url",
        &relay.url,
        "--repo-id",
        repo_id,
        "--chunk-size",
        "1",
        "--json",
    ]);
    let status = rickygit_json(&[
        "relay",
        "status",
        "--repo",
        repo_a_arg,
        "--url",
        &relay.url,
        "--repo-id",
        repo_id,
        "--json",
    ]);

    let repo_b = tempfile::tempdir().unwrap();
    init_git_repo(repo_b.path());
    create_initial_commit(repo_b.path());
    let repo_b_arg = repo_b.path().to_str().unwrap();
    rickygit_json(&["init", "--repo", repo_b_arg, "--json"]);
    let pull = rickygit_json(&[
        "relay",
        "pull",
        "--repo",
        repo_b_arg,
        "--url",
        &relay.url,
        "--repo-id",
        repo_id,
        "--json",
    ]);
    let list = rickygit_json(&["intent", "list", "--repo", repo_b_arg, "--json"]);
    let second_pull = rickygit_json(&[
        "relay",
        "pull",
        "--repo",
        repo_b_arg,
        "--url",
        &relay.url,
        "--repo-id",
        repo_id,
        "--json",
    ]);

    assert_eq!(push["status"], "ok");
    assert_eq!(push["accepted_object_count"], 2);
    assert_eq!(push["duplicate_object_count"], 0);
    assert_eq!(push["object_ids"].as_array().unwrap().len(), 2);
    assert_eq!(status["status"], "ok");
    assert_eq!(status["local_object_count"], 2);
    assert_eq!(status["relay_object_count"], 2);
    assert_eq!(pull["status"], "ok");
    assert_eq!(pull["object_count"], 2);
    assert_eq!(pull["written_object_count"], 2);
    assert_eq!(pull["duplicate_object_count"], 0);
    assert_eq!(list["intents"].as_array().unwrap().len(), 1);
    assert_eq!(list["intents"][0]["object_id"], intent_id);
    assert_eq!(second_pull["object_count"], 0);
    assert_eq!(second_pull["written_object_count"], 0);
}

#[test]
fn current_commands_do_not_write_rickydata_git_metadata() {
    let repo = tempfile::tempdir().unwrap();
    init_git_repo(repo.path());

    let initial_git_snapshot = git_file_snapshot(repo.path());
    let valid_intent = fixture_path("work-intent.valid.json");
    let commands = vec![
        vec!["doctor", "--json"],
        vec!["manifest", "--json"],
        vec!["schema", "--json"],
        vec!["inspect", "--repo", repo.path().to_str().unwrap(), "--json"],
        vec!["status", "--repo", repo.path().to_str().unwrap(), "--json"],
        vec![
            "discovery",
            "--repo",
            repo.path().to_str().unwrap(),
            "--json",
        ],
        vec!["intent", "validate", &valid_intent, "--json"],
        vec!["intent", "hash", &valid_intent, "--json"],
    ];

    for args in commands {
        let mut cmd = Command::cargo_bin("rickygit").unwrap();
        cmd.current_dir(repo.path()).args(args).assert().success();
        assert!(!repo.path().join(".git/rickydata").exists());
        assert!(!repo.path().join(".git/refs/rickydata").exists());
        assert_packed_refs_has_no_rickydata_refs(repo.path());
        assert_eq!(initial_git_snapshot, git_file_snapshot(repo.path()));
    }
}

#[test]
fn proof_reports_git_relay_and_kfdb_parity() {
    let relay = start_relay();
    let kfdb = start_kfdb();
    let repo_id = "proof-test";
    let remote = tempfile::tempdir().unwrap();
    git_success(remote.path(), ["init", "--bare"]);

    let repo = tempfile::tempdir().unwrap();
    init_git_repo(repo.path());
    create_initial_commit(repo.path());
    git_success(
        repo.path(),
        ["remote", "add", "origin", remote.path().to_str().unwrap()],
    );
    let repo_arg = repo.path().to_str().unwrap();
    rickygit_json(&["init", "--repo", repo_arg, "--json"]);
    rickygit_json(&[
        "intent",
        "write",
        "--repo",
        repo_arg,
        &fixture_path("work-intent.valid.json"),
        "--json",
    ]);
    rickygit_json(&[
        "sync", "push", "--repo", repo_arg, "--remote", "origin", "--json",
    ]);
    rickygit_json(&[
        "relay",
        "push",
        "--repo",
        repo_arg,
        "--url",
        &relay.url,
        "--repo-id",
        repo_id,
        "--json",
    ]);

    let proof = rickygit_json(&[
        "proof",
        "--repo",
        repo_arg,
        "--remote",
        "origin",
        "--relay-url",
        &relay.url,
        "--repo-id",
        repo_id,
        "--kfdb-url",
        &kfdb.url,
        "--json",
    ]);

    assert_eq!(proof["status"], "ok");
    assert_eq!(proof["local"]["object_count"], 1);
    assert_eq!(proof["git_remote"]["matching_ref_count"], 1);
    assert_eq!(proof["relay"]["status"], "ok");
    assert_eq!(proof["relay"]["relay_object_count"], 1);
    assert_eq!(proof["kfdb"]["status"], "ok");
    assert_eq!(proof["kfdb"]["object_mirror_count"], 1);
    assert_eq!(proof["kfdb"]["prepared_patch_count"], 0);
    assert_eq!(proof["diagnostics"].as_array().unwrap().len(), 0);
}

struct RelayFixture {
    shutdown: Option<tokio::sync::oneshot::Sender<()>>,
    handle: Option<JoinHandle<()>>,
    url: String,
    _store: tempfile::TempDir,
}

impl Drop for RelayFixture {
    fn drop(&mut self) {
        if let Some(shutdown) = self.shutdown.take() {
            let _ = shutdown.send(());
        }
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
}

struct KfdbFixture {
    shutdown: Option<tokio::sync::oneshot::Sender<()>>,
    handle: Option<JoinHandle<()>>,
    url: String,
}

struct ProjectKfdbFixture {
    shutdown: Option<tokio::sync::oneshot::Sender<()>>,
    handle: Option<JoinHandle<()>>,
    url: String,
    requests: Arc<Mutex<Vec<Value>>>,
    _response_status: StatusCode,
    _response_body: Option<Value>,
}

impl Drop for KfdbFixture {
    fn drop(&mut self) {
        if let Some(shutdown) = self.shutdown.take() {
            let _ = shutdown.send(());
        }
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
}

impl Drop for ProjectKfdbFixture {
    fn drop(&mut self) {
        if let Some(shutdown) = self.shutdown.take() {
            let _ = shutdown.send(());
        }
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
}

#[derive(Clone)]
struct ProjectKfdbState {
    requests: Arc<Mutex<Vec<Value>>>,
    response_status: StatusCode,
    response_body: Option<Value>,
}

fn start_project_kfdb(
    response_status: StatusCode,
    response_body: Option<Value>,
) -> ProjectKfdbFixture {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    listener.set_nonblocking(true).unwrap();
    let requests = Arc::new(Mutex::new(Vec::new()));
    let state = ProjectKfdbState {
        requests: Arc::clone(&requests),
        response_status,
        response_body: response_body.clone(),
    };
    let (shutdown, shutdown_rx) = tokio::sync::oneshot::channel();
    let handle = std::thread::spawn(move || {
        let runtime = tokio::runtime::Runtime::new().unwrap();
        runtime.block_on(async move {
            let listener = tokio::net::TcpListener::from_std(listener).unwrap();
            let app = axum::Router::new()
                .route("/api/v1/write", post(mock_project_kfdb_write))
                .with_state(state);
            axum::serve(listener, app)
                .with_graceful_shutdown(async {
                    let _ = shutdown_rx.await;
                })
                .await
                .unwrap();
        });
    });
    let url = format!("http://{addr}");
    for _ in 0..100 {
        let response = reqwest::blocking::Client::new()
            .post(format!("{url}/api/v1/write"))
            .json(&serde_json::json!({ "operations": [], "skip_embedding": true }))
            .send();
        match response {
            Ok(_) => {
                requests.lock().unwrap().clear();
                return ProjectKfdbFixture {
                    shutdown: Some(shutdown),
                    handle: Some(handle),
                    url,
                    requests,
                    _response_status: response_status,
                    _response_body: response_body,
                };
            }
            _ => {
                assert!(
                    !handle.is_finished(),
                    "mock project KFDB exited before write check succeeded"
                );
                std::thread::sleep(Duration::from_millis(50));
            }
        }
    }
    panic!("mock project KFDB did not become healthy at {url}");
}

async fn mock_project_kfdb_write(
    State(state): State<ProjectKfdbState>,
    headers: axum::http::HeaderMap,
    Json(body): Json<Value>,
) -> (StatusCode, Json<Value>) {
    let operations = body
        .get("operations")
        .and_then(Value::as_array)
        .map(Vec::len)
        .unwrap_or_default();
    state.requests.lock().unwrap().push(serde_json::json!({
        "derive_session_id": headers
            .get("x-derive-session-id")
            .and_then(|value| value.to_str().ok())
            .unwrap_or_default(),
        "derive_key": headers
            .get("x-derive-key")
            .and_then(|value| value.to_str().ok())
            .unwrap_or_default(),
        "wallet_address": headers
            .get("x-wallet-address")
            .and_then(|value| value.to_str().ok())
            .unwrap_or_default(),
        "body": body,
    }));
    let response = state.response_body.clone().unwrap_or_else(|| {
        serde_json::json!({
            "operations_executed": operations,
            "execution_time_ms": 1.0,
            "affected_ids": []
        })
    });
    (state.response_status, Json(response))
}

fn start_kfdb() -> KfdbFixture {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    listener.set_nonblocking(true).unwrap();
    let (shutdown, shutdown_rx) = tokio::sync::oneshot::channel();
    let handle = std::thread::spawn(move || {
        let runtime = tokio::runtime::Runtime::new().unwrap();
        runtime.block_on(async move {
            let listener = tokio::net::TcpListener::from_std(listener).unwrap();
            let app = axum::Router::new().route("/api/v1/query", post(mock_kfdb_query));
            axum::serve(listener, app)
                .with_graceful_shutdown(async {
                    let _ = shutdown_rx.await;
                })
                .await
                .unwrap();
        });
    });
    let url = format!("http://{addr}");
    for _ in 0..100 {
        let response = reqwest::blocking::Client::new()
            .post(format!("{url}/api/v1/query"))
            .json(
                &serde_json::json!({ "query": "MATCH (n:RickydataObjectMirror) RETURN COUNT(n)" }),
            )
            .send();
        match response {
            Ok(response) if response.status().is_success() => {
                return KfdbFixture {
                    shutdown: Some(shutdown),
                    handle: Some(handle),
                    url,
                };
            }
            _ => {
                assert!(
                    !handle.is_finished(),
                    "mock KFDB exited before query check succeeded"
                );
                std::thread::sleep(Duration::from_millis(50));
            }
        }
    }
    panic!("mock KFDB did not become healthy at {url}");
}

async fn mock_kfdb_query(Json(body): Json<Value>) -> Json<Value> {
    let query = body
        .get("query")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let data = if query.contains("RickydataPreparedPatch")
        && query.contains("RETURN n.object_id AS object_id")
    {
        serde_json::json!([])
    } else {
        let count = if query.contains("RickydataObjectMirror") {
            1
        } else if query.contains("RickydataPreparedPatch") {
            99
        } else {
            0
        };
        serde_json::json!([{ "count": { "Integer": count } }])
    };
    let nodes_scanned = data.as_array().map(Vec::len).unwrap_or_default();
    Json(serde_json::json!({
        "data": data,
        "stats": {
            "execution_time_ms": 1.0,
            "nodes_scanned": nodes_scanned,
            "edges_traversed": 0,
            "results_returned": 1
        },
        "warnings": [],
        "has_more": false
    }))
}

fn start_relay() -> RelayFixture {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    listener.set_nonblocking(true).unwrap();

    let store = tempfile::tempdir().unwrap();
    let store_path = store.path().to_path_buf();
    let (shutdown, shutdown_rx) = tokio::sync::oneshot::channel();
    let handle = std::thread::spawn(move || {
        let runtime = tokio::runtime::Runtime::new().unwrap();
        runtime.block_on(async move {
            let listener = tokio::net::TcpListener::from_std(listener).unwrap();
            axum::serve(
                listener,
                rickydata_git_relay::router(rickydata_git_relay::FileRelayStore::new(store_path)),
            )
            .with_graceful_shutdown(async {
                let _ = shutdown_rx.await;
            })
            .await
            .unwrap();
        });
    });
    let url = format!("http://{addr}");
    for _ in 0..100 {
        match reqwest::blocking::get(format!("{url}/health")) {
            Ok(response) if response.status().is_success() => {
                return RelayFixture {
                    shutdown: Some(shutdown),
                    handle: Some(handle),
                    url,
                    _store: store,
                };
            }
            _ => {
                assert!(
                    !handle.is_finished(),
                    "relay exited before health check succeeded"
                );
                std::thread::sleep(Duration::from_millis(50));
            }
        }
    }
    panic!("relay did not become healthy at {url}");
}

#[test]
fn key_generate_creates_signing_key_and_emits_public_key() {
    let dir = tempfile::tempdir().unwrap();
    let key_path = dir.path().join("signer.key");
    let key_arg = key_path.to_str().unwrap();

    let report = rickygit_json(&["key", "generate", "--output", key_arg, "--json"]);

    assert_eq!(report["status"], "ok");
    assert_eq!(report["algorithm"], "ed25519");
    let pk = report["public_key"].as_str().unwrap();
    assert_eq!(
        pk.len(),
        64,
        "ed25519 public key should be 32 bytes / 64 hex chars"
    );
    assert!(key_path.exists());
    let key_bytes = std::fs::read(&key_path).unwrap();
    assert_eq!(
        key_bytes.len(),
        32,
        "signing key seed should be 32 raw bytes"
    );

    let show = rickygit_json(&["key", "show", "--signing-key-file", key_arg, "--json"]);
    assert_eq!(show["algorithm"], "ed25519");
    assert_eq!(show["public_key"].as_str().unwrap(), pk);
}

#[test]
fn key_generate_refuses_to_overwrite_existing_file() {
    let dir = tempfile::tempdir().unwrap();
    let key_path = dir.path().join("signer.key");
    std::fs::write(&key_path, b"existing").unwrap();
    let key_arg = key_path.to_str().unwrap();

    let mut cmd = Command::cargo_bin("rickygit").unwrap();
    cmd.args(["key", "generate", "--output", key_arg, "--json"])
        .assert()
        .failure()
        .stdout(predicate::str::contains("already exists"));
}

#[test]
fn intent_write_with_signing_key_produces_signed_object_and_verify_reports_it() {
    let repo = tempfile::tempdir().unwrap();
    init_git_repo(repo.path());
    let repo_arg = repo.path().to_str().unwrap();
    rickygit_json(&["init", "--repo", repo_arg, "--json"]);

    let key_dir = tempfile::tempdir().unwrap();
    let key_path = key_dir.path().join("signer.key");
    let key_arg = key_path.to_str().unwrap();
    let key_report = rickygit_json(&["key", "generate", "--output", key_arg, "--json"]);
    let expected_public_key = key_report["public_key"].as_str().unwrap().to_string();

    let intent_path = fixture_path("work-intent.valid.json");
    let write = rickygit_json(&[
        "intent",
        "write",
        "--repo",
        repo_arg,
        &intent_path,
        "--signing-key-file",
        key_arg,
        "--signer-label",
        "alice",
        "--json",
    ]);
    assert_eq!(write["valid"], true);
    let object_id = write["object"]["object_id"].as_str().unwrap();
    assert_eq!(
        object_id, "sha256:a0c9cd2e0309cc8d965aebc13c2d8acf09b4b1df507b8e83374f0e1a538ff071",
        "signing must not change object_id"
    );

    let read = rickygit_json(&[
        "object",
        "read",
        "--repo",
        repo_arg,
        "--object-id",
        object_id,
        "--json",
    ]);
    let signatures = read["object"]["signatures"].as_array().unwrap();
    assert_eq!(signatures.len(), 1);
    assert_eq!(signatures[0]["algorithm"], "ed25519");
    assert_eq!(signatures[0]["public_key"], expected_public_key);
    assert_eq!(signatures[0]["signer_label"], "alice");

    let verify = rickygit_json(&[
        "object",
        "verify",
        "--repo",
        repo_arg,
        "--object-id",
        object_id,
        "--json",
    ]);
    assert_eq!(verify["valid"], true);
    assert_eq!(verify["signature_count"], 1);
    assert_eq!(verify["valid_signature_count"], 1);
}

#[test]
fn sync_verify_reports_signed_object_counts() {
    let repo = tempfile::tempdir().unwrap();
    init_git_repo(repo.path());
    create_initial_commit(repo.path());
    let repo_arg = repo.path().to_str().unwrap();
    rickygit_json(&["init", "--repo", repo_arg, "--json"]);

    let key_dir = tempfile::tempdir().unwrap();
    let key_path = key_dir.path().join("signer.key");
    let key_arg = key_path.to_str().unwrap();
    rickygit_json(&["key", "generate", "--output", key_arg, "--json"]);

    let intent_path = fixture_path("work-intent.valid.json");
    // First write: signed
    rickygit_json(&[
        "intent",
        "write",
        "--repo",
        repo_arg,
        &intent_path,
        "--signing-key-file",
        key_arg,
        "--json",
    ]);
    // Second write: unsigned object of a different kind so the object_id differs.
    rickygit_json(&[
        "object",
        "write",
        "--repo",
        repo_arg,
        "--kind",
        "example.unsigned",
        "--body-file",
        &intent_path,
        "--json",
    ]);

    let verify = rickygit_json(&["sync", "verify", "--repo", repo_arg, "--json"]);
    assert_eq!(verify["status"], "ok");
    assert_eq!(verify["object_count"], 2);
    assert_eq!(verify["valid_object_count"], 2);
    assert_eq!(verify["signed_object_count"], 1);
    assert_eq!(verify["valid_signature_count"], 1);

    let proof = rickygit_json(&["proof", "--repo", repo_arg, "--json"]);
    assert_eq!(proof["status"], "ok");
    assert_eq!(proof["signature_summary"]["object_count"], 2);
    assert_eq!(proof["signature_summary"]["signed_object_count"], 1);
    assert_eq!(proof["signature_summary"]["valid_signature_count"], 1);
}

#[test]
fn object_write_without_signing_key_remains_unsigned() {
    let repo = tempfile::tempdir().unwrap();
    init_git_repo(repo.path());
    let repo_arg = repo.path().to_str().unwrap();
    rickygit_json(&["init", "--repo", repo_arg, "--json"]);
    let body_file = fixture_path("work-intent.valid.json");

    let write = rickygit_json(&[
        "object",
        "write",
        "--repo",
        repo_arg,
        "--kind",
        "agent.intent",
        "--body-file",
        &body_file,
        "--json",
    ]);
    let object_id = write["object_id"].as_str().unwrap();
    let read = rickygit_json(&[
        "object",
        "read",
        "--repo",
        repo_arg,
        "--object-id",
        object_id,
        "--json",
    ]);
    assert!(
        read["object"].get("signatures").is_none()
            || read["object"]["signatures"].as_array().unwrap().is_empty(),
        "unsigned object must not carry a signatures field"
    );
    let verify = rickygit_json(&[
        "object",
        "verify",
        "--repo",
        repo_arg,
        "--object-id",
        object_id,
        "--json",
    ]);
    assert_eq!(verify["signature_count"], 0);
    assert_eq!(verify["valid_signature_count"], 0);
}

#[test]
fn sync_push_with_signing_key_emits_signed_ref_expectations() {
    let remote = tempfile::tempdir().unwrap();
    git_success(remote.path(), ["init", "--bare"]);

    let repo = tempfile::tempdir().unwrap();
    init_git_repo(repo.path());
    let repo_arg = repo.path().to_str().unwrap();
    rickygit_json(&["init", "--repo", repo_arg, "--json"]);
    git_success(
        repo.path(),
        ["remote", "add", "origin", remote.path().to_str().unwrap()],
    );

    // Write an intent so there is at least one refs/rickydata/* ref to attest.
    rickygit_json(&[
        "intent",
        "write",
        "--repo",
        repo_arg,
        &fixture_path("work-intent.valid.json"),
        "--json",
    ]);

    let key_dir = tempfile::tempdir().unwrap();
    let key_path = key_dir.path().join("signer.key");
    let key_arg = key_path.to_str().unwrap();
    let key_report = rickygit_json(&["key", "generate", "--output", key_arg, "--json"]);
    let expected_public_key = key_report["public_key"].as_str().unwrap().to_string();

    let push = rickygit_json(&[
        "sync",
        "push",
        "--repo",
        repo_arg,
        "--remote",
        "origin",
        "--signing-key-file",
        key_arg,
        "--signer-label",
        "alice",
        "--json",
    ]);

    let expectations = push["signed_ref_expectations"].as_array().unwrap();
    assert!(
        !expectations.is_empty(),
        "expected at least one signed expectation for the intent object ref"
    );
    for expectation in expectations {
        assert!(
            expectation["ref_name"]
                .as_str()
                .unwrap()
                .starts_with("refs/rickydata/"),
            "expectation ref_name should be under refs/rickydata: {expectation}"
        );
        assert_eq!(
            expectation["expected_previous_oid"], expectation["new_oid"],
            "sync push attestation expects the ref to stay at the same oid"
        );
        assert_eq!(expectation["signature"]["algorithm"], "ed25519");
        assert_eq!(expectation["signature"]["public_key"], expected_public_key);
        assert_eq!(expectation["signature"]["signer_label"], "alice");
    }
}

#[test]
fn sync_push_without_signing_key_omits_signed_ref_expectations() {
    let remote = tempfile::tempdir().unwrap();
    git_success(remote.path(), ["init", "--bare"]);

    let repo = tempfile::tempdir().unwrap();
    init_git_repo(repo.path());
    let repo_arg = repo.path().to_str().unwrap();
    rickygit_json(&["init", "--repo", repo_arg, "--json"]);
    git_success(
        repo.path(),
        ["remote", "add", "origin", remote.path().to_str().unwrap()],
    );
    rickygit_json(&[
        "intent",
        "write",
        "--repo",
        repo_arg,
        &fixture_path("work-intent.valid.json"),
        "--json",
    ]);

    let push = rickygit_json(&[
        "sync", "push", "--repo", repo_arg, "--remote", "origin", "--json",
    ]);
    // skip_serializing_if = "Vec::is_empty" should keep the field out of the JSON entirely.
    assert!(
        push.get("signed_ref_expectations").is_none(),
        "unsigned push must not emit a signed_ref_expectations field: {push}"
    );
}

#[test]
fn key_init_creates_agent_key_file() {
    let home = tempfile::tempdir().unwrap();
    let report = rickygit_json_env(
        &["key", "init", "--agent-id", "agent:test", "--json"],
        &[("HOME", home.path().to_str().unwrap())],
    );
    assert_eq!(report["status"], "ok");
    assert_eq!(report["algorithm"], "ed25519");
    assert_eq!(report["agent_id"], "agent:test");
    let key_path = home.path().join(".rickydata/signing-keys/agent_test.key");
    assert!(
        key_path.exists(),
        "key file must exist at {}",
        key_path.display()
    );
    let key_bytes = std::fs::read(&key_path).unwrap();
    assert_eq!(key_bytes.len(), 32);
    let pk = report["public_key"].as_str().unwrap();
    assert_eq!(pk.len(), 64);
}

#[test]
fn key_init_force_overwrites_existing() {
    let home = tempfile::tempdir().unwrap();
    let first = rickygit_json_env(
        &["key", "init", "--agent-id", "agent:overwrite", "--json"],
        &[("HOME", home.path().to_str().unwrap())],
    );
    assert_eq!(first["status"], "ok");
    let first_pk = first["public_key"].as_str().unwrap().to_string();

    let second = rickygit_json_env(
        &[
            "key",
            "init",
            "--agent-id",
            "agent:overwrite",
            "--force",
            "--json",
        ],
        &[("HOME", home.path().to_str().unwrap())],
    );
    assert_eq!(second["status"], "ok");
    let second_pk = second["public_key"].as_str().unwrap().to_string();
    assert_ne!(first_pk, second_pk, "force should generate new key");
}

#[test]
fn key_init_without_force_refuses_overwrite() {
    let home = tempfile::tempdir().unwrap();
    rickygit_json_env(
        &["key", "init", "--agent-id", "agent:refuse", "--json"],
        &[("HOME", home.path().to_str().unwrap())],
    );
    let mut cmd = Command::cargo_bin("rickygit").unwrap();
    cmd.env("HOME", home.path().to_str().unwrap())
        .args(["key", "init", "--agent-id", "agent:refuse", "--json"])
        .assert()
        .failure()
        .stdout(predicate::str::contains("already exists"));
}

#[test]
fn resolve_signing_key_env_fallback_uses_key_file_env() {
    let home = tempfile::tempdir().unwrap();
    let repo = tempfile::tempdir().unwrap();
    init_git_repo(repo.path());
    create_initial_commit(repo.path());
    let repo_arg = repo.path().to_str().unwrap();
    rickygit_json(&["init", "--repo", repo_arg, "--json"]);

    let key_report = rickygit_json_env(
        &["key", "init", "--agent-id", "agent:envtest", "--json"],
        &[("HOME", home.path().to_str().unwrap())],
    );
    let key_path = key_report["key_path"].as_str().unwrap();

    let work = rickygit_json_env(
        &[
            "work",
            "start",
            "--repo",
            repo_arg,
            "--objective",
            "test env fallback",
            "--agent-id",
            "agent:envtest",
            "--json",
        ],
        &[
            ("HOME", home.path().to_str().unwrap()),
            ("RICKYGIT_SIGNING_KEY_FILE", key_path),
        ],
    );
    assert_eq!(work["status"], "ok");
    assert_ne!(
        work["intent_object"]["object_id"].as_str(),
        None,
        "should produce signed intent"
    );
}

#[test]
fn resolve_signing_key_agent_id_fallback_auto_signs() {
    let home = tempfile::tempdir().unwrap();
    let repo = tempfile::tempdir().unwrap();
    init_git_repo(repo.path());
    create_initial_commit(repo.path());
    let repo_arg = repo.path().to_str().unwrap();
    rickygit_json(&["init", "--repo", repo_arg, "--json"]);

    rickygit_json_env(
        &["key", "init", "--agent-id", "agent:autosign", "--json"],
        &[("HOME", home.path().to_str().unwrap())],
    );

    let work = rickygit_json_env(
        &[
            "work",
            "start",
            "--repo",
            repo_arg,
            "--objective",
            "test agent-id auto-sign",
            "--agent-id",
            "agent:autosign",
            "--json",
        ],
        &[("HOME", home.path().to_str().unwrap())],
    );
    assert_eq!(work["status"], "ok");

    let verify = rickygit_json(&["sync", "verify", "--repo", repo_arg, "--json"]);
    let signed_count = verify["signed_object_count"].as_u64().unwrap_or(0);
    assert!(
        signed_count > 0,
        "auto-signing should produce signed objects, got verify: {verify}"
    );
}

#[test]
fn doctor_with_relay_url_reports_health() {
    let relay = start_relay();
    let report = rickygit_json(&["doctor", "--relay-url", &relay.url, "--json"]);
    assert_eq!(report["status"], "ok");
    assert_eq!(report["relay_health"], "ok");
}

#[test]
fn doctor_with_agent_id_reports_signing_key_status() {
    let home = tempfile::tempdir().unwrap();
    let report_missing = rickygit_json_env(
        &["doctor", "--agent-id", "agent:nokey", "--json"],
        &[("HOME", home.path().to_str().unwrap())],
    );
    assert_eq!(report_missing["signing_key_configured"], false);

    rickygit_json_env(
        &["key", "init", "--agent-id", "agent:haskey", "--json"],
        &[("HOME", home.path().to_str().unwrap())],
    );
    let report_present = rickygit_json_env(
        &["doctor", "--agent-id", "agent:haskey", "--json"],
        &[("HOME", home.path().to_str().unwrap())],
    );
    assert_eq!(report_present["signing_key_configured"], true);
}

// ---------- TEE mock ----------
// The mock signer and the live-health test only apply when the `tee` feature
// links the rickydata_auth signer client. The public build omits the feature.

#[cfg(feature = "tee")]
struct TeeFixture {
    shutdown: Option<tokio::sync::oneshot::Sender<()>>,
    handle: Option<JoinHandle<()>>,
    url: String,
}

#[cfg(feature = "tee")]
impl Drop for TeeFixture {
    fn drop(&mut self) {
        if let Some(shutdown) = self.shutdown.take() {
            let _ = shutdown.send(());
        }
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
}

#[cfg(feature = "tee")]
async fn mock_tee_health() -> Json<Value> {
    Json(serde_json::json!({
        "productionSigningEnabled": true,
        "status": "ok"
    }))
}

#[cfg(feature = "tee")]
fn start_tee() -> TeeFixture {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    listener.set_nonblocking(true).unwrap();
    let (shutdown, shutdown_rx) = tokio::sync::oneshot::channel();
    let handle = std::thread::spawn(move || {
        let runtime = tokio::runtime::Runtime::new().unwrap();
        runtime.block_on(async move {
            let listener = tokio::net::TcpListener::from_std(listener).unwrap();
            let app = axum::Router::new().route("/health", get(mock_tee_health));
            axum::serve(listener, app)
                .with_graceful_shutdown(async {
                    let _ = shutdown_rx.await;
                })
                .await
                .unwrap();
        });
    });
    let url = format!("http://{addr}");
    for _ in 0..100 {
        match reqwest::blocking::get(format!("{url}/health")) {
            Ok(response) if response.status().is_success() => {
                return TeeFixture {
                    shutdown: Some(shutdown),
                    handle: Some(handle),
                    url,
                };
            }
            _ => {
                assert!(
                    !handle.is_finished(),
                    "mock TEE exited before health check succeeded"
                );
                std::thread::sleep(Duration::from_millis(50));
            }
        }
    }
    panic!("mock TEE did not become healthy at {url}");
}

#[cfg(feature = "tee")]
#[test]
fn receipt_verify_reports_signatures_and_tee_health() {
    let repo = tempfile::tempdir().unwrap();
    init_git_repo(repo.path());
    let repo_arg = repo.path().to_str().unwrap();
    rickygit_json(&["init", "--repo", repo_arg, "--json"]);

    let key_dir = tempfile::tempdir().unwrap();
    let key_path = key_dir.path().join("signer.key");
    let key_arg = key_path.to_str().unwrap();
    rickygit_json(&["key", "generate", "--output", key_arg, "--json"]);

    let intent_path = fixture_path("work-intent.valid.json");
    let write = rickygit_json(&[
        "intent",
        "write",
        "--repo",
        repo_arg,
        &intent_path,
        "--signing-key-file",
        key_arg,
        "--json",
    ]);
    assert_eq!(write["valid"], true);
    let object_id = write["object"]["object_id"].as_str().unwrap();

    let tee = start_tee();
    let report = rickygit_json(&[
        "receipt",
        "verify",
        "--repo",
        repo_arg,
        "--object-id",
        object_id,
        "--tee-url",
        &tee.url,
        "--json",
    ]);

    assert_eq!(report["status"], "ok");
    assert_eq!(report["object_id"], object_id);
    assert_eq!(report["has_signatures"], true);
    assert!(
        report["signature_count"].as_u64().unwrap() >= 1,
        "expected at least one signature, got: {report}"
    );
    assert_eq!(report["tee_reachable"], true);
    assert_eq!(report["tee_production_signing"], true);
}

#[test]
fn receipt_verify_without_tee_url_skips_tee_check() {
    let repo = tempfile::tempdir().unwrap();
    init_git_repo(repo.path());
    let repo_arg = repo.path().to_str().unwrap();
    rickygit_json(&["init", "--repo", repo_arg, "--json"]);

    let intent_path = fixture_path("work-intent.valid.json");
    let write = rickygit_json(&[
        "intent",
        "write",
        "--repo",
        repo_arg,
        &intent_path,
        "--json",
    ]);
    assert_eq!(write["valid"], true);
    let object_id = write["object"]["object_id"].as_str().unwrap();

    let report = rickygit_json(&[
        "receipt",
        "verify",
        "--repo",
        repo_arg,
        "--object-id",
        object_id,
        "--json",
    ]);

    assert_eq!(report["status"], "ok");
    assert_eq!(report["object_id"], object_id);
    assert_eq!(report["has_signatures"], false);
    assert_eq!(report["signature_count"], 0);
    assert!(
        report["tee_reachable"].is_null(),
        "tee_reachable should be null without --tee-url, got: {report}"
    );
    assert!(
        report["tee_production_signing"].is_null(),
        "tee_production_signing should be null without --tee-url, got: {report}"
    );
}

#[test]
fn note_send_inbox_and_list_round_trip() {
    let repo = tempfile::tempdir().unwrap();
    init_git_repo(repo.path());
    let repo_arg = repo.path().to_str().unwrap();
    rickygit_json(&["init", "--repo", repo_arg, "--json"]);

    let sent = rickygit_json(&[
        "note",
        "send",
        "--repo",
        repo_arg,
        "--from",
        "agent:hermes",
        "--to",
        "claude-code",
        "--text",
        "AllPsy factor-fit rerun done",
        "--thread",
        "allpsy",
        "--json",
    ]);
    assert_eq!(sent["status"], "ok");
    assert_eq!(sent["valid"], true);
    assert!(
        sent["object"]["object_id"]
            .as_str()
            .unwrap()
            .starts_with("sha256:")
    );
    assert!(sent["note"]["created_at_ms"].as_u64().unwrap() > 0);

    rickygit_json(&[
        "note",
        "send",
        "--repo",
        repo_arg,
        "--from",
        "agent:hermes",
        "--to",
        "all",
        "--text",
        "fleet heartbeat",
        "--json",
    ]);
    rickygit_json(&[
        "note",
        "send",
        "--repo",
        repo_arg,
        "--from",
        "codex",
        "--to",
        "kai",
        "--text",
        "blocked on token",
        "--json",
    ]);

    // claude-code sees its direct note plus the broadcast, but not the codex->kai note.
    let inbox = rickygit_json(&[
        "note",
        "inbox",
        "--repo",
        repo_arg,
        "--agent",
        "claude-code",
        "--json",
    ]);
    assert_eq!(inbox["count"], 2);
    assert!(inbox["read_marker_ms"].as_u64().unwrap() > 0);
    assert_eq!(inbox["marker_advanced"], true);

    // Reading again advances past everything: nothing new.
    let inbox_again = rickygit_json(&[
        "note",
        "inbox",
        "--repo",
        repo_arg,
        "--agent",
        "claude-code",
        "--json",
    ]);
    assert_eq!(inbox_again["count"], 0);

    // kai receives the broadcast and the direct note addressed to it.
    let kai_inbox = rickygit_json(&[
        "note", "inbox", "--repo", repo_arg, "--agent", "kai", "--json",
    ]);
    assert_eq!(kai_inbox["count"], 2);

    // Full history is visible regardless of read markers.
    let list = rickygit_json(&["note", "list", "--repo", repo_arg, "--json"]);
    assert_eq!(list["count"], 3);
    let thread_filtered = rickygit_json(&[
        "note", "list", "--repo", repo_arg, "--thread", "allpsy", "--json",
    ]);
    assert_eq!(thread_filtered["count"], 1);
    let to_filtered = rickygit_json(&["note", "list", "--repo", repo_arg, "--to", "kai", "--json"]);
    assert_eq!(to_filtered["count"], 1);
}

#[test]
fn note_inbox_peek_does_not_advance_marker() {
    let repo = tempfile::tempdir().unwrap();
    init_git_repo(repo.path());
    let repo_arg = repo.path().to_str().unwrap();
    rickygit_json(&["init", "--repo", repo_arg, "--json"]);
    rickygit_json(&[
        "note",
        "send",
        "--repo",
        repo_arg,
        "--from",
        "codex",
        "--to",
        "claude-code",
        "--text",
        "who owns packet routing?",
        "--json",
    ]);

    let first = rickygit_json(&[
        "note",
        "inbox",
        "--repo",
        repo_arg,
        "--agent",
        "claude-code",
        "--peek",
        "--json",
    ]);
    assert_eq!(first["count"], 1);
    assert_eq!(first["marker_advanced"], false);

    // A peek must not consume the note: a second peek still shows it.
    let second = rickygit_json(&[
        "note",
        "inbox",
        "--repo",
        repo_arg,
        "--agent",
        "claude-code",
        "--peek",
        "--json",
    ]);
    assert_eq!(second["count"], 1);
}

#[test]
fn note_send_rejects_empty_body() {
    let repo = tempfile::tempdir().unwrap();
    init_git_repo(repo.path());
    let repo_arg = repo.path().to_str().unwrap();
    rickygit_json(&["init", "--repo", repo_arg, "--json"]);

    let report = rickygit_json(&[
        "note",
        "send",
        "--repo",
        repo_arg,
        "--from",
        "agent:hermes",
        "--to",
        "kai",
        "--text",
        "   ",
        "--json",
    ]);
    assert_eq!(report["status"], "invalid");
    assert_eq!(report["valid"], false);
    assert!(report["object"].is_null());
    assert_eq!(report["diagnostics"][0]["code"], "NOTE003");
}

#[test]
fn note_recovers_across_clone_via_refs() {
    let remote = tempfile::tempdir().unwrap();
    git_success(remote.path(), ["init", "--bare"]);
    let repo_a = tempfile::tempdir().unwrap();
    init_git_repo(repo_a.path());
    let repo_a_arg = repo_a.path().to_str().unwrap();
    rickygit_json(&["init", "--repo", repo_a_arg, "--json"]);
    let sent = rickygit_json(&[
        "note",
        "send",
        "--repo",
        repo_a_arg,
        "--from",
        "agent:hermes",
        "--to",
        "claude-code",
        "--text",
        "cross-fleet hello",
        "--json",
    ]);
    let object_id = sent["object"]["object_id"].as_str().unwrap();
    git_success(
        repo_a.path(),
        ["remote", "add", "origin", remote.path().to_str().unwrap()],
    );
    git_success(
        repo_a.path(),
        ["push", "origin", "refs/rickydata/*:refs/rickydata/*"],
    );

    let checkout = tempfile::tempdir().unwrap();
    git_success(
        checkout.path(),
        [
            "clone",
            remote.path().to_str().unwrap(),
            checkout.path().join("repo-b").to_str().unwrap(),
        ],
    );
    let repo_b = checkout.path().join("repo-b");
    git_success(
        &repo_b,
        ["fetch", "origin", "refs/rickydata/*:refs/rickydata/*"],
    );
    let repo_b_arg = repo_b.to_str().unwrap();
    rickygit_json(&["init", "--repo", repo_b_arg, "--json"]);
    std::fs::remove_dir_all(repo_b.join(".git/rickydata/cache/objects")).unwrap();

    // The note is recoverable from refs alone, with no local object cache.
    let inbox = rickygit_json(&[
        "note",
        "inbox",
        "--repo",
        repo_b_arg,
        "--agent",
        "claude-code",
        "--json",
    ]);
    assert_eq!(inbox["count"], 1);
    assert_eq!(inbox["notes"][0]["object_id"], object_id);
    assert_eq!(inbox["notes"][0]["note"]["body"], "cross-fleet hello");
}

#[test]
fn work_start_in_place_records_against_main_tree_without_worktree() {
    let repo = tempfile::tempdir().unwrap();
    init_git_repo(repo.path());
    create_initial_commit(repo.path());
    let repo_arg = repo.path().to_str().unwrap();
    rickygit_json(&["init", "--repo", repo_arg, "--json"]);

    let started = rickygit_json(&[
        "work",
        "start",
        "--repo",
        repo_arg,
        "--objective",
        "in-place trial",
        "--agent-id",
        "agent:cc",
        "--idempotency-key",
        "k1",
        "--in-place",
        "--json",
    ]);
    assert_eq!(started["attempt"]["in_place"], true);
    assert_eq!(started["worktree_created"], false);
    let attempt_id = started["attempt"]["attempt_id"].as_str().unwrap();

    // No isolated worktree was allocated.
    let worktrees = repo.path().join(".git/rickydata/worktrees");
    let worktree_count = std::fs::read_dir(&worktrees)
        .map(|d| d.count())
        .unwrap_or(0);
    assert_eq!(
        worktree_count, 0,
        "in-place attempt must not create a worktree"
    );

    // Edits to the main working tree are detected and packaged in place.
    std::fs::write(repo.path().join("feature.txt"), "brand new\n").unwrap();
    std::fs::write(repo.path().join("README.md"), "# test repo\nedited\n").unwrap();

    let detect = rickygit_json(&[
        "change",
        "detect",
        "--repo",
        repo_arg,
        "--attempt-id",
        attempt_id,
        "--json",
    ]);
    let files = detect["change"]["file_paths"].as_array().unwrap();
    let names: Vec<&str> = files.iter().map(|v| v.as_str().unwrap()).collect();
    assert!(names.contains(&"feature.txt"), "got {names:?}");
    assert!(names.contains(&"README.md"), "got {names:?}");

    let prepare = rickygit_json(&[
        "patch",
        "prepare",
        "--repo",
        repo_arg,
        "--attempt-id",
        attempt_id,
        "--json",
    ]);
    assert!(
        prepare["patch"]["patch_id"]
            .as_str()
            .unwrap()
            .starts_with("sha256:")
    );
}

#[test]
fn attempt_start_in_place_flag_sets_marker() {
    let repo = tempfile::tempdir().unwrap();
    init_git_repo(repo.path());
    create_initial_commit(repo.path());
    let repo_arg = repo.path().to_str().unwrap();
    rickygit_json(&["init", "--repo", repo_arg, "--json"]);
    let write = rickygit_json(&[
        "intent",
        "write",
        "--repo",
        repo_arg,
        &fixture_path("work-intent.valid.json"),
        "--json",
    ]);
    let intent_id = write["object"]["object_id"].as_str().unwrap();

    let started = rickygit_json(&[
        "attempt",
        "start",
        "--repo",
        repo_arg,
        "--intent-id",
        intent_id,
        "--agent-id",
        "agent:cc",
        "--idempotency-key",
        "k1",
        "--in-place",
        "--json",
    ]);
    assert_eq!(started["attempt"]["in_place"], true);
    assert_eq!(started["worktree_created"], false);
    let worktrees = repo.path().join(".git/rickydata/worktrees");
    let worktree_count = std::fs::read_dir(&worktrees)
        .map(|d| d.count())
        .unwrap_or(0);
    assert_eq!(worktree_count, 0);
}
