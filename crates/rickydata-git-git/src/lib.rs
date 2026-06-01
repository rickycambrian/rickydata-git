use rickydata_git_core::{
    CanonicalObject, DEFAULT_SCHEMA_VERSION, SIGNATURE_ALGORITHM_ED25519, SignedRefExpectation,
    canonical_json, canonical_object_id, stable_json_hash, verify_ref_expectation_signature,
    verify_signature,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::path::{Path, PathBuf};

pub const RICKYDATA_STORE_VERSION: &str = "rickydata.git.store.v1";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct RepoInspection {
    pub requested_path: PathBuf,
    pub is_git_repo: bool,
    pub root_path: Option<PathBuf>,
    pub git_dir: Option<PathBuf>,
    pub branch: Option<String>,
    pub head_commit: Option<String>,
    pub dirty: Option<bool>,
    pub object_format: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct RickydataInitReport {
    pub status: InitStatus,
    pub repo_root: Option<PathBuf>,
    pub git_dir: PathBuf,
    pub metadata_dir: PathBuf,
    pub object_dir: PathBuf,
    pub bundle_dir: PathBuf,
    pub temp_dir: PathBuf,
    pub refs_dir: PathBuf,
    pub store_version: String,
    pub created_paths: Vec<PathBuf>,
    pub existing_paths: Vec<PathBuf>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ObjectWriteReport {
    pub status: ObjectWriteStatus,
    pub object_id: String,
    pub body_hash: String,
    pub kind: String,
    pub schema_version: String,
    pub cache_path: PathBuf,
    pub ref_name: String,
    pub git_object_id: String,
    pub bytes_written: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ObjectWriteStatus {
    Written,
    AlreadyExists,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct ObjectReadReport {
    pub object_id: String,
    pub cache_path: PathBuf,
    pub source: ObjectReadSource,
    pub object: CanonicalObject<Value>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ObjectListEntry {
    pub object_id: String,
    pub kind: String,
    pub body_hash: String,
    pub ref_name: String,
    pub git_object_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ObjectVerifyReport {
    pub object_id: String,
    pub cache_path: PathBuf,
    pub source: ObjectReadSource,
    pub valid: bool,
    pub diagnostics: Vec<ObjectDiagnostic>,
    pub computed_object_id: Option<String>,
    pub computed_body_hash: Option<String>,
    #[serde(default)]
    pub signature_count: u32,
    #[serde(default)]
    pub valid_signature_count: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ObjectReadSource {
    Cache,
    GitRef,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ObjectDiagnostic {
    pub code: String,
    pub message: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum InitStatus {
    Created,
    AlreadyInitialized,
}

impl RepoInspection {
    pub fn not_git_repo(path: impl Into<PathBuf>) -> Self {
        Self {
            requested_path: path.into(),
            is_git_repo: false,
            root_path: None,
            git_dir: None,
            branch: None,
            head_commit: None,
            dirty: None,
            object_format: None,
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum GitInspectError {
    #[error("failed to discover repository: {0}")]
    Discover(String),
    #[error("path is not inside a Git repository: {0}")]
    NotGitRepository(PathBuf),
    #[error("failed to inspect repository status: {0}")]
    Status(String),
    #[error("failed to initialize Rickydata Git metadata: {0}")]
    InitIo(String),
    #[error(
        "Rickydata Git store is not initialized at {0}; run `rickygit init --repo <path> --json` first"
    )]
    StoreNotInitialized(PathBuf),
    #[error("unsupported Rickydata Git store version `{found}` at {path}; expected `{expected}`")]
    StoreVersionMismatch {
        path: PathBuf,
        found: String,
        expected: &'static str,
    },
    #[error("invalid object id `{0}`; expected sha256:<64 lowercase hex chars>")]
    InvalidObjectId(String),
    #[error("cached object does not exist: {0}")]
    ObjectNotFound(PathBuf),
    #[error("cached object bytes conflict for {object_id} at {path}")]
    ObjectConflict { object_id: String, path: PathBuf },
    #[error("failed to read or write cached object: {0}")]
    ObjectIo(String),
    #[error("failed to parse cached object JSON: {0}")]
    ObjectParse(String),
    #[error("failed to write Rickydata object to Git object database or refs: {0}")]
    ObjectGitWrite(String),
    #[error("failed to read Rickydata object from Git object database or refs: {0}")]
    ObjectGitRead(String),
    #[error(
        "signed ref expectation for {ref_name} did not match repository state: expected {expected:?}, found {found:?}"
    )]
    RefExpectationMismatch {
        ref_name: String,
        expected: Option<String>,
        found: Option<String>,
    },
    #[error("signed ref expectation signature for {ref_name} did not verify")]
    RefExpectationBadSignature { ref_name: String },
    #[error("signed ref expectation signature for {ref_name} failed to validate: {message}")]
    RefExpectationSignatureError { ref_name: String, message: String },
}

pub fn inspect_repository(path: impl AsRef<Path>) -> Result<RepoInspection, GitInspectError> {
    let requested_path = path.as_ref().to_path_buf();
    let repo = match gix::discover(path.as_ref()) {
        Ok(repo) => repo,
        Err(error) if has_git_marker(path.as_ref()) => {
            return Err(GitInspectError::Discover(error.to_string()));
        }
        Err(_) => return Ok(RepoInspection::not_git_repo(requested_path)),
    };

    let branch = repo
        .head_name()
        .ok()
        .flatten()
        .map(|name| name.shorten().to_string());
    let head_commit = repo.head_id().ok().map(|id| id.to_string());
    let dirty = status_has_any_change(&repo)?;

    Ok(RepoInspection {
        requested_path,
        is_git_repo: true,
        root_path: repo.workdir().map(Path::to_path_buf),
        git_dir: Some(repo.git_dir().to_path_buf()),
        branch,
        head_commit,
        dirty: Some(dirty),
        object_format: Some(repo.object_hash().to_string()),
    })
}

pub fn init_rickydata_repository(
    path: impl AsRef<Path>,
) -> Result<RickydataInitReport, GitInspectError> {
    let requested_path = path.as_ref();
    let repo = match gix::discover(requested_path) {
        Ok(repo) => repo,
        Err(error) if has_git_marker(requested_path) => {
            return Err(GitInspectError::Discover(error.to_string()));
        }
        Err(_) => {
            return Err(GitInspectError::NotGitRepository(
                requested_path.to_path_buf(),
            ));
        }
    };

    let git_dir = repo.git_dir().to_path_buf();
    let metadata_dir = git_dir.join("rickydata");
    let cache_dir = metadata_dir.join("cache");
    let object_dir = cache_dir.join("objects").join("sha256");
    let bundle_dir = cache_dir.join("bundles");
    let temp_dir = metadata_dir.join("tmp");
    let refs_dir = git_dir.join("refs").join("rickydata");
    let version_file = metadata_dir.join("VERSION");

    let mut created_paths = Vec::new();
    let mut existing_paths = Vec::new();

    for path in [
        &metadata_dir,
        &cache_dir,
        &object_dir,
        &bundle_dir,
        &temp_dir,
        &refs_dir,
        &refs_dir.join("objects"),
        &refs_dir.join("discovery"),
        &refs_dir.join("intents"),
        &refs_dir.join("attempts"),
        &refs_dir.join("runs"),
        &refs_dir.join("policies"),
    ] {
        ensure_dir(path, &mut created_paths, &mut existing_paths)?;
    }

    ensure_version_file(&version_file, &mut created_paths, &mut existing_paths)?;

    let status = if created_paths.is_empty() {
        InitStatus::AlreadyInitialized
    } else {
        InitStatus::Created
    };

    Ok(RickydataInitReport {
        status,
        repo_root: repo.workdir().map(Path::to_path_buf),
        git_dir,
        metadata_dir,
        object_dir,
        bundle_dir,
        temp_dir,
        refs_dir,
        store_version: RICKYDATA_STORE_VERSION.to_string(),
        created_paths,
        existing_paths,
    })
}

pub fn write_cached_object(
    path: impl AsRef<Path>,
    kind: &str,
    body: Value,
) -> Result<ObjectWriteReport, GitInspectError> {
    let store = open_initialized_store(path.as_ref())?;
    let canonical_body = canonical_json(&body);
    let object = CanonicalObject::new(kind, DEFAULT_SCHEMA_VERSION, 0, canonical_body)
        .map_err(|error| GitInspectError::ObjectIo(error.to_string()))?;
    let object_value = serde_json::to_value(&object)
        .map_err(|error| GitInspectError::ObjectIo(error.to_string()))?;
    let object_bytes = serde_json::to_vec(&canonical_json(&object_value))
        .map_err(|error| GitInspectError::ObjectIo(error.to_string()))?;
    let cache_path = object_cache_path(&store.git_dir, &object.object_id)?;

    let status = write_object_cache_bytes(&store, &cache_path, &object.object_id, &object_bytes)?;
    let ref_name = object_ref_name(&object.object_id)?;
    let git_object_id =
        write_git_object_ref(&store.repo, &object.object_id, &ref_name, &object_bytes)?;

    Ok(ObjectWriteReport {
        status,
        object_id: object.object_id,
        body_hash: object.body_hash,
        kind: object.kind,
        schema_version: object.schema_version,
        cache_path,
        ref_name,
        git_object_id,
        bytes_written: object_bytes.len() as u64,
    })
}

pub fn write_canonical_object(
    path: impl AsRef<Path>,
    object: &CanonicalObject<Value>,
) -> Result<ObjectWriteReport, GitInspectError> {
    let store = open_initialized_store(path.as_ref())?;
    let mut diagnostics = Vec::new();
    object_field_diagnostics(&object.object_id, object, &mut diagnostics)?;
    if !diagnostics.is_empty() {
        let summary = diagnostics
            .iter()
            .map(|diagnostic| format!("{}: {}", diagnostic.code, diagnostic.message))
            .collect::<Vec<_>>()
            .join("; ");
        return Err(GitInspectError::ObjectIo(format!(
            "canonical object {} failed validation: {summary}",
            object.object_id
        )));
    }

    let object_bytes = canonical_object_bytes(object)?;
    let cache_path = object_cache_path(&store.git_dir, &object.object_id)?;
    let status = write_object_cache_bytes(&store, &cache_path, &object.object_id, &object_bytes)?;
    let ref_name = object_ref_name(&object.object_id)?;
    let git_object_id =
        write_git_object_ref(&store.repo, &object.object_id, &ref_name, &object_bytes)?;

    Ok(ObjectWriteReport {
        status,
        object_id: object.object_id.clone(),
        body_hash: object.body_hash.clone(),
        kind: object.kind.clone(),
        schema_version: object.schema_version.clone(),
        cache_path,
        ref_name,
        git_object_id,
        bytes_written: object_bytes.len() as u64,
    })
}

fn write_object_cache_bytes(
    store: &RickydataStore,
    cache_path: &Path,
    object_id: &str,
    object_bytes: &[u8],
) -> Result<ObjectWriteStatus, GitInspectError> {
    let parent = cache_path
        .parent()
        .expect("object cache path should have a parent directory");
    std::fs::create_dir_all(parent)
        .map_err(|error| GitInspectError::ObjectIo(error.to_string()))?;

    if cache_path.exists() {
        let existing = std::fs::read(cache_path)
            .map_err(|error| GitInspectError::ObjectIo(error.to_string()))?;
        if existing != object_bytes {
            return Err(GitInspectError::ObjectConflict {
                object_id: object_id.to_string(),
                path: cache_path.to_path_buf(),
            });
        }
        Ok(ObjectWriteStatus::AlreadyExists)
    } else {
        let temp_path = store
            .temp_dir
            .join(format!("{}.tmp", object_id.replace(':', "-")));
        std::fs::write(&temp_path, object_bytes)
            .map_err(|error| GitInspectError::ObjectIo(error.to_string()))?;
        std::fs::rename(&temp_path, cache_path)
            .map_err(|error| GitInspectError::ObjectIo(error.to_string()))?;
        Ok(ObjectWriteStatus::Written)
    }
}

pub fn read_cached_object(
    path: impl AsRef<Path>,
    object_id: &str,
) -> Result<ObjectReadReport, GitInspectError> {
    let store = open_initialized_store(path.as_ref())?;
    let cache_path = object_cache_path(&store.git_dir, object_id)?;
    let (source, object) = read_object_from_cache_or_ref(&store, &cache_path, object_id)?;

    Ok(ObjectReadReport {
        object_id: object_id.to_string(),
        cache_path,
        source,
        object,
    })
}

pub fn list_ref_backed_objects(
    path: impl AsRef<Path>,
    kind: Option<&str>,
) -> Result<Vec<ObjectListEntry>, GitInspectError> {
    let store = open_initialized_store(path.as_ref())?;
    let refs = store
        .repo
        .references()
        .map_err(|error| GitInspectError::ObjectGitRead(error.to_string()))?;
    let iter = refs
        .prefixed("refs/rickydata/objects/sha256/")
        .map_err(|error| GitInspectError::ObjectGitRead(error.to_string()))?;
    let mut entries = Vec::new();

    for reference in iter {
        let reference =
            reference.map_err(|error| GitInspectError::ObjectGitRead(error.to_string()))?;
        let ref_name = reference.name().as_bstr().to_string();
        let git_object_id = reference.id().to_string();
        let blob = store
            .repo
            .find_blob(reference.id().detach())
            .map_err(|error| GitInspectError::ObjectGitRead(error.to_string()))?;
        let object = read_object_bytes(&blob.data)?;
        if kind.is_some_and(|expected| object.kind != expected) {
            continue;
        }
        entries.push(ObjectListEntry {
            object_id: object.object_id,
            kind: object.kind,
            body_hash: object.body_hash,
            ref_name,
            git_object_id,
        });
    }
    entries.sort_by(|left, right| left.object_id.cmp(&right.object_id));
    Ok(entries)
}

pub fn verify_cached_object(
    path: impl AsRef<Path>,
    object_id: &str,
) -> Result<ObjectVerifyReport, GitInspectError> {
    let store = open_initialized_store(path.as_ref())?;
    let cache_path = object_cache_path(&store.git_dir, object_id)?;
    let mut diagnostics = Vec::new();

    if !cache_path.exists() {
        return match read_object_from_cache_or_ref(&store, &cache_path, object_id) {
            Ok((source, object)) => {
                verify_object_fields(object_id, cache_path, source, object, diagnostics)
            }
            Err(GitInspectError::ObjectNotFound(_)) => {
                diagnostics.push(ObjectDiagnostic {
                    code: "OBJECT001".to_string(),
                    message: "cached object file and ref-backed object are missing".to_string(),
                });
                Ok(ObjectVerifyReport {
                    object_id: object_id.to_string(),
                    cache_path,
                    source: ObjectReadSource::Cache,
                    valid: false,
                    diagnostics,
                    computed_object_id: None,
                    computed_body_hash: None,
                    signature_count: 0,
                    valid_signature_count: 0,
                })
            }
            Err(error) => {
                diagnostics.push(ObjectDiagnostic {
                    code: "OBJECT007".to_string(),
                    message: error.to_string(),
                });
                Ok(ObjectVerifyReport {
                    object_id: object_id.to_string(),
                    cache_path,
                    source: ObjectReadSource::GitRef,
                    valid: false,
                    diagnostics,
                    computed_object_id: None,
                    computed_body_hash: None,
                    signature_count: 0,
                    valid_signature_count: 0,
                })
            }
        };
    }

    let object = match read_object_file(&cache_path) {
        Ok(object) => object,
        Err(error) => {
            diagnostics.push(ObjectDiagnostic {
                code: "OBJECT002".to_string(),
                message: error.to_string(),
            });
            return Ok(ObjectVerifyReport {
                object_id: object_id.to_string(),
                cache_path,
                source: ObjectReadSource::Cache,
                valid: false,
                diagnostics,
                computed_object_id: None,
                computed_body_hash: None,
                signature_count: 0,
                valid_signature_count: 0,
            });
        }
    };
    append_ref_consistency_diagnostics(&store.repo, object_id, &object, &mut diagnostics)?;

    verify_object_fields(
        object_id,
        cache_path,
        ObjectReadSource::Cache,
        object,
        diagnostics,
    )
}

fn read_object_from_cache_or_ref(
    store: &RickydataStore,
    cache_path: &Path,
    object_id: &str,
) -> Result<(ObjectReadSource, CanonicalObject<Value>), GitInspectError> {
    if cache_path.exists() {
        return read_object_file(cache_path).map(|object| (ObjectReadSource::Cache, object));
    }

    let ref_name = object_ref_name(object_id)?;
    let Some(bytes) = read_git_object_ref(&store.repo, &ref_name)? else {
        return Err(GitInspectError::ObjectNotFound(cache_path.to_path_buf()));
    };
    let object = read_object_bytes(&bytes)?;
    let mut diagnostics = Vec::new();
    object_field_diagnostics(object_id, &object, &mut diagnostics)?;
    if !diagnostics.is_empty() {
        let codes = diagnostics
            .iter()
            .map(|diagnostic| diagnostic.code.as_str())
            .collect::<Vec<_>>()
            .join(",");
        return Err(GitInspectError::ObjectGitRead(format!(
            "ref-backed object {ref_name} does not match requested {object_id}: {codes}"
        )));
    }
    write_object_cache_bytes(store, cache_path, object_id, &bytes)?;
    Ok((ObjectReadSource::GitRef, object))
}

fn append_ref_consistency_diagnostics(
    repo: &gix::Repository,
    object_id: &str,
    object: &CanonicalObject<Value>,
    diagnostics: &mut Vec<ObjectDiagnostic>,
) -> Result<(), GitInspectError> {
    let ref_name = object_ref_name(object_id)?;
    let Some(ref_bytes) = read_git_object_ref(repo, &ref_name)? else {
        return Ok(());
    };
    let object_bytes = canonical_object_bytes(object)?;
    if ref_bytes != object_bytes {
        diagnostics.push(ObjectDiagnostic {
            code: "OBJECT006".to_string(),
            message: "ref-backed object bytes do not match the local cached object".to_string(),
        });
    }
    Ok(())
}

fn verify_object_fields(
    object_id: &str,
    cache_path: PathBuf,
    source: ObjectReadSource,
    object: CanonicalObject<Value>,
    mut diagnostics: Vec<ObjectDiagnostic>,
) -> Result<ObjectVerifyReport, GitInspectError> {
    let (computed_object_id, computed_body_hash) =
        object_field_diagnostics(object_id, &object, &mut diagnostics)?;
    let signature_summary = append_signature_diagnostics(&object, &mut diagnostics)?;
    let valid = !diagnostics.iter().any(|d| !is_signature_warning(&d.code));

    Ok(ObjectVerifyReport {
        object_id: object_id.to_string(),
        cache_path,
        source,
        valid,
        diagnostics,
        computed_object_id: Some(computed_object_id),
        computed_body_hash: Some(computed_body_hash),
        signature_count: signature_summary.signature_count,
        valid_signature_count: signature_summary.valid_signature_count,
    })
}

fn is_signature_warning(code: &str) -> bool {
    matches!(code, "OBJECT008" | "OBJECT009")
}

#[derive(Debug, Clone, Copy, Default)]
pub struct SignatureSummary {
    pub signature_count: u32,
    pub valid_signature_count: u32,
}

fn append_signature_diagnostics(
    object: &CanonicalObject<Value>,
    diagnostics: &mut Vec<ObjectDiagnostic>,
) -> Result<SignatureSummary, GitInspectError> {
    let mut summary = SignatureSummary {
        signature_count: object.signatures.len() as u32,
        valid_signature_count: 0,
    };
    for (index, signature) in object.signatures.iter().enumerate() {
        if signature.algorithm != SIGNATURE_ALGORITHM_ED25519 {
            diagnostics.push(ObjectDiagnostic {
                code: "OBJECT009".to_string(),
                message: format!(
                    "signature[{index}] uses unknown algorithm `{}`",
                    signature.algorithm
                ),
            });
            continue;
        }
        match verify_signature(
            &object.kind,
            &object.schema_version,
            &object.body,
            signature,
        ) {
            Ok(true) => {
                summary.valid_signature_count += 1;
            }
            Ok(false) => {
                diagnostics.push(ObjectDiagnostic {
                    code: "OBJECT008".to_string(),
                    message: format!(
                        "signature[{index}] from public_key {} did not verify against canonical body",
                        signature.public_key
                    ),
                });
            }
            Err(error) => {
                diagnostics.push(ObjectDiagnostic {
                    code: "OBJECT008".to_string(),
                    message: format!("signature[{index}] verification failed: {error}"),
                });
            }
        }
    }
    Ok(summary)
}

fn object_field_diagnostics(
    object_id: &str,
    object: &CanonicalObject<Value>,
    diagnostics: &mut Vec<ObjectDiagnostic>,
) -> Result<(String, String), GitInspectError> {
    if object.object_id != object_id {
        diagnostics.push(ObjectDiagnostic {
            code: "OBJECT003".to_string(),
            message: "object_id field does not match requested object id".to_string(),
        });
    }
    let computed_body_hash = stable_json_hash(&object.body)
        .map_err(|error| GitInspectError::ObjectIo(error.to_string()))?;
    let computed_object_id =
        canonical_object_id(&object.kind, &object.schema_version, &object.body)
            .map_err(|error| GitInspectError::ObjectIo(error.to_string()))?;

    if object.object_id != computed_object_id {
        diagnostics.push(ObjectDiagnostic {
            code: "OBJECT004".to_string(),
            message: "object_id does not match canonical kind/schema/body hash".to_string(),
        });
    }
    if object.body_hash != computed_body_hash {
        diagnostics.push(ObjectDiagnostic {
            code: "OBJECT005".to_string(),
            message: "body_hash does not match canonical body hash".to_string(),
        });
    }
    Ok((computed_object_id, computed_body_hash))
}

fn status_has_any_change(repo: &gix::Repository) -> Result<bool, GitInspectError> {
    let status = repo
        .status(gix::progress::Discard)
        .map_err(|error| GitInspectError::Status(error.to_string()))?
        .untracked_files(gix::status::UntrackedFiles::Files);
    let mut iter = status
        .into_iter(Vec::new())
        .map_err(|error| GitInspectError::Status(error.to_string()))?;

    if let Some(item) = iter.next() {
        item.map_err(|error| GitInspectError::Status(error.to_string()))?;
        return Ok(true);
    }

    Ok(false)
}

fn has_git_marker(path: &Path) -> bool {
    path.ancestors().any(|ancestor| {
        ancestor.join(".git").exists()
            || ancestor
                .file_name()
                .is_some_and(|name| name == std::ffi::OsStr::new(".git"))
    })
}

struct RickydataStore {
    repo: gix::Repository,
    git_dir: PathBuf,
    temp_dir: PathBuf,
}

fn open_initialized_store(path: &Path) -> Result<RickydataStore, GitInspectError> {
    let repo = match gix::discover(path) {
        Ok(repo) => repo,
        Err(error) if has_git_marker(path) => {
            return Err(GitInspectError::Discover(error.to_string()));
        }
        Err(_) => return Err(GitInspectError::NotGitRepository(path.to_path_buf())),
    };

    let git_dir = repo.git_dir().to_path_buf();
    let metadata_dir = git_dir.join("rickydata");
    let temp_dir = metadata_dir.join("tmp");
    let version_file = metadata_dir.join("VERSION");

    if !version_file.exists() {
        return Err(GitInspectError::StoreNotInitialized(metadata_dir));
    }
    ensure_version_matches(&version_file)?;
    if !temp_dir.exists() {
        return Err(GitInspectError::StoreNotInitialized(metadata_dir));
    }

    Ok(RickydataStore {
        repo,
        git_dir,
        temp_dir,
    })
}

fn object_cache_path(git_dir: &Path, object_id: &str) -> Result<PathBuf, GitInspectError> {
    let hex = object_hex(object_id)?;
    Ok(git_dir
        .join("rickydata/cache/objects/sha256")
        .join(&hex[0..2])
        .join(format!("{hex}.json")))
}

fn object_ref_name(object_id: &str) -> Result<String, GitInspectError> {
    let hex = object_hex(object_id)?;
    Ok(format!(
        "refs/rickydata/objects/sha256/{}/{}",
        &hex[0..2],
        hex
    ))
}

fn object_hex(object_id: &str) -> Result<String, GitInspectError> {
    let Some(hex) = object_id.strip_prefix("sha256:") else {
        return Err(GitInspectError::InvalidObjectId(object_id.to_string()));
    };
    if hex.len() != 64 || !hex.chars().all(|ch| ch.is_ascii_hexdigit()) {
        return Err(GitInspectError::InvalidObjectId(object_id.to_string()));
    }
    Ok(hex.to_ascii_lowercase())
}

fn read_object_file(path: &Path) -> Result<CanonicalObject<Value>, GitInspectError> {
    let bytes =
        std::fs::read(path).map_err(|error| GitInspectError::ObjectIo(error.to_string()))?;
    read_object_bytes(&bytes)
}

fn read_object_bytes(bytes: &[u8]) -> Result<CanonicalObject<Value>, GitInspectError> {
    serde_json::from_slice(bytes).map_err(|error| GitInspectError::ObjectParse(error.to_string()))
}

fn canonical_object_bytes(object: &CanonicalObject<Value>) -> Result<Vec<u8>, GitInspectError> {
    let value = serde_json::to_value(object)
        .map_err(|error| GitInspectError::ObjectIo(error.to_string()))?;
    serde_json::to_vec(&canonical_json(&value))
        .map_err(|error| GitInspectError::ObjectIo(error.to_string()))
}

fn write_git_object_ref(
    repo: &gix::Repository,
    object_id: &str,
    ref_name: &str,
    object_bytes: &[u8],
) -> Result<String, GitInspectError> {
    let blob_id = repo
        .write_blob(object_bytes)
        .map_err(|error| GitInspectError::ObjectGitWrite(error.to_string()))?;
    let git_object_id = blob_id.to_string();

    if let Some(existing) = repo
        .try_find_reference(ref_name)
        .map_err(|error| GitInspectError::ObjectGitWrite(error.to_string()))?
    {
        let existing_id = existing.id().to_string();
        if existing_id != git_object_id {
            return Err(GitInspectError::ObjectGitWrite(format!(
                "{ref_name} points to {existing_id}, expected {git_object_id} for {object_id}"
            )));
        }
        return Ok(git_object_id);
    }

    repo.reference(
        ref_name,
        blob_id.detach(),
        gix::refs::transaction::PreviousValue::MustNotExist,
        format!("rickygit object {object_id}"),
    )
    .map_err(|error| GitInspectError::ObjectGitWrite(error.to_string()))?;
    Ok(git_object_id)
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct SignedRefUpdateReport {
    pub ref_name: String,
    pub new_oid: String,
    pub previous_oid: Option<String>,
    pub status: SignedRefUpdateStatus,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum SignedRefUpdateStatus {
    Created,
    UpdatedFromExpected,
    AlreadyAtNewOid,
}

pub fn write_git_object_ref_with_expectation(
    repo_path: impl AsRef<Path>,
    expectation: &SignedRefExpectation,
) -> Result<SignedRefUpdateReport, GitInspectError> {
    let store = open_initialized_store(repo_path.as_ref())?;
    let valid = verify_ref_expectation_signature(expectation).map_err(|error| {
        GitInspectError::RefExpectationSignatureError {
            ref_name: expectation.ref_name.clone(),
            message: error.to_string(),
        }
    })?;
    if !valid {
        return Err(GitInspectError::RefExpectationBadSignature {
            ref_name: expectation.ref_name.clone(),
        });
    }

    let current_oid = store
        .repo
        .try_find_reference(expectation.ref_name.as_str())
        .map_err(|error| GitInspectError::ObjectGitRead(error.to_string()))?
        .map(|reference| reference.id().to_string());

    match (
        expectation.expected_previous_oid.as_deref(),
        current_oid.as_deref(),
    ) {
        (None, None) => {
            // Caller asserts the ref must not yet exist; create it pointing at new_oid.
        }
        (Some(expected), Some(found)) if expected == found => {
            // Caller asserts the ref is at `expected`; we'll move it to new_oid (if different).
            if found == expectation.new_oid {
                return Ok(SignedRefUpdateReport {
                    ref_name: expectation.ref_name.clone(),
                    new_oid: expectation.new_oid.clone(),
                    previous_oid: current_oid,
                    status: SignedRefUpdateStatus::AlreadyAtNewOid,
                });
            }
        }
        (expected, found) => {
            return Err(GitInspectError::RefExpectationMismatch {
                ref_name: expectation.ref_name.clone(),
                expected: expected.map(str::to_string),
                found: found.map(str::to_string),
            });
        }
    }

    let new_oid = gix::ObjectId::from_hex(expectation.new_oid.as_bytes())
        .map_err(|error| GitInspectError::ObjectGitWrite(error.to_string()))?;
    let previous_value = match expectation.expected_previous_oid.as_deref() {
        None => gix::refs::transaction::PreviousValue::MustNotExist,
        Some(prev) => {
            let prev_oid = gix::ObjectId::from_hex(prev.as_bytes())
                .map_err(|error| GitInspectError::ObjectGitWrite(error.to_string()))?;
            gix::refs::transaction::PreviousValue::MustExistAndMatch(prev_oid.into())
        }
    };

    store
        .repo
        .reference(
            expectation.ref_name.as_str(),
            new_oid,
            previous_value,
            format!(
                "rickygit signed ref expectation {}",
                expectation.signature.public_key
            ),
        )
        .map_err(|error| GitInspectError::ObjectGitWrite(error.to_string()))?;

    let status = match expectation.expected_previous_oid.as_deref() {
        None => SignedRefUpdateStatus::Created,
        Some(_) => SignedRefUpdateStatus::UpdatedFromExpected,
    };

    Ok(SignedRefUpdateReport {
        ref_name: expectation.ref_name.clone(),
        new_oid: expectation.new_oid.clone(),
        previous_oid: current_oid,
        status,
    })
}

fn read_git_object_ref(
    repo: &gix::Repository,
    ref_name: &str,
) -> Result<Option<Vec<u8>>, GitInspectError> {
    let Some(reference) = repo
        .try_find_reference(ref_name)
        .map_err(|error| GitInspectError::ObjectGitRead(error.to_string()))?
    else {
        return Ok(None);
    };
    let blob = repo
        .find_blob(reference.id().detach())
        .map_err(|error| GitInspectError::ObjectGitRead(error.to_string()))?;
    Ok(Some(blob.data.clone()))
}

fn ensure_dir(
    path: &Path,
    created_paths: &mut Vec<PathBuf>,
    existing_paths: &mut Vec<PathBuf>,
) -> Result<(), GitInspectError> {
    if path.exists() {
        existing_paths.push(path.to_path_buf());
        return Ok(());
    }

    std::fs::create_dir_all(path).map_err(|error| GitInspectError::InitIo(error.to_string()))?;
    created_paths.push(path.to_path_buf());
    Ok(())
}

fn ensure_version_file(
    path: &Path,
    created_paths: &mut Vec<PathBuf>,
    existing_paths: &mut Vec<PathBuf>,
) -> Result<(), GitInspectError> {
    if path.exists() {
        ensure_version_matches(path)?;
        existing_paths.push(path.to_path_buf());
        return Ok(());
    }

    std::fs::write(path, format!("{RICKYDATA_STORE_VERSION}\n"))
        .map_err(|error| GitInspectError::InitIo(error.to_string()))?;
    created_paths.push(path.to_path_buf());
    Ok(())
}

fn ensure_version_matches(path: &Path) -> Result<(), GitInspectError> {
    let found = std::fs::read_to_string(path)
        .map_err(|error| GitInspectError::InitIo(error.to_string()))?;
    let found = found.trim().to_string();
    if found != RICKYDATA_STORE_VERSION {
        return Err(GitInspectError::StoreVersionMismatch {
            path: path.to_path_buf(),
            found,
            expected: RICKYDATA_STORE_VERSION,
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::process::Command;
    use tempfile::TempDir;

    #[test]
    fn reports_not_git_for_plain_directory() {
        let temp = TempDir::new().unwrap();
        let inspection = inspect_repository(temp.path()).unwrap();

        assert!(!inspection.is_git_repo);
        assert_eq!(inspection.dirty, None);
    }

    #[test]
    fn inspects_temp_repo_and_matches_git_branch() {
        let repo = TempRepo::new();
        repo.git(["checkout", "-b", "agentic-main"]);
        repo.write_file("README.md", "# temp\n");
        repo.git(["add", "README.md"]);
        repo.git(["commit", "-m", "initial"]);

        let inspection = inspect_repository(repo.path()).unwrap();
        let git_branch = repo.git_output(["branch", "--show-current"]);
        let git_head = repo.git_output(["rev-parse", "HEAD"]);

        assert!(inspection.is_git_repo);
        assert_eq!(inspection.branch.as_deref(), Some(git_branch.trim()));
        assert_eq!(inspection.head_commit.as_deref(), Some(git_head.trim()));
        assert_eq!(inspection.dirty, Some(false));
        assert!(inspection.git_dir.unwrap().ends_with(".git"));
    }

    #[test]
    fn inspect_does_not_create_rickydata_git_metadata() {
        let repo = TempRepo::new();
        let metadata_dir = repo.path().join(".git").join("rickydata");

        let _inspection = inspect_repository(repo.path()).unwrap();

        assert!(!metadata_dir.exists());
    }

    #[test]
    fn dirty_state_includes_untracked_files() {
        let repo = TempRepo::new();
        repo.write_file("untracked.txt", "new\n");

        let inspection = inspect_repository(repo.path()).unwrap();

        assert_eq!(inspection.dirty, Some(true));
    }

    #[test]
    fn corrupted_git_marker_is_reported_as_error() {
        let temp = TempDir::new().unwrap();
        std::fs::write(temp.path().join(".git"), "gitdir: missing\n").unwrap();

        let error = inspect_repository(temp.path()).unwrap_err();

        assert!(matches!(error, GitInspectError::Discover(_)));
    }

    #[test]
    fn init_creates_rickydata_store_layout() {
        let repo = TempRepo::new();

        let report = init_rickydata_repository(repo.path()).unwrap();

        assert_eq!(report.status, InitStatus::Created);
        assert_eq!(report.store_version, RICKYDATA_STORE_VERSION);
        assert!(report.metadata_dir.ends_with(".git/rickydata"));
        assert!(
            report
                .object_dir
                .ends_with(".git/rickydata/cache/objects/sha256")
        );
        assert!(report.bundle_dir.ends_with(".git/rickydata/cache/bundles"));
        assert!(report.refs_dir.ends_with(".git/refs/rickydata"));
        assert!(report.metadata_dir.join("VERSION").exists());
        assert!(repo.path().join(".git/refs/rickydata/intents").is_dir());
        assert_eq!(repo.git_output(["status", "--short"]), "");
    }

    #[test]
    fn init_is_idempotent() {
        let repo = TempRepo::new();

        let first = init_rickydata_repository(repo.path()).unwrap();
        let second = init_rickydata_repository(repo.path()).unwrap();

        assert_eq!(first.status, InitStatus::Created);
        assert_eq!(second.status, InitStatus::AlreadyInitialized);
        assert!(second.created_paths.is_empty());
        assert!(!second.existing_paths.is_empty());
        assert_eq!(repo.git_output(["status", "--short"]), "");
    }

    #[test]
    fn init_fails_for_non_git_directory() {
        let temp = TempDir::new().unwrap();

        let error = init_rickydata_repository(temp.path()).unwrap_err();

        assert!(matches!(error, GitInspectError::NotGitRepository(_)));
    }

    #[test]
    fn init_rejects_unknown_store_version() {
        let repo = TempRepo::new();
        let metadata_dir = repo.path().join(".git/rickydata");
        std::fs::create_dir_all(&metadata_dir).unwrap();
        std::fs::write(metadata_dir.join("VERSION"), "future.version\n").unwrap();

        let error = init_rickydata_repository(repo.path()).unwrap_err();

        assert!(matches!(
            error,
            GitInspectError::StoreVersionMismatch { .. }
        ));
    }

    #[test]
    fn cached_object_write_read_and_verify_round_trip() {
        let repo = TempRepo::new();
        init_rickydata_repository(repo.path()).unwrap();
        let body = serde_json::json!({ "b": 2, "a": { "z": true, "m": [3, 2, 1] } });

        let write = write_cached_object(repo.path(), "example.object", body).unwrap();
        let read = read_cached_object(repo.path(), &write.object_id).unwrap();
        let verify = verify_cached_object(repo.path(), &write.object_id).unwrap();

        assert_eq!(write.status, ObjectWriteStatus::Written);
        assert_eq!(
            write.object_id,
            "sha256:df33e1cc18ba455b88ee5198702481a43595c65ebb45354010d83ce16a47bb0c"
        );
        assert!(write.cache_path.exists());
        assert!(write.ref_name.starts_with("refs/rickydata/objects/sha256/"));
        assert_eq!(
            repo.git_output(["cat-file", "-t", &write.git_object_id])
                .trim(),
            "blob"
        );
        assert_eq!(read.object.object_id, write.object_id);
        assert_eq!(read.source, ObjectReadSource::Cache);
        assert!(verify.valid);
        assert_eq!(verify.source, ObjectReadSource::Cache);
        assert!(verify.diagnostics.is_empty());
        assert_eq!(repo.git_output(["status", "--short"]), "");
    }

    #[test]
    fn cached_object_read_recovers_from_git_ref_after_cache_deletion() {
        let repo = TempRepo::new();
        init_rickydata_repository(repo.path()).unwrap();
        let write =
            write_cached_object(repo.path(), "example.object", serde_json::json!({ "a": 1 }))
                .unwrap();

        std::fs::remove_file(&write.cache_path).unwrap();
        let read = read_cached_object(repo.path(), &write.object_id).unwrap();
        let verify = verify_cached_object(repo.path(), &write.object_id).unwrap();

        assert_eq!(read.source, ObjectReadSource::GitRef);
        assert_eq!(read.object.object_id, write.object_id);
        assert!(read.cache_path.exists());
        assert!(verify.valid);
        assert_eq!(verify.source, ObjectReadSource::Cache);
    }

    #[test]
    fn lists_ref_backed_objects_by_kind() {
        let repo = TempRepo::new();
        init_rickydata_repository(repo.path()).unwrap();
        let intent =
            write_cached_object(repo.path(), "agent.intent", serde_json::json!({ "a": 1 }))
                .unwrap();
        write_cached_object(repo.path(), "agent.run", serde_json::json!({ "b": 2 })).unwrap();

        let entries = list_ref_backed_objects(repo.path(), Some("agent.intent")).unwrap();

        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].object_id, intent.object_id);
        assert_eq!(entries[0].kind, "agent.intent");
        assert_eq!(entries[0].ref_name, intent.ref_name);
        assert_eq!(entries[0].git_object_id, intent.git_object_id);
    }

    #[test]
    fn cached_object_write_is_idempotent() {
        let repo = TempRepo::new();
        init_rickydata_repository(repo.path()).unwrap();
        let body = serde_json::json!({ "objective": "fix issue", "issue": 42 });

        let first = write_cached_object(repo.path(), "agent.intent", body.clone()).unwrap();
        let second = write_cached_object(repo.path(), "agent.intent", body).unwrap();

        assert_eq!(first.status, ObjectWriteStatus::Written);
        assert_eq!(second.status, ObjectWriteStatus::AlreadyExists);
        assert_eq!(first.object_id, second.object_id);
        assert_eq!(first.cache_path, second.cache_path);
    }

    #[test]
    fn cached_object_write_requires_initialized_store() {
        let repo = TempRepo::new();

        let error =
            write_cached_object(repo.path(), "example.object", serde_json::json!({})).unwrap_err();

        assert!(matches!(error, GitInspectError::StoreNotInitialized(_)));
    }

    #[test]
    fn cached_object_verify_reports_tampering() {
        let repo = TempRepo::new();
        init_rickydata_repository(repo.path()).unwrap();
        let write =
            write_cached_object(repo.path(), "example.object", serde_json::json!({ "a": 1 }))
                .unwrap();

        std::fs::write(&write.cache_path, br#"{"bad":true}"#).unwrap();
        let verify = verify_cached_object(repo.path(), &write.object_id).unwrap();

        assert!(!verify.valid);
        assert_eq!(verify.diagnostics[0].code, "OBJECT002");
    }

    #[test]
    fn signed_canonical_object_round_trips_through_cache_and_ref() {
        use rickydata_git_core::{generate_signing_keypair, sign_object};

        let repo = TempRepo::new();
        init_rickydata_repository(repo.path()).unwrap();

        let body = serde_json::json!({ "objective": "signed roundtrip", "issue": 17 });
        let mut object = CanonicalObject::new(
            "agent.intent",
            DEFAULT_SCHEMA_VERSION,
            0,
            canonical_json(&body),
        )
        .unwrap();
        let signing_key = generate_signing_keypair();
        let signature = sign_object(&object, &signing_key, Some("alice".into())).unwrap();
        object.signatures.push(signature);

        let write = write_canonical_object(repo.path(), &object).unwrap();
        let read = read_cached_object(repo.path(), &write.object_id).unwrap();
        let verify = verify_cached_object(repo.path(), &write.object_id).unwrap();

        assert_eq!(read.object.signatures.len(), 1);
        assert_eq!(read.object.object_id, object.object_id);
        assert!(
            verify.valid,
            "signed object should verify: {:?}",
            verify.diagnostics
        );
        assert_eq!(verify.signature_count, 1);
        assert_eq!(verify.valid_signature_count, 1);
        assert!(verify.diagnostics.is_empty());

        // Drop the cache and re-read from the git ref to confirm signature survives that path too.
        std::fs::remove_file(&write.cache_path).unwrap();
        let recovered = read_cached_object(repo.path(), &write.object_id).unwrap();
        assert_eq!(recovered.source, ObjectReadSource::GitRef);
        assert_eq!(recovered.object.signatures.len(), 1);
        let reverify = verify_cached_object(repo.path(), &write.object_id).unwrap();
        assert_eq!(reverify.signature_count, 1);
        assert_eq!(reverify.valid_signature_count, 1);
        assert!(reverify.valid);
    }

    #[test]
    fn unsigned_object_reports_zero_signature_counts() {
        let repo = TempRepo::new();
        init_rickydata_repository(repo.path()).unwrap();
        let write =
            write_cached_object(repo.path(), "example.object", serde_json::json!({ "a": 1 }))
                .unwrap();

        let verify = verify_cached_object(repo.path(), &write.object_id).unwrap();
        assert!(verify.valid);
        assert_eq!(verify.signature_count, 0);
        assert_eq!(verify.valid_signature_count, 0);

        // Unsigned objects must serialize without a `signatures` key for backward compatibility.
        let bytes = std::fs::read(&write.cache_path).unwrap();
        let text = std::str::from_utf8(&bytes).unwrap();
        assert!(
            !text.contains("\"signatures\""),
            "unsigned cached object must not include signatures key: {text}"
        );
    }

    #[test]
    fn bad_signature_produces_warning_not_invalid() {
        use rickydata_git_core::{ActorSignature, generate_signing_keypair, sign_object};

        let repo = TempRepo::new();
        init_rickydata_repository(repo.path()).unwrap();

        let body = serde_json::json!({ "objective": "tamper" });
        let mut object = CanonicalObject::new(
            "agent.intent",
            DEFAULT_SCHEMA_VERSION,
            0,
            canonical_json(&body),
        )
        .unwrap();
        let signing_key = generate_signing_keypair();
        let valid_signature = sign_object(&object, &signing_key, None).unwrap();

        // Corrupt the signature hex (flip a byte) but keep the public_key valid.
        let mut bad_sig_bytes = hex::decode(&valid_signature.signature).unwrap();
        bad_sig_bytes[0] ^= 0xFF;
        let bad_signature = ActorSignature {
            algorithm: SIGNATURE_ALGORITHM_ED25519.to_string(),
            public_key: valid_signature.public_key.clone(),
            signature: hex::encode(&bad_sig_bytes),
            signed_at_ms: None,
            signer_label: None,
        };
        object.signatures.push(bad_signature);

        let write = write_canonical_object(repo.path(), &object).unwrap();
        let verify = verify_cached_object(repo.path(), &write.object_id).unwrap();

        assert_eq!(verify.signature_count, 1);
        assert_eq!(verify.valid_signature_count, 0);
        assert!(
            verify.valid,
            "signature warnings must not mark object invalid"
        );
        assert!(verify.diagnostics.iter().any(|d| d.code == "OBJECT008"));
    }

    #[test]
    fn unknown_signature_algorithm_is_warning() {
        use rickydata_git_core::ActorSignature;

        let repo = TempRepo::new();
        init_rickydata_repository(repo.path()).unwrap();

        let body = serde_json::json!({ "objective": "unknown alg" });
        let mut object = CanonicalObject::new(
            "agent.intent",
            DEFAULT_SCHEMA_VERSION,
            0,
            canonical_json(&body),
        )
        .unwrap();
        object.signatures.push(ActorSignature {
            algorithm: "secp256k1-future".to_string(),
            public_key: "deadbeef".to_string(),
            signature: "cafebabe".to_string(),
            signed_at_ms: None,
            signer_label: None,
        });

        let write = write_canonical_object(repo.path(), &object).unwrap();
        let verify = verify_cached_object(repo.path(), &write.object_id).unwrap();

        assert_eq!(verify.signature_count, 1);
        assert_eq!(verify.valid_signature_count, 0);
        assert!(verify.valid);
        assert!(verify.diagnostics.iter().any(|d| d.code == "OBJECT009"));
    }

    #[test]
    fn signed_ref_expectation_creates_new_ref_when_none_existed() {
        use rickydata_git_core::{generate_signing_keypair, sign_ref_expectation};

        let repo = TempRepo::new();
        init_rickydata_repository(repo.path()).unwrap();
        // Write an object to get a known blob oid we can point a ref at.
        let write =
            write_cached_object(repo.path(), "example.object", serde_json::json!({ "a": 1 }))
                .unwrap();

        let key = generate_signing_keypair();
        let ref_name = "refs/rickydata/expectations/test-create";
        let expectation = sign_ref_expectation(
            ref_name,
            None,
            &write.git_object_id,
            &key,
            Some("alice".into()),
        )
        .unwrap();

        let report = write_git_object_ref_with_expectation(repo.path(), &expectation).unwrap();
        assert_eq!(report.status, SignedRefUpdateStatus::Created);
        assert_eq!(report.ref_name, ref_name);
        assert_eq!(report.new_oid, write.git_object_id);
        assert_eq!(report.previous_oid, None);
    }

    #[test]
    fn signed_ref_expectation_rejects_mismatched_previous_oid() {
        use rickydata_git_core::{generate_signing_keypair, sign_ref_expectation};

        let repo = TempRepo::new();
        init_rickydata_repository(repo.path()).unwrap();
        let write =
            write_cached_object(repo.path(), "example.object", serde_json::json!({ "a": 1 }))
                .unwrap();
        let key = generate_signing_keypair();
        let ref_name = "refs/rickydata/expectations/test-mismatch";

        // Claim there's a previous oid, but the ref doesn't exist at all.
        let fake_previous = "0".repeat(40);
        let expectation = sign_ref_expectation(
            ref_name,
            Some(&fake_previous),
            &write.git_object_id,
            &key,
            None,
        )
        .unwrap();
        let error = write_git_object_ref_with_expectation(repo.path(), &expectation).unwrap_err();
        assert!(matches!(
            error,
            GitInspectError::RefExpectationMismatch { ref_name: r, .. } if r == ref_name
        ));
    }

    #[test]
    fn signed_ref_expectation_rejects_tampered_signature() {
        use rickydata_git_core::{generate_signing_keypair, sign_ref_expectation};

        let repo = TempRepo::new();
        init_rickydata_repository(repo.path()).unwrap();
        let write =
            write_cached_object(repo.path(), "example.object", serde_json::json!({ "a": 1 }))
                .unwrap();
        let key = generate_signing_keypair();
        let ref_name = "refs/rickydata/expectations/test-tampered";

        let mut expectation =
            sign_ref_expectation(ref_name, None, &write.git_object_id, &key, None).unwrap();
        // Flip a byte in the signature hex.
        let mut sig_bytes = hex::decode(&expectation.signature.signature).unwrap();
        sig_bytes[0] ^= 0xFF;
        expectation.signature.signature = hex::encode(&sig_bytes);

        let error = write_git_object_ref_with_expectation(repo.path(), &expectation).unwrap_err();
        assert!(matches!(
            error,
            GitInspectError::RefExpectationBadSignature { ref_name: r } if r == ref_name
        ));
    }

    #[test]
    fn signed_ref_expectation_is_idempotent_when_already_at_new_oid() {
        use rickydata_git_core::{generate_signing_keypair, sign_ref_expectation};

        let repo = TempRepo::new();
        init_rickydata_repository(repo.path()).unwrap();
        let write =
            write_cached_object(repo.path(), "example.object", serde_json::json!({ "a": 1 }))
                .unwrap();
        let key = generate_signing_keypair();
        let ref_name = "refs/rickydata/expectations/test-idempotent";

        // First create at new_oid with no expected previous.
        let create =
            sign_ref_expectation(ref_name, None, &write.git_object_id, &key, None).unwrap();
        let first = write_git_object_ref_with_expectation(repo.path(), &create).unwrap();
        assert_eq!(first.status, SignedRefUpdateStatus::Created);

        // Now sign an expectation that says "previous was new_oid, new is new_oid" — should be a no-op.
        let again = sign_ref_expectation(
            ref_name,
            Some(&write.git_object_id),
            &write.git_object_id,
            &key,
            None,
        )
        .unwrap();
        let second = write_git_object_ref_with_expectation(repo.path(), &again).unwrap();
        assert_eq!(second.status, SignedRefUpdateStatus::AlreadyAtNewOid);
        assert_eq!(
            second.previous_oid.as_deref(),
            Some(write.git_object_id.as_str())
        );
    }

    struct TempRepo {
        dir: TempDir,
    }

    impl TempRepo {
        fn new() -> Self {
            let dir = TempDir::new().unwrap();
            let repo = Self { dir };
            repo.git(["init", "-b", "main"]);
            repo.git(["config", "user.email", "agent@example.com"]);
            repo.git(["config", "user.name", "Agent"]);
            repo
        }

        fn path(&self) -> &Path {
            self.dir.path()
        }

        fn write_file(&self, relative: &str, content: &str) {
            std::fs::write(self.path().join(relative), content).unwrap();
        }

        fn git<const N: usize>(&self, args: [&str; N]) {
            let output = Command::new("git")
                .args(args)
                .current_dir(self.path())
                .output()
                .unwrap();
            assert!(output.status.success());
        }

        fn git_output<const N: usize>(&self, args: [&str; N]) -> String {
            let output = Command::new("git")
                .args(args)
                .current_dir(self.path())
                .output()
                .unwrap();
            assert!(output.status.success());
            String::from_utf8(output.stdout).unwrap()
        }
    }
}
