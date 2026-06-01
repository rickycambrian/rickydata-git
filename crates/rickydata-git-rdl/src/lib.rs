use rdl_core::{EffectDeclaration, ErrorCase, FixSafety, ToolManifest, stable_json_hash};
use rickydata_git_agent::{
    AgentAttempt, AgentRun, ChangeEvidence, ContractManifest, DiscoveryObject, IntentDiagnostic,
    LanguageAdapterManifest, PatchRetirement, PreparedPatch, WorkIntent,
};
use rickydata_git_core::{PrivacyClass, SignedRefExpectation, SymbolRef};
use rickydata_git_git::{
    ObjectListEntry, ObjectReadReport, ObjectReadSource, ObjectVerifyReport, ObjectWriteReport,
    RepoInspection, RickydataInitReport,
};
use schemars::{JsonSchema, schema_for};
use serde::{Deserialize, Serialize};

pub const CAP_GIT_READ: &str = "GitRead";
pub const CAP_LOCAL_FILE_READ: &str = "LocalFileRead";
pub const CAP_LOCAL_FILE_WRITE: &str = "LocalFileWrite";
pub const CAP_LOCAL_METADATA_READ: &str = "LocalMetadataRead";
pub const CAP_LOCAL_METADATA_WRITE: &str = "LocalMetadataWrite";
pub const CAP_GIT_OBJECT_WRITE: &str = "GitObjectWrite";
pub const CAP_GIT_REF_UPDATE: &str = "GitRefUpdate";
pub const CAP_WORKTREE_CREATE: &str = "WorktreeCreate";
pub const CAP_ISSUE_READ: &str = "IssueRead";
pub const CAP_ISSUE_WRITE: &str = "IssueWrite";
pub const CAP_TRACE_WRITE: &str = "TraceWrite";
pub const CAP_SECRET_DECRYPT: &str = "SecretDecrypt";
pub const CAP_TEE_ATTEST: &str = "TeeAttest";
pub const CAP_RECEIPT_VERIFY: &str = "ReceiptVerify";
pub const CAP_PAY_AUTHORIZE: &str = "PayAuthorize";
pub const CAP_RELAY_READ: &str = "RelayRead";
pub const CAP_KFDB_READ: &str = "KfdbRead";
pub const CAP_ACTOR_SIGN: &str = "ActorSign";

fn default_json_true() -> bool {
    true
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct DoctorInput {
    #[serde(default = "default_json_true")]
    pub json: bool,
    pub relay_url: Option<String>,
    pub tee_url: Option<String>,
    pub agent_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct RepoInspectInput {
    pub repo: String,
    #[serde(default = "default_json_true")]
    pub json: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct RepoInitInput {
    pub repo: String,
    #[serde(default = "default_json_true")]
    pub json: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct RepoStatusInput {
    pub repo: String,
    pub remote: Option<String>,
    #[serde(default = "default_json_true")]
    pub json: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ManifestEmitInput {
    #[serde(default = "default_json_true")]
    pub json: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct SchemaEmitInput {
    #[serde(default = "default_json_true")]
    pub json: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct DiscoveryEmitInput {
    pub repo: String,
    #[serde(default = "default_json_true")]
    pub json: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct IntentFileInput {
    pub intent_file: String,
    #[serde(default = "default_json_true")]
    pub json: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct IntentWriteInput {
    pub repo: String,
    pub intent_file: String,
    #[serde(default = "default_json_true")]
    pub json: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct IntentListInput {
    pub repo: String,
    #[serde(default = "default_json_true")]
    pub json: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct IntentShowInput {
    pub repo: String,
    pub object_id: String,
    #[serde(default = "default_json_true")]
    pub json: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ObjectWriteInput {
    pub repo: String,
    pub kind: String,
    pub body_file: String,
    #[serde(default = "default_json_true")]
    pub json: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ObjectReadInput {
    pub repo: String,
    pub object_id: String,
    #[serde(default = "default_json_true")]
    pub json: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ObjectVerifyInput {
    pub repo: String,
    pub object_id: String,
    #[serde(default = "default_json_true")]
    pub json: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct AttemptStartInput {
    pub repo: String,
    pub intent_id: String,
    pub agent_id: String,
    pub idempotency_key: Option<String>,
    pub base_commit: Option<String>,
    pub lease_expires_at_ms: Option<u64>,
    #[serde(default = "default_json_true")]
    pub json: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct AttemptListInput {
    pub repo: String,
    #[serde(default = "default_json_true")]
    pub json: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct AttemptShowInput {
    pub repo: String,
    pub attempt_id: String,
    #[serde(default = "default_json_true")]
    pub json: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct AttemptTransitionInput {
    pub repo: String,
    pub attempt_id: String,
    pub reason: Option<String>,
    pub by: Option<String>,
    #[serde(default = "default_json_true")]
    pub json: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct IssueImportInput {
    pub repo: String,
    pub platform: String,
    pub issue_repository: String,
    pub issue_id: String,
    pub objective: String,
    pub url: Option<String>,
    pub base_commit: Option<String>,
    pub created_by: Option<String>,
    pub privacy: String,
    #[serde(default = "default_json_true")]
    pub json: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct WorkStartInput {
    pub repo: String,
    pub platform: String,
    pub issue_repository: String,
    pub issue_id: String,
    pub objective: String,
    pub url: Option<String>,
    pub agent_id: String,
    pub idempotency_key: Option<String>,
    pub base_commit: Option<String>,
    pub lease_expires_at_ms: Option<u64>,
    pub created_by: Option<String>,
    pub privacy: String,
    #[serde(default = "default_json_true")]
    pub json: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct RunExecInput {
    pub repo: String,
    pub attempt_id: String,
    pub command: Vec<String>,
    #[serde(default)]
    pub record_command_argv: bool,
    #[serde(default = "default_json_true")]
    pub json: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct RunListInput {
    pub repo: String,
    #[serde(default = "default_json_true")]
    pub json: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct RunShowInput {
    pub repo: String,
    pub run_id: String,
    #[serde(default = "default_json_true")]
    pub json: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ChangeDetectInput {
    pub repo: String,
    pub attempt_id: String,
    #[serde(default)]
    pub run_ids: Vec<String>,
    #[serde(default = "default_json_true")]
    pub json: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ChangeListInput {
    pub repo: String,
    #[serde(default = "default_json_true")]
    pub json: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ChangeShowInput {
    pub repo: String,
    pub change_id: String,
    #[serde(default = "default_json_true")]
    pub json: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct PatchPrepareInput {
    pub repo: String,
    pub attempt_id: String,
    #[serde(default = "default_json_true")]
    pub json: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct PatchListInput {
    pub repo: String,
    #[serde(default = "default_json_true")]
    pub json: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct PatchShowInput {
    pub repo: String,
    pub patch_id: String,
    #[serde(default = "default_json_true")]
    pub json: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct PatchExportInput {
    pub repo: String,
    pub patch_id: String,
    pub output: String,
    #[serde(default)]
    pub force: bool,
    #[serde(default = "default_json_true")]
    pub json: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct PatchApplyInput {
    pub repo: String,
    pub patch_id: String,
    #[serde(default)]
    pub allow_dirty: bool,
    #[serde(default)]
    pub allow_base_drift: bool,
    #[serde(default)]
    pub applied_by: Option<String>,
    #[serde(default)]
    pub reason: Option<String>,
    #[serde(default)]
    pub idempotency_key: Option<String>,
    #[serde(default = "default_json_true")]
    pub json: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct PatchCheckoutInput {
    pub repo: String,
    pub patch_id: String,
    pub path: Option<String>,
    #[serde(default)]
    pub force: bool,
    #[serde(default)]
    pub allow_base_drift: bool,
    #[serde(default = "default_json_true")]
    pub json: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct PatchRetireInput {
    pub repo: String,
    pub patch_id: String,
    pub reason: String,
    pub retired_by: Option<String>,
    #[serde(default)]
    pub idempotency_key: Option<String>,
    #[serde(default = "default_json_true")]
    pub json: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct RelayInput {
    pub repo: String,
    pub url: String,
    pub repo_id: Option<String>,
    pub idempotency_key: Option<String>,
    pub limit: Option<usize>,
    #[serde(default = "default_json_true")]
    pub json: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ProofInput {
    pub repo: String,
    pub remote: Option<String>,
    pub relay_url: Option<String>,
    pub repo_id: Option<String>,
    pub kfdb_url: Option<String>,
    pub kfdb_bearer_token: Option<String>,
    pub kfdb_bearer_token_env: Option<String>,
    pub tee_url: Option<String>,
    #[serde(default = "default_json_true")]
    pub json: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct SyncInput {
    pub repo: String,
    pub remote: String,
    #[serde(default = "default_json_true")]
    pub json: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct SyncVerifyInput {
    pub repo: String,
    #[serde(default = "default_json_true")]
    pub json: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct DoctorReport {
    pub status: String,
    pub workspace: String,
    pub git_backend: String,
    pub rdl_contracts: usize,
    pub mutating_commands_enabled: bool,
    pub signing_key_configured: Option<bool>,
    pub relay_health: Option<String>,
    pub signer_tee_reachable: Option<bool>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct IntentValidationReport {
    pub valid: bool,
    pub diagnostics: Vec<IntentDiagnostic>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct IntentHashReport {
    pub object_id: String,
    pub body_hash: String,
    pub canonical_hash: String,
    pub valid: bool,
    pub diagnostics: Vec<IntentDiagnostic>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct IntentWriteReport {
    pub valid: bool,
    pub diagnostics: Vec<IntentDiagnostic>,
    pub object: Option<ObjectWriteReport>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct IntentListReport {
    pub intents: Vec<ObjectListEntry>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct IntentShowReport {
    pub object_id: String,
    pub source: ObjectReadSource,
    pub intent: WorkIntent,
    pub valid: bool,
    pub diagnostics: Vec<IntentDiagnostic>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct AttemptStartReport {
    pub attempt: AgentAttempt,
    pub object: ObjectWriteReport,
    pub local_worktree_path: String,
    pub worktree_created: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct AttemptListEntry {
    pub object_id: String,
    pub ref_name: String,
    pub git_object_id: String,
    pub attempt: AgentAttempt,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct AttemptListReport {
    pub attempts: Vec<AttemptListEntry>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct AttemptShowReport {
    pub object_id: String,
    pub source: ObjectReadSource,
    pub attempt: AgentAttempt,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct RunExecReport {
    pub run: AgentRun,
    pub object: ObjectWriteReport,
    pub trace_object: ObjectWriteReport,
    pub exit_code: Option<i32>,
    pub command_hash: String,
    pub stdout_hash: String,
    pub stderr_hash: String,
    pub stdout_bytes: u64,
    pub stderr_bytes: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct RunListEntry {
    pub object_id: String,
    pub ref_name: String,
    pub git_object_id: String,
    pub run: AgentRun,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct RunListReport {
    pub runs: Vec<RunListEntry>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct RunShowReport {
    pub object_id: String,
    pub source: ObjectReadSource,
    pub run: AgentRun,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct ChangeDetectReport {
    pub change: ChangeEvidence,
    pub object: ObjectWriteReport,
    pub changed: bool,
    pub diff_bytes: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ChangeListEntry {
    pub object_id: String,
    pub ref_name: String,
    pub git_object_id: String,
    pub change: ChangeEvidence,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ChangeListReport {
    pub changes: Vec<ChangeListEntry>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ChangeShowReport {
    pub object_id: String,
    pub source: ObjectReadSource,
    pub change: ChangeEvidence,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct PatchPrepareReport {
    pub patch: PreparedPatch,
    pub object: ObjectWriteReport,
    pub change_count: usize,
    pub file_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct PatchListEntry {
    pub object_id: String,
    pub ref_name: String,
    pub git_object_id: String,
    pub patch: PreparedPatch,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct PatchListReport {
    pub patches: Vec<PatchListEntry>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct PatchShowReport {
    pub object_id: String,
    pub source: ObjectReadSource,
    pub patch: PreparedPatch,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct PatchExportReport {
    pub patch_id: String,
    pub attempt_id: String,
    pub base_commit: String,
    pub output_path: String,
    pub diff_hash: String,
    pub diff_bytes: u64,
    pub file_count: usize,
    pub file_paths: Vec<String>,
    pub overwritten: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct PatchApplyReport {
    pub patch_id: String,
    pub attempt_id: String,
    pub base_commit: String,
    pub head_commit: String,
    pub applied: bool,
    pub diff_hash: String,
    pub diff_bytes: u64,
    pub file_count: usize,
    pub file_paths: Vec<String>,
    pub application: rickydata_git_agent::PatchApplication,
    pub object: ObjectWriteReport,
    #[serde(default)]
    pub replayed: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct PatchCheckoutReport {
    pub patch_id: String,
    pub attempt_id: String,
    pub base_commit: String,
    pub head_commit: String,
    pub checkout_path: String,
    pub applied: bool,
    pub diff_hash: String,
    pub diff_bytes: u64,
    pub file_count: usize,
    pub file_paths: Vec<String>,
    pub replaced: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct PatchRetireReport {
    pub retirement: PatchRetirement,
    pub object: ObjectWriteReport,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct SyncReport {
    pub status: String,
    pub direction: String,
    pub remote: String,
    pub refspec: String,
    pub stdout_hash: String,
    pub stderr_hash: String,
    pub stdout_bytes: u64,
    pub stderr_bytes: u64,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub signed_ref_expectations: Vec<SignedRefExpectation>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct SyncDivergentRef {
    pub ref_name: String,
    pub local_object_id: String,
    pub remote_object_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct SyncStatusReport {
    pub status: String,
    pub remote: String,
    pub refspec: String,
    pub local_ref_count: usize,
    pub remote_ref_count: usize,
    pub matching_ref_count: usize,
    pub local_only_refs: Vec<String>,
    pub remote_only_refs: Vec<String>,
    pub divergent_refs: Vec<SyncDivergentRef>,
    pub local_refs_hash: String,
    pub remote_refs_hash: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct SyncVerifyObjectDiagnostic {
    pub object_id: String,
    pub ref_name: String,
    pub valid: bool,
    pub source: String,
    pub diagnostics: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct SyncVerifyPatchDiagnostic {
    pub patch_id: String,
    pub attempt_id: String,
    pub valid: bool,
    pub diff_object_ids: Vec<String>,
    pub diagnostics: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct SyncVerifyReport {
    pub status: String,
    pub object_count: usize,
    pub valid_object_count: usize,
    pub recoverable_object_count: usize,
    pub invalid_objects: Vec<SyncVerifyObjectDiagnostic>,
    pub patch_count: usize,
    pub valid_patch_count: usize,
    pub retired_patch_count: usize,
    pub invalid_patches: Vec<SyncVerifyPatchDiagnostic>,
    #[serde(default)]
    pub signed_object_count: usize,
    #[serde(default)]
    pub valid_signature_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct RepoDataLocations {
    pub metadata_dir: Option<String>,
    pub object_cache_dir: Option<String>,
    pub bundle_dir: Option<String>,
    pub temp_dir: Option<String>,
    pub attempt_worktrees_dir: Option<String>,
    pub review_worktrees_dir: Option<String>,
    pub refs_dir: Option<String>,
    pub object_ref_prefix: String,
    pub refspec: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct RepoStoreStatus {
    pub initialized: bool,
    pub store_version: Option<String>,
    pub version_path: Option<String>,
    pub diagnostics: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct RepoStatusReport {
    pub status: String,
    pub inspection: RepoInspection,
    pub data_locations: RepoDataLocations,
    pub store: RepoStoreStatus,
    pub verify: Option<SyncVerifyReport>,
    pub sync: Option<SyncStatusReport>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct DiscoveryEmitReport {
    pub object_id: String,
    pub body_hash: String,
    pub discovery: DiscoveryObject,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct CommandErrorReport {
    pub status: String,
    pub kind: String,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ReceiptVerifyInput {
    pub repo: String,
    pub object_id: String,
    pub tee_url: Option<String>,
    #[serde(default = "default_json_true")]
    pub json: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ReceiptVerifyReport {
    pub status: String,
    pub object_id: String,
    pub has_signatures: bool,
    pub signature_count: usize,
    pub tee_reachable: Option<bool>,
    pub tee_production_signing: Option<bool>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct KeyGenerateInput {
    pub output: String,
    #[serde(default = "default_json_true")]
    pub json: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct KeyGenerateReport {
    pub status: String,
    pub algorithm: String,
    pub public_key: String,
    pub output_path: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct KeyInitInput {
    pub agent_id: String,
    #[serde(default)]
    pub force: bool,
    #[serde(default = "default_json_true")]
    pub json: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct KeyInitReport {
    pub status: String,
    pub algorithm: String,
    pub public_key: String,
    pub agent_id: String,
    pub key_path: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct KeyShowInput {
    pub signing_key_file: String,
    #[serde(default = "default_json_true")]
    pub json: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct KeyShowReport {
    pub algorithm: String,
    pub public_key: String,
    pub signing_key_file: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandManifestExport {
    #[serde(flatten)]
    pub manifest: ToolManifest,
    pub stable_hash: String,
    pub input_schema_hash: String,
    pub output_schema_hash: String,
}

pub fn command_manifest_exports() -> Vec<CommandManifestExport> {
    let mut manifests = command_manifests()
        .into_iter()
        .map(|manifest| CommandManifestExport {
            stable_hash: manifest.stable_hash(),
            input_schema_hash: manifest.input_schema_hash(),
            output_schema_hash: stable_json_hash(&manifest.output_schema),
            manifest,
        })
        .collect::<Vec<_>>();
    manifests.sort_by(|left, right| left.manifest.name.cmp(&right.manifest.name));
    manifests
}

pub fn command_manifests() -> Vec<ToolManifest> {
    vec![
        attempt_start_manifest(),
        attempt_list_manifest(),
        attempt_show_manifest(),
        attempt_status_manifest(),
        attempt_abandon_manifest(),
        attempt_submit_manifest(),
        repo_inspect_manifest(),
        repo_init_manifest(),
        repo_status_manifest(),
        doctor_manifest(),
        manifest_emit_manifest(),
        schema_emit_manifest(),
        discovery_emit_manifest(),
        issue_import_manifest(),
        work_start_manifest(),
        intent_validate_manifest(),
        intent_hash_manifest(),
        intent_list_manifest(),
        intent_show_manifest(),
        intent_write_manifest(),
        object_write_manifest(),
        object_read_manifest(),
        object_verify_manifest(),
        patch_prepare_manifest(),
        patch_list_manifest(),
        patch_show_manifest(),
        patch_export_manifest(),
        patch_apply_manifest(),
        patch_checkout_manifest(),
        patch_retire_manifest(),
        patch_review_queue_manifest(),
        change_detect_manifest(),
        change_list_manifest(),
        change_show_manifest(),
        run_exec_manifest(),
        run_list_manifest(),
        run_show_manifest(),
        sync_status_manifest(),
        sync_verify_manifest(),
        sync_pull_manifest(),
        sync_push_manifest(),
        relay_push_manifest(),
        relay_pull_manifest(),
        relay_status_manifest(),
        proof_manifest(),
        receipt_verify_manifest(),
        key_generate_manifest(),
        key_init_manifest(),
        key_show_manifest(),
    ]
}

fn key_generate_manifest() -> ToolManifest {
    manifest(
        "key_generate",
        "Generate a fresh ed25519 actor signing key and persist it as raw bytes",
        vec![CAP_LOCAL_FILE_WRITE, CAP_ACTOR_SIGN],
        FixSafety::LocalEdit,
        schema::<KeyGenerateInput>(),
        schema::<KeyGenerateReport>(),
    )
}

fn key_show_manifest() -> ToolManifest {
    manifest(
        "key_show",
        "Inspect an ed25519 actor signing key file and report its public key",
        vec![CAP_LOCAL_FILE_READ, CAP_ACTOR_SIGN],
        FixSafety::FormatOnly,
        schema::<KeyShowInput>(),
        schema::<KeyShowReport>(),
    )
}

fn key_init_manifest() -> ToolManifest {
    manifest(
        "key_init",
        "Initialize a named agent signing key at ~/.rickydata/signing-keys/{agent_id}.key",
        vec![CAP_LOCAL_FILE_WRITE, CAP_ACTOR_SIGN],
        FixSafety::LocalEdit,
        schema::<KeyInitInput>(),
        schema::<KeyInitReport>(),
    )
}

fn repo_init_manifest() -> ToolManifest {
    manifest(
        "repo_init",
        "Initialize the local Rickydata Git metadata store for a Git repository",
        vec![CAP_GIT_READ, CAP_LOCAL_METADATA_WRITE],
        FixSafety::LocalEdit,
        schema::<RepoInitInput>(),
        schema::<RickydataInitReport>(),
    )
}

fn repo_status_manifest() -> ToolManifest {
    manifest(
        "repo_status",
        "Report Git, Rickydata store, verification, and optional remote parity status without mutation",
        vec![CAP_GIT_READ, CAP_LOCAL_METADATA_READ],
        FixSafety::FormatOnly,
        schema::<RepoStatusInput>(),
        schema::<RepoStatusReport>(),
    )
}

fn attempt_start_manifest() -> ToolManifest {
    manifest(
        "attempt_start",
        "Start an immutable AgentAttempt for an existing WorkIntent and allocate a hidden local worktree",
        vec![
            CAP_GIT_READ,
            CAP_LOCAL_METADATA_READ,
            CAP_LOCAL_METADATA_WRITE,
            CAP_GIT_OBJECT_WRITE,
            CAP_GIT_REF_UPDATE,
            CAP_WORKTREE_CREATE,
        ],
        FixSafety::LocalEdit,
        schema::<AttemptStartInput>(),
        schema::<AttemptStartReport>(),
    )
}

fn attempt_list_manifest() -> ToolManifest {
    manifest(
        "attempt_list",
        "List repo-native AgentAttempt objects from refs/rickydata",
        vec![
            CAP_GIT_READ,
            CAP_LOCAL_METADATA_READ,
            CAP_LOCAL_METADATA_WRITE,
        ],
        FixSafety::LocalEdit,
        schema::<AttemptListInput>(),
        schema::<AttemptListReport>(),
    )
}

fn attempt_show_manifest() -> ToolManifest {
    manifest(
        "attempt_show",
        "Show one repo-native AgentAttempt by attempt id",
        vec![
            CAP_GIT_READ,
            CAP_LOCAL_METADATA_READ,
            CAP_LOCAL_METADATA_WRITE,
        ],
        FixSafety::LocalEdit,
        schema::<AttemptShowInput>(),
        schema::<AttemptShowReport>(),
    )
}

fn attempt_status_manifest() -> ToolManifest {
    manifest(
        "attempt_status",
        "Inspect one agent attempt, its effective status, and its worktree diff state",
        vec![CAP_GIT_READ, CAP_LOCAL_METADATA_READ],
        FixSafety::FormatOnly,
        schema::<AttemptShowInput>(),
        schema::<serde_json::Value>(),
    )
}

fn attempt_abandon_manifest() -> ToolManifest {
    manifest(
        "attempt_abandon",
        "Record an append-only abandoned status transition for an attempt",
        vec![
            CAP_LOCAL_METADATA_WRITE,
            CAP_GIT_OBJECT_WRITE,
            CAP_GIT_REF_UPDATE,
        ],
        FixSafety::LocalEdit,
        schema::<AttemptTransitionInput>(),
        schema::<serde_json::Value>(),
    )
}

fn attempt_submit_manifest() -> ToolManifest {
    manifest(
        "attempt_submit",
        "Record an append-only submitted status transition for an attempt",
        vec![
            CAP_LOCAL_METADATA_WRITE,
            CAP_GIT_OBJECT_WRITE,
            CAP_GIT_REF_UPDATE,
        ],
        FixSafety::LocalEdit,
        schema::<AttemptTransitionInput>(),
        schema::<serde_json::Value>(),
    )
}

fn repo_inspect_manifest() -> ToolManifest {
    manifest(
        "repo_inspect",
        "Inspect a Git repository without mutating .git state",
        vec![CAP_GIT_READ],
        FixSafety::FormatOnly,
        schema::<RepoInspectInput>(),
        schema::<RepoInspection>(),
    )
}

fn doctor_manifest() -> ToolManifest {
    manifest(
        "doctor",
        "Report local rickydata_git workspace readiness",
        vec![CAP_GIT_READ],
        FixSafety::BehaviorPreserving,
        schema::<DoctorInput>(),
        schema::<DoctorReport>(),
    )
}

fn manifest_emit_manifest() -> ToolManifest {
    manifest(
        "manifest_emit",
        "Emit compiled Rickydata Git command manifests",
        Vec::new(),
        FixSafety::FormatOnly,
        schema::<ManifestEmitInput>(),
        command_manifest_export_schema(),
    )
}

fn schema_emit_manifest() -> ToolManifest {
    manifest(
        "schema_emit",
        "Emit compiled Rickydata Git JSON schema catalog",
        Vec::new(),
        FixSafety::FormatOnly,
        schema::<SchemaEmitInput>(),
        serde_json::json!({
            "type": "object",
            "title": "SchemaCatalog",
            "description": "Compiled schema catalog emitted by rickydata-git-agent"
        }),
    )
}

fn discovery_emit_manifest() -> ToolManifest {
    manifest(
        "discovery_emit",
        "Emit the compiled rust-rdl discovery object for a repository",
        vec![CAP_GIT_READ],
        FixSafety::FormatOnly,
        schema::<DiscoveryEmitInput>(),
        schema::<DiscoveryEmitReport>(),
    )
}

fn issue_import_manifest() -> ToolManifest {
    manifest(
        "issue_import",
        "Import an external issue into a repo-native WorkIntent",
        vec![CAP_ISSUE_READ, CAP_GIT_OBJECT_WRITE, CAP_GIT_REF_UPDATE],
        FixSafety::LocalEdit,
        schema::<IssueImportInput>(),
        schema::<serde_json::Value>(),
    )
}

fn work_start_manifest() -> ToolManifest {
    manifest(
        "work_start",
        "Create a WorkIntent from an issue and start an isolated agent attempt",
        vec![
            CAP_ISSUE_READ,
            CAP_GIT_OBJECT_WRITE,
            CAP_GIT_REF_UPDATE,
            CAP_WORKTREE_CREATE,
        ],
        FixSafety::LocalEdit,
        schema::<WorkStartInput>(),
        schema::<serde_json::Value>(),
    )
}

fn intent_validate_manifest() -> ToolManifest {
    manifest(
        "intent_validate",
        "Validate a work intent document before an agent starts work",
        vec![CAP_LOCAL_FILE_READ],
        FixSafety::BehaviorPreserving,
        schema::<IntentFileInput>(),
        schema::<IntentValidationReport>(),
    )
}

fn intent_hash_manifest() -> ToolManifest {
    manifest(
        "intent_hash",
        "Compute a stable canonical hash and validation diagnostics for a work intent document",
        vec![CAP_LOCAL_FILE_READ],
        FixSafety::FormatOnly,
        schema::<IntentFileInput>(),
        schema::<IntentHashReport>(),
    )
}

fn intent_write_manifest() -> ToolManifest {
    manifest(
        "intent_write",
        "Validate and persist a WorkIntent as a repo-native agent.intent object",
        vec![
            CAP_GIT_READ,
            CAP_LOCAL_FILE_READ,
            CAP_LOCAL_METADATA_WRITE,
            CAP_GIT_OBJECT_WRITE,
            CAP_GIT_REF_UPDATE,
        ],
        FixSafety::LocalEdit,
        schema::<IntentWriteInput>(),
        schema::<IntentWriteReport>(),
    )
}

fn intent_list_manifest() -> ToolManifest {
    manifest(
        "intent_list",
        "List repo-native WorkIntent objects from refs/rickydata",
        vec![CAP_GIT_READ, CAP_LOCAL_METADATA_READ],
        FixSafety::FormatOnly,
        schema::<IntentListInput>(),
        schema::<IntentListReport>(),
    )
}

fn intent_show_manifest() -> ToolManifest {
    manifest(
        "intent_show",
        "Show one repo-native WorkIntent object by object id",
        vec![
            CAP_GIT_READ,
            CAP_LOCAL_METADATA_READ,
            CAP_LOCAL_METADATA_WRITE,
        ],
        FixSafety::LocalEdit,
        schema::<IntentShowInput>(),
        schema::<IntentShowReport>(),
    )
}

fn object_write_manifest() -> ToolManifest {
    manifest(
        "object_write",
        "Write a canonical object envelope into the local cache and refs/rickydata object store",
        vec![
            CAP_GIT_READ,
            CAP_LOCAL_FILE_READ,
            CAP_LOCAL_METADATA_WRITE,
            CAP_GIT_OBJECT_WRITE,
            CAP_GIT_REF_UPDATE,
        ],
        FixSafety::LocalEdit,
        schema::<ObjectWriteInput>(),
        schema::<ObjectWriteReport>(),
    )
}

fn object_read_manifest() -> ToolManifest {
    manifest(
        "object_read",
        "Read a canonical object envelope from the local cache or refs/rickydata object store",
        vec![
            CAP_GIT_READ,
            CAP_LOCAL_METADATA_READ,
            CAP_LOCAL_METADATA_WRITE,
        ],
        FixSafety::LocalEdit,
        schema::<ObjectReadInput>(),
        schema::<ObjectReadReport>(),
    )
}

fn object_verify_manifest() -> ToolManifest {
    manifest(
        "object_verify",
        "Verify a canonical object envelope from the local cache or refs/rickydata object store",
        vec![
            CAP_GIT_READ,
            CAP_LOCAL_METADATA_READ,
            CAP_LOCAL_METADATA_WRITE,
        ],
        FixSafety::LocalEdit,
        schema::<ObjectVerifyInput>(),
        schema::<ObjectVerifyReport>(),
    )
}

fn change_detect_manifest() -> ToolManifest {
    manifest(
        "change_detect",
        "Detect attempt worktree changes and write repo-native ChangeEvidence",
        vec![
            CAP_GIT_READ,
            CAP_LOCAL_METADATA_READ,
            CAP_LOCAL_METADATA_WRITE,
            CAP_GIT_OBJECT_WRITE,
            CAP_GIT_REF_UPDATE,
        ],
        FixSafety::LocalEdit,
        schema::<ChangeDetectInput>(),
        schema::<ChangeDetectReport>(),
    )
}

fn patch_prepare_manifest() -> ToolManifest {
    manifest(
        "patch_prepare",
        "Prepare a repo-native patch summary from existing ChangeEvidence objects",
        vec![
            CAP_GIT_READ,
            CAP_LOCAL_METADATA_READ,
            CAP_LOCAL_METADATA_WRITE,
            CAP_GIT_OBJECT_WRITE,
            CAP_GIT_REF_UPDATE,
        ],
        FixSafety::LocalEdit,
        schema::<PatchPrepareInput>(),
        schema::<PatchPrepareReport>(),
    )
}

fn patch_list_manifest() -> ToolManifest {
    manifest(
        "patch_list",
        "List repo-native PreparedPatch objects from refs/rickydata",
        vec![
            CAP_GIT_READ,
            CAP_LOCAL_METADATA_READ,
            CAP_LOCAL_METADATA_WRITE,
        ],
        FixSafety::LocalEdit,
        schema::<PatchListInput>(),
        schema::<PatchListReport>(),
    )
}

fn patch_show_manifest() -> ToolManifest {
    manifest(
        "patch_show",
        "Show one repo-native PreparedPatch object by patch id",
        vec![
            CAP_GIT_READ,
            CAP_LOCAL_METADATA_READ,
            CAP_LOCAL_METADATA_WRITE,
        ],
        FixSafety::LocalEdit,
        schema::<PatchShowInput>(),
        schema::<PatchShowReport>(),
    )
}

fn patch_export_manifest() -> ToolManifest {
    manifest(
        "patch_export",
        "Export a repo-native PreparedPatch as a normal Git patch file",
        vec![
            CAP_GIT_READ,
            CAP_LOCAL_METADATA_READ,
            CAP_LOCAL_METADATA_WRITE,
            CAP_LOCAL_FILE_WRITE,
        ],
        FixSafety::LocalEdit,
        schema::<PatchExportInput>(),
        schema::<PatchExportReport>(),
    )
}

fn patch_apply_manifest() -> ToolManifest {
    manifest(
        "patch_apply",
        "Apply a repo-native PreparedPatch to a clean worktree after Git validation",
        vec![
            CAP_GIT_READ,
            CAP_LOCAL_METADATA_READ,
            CAP_LOCAL_METADATA_WRITE,
            CAP_LOCAL_FILE_WRITE,
        ],
        FixSafety::LocalEdit,
        schema::<PatchApplyInput>(),
        schema::<PatchApplyReport>(),
    )
}

fn patch_checkout_manifest() -> ToolManifest {
    manifest(
        "patch_checkout",
        "Check out a repo-native PreparedPatch into an isolated review worktree",
        vec![
            CAP_GIT_READ,
            CAP_LOCAL_METADATA_READ,
            CAP_LOCAL_METADATA_WRITE,
            CAP_WORKTREE_CREATE,
            CAP_LOCAL_FILE_WRITE,
        ],
        FixSafety::LocalEdit,
        schema::<PatchCheckoutInput>(),
        schema::<PatchCheckoutReport>(),
    )
}

fn patch_retire_manifest() -> ToolManifest {
    manifest(
        "patch_retire",
        "Record a repo-native retirement marker for a legacy or superseded PreparedPatch",
        vec![
            CAP_GIT_READ,
            CAP_LOCAL_METADATA_READ,
            CAP_LOCAL_METADATA_WRITE,
            CAP_GIT_OBJECT_WRITE,
            CAP_GIT_REF_UPDATE,
        ],
        FixSafety::LocalEdit,
        schema::<PatchRetireInput>(),
        schema::<PatchRetireReport>(),
    )
}

fn patch_review_queue_manifest() -> ToolManifest {
    manifest(
        "patch_review_queue",
        "List active prepared patches with apply readiness diagnostics",
        vec![CAP_GIT_READ, CAP_LOCAL_METADATA_READ],
        FixSafety::FormatOnly,
        schema::<PatchListInput>(),
        schema::<serde_json::Value>(),
    )
}

fn sync_pull_manifest() -> ToolManifest {
    manifest(
        "sync_pull",
        "Fetch refs/rickydata from a normal Git remote",
        vec![CAP_GIT_READ, CAP_GIT_REF_UPDATE],
        FixSafety::LocalEdit,
        schema::<SyncInput>(),
        schema::<SyncReport>(),
    )
}

fn sync_status_manifest() -> ToolManifest {
    manifest(
        "sync_status",
        "Compare local and remote refs/rickydata parity without mutating refs",
        vec![CAP_GIT_READ],
        FixSafety::FormatOnly,
        schema::<SyncInput>(),
        schema::<SyncStatusReport>(),
    )
}

fn sync_verify_manifest() -> ToolManifest {
    manifest(
        "sync_verify",
        "Verify local refs/rickydata objects and prepared patch evidence are recoverable",
        vec![
            CAP_GIT_READ,
            CAP_LOCAL_METADATA_READ,
            CAP_LOCAL_METADATA_WRITE,
        ],
        FixSafety::LocalEdit,
        schema::<SyncVerifyInput>(),
        schema::<SyncVerifyReport>(),
    )
}

fn sync_push_manifest() -> ToolManifest {
    manifest(
        "sync_push",
        "Push refs/rickydata to a normal Git remote",
        vec![CAP_GIT_READ, CAP_GIT_REF_UPDATE],
        FixSafety::LocalEdit,
        schema::<SyncInput>(),
        schema::<SyncReport>(),
    )
}

fn relay_push_manifest() -> ToolManifest {
    manifest(
        "relay_push",
        "Push local Rickydata objects to a relay as a content-addressed bundle",
        vec![CAP_GIT_READ, CAP_LOCAL_METADATA_READ],
        FixSafety::LocalEdit,
        schema::<RelayInput>(),
        schema::<serde_json::Value>(),
    )
}

fn relay_pull_manifest() -> ToolManifest {
    manifest(
        "relay_pull",
        "Pull missing Rickydata objects from a relay into local cache and refs",
        vec![
            CAP_LOCAL_METADATA_WRITE,
            CAP_GIT_OBJECT_WRITE,
            CAP_GIT_REF_UPDATE,
        ],
        FixSafety::LocalEdit,
        schema::<RelayInput>(),
        schema::<serde_json::Value>(),
    )
}

fn relay_status_manifest() -> ToolManifest {
    manifest(
        "relay_status",
        "Compare local Rickydata object count with relay status",
        vec![CAP_GIT_READ, CAP_LOCAL_METADATA_READ],
        FixSafety::FormatOnly,
        schema::<RelayInput>(),
        schema::<serde_json::Value>(),
    )
}

fn proof_manifest() -> ToolManifest {
    manifest(
        "proof",
        "Verify local Rickydata objects, optional Git remote parity, relay status, and KFDB projections",
        vec![
            CAP_GIT_READ,
            CAP_LOCAL_METADATA_READ,
            CAP_RELAY_READ,
            CAP_KFDB_READ,
        ],
        FixSafety::FormatOnly,
        schema::<ProofInput>(),
        schema::<serde_json::Value>(),
    )
}

fn receipt_verify_manifest() -> ToolManifest {
    manifest(
        "receipt_verify",
        "Verify signatures on a Rickydata object and optionally check signer TEE health",
        vec![CAP_GIT_READ, CAP_LOCAL_METADATA_READ, CAP_RECEIPT_VERIFY],
        FixSafety::FormatOnly,
        schema::<ReceiptVerifyInput>(),
        schema::<ReceiptVerifyReport>(),
    )
}

fn change_list_manifest() -> ToolManifest {
    manifest(
        "change_list",
        "List repo-native ChangeEvidence objects from refs/rickydata",
        vec![
            CAP_GIT_READ,
            CAP_LOCAL_METADATA_READ,
            CAP_LOCAL_METADATA_WRITE,
        ],
        FixSafety::LocalEdit,
        schema::<ChangeListInput>(),
        schema::<ChangeListReport>(),
    )
}

fn change_show_manifest() -> ToolManifest {
    manifest(
        "change_show",
        "Show one repo-native ChangeEvidence object by change id",
        vec![
            CAP_GIT_READ,
            CAP_LOCAL_METADATA_READ,
            CAP_LOCAL_METADATA_WRITE,
        ],
        FixSafety::LocalEdit,
        schema::<ChangeShowInput>(),
        schema::<ChangeShowReport>(),
    )
}

fn run_exec_manifest() -> ToolManifest {
    manifest(
        "run_exec",
        "Execute a command inside an attempt worktree and write AgentRun plus AgentRunTrace evidence",
        vec![
            CAP_GIT_READ,
            CAP_LOCAL_METADATA_READ,
            CAP_LOCAL_METADATA_WRITE,
            CAP_GIT_OBJECT_WRITE,
            CAP_GIT_REF_UPDATE,
            CAP_TRACE_WRITE,
        ],
        FixSafety::LocalEdit,
        schema::<RunExecInput>(),
        schema::<RunExecReport>(),
    )
}

fn run_list_manifest() -> ToolManifest {
    manifest(
        "run_list",
        "List repo-native AgentRun evidence objects from refs/rickydata",
        vec![
            CAP_GIT_READ,
            CAP_LOCAL_METADATA_READ,
            CAP_LOCAL_METADATA_WRITE,
        ],
        FixSafety::LocalEdit,
        schema::<RunListInput>(),
        schema::<RunListReport>(),
    )
}

fn run_show_manifest() -> ToolManifest {
    manifest(
        "run_show",
        "Show one repo-native AgentRun evidence object by run id",
        vec![
            CAP_GIT_READ,
            CAP_LOCAL_METADATA_READ,
            CAP_LOCAL_METADATA_WRITE,
        ],
        FixSafety::LocalEdit,
        schema::<RunShowInput>(),
        schema::<RunShowReport>(),
    )
}

fn manifest(
    name: &str,
    description: &str,
    capabilities_required: Vec<&str>,
    fix_safety: FixSafety,
    input_schema: serde_json::Value,
    output_schema: serde_json::Value,
) -> ToolManifest {
    let capabilities = capabilities_required
        .into_iter()
        .map(str::to_string)
        .collect::<Vec<_>>();
    let effects = classify_effects(&capabilities);
    let mut manifest = ToolManifest::new(
        name.to_string(),
        env!("CARGO_PKG_VERSION").to_string(),
        description.to_string(),
        capabilities,
        effects,
        input_schema,
        vec![ErrorCase {
            name: "CommandError".to_string(),
            description: "The command failed and returned structured JSON".to_string(),
            http_status: None,
        }],
        fix_safety,
    );
    manifest.output_schema = output_schema;
    manifest
}

pub fn rdl_discovery_object(
    repository: impl Into<String>,
    commit: Option<String>,
    tree: Option<String>,
) -> DiscoveryObject {
    let contract_manifests = command_manifest_exports()
        .iter()
        .map(command_contract_manifest)
        .collect::<Vec<_>>();

    DiscoveryObject {
        repository: repository.into(),
        commit,
        tree,
        language_adapters: vec![LanguageAdapterManifest {
            language: "rust".to_string(),
            adapter_name: "rust-rdl".to_string(),
            adapter_version: env!("CARGO_PKG_VERSION").to_string(),
            source: "compiled:rickydata-git-rdl".to_string(),
            supported_object_kinds: vec![
                "CommandManifest".to_string(),
                "ContractManifest".to_string(),
                "DiscoveryObject".to_string(),
            ],
        }],
        symbol_index_refs: Vec::new(),
        contract_manifests,
        privacy: PrivacyClass::PublicMetadata,
        encrypted_body: None,
    }
}

fn command_contract_manifest(export: &CommandManifestExport) -> ContractManifest {
    ContractManifest {
        name: export.manifest.name.clone(),
        version: export.manifest.version.clone(),
        description: export.manifest.description.clone(),
        capabilities_required: export.manifest.capabilities_required.clone(),
        input_schema_hash: export.input_schema_hash.clone(),
        output_schema_hash: Some(export.output_schema_hash.clone()),
        source_symbols: vec![SymbolRef {
            language: "rust".to_string(),
            file_path: "crates/rickydata-git-rdl/src/lib.rs".to_string(),
            symbol_name: manifest_symbol_name(&export.manifest.name),
            range: None,
            content_hash: export.stable_hash.clone(),
        }],
    }
}

fn manifest_symbol_name(name: &str) -> String {
    match name {
        "attempt_start" => "attempt_start_manifest",
        "attempt_list" => "attempt_list_manifest",
        "attempt_show" => "attempt_show_manifest",
        "discovery_emit" => "discovery_emit_manifest",
        "doctor" => "doctor_manifest",
        "intent_hash" => "intent_hash_manifest",
        "intent_list" => "intent_list_manifest",
        "intent_show" => "intent_show_manifest",
        "intent_validate" => "intent_validate_manifest",
        "intent_write" => "intent_write_manifest",
        "manifest_emit" => "manifest_emit_manifest",
        "object_read" => "object_read_manifest",
        "object_verify" => "object_verify_manifest",
        "object_write" => "object_write_manifest",
        "change_detect" => "change_detect_manifest",
        "change_list" => "change_list_manifest",
        "change_show" => "change_show_manifest",
        "patch_apply" => "patch_apply_manifest",
        "patch_checkout" => "patch_checkout_manifest",
        "patch_list" => "patch_list_manifest",
        "patch_export" => "patch_export_manifest",
        "patch_prepare" => "patch_prepare_manifest",
        "patch_retire" => "patch_retire_manifest",
        "patch_show" => "patch_show_manifest",
        "repo_init" => "repo_init_manifest",
        "repo_inspect" => "repo_inspect_manifest",
        "repo_status" => "repo_status_manifest",
        "run_exec" => "run_exec_manifest",
        "run_list" => "run_list_manifest",
        "run_show" => "run_show_manifest",
        "schema_emit" => "schema_emit_manifest",
        "sync_status" => "sync_status_manifest",
        "sync_verify" => "sync_verify_manifest",
        "sync_pull" => "sync_pull_manifest",
        "sync_push" => "sync_push_manifest",
        "receipt_verify" => "receipt_verify_manifest",
        "key_generate" => "key_generate_manifest",
        "key_init" => "key_init_manifest",
        "key_show" => "key_show_manifest",
        other => other,
    }
    .to_string()
}

fn schema<T: JsonSchema>() -> serde_json::Value {
    serde_json::to_value(schema_for!(T)).expect("schema serialization should not fail")
}

fn command_manifest_export_schema() -> serde_json::Value {
    serde_json::json!({
        "type": "array",
        "title": "CommandManifestExportList",
        "items": {
            "type": "object",
            "description": "Flattened RDL ToolManifest plus stable_hash, input_schema_hash, and output_schema_hash"
        }
    })
}

fn classify_effects(capabilities: &[String]) -> EffectDeclaration {
    let mut reads = Vec::new();
    let mut mutates = Vec::new();
    for capability in capabilities {
        match capability.as_str() {
            CAP_GIT_READ
            | CAP_LOCAL_FILE_READ
            | CAP_LOCAL_METADATA_READ
            | CAP_ISSUE_READ
            | CAP_RELAY_READ
            | CAP_KFDB_READ
            | CAP_RECEIPT_VERIFY => reads.push(capability.clone()),
            _ => mutates.push(capability.clone()),
        }
    }
    EffectDeclaration {
        reads,
        mutates,
        raises: vec!["CommandError".to_string()],
        calls_external: false,
        allocates: false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn manifests_include_initial_contracts() {
        let names = command_manifest_exports()
            .into_iter()
            .map(|export| export.manifest.name)
            .collect::<Vec<_>>();

        assert_eq!(
            names,
            vec![
                "attempt_abandon",
                "attempt_list",
                "attempt_show",
                "attempt_start",
                "attempt_status",
                "attempt_submit",
                "change_detect",
                "change_list",
                "change_show",
                "discovery_emit",
                "doctor",
                "intent_hash",
                "intent_list",
                "intent_show",
                "intent_validate",
                "intent_write",
                "issue_import",
                "key_generate",
                "key_init",
                "key_show",
                "manifest_emit",
                "object_read",
                "object_verify",
                "object_write",
                "patch_apply",
                "patch_checkout",
                "patch_export",
                "patch_list",
                "patch_prepare",
                "patch_retire",
                "patch_review_queue",
                "patch_show",
                "proof",
                "receipt_verify",
                "relay_pull",
                "relay_push",
                "relay_status",
                "repo_init",
                "repo_inspect",
                "repo_status",
                "run_exec",
                "run_list",
                "run_show",
                "schema_emit",
                "sync_pull",
                "sync_push",
                "sync_status",
                "sync_verify",
                "work_start"
            ]
        );
    }

    #[test]
    fn manifest_hashes_are_stable() {
        let first = command_manifest_exports();
        let second = command_manifest_exports();

        assert_eq!(first.len(), second.len());
        for (left, right) in first.iter().zip(second.iter()) {
            assert_eq!(left.stable_hash, right.stable_hash);
            assert_eq!(left.input_schema_hash, right.input_schema_hash);
            assert_eq!(left.output_schema_hash, right.output_schema_hash);
            assert!(left.stable_hash.starts_with("sha256:"));
        }
    }

    #[test]
    fn intent_file_commands_declare_local_file_reads() {
        let manifests = command_manifest_exports();
        let intent_validate = manifests
            .iter()
            .find(|export| export.manifest.name == "intent_validate")
            .unwrap();
        let intent_hash = manifests
            .iter()
            .find(|export| export.manifest.name == "intent_hash")
            .unwrap();
        let intent_write = manifests
            .iter()
            .find(|export| export.manifest.name == "intent_write")
            .unwrap();
        let intent_list = manifests
            .iter()
            .find(|export| export.manifest.name == "intent_list")
            .unwrap();
        let intent_show = manifests
            .iter()
            .find(|export| export.manifest.name == "intent_show")
            .unwrap();

        for export in [intent_validate, intent_hash] {
            assert!(
                export
                    .manifest
                    .capabilities_required
                    .contains(&CAP_LOCAL_FILE_READ.to_string())
            );
            assert!(
                export
                    .manifest
                    .effects
                    .reads
                    .contains(&CAP_LOCAL_FILE_READ.to_string())
            );
            assert!(export.manifest.effects.mutates.is_empty());
        }
        assert!(
            intent_write
                .manifest
                .effects
                .reads
                .contains(&CAP_LOCAL_FILE_READ.to_string())
        );
        assert!(
            intent_write
                .manifest
                .effects
                .mutates
                .contains(&CAP_LOCAL_METADATA_WRITE.to_string())
        );
        assert!(
            intent_write
                .manifest
                .effects
                .mutates
                .contains(&CAP_GIT_OBJECT_WRITE.to_string())
        );
        assert!(
            intent_write
                .manifest
                .effects
                .mutates
                .contains(&CAP_GIT_REF_UPDATE.to_string())
        );
        assert!(
            intent_list
                .manifest
                .effects
                .reads
                .contains(&CAP_LOCAL_METADATA_READ.to_string())
        );
        assert!(intent_list.manifest.effects.mutates.is_empty());
        assert!(
            intent_show
                .manifest
                .effects
                .reads
                .contains(&CAP_LOCAL_METADATA_READ.to_string())
        );
        assert!(
            intent_show
                .manifest
                .effects
                .mutates
                .contains(&CAP_LOCAL_METADATA_WRITE.to_string())
        );
    }

    #[test]
    fn attempt_start_declares_attempt_mutations_and_worktree_creation() {
        let manifests = command_manifest_exports();
        let attempt_start = manifests
            .iter()
            .find(|export| export.manifest.name == "attempt_start")
            .unwrap();
        let attempt_list = manifests
            .iter()
            .find(|export| export.manifest.name == "attempt_list")
            .unwrap();
        let attempt_show = manifests
            .iter()
            .find(|export| export.manifest.name == "attempt_show")
            .unwrap();

        for export in [attempt_start, attempt_list, attempt_show] {
            assert!(
                export
                    .manifest
                    .capabilities_required
                    .contains(&CAP_GIT_READ.to_string())
            );
            assert!(
                export
                    .manifest
                    .capabilities_required
                    .contains(&CAP_LOCAL_METADATA_READ.to_string())
            );
        }
        for capability in [
            CAP_LOCAL_METADATA_WRITE,
            CAP_GIT_OBJECT_WRITE,
            CAP_GIT_REF_UPDATE,
            CAP_WORKTREE_CREATE,
        ] {
            assert!(
                attempt_start
                    .manifest
                    .capabilities_required
                    .contains(&capability.to_string())
            );
        }
        assert!(
            attempt_start
                .manifest
                .effects
                .mutates
                .contains(&CAP_WORKTREE_CREATE.to_string())
        );
        assert!(
            attempt_list
                .manifest
                .effects
                .mutates
                .contains(&CAP_LOCAL_METADATA_WRITE.to_string())
        );
        assert!(
            attempt_show
                .manifest
                .effects
                .mutates
                .contains(&CAP_LOCAL_METADATA_WRITE.to_string())
        );
        assert_eq!(attempt_start.manifest.fix_safety, FixSafety::LocalEdit);
    }

    #[test]
    fn run_exec_declares_trace_and_object_mutations() {
        let manifests = command_manifest_exports();
        let run_exec = manifests
            .iter()
            .find(|export| export.manifest.name == "run_exec")
            .unwrap();
        let run_list = manifests
            .iter()
            .find(|export| export.manifest.name == "run_list")
            .unwrap();
        let run_show = manifests
            .iter()
            .find(|export| export.manifest.name == "run_show")
            .unwrap();

        for capability in [
            CAP_GIT_READ,
            CAP_LOCAL_METADATA_READ,
            CAP_LOCAL_METADATA_WRITE,
            CAP_GIT_OBJECT_WRITE,
            CAP_GIT_REF_UPDATE,
            CAP_TRACE_WRITE,
        ] {
            assert!(
                run_exec
                    .manifest
                    .capabilities_required
                    .contains(&capability.to_string())
            );
        }
        assert!(
            run_exec
                .manifest
                .effects
                .mutates
                .contains(&CAP_TRACE_WRITE.to_string())
        );
        for export in [run_list, run_show] {
            assert!(
                export
                    .manifest
                    .effects
                    .reads
                    .contains(&CAP_LOCAL_METADATA_READ.to_string())
            );
            assert!(
                export
                    .manifest
                    .effects
                    .mutates
                    .contains(&CAP_LOCAL_METADATA_WRITE.to_string())
            );
        }
        assert_eq!(run_exec.manifest.fix_safety, FixSafety::LocalEdit);
    }

    #[test]
    fn change_commands_declare_change_evidence_effects() {
        let manifests = command_manifest_exports();
        let change_detect = manifests
            .iter()
            .find(|export| export.manifest.name == "change_detect")
            .unwrap();
        let change_list = manifests
            .iter()
            .find(|export| export.manifest.name == "change_list")
            .unwrap();
        let change_show = manifests
            .iter()
            .find(|export| export.manifest.name == "change_show")
            .unwrap();

        for capability in [
            CAP_GIT_READ,
            CAP_LOCAL_METADATA_READ,
            CAP_LOCAL_METADATA_WRITE,
            CAP_GIT_OBJECT_WRITE,
            CAP_GIT_REF_UPDATE,
        ] {
            assert!(
                change_detect
                    .manifest
                    .capabilities_required
                    .contains(&capability.to_string())
            );
        }
        assert!(
            change_detect
                .manifest
                .effects
                .mutates
                .contains(&CAP_LOCAL_METADATA_WRITE.to_string())
        );
        for export in [change_list, change_show] {
            assert!(
                export
                    .manifest
                    .effects
                    .reads
                    .contains(&CAP_LOCAL_METADATA_READ.to_string())
            );
            assert!(
                export
                    .manifest
                    .effects
                    .mutates
                    .contains(&CAP_LOCAL_METADATA_WRITE.to_string())
            );
        }
        assert_eq!(change_detect.manifest.fix_safety, FixSafety::LocalEdit);
    }

    #[test]
    fn patch_commands_declare_patch_evidence_effects() {
        let manifests = command_manifest_exports();
        let patch_prepare = manifests
            .iter()
            .find(|export| export.manifest.name == "patch_prepare")
            .unwrap();
        let patch_apply = manifests
            .iter()
            .find(|export| export.manifest.name == "patch_apply")
            .unwrap();
        let patch_export = manifests
            .iter()
            .find(|export| export.manifest.name == "patch_export")
            .unwrap();
        let patch_checkout = manifests
            .iter()
            .find(|export| export.manifest.name == "patch_checkout")
            .unwrap();
        let patch_retire = manifests
            .iter()
            .find(|export| export.manifest.name == "patch_retire")
            .unwrap();
        let patch_list = manifests
            .iter()
            .find(|export| export.manifest.name == "patch_list")
            .unwrap();
        let patch_show = manifests
            .iter()
            .find(|export| export.manifest.name == "patch_show")
            .unwrap();

        for capability in [
            CAP_GIT_READ,
            CAP_LOCAL_METADATA_READ,
            CAP_LOCAL_METADATA_WRITE,
            CAP_GIT_OBJECT_WRITE,
            CAP_GIT_REF_UPDATE,
        ] {
            assert!(
                patch_prepare
                    .manifest
                    .capabilities_required
                    .contains(&capability.to_string())
            );
        }
        assert!(
            patch_prepare
                .manifest
                .effects
                .mutates
                .contains(&CAP_GIT_OBJECT_WRITE.to_string())
        );
        for capability in [
            CAP_GIT_READ,
            CAP_LOCAL_METADATA_READ,
            CAP_LOCAL_METADATA_WRITE,
            CAP_LOCAL_FILE_WRITE,
        ] {
            assert!(
                patch_export
                    .manifest
                    .capabilities_required
                    .contains(&capability.to_string())
            );
        }
        assert!(
            patch_export
                .manifest
                .effects
                .mutates
                .contains(&CAP_LOCAL_FILE_WRITE.to_string())
        );
        for capability in [
            CAP_GIT_READ,
            CAP_LOCAL_METADATA_READ,
            CAP_LOCAL_METADATA_WRITE,
            CAP_LOCAL_FILE_WRITE,
        ] {
            assert!(
                patch_apply
                    .manifest
                    .capabilities_required
                    .contains(&capability.to_string())
            );
        }
        assert!(
            patch_apply
                .manifest
                .effects
                .mutates
                .contains(&CAP_LOCAL_FILE_WRITE.to_string())
        );
        for capability in [
            CAP_GIT_READ,
            CAP_LOCAL_METADATA_READ,
            CAP_LOCAL_METADATA_WRITE,
            CAP_WORKTREE_CREATE,
            CAP_LOCAL_FILE_WRITE,
        ] {
            assert!(
                patch_checkout
                    .manifest
                    .capabilities_required
                    .contains(&capability.to_string())
            );
        }
        assert!(
            patch_checkout
                .manifest
                .effects
                .mutates
                .contains(&CAP_WORKTREE_CREATE.to_string())
        );
        for capability in [
            CAP_GIT_READ,
            CAP_LOCAL_METADATA_READ,
            CAP_LOCAL_METADATA_WRITE,
            CAP_GIT_OBJECT_WRITE,
            CAP_GIT_REF_UPDATE,
        ] {
            assert!(
                patch_retire
                    .manifest
                    .capabilities_required
                    .contains(&capability.to_string())
            );
        }
        assert!(
            patch_retire
                .manifest
                .effects
                .mutates
                .contains(&CAP_GIT_OBJECT_WRITE.to_string())
        );
        for export in [patch_list, patch_show] {
            assert!(
                export
                    .manifest
                    .effects
                    .reads
                    .contains(&CAP_LOCAL_METADATA_READ.to_string())
            );
            assert!(
                export
                    .manifest
                    .effects
                    .mutates
                    .contains(&CAP_LOCAL_METADATA_WRITE.to_string())
            );
        }
        assert_eq!(patch_prepare.manifest.fix_safety, FixSafety::LocalEdit);
        assert_eq!(patch_apply.manifest.fix_safety, FixSafety::LocalEdit);
        assert_eq!(patch_export.manifest.fix_safety, FixSafety::LocalEdit);
        assert_eq!(patch_checkout.manifest.fix_safety, FixSafety::LocalEdit);
        assert_eq!(patch_retire.manifest.fix_safety, FixSafety::LocalEdit);
    }

    #[test]
    fn sync_commands_declare_git_ref_effects() {
        let manifests = command_manifest_exports();
        let sync_pull = manifests
            .iter()
            .find(|export| export.manifest.name == "sync_pull")
            .unwrap();
        let sync_status = manifests
            .iter()
            .find(|export| export.manifest.name == "sync_status")
            .unwrap();
        let sync_verify = manifests
            .iter()
            .find(|export| export.manifest.name == "sync_verify")
            .unwrap();
        let sync_push = manifests
            .iter()
            .find(|export| export.manifest.name == "sync_push")
            .unwrap();

        assert_eq!(sync_status.manifest.fix_safety, FixSafety::FormatOnly);
        assert!(sync_status.manifest.effects.mutates.is_empty());
        assert!(
            sync_status
                .manifest
                .effects
                .reads
                .contains(&CAP_GIT_READ.to_string())
        );
        for export in [sync_pull, sync_push] {
            assert!(
                export
                    .manifest
                    .effects
                    .reads
                    .contains(&CAP_GIT_READ.to_string())
            );
            assert!(
                export
                    .manifest
                    .effects
                    .mutates
                    .contains(&CAP_GIT_REF_UPDATE.to_string())
            );
            assert_eq!(export.manifest.fix_safety, FixSafety::LocalEdit);
        }
        assert_eq!(sync_verify.manifest.fix_safety, FixSafety::LocalEdit);
        assert!(
            sync_verify
                .manifest
                .effects
                .reads
                .contains(&CAP_GIT_READ.to_string())
        );
        assert!(
            sync_verify
                .manifest
                .effects
                .mutates
                .contains(&CAP_LOCAL_METADATA_WRITE.to_string())
        );
    }

    #[test]
    fn init_declares_local_metadata_mutation() {
        let manifests = command_manifest_exports();
        let repo_init = manifests
            .iter()
            .find(|export| export.manifest.name == "repo_init")
            .unwrap();
        let repo_status = manifests
            .iter()
            .find(|export| export.manifest.name == "repo_status")
            .unwrap();

        assert!(
            repo_init
                .manifest
                .capabilities_required
                .contains(&CAP_LOCAL_METADATA_WRITE.to_string())
        );
        assert!(
            repo_init
                .manifest
                .effects
                .mutates
                .contains(&CAP_LOCAL_METADATA_WRITE.to_string())
        );
        assert_eq!(repo_init.manifest.fix_safety, FixSafety::LocalEdit);
        assert_eq!(repo_status.manifest.fix_safety, FixSafety::FormatOnly);
        assert!(repo_status.manifest.effects.mutates.is_empty());
        assert!(
            repo_status
                .manifest
                .effects
                .reads
                .contains(&CAP_GIT_READ.to_string())
        );
        assert!(
            repo_status
                .manifest
                .effects
                .reads
                .contains(&CAP_LOCAL_METADATA_READ.to_string())
        );
    }

    #[test]
    fn object_commands_declare_cache_effects() {
        let manifests = command_manifest_exports();
        let object_write = manifests
            .iter()
            .find(|export| export.manifest.name == "object_write")
            .unwrap();
        let object_read = manifests
            .iter()
            .find(|export| export.manifest.name == "object_read")
            .unwrap();
        let object_verify = manifests
            .iter()
            .find(|export| export.manifest.name == "object_verify")
            .unwrap();

        assert!(
            object_write
                .manifest
                .capabilities_required
                .contains(&CAP_GIT_OBJECT_WRITE.to_string())
        );
        assert!(
            object_write
                .manifest
                .capabilities_required
                .contains(&CAP_GIT_REF_UPDATE.to_string())
        );
        assert!(
            object_write
                .manifest
                .effects
                .mutates
                .contains(&CAP_GIT_OBJECT_WRITE.to_string())
        );
        assert!(
            object_write
                .manifest
                .effects
                .mutates
                .contains(&CAP_GIT_REF_UPDATE.to_string())
        );
        assert!(
            object_write
                .manifest
                .effects
                .mutates
                .contains(&CAP_LOCAL_METADATA_WRITE.to_string())
        );
        for export in [object_read, object_verify] {
            assert!(
                export
                    .manifest
                    .effects
                    .reads
                    .contains(&CAP_LOCAL_METADATA_READ.to_string())
            );
            assert!(
                export
                    .manifest
                    .effects
                    .mutates
                    .contains(&CAP_LOCAL_METADATA_WRITE.to_string())
            );
        }
    }

    #[test]
    fn command_schemas_are_derived_from_shared_types() {
        let manifests = command_manifest_exports();
        let repo_inspect = manifests
            .iter()
            .find(|export| export.manifest.name == "repo_inspect")
            .unwrap();
        let attempt_start = manifests
            .iter()
            .find(|export| export.manifest.name == "attempt_start")
            .unwrap();
        let attempt_list = manifests
            .iter()
            .find(|export| export.manifest.name == "attempt_list")
            .unwrap();
        let attempt_show = manifests
            .iter()
            .find(|export| export.manifest.name == "attempt_show")
            .unwrap();
        let run_exec = manifests
            .iter()
            .find(|export| export.manifest.name == "run_exec")
            .unwrap();
        let run_list = manifests
            .iter()
            .find(|export| export.manifest.name == "run_list")
            .unwrap();
        let run_show = manifests
            .iter()
            .find(|export| export.manifest.name == "run_show")
            .unwrap();
        let change_detect = manifests
            .iter()
            .find(|export| export.manifest.name == "change_detect")
            .unwrap();
        let change_list = manifests
            .iter()
            .find(|export| export.manifest.name == "change_list")
            .unwrap();
        let change_show = manifests
            .iter()
            .find(|export| export.manifest.name == "change_show")
            .unwrap();
        let repo_init = manifests
            .iter()
            .find(|export| export.manifest.name == "repo_init")
            .unwrap();
        let repo_status = manifests
            .iter()
            .find(|export| export.manifest.name == "repo_status")
            .unwrap();
        let intent_hash = manifests
            .iter()
            .find(|export| export.manifest.name == "intent_hash")
            .unwrap();
        let intent_write = manifests
            .iter()
            .find(|export| export.manifest.name == "intent_write")
            .unwrap();
        let intent_list = manifests
            .iter()
            .find(|export| export.manifest.name == "intent_list")
            .unwrap();
        let intent_show = manifests
            .iter()
            .find(|export| export.manifest.name == "intent_show")
            .unwrap();
        let object_write = manifests
            .iter()
            .find(|export| export.manifest.name == "object_write")
            .unwrap();
        let patch_prepare = manifests
            .iter()
            .find(|export| export.manifest.name == "patch_prepare")
            .unwrap();
        let patch_apply = manifests
            .iter()
            .find(|export| export.manifest.name == "patch_apply")
            .unwrap();
        let patch_export = manifests
            .iter()
            .find(|export| export.manifest.name == "patch_export")
            .unwrap();
        let patch_checkout = manifests
            .iter()
            .find(|export| export.manifest.name == "patch_checkout")
            .unwrap();
        let patch_retire = manifests
            .iter()
            .find(|export| export.manifest.name == "patch_retire")
            .unwrap();
        let patch_list = manifests
            .iter()
            .find(|export| export.manifest.name == "patch_list")
            .unwrap();
        let patch_show = manifests
            .iter()
            .find(|export| export.manifest.name == "patch_show")
            .unwrap();
        let sync_pull = manifests
            .iter()
            .find(|export| export.manifest.name == "sync_pull")
            .unwrap();
        let sync_status = manifests
            .iter()
            .find(|export| export.manifest.name == "sync_status")
            .unwrap();
        let sync_verify = manifests
            .iter()
            .find(|export| export.manifest.name == "sync_verify")
            .unwrap();
        let sync_push = manifests
            .iter()
            .find(|export| export.manifest.name == "sync_push")
            .unwrap();

        assert_eq!(
            repo_inspect.manifest.input_schema["title"],
            "RepoInspectInput"
        );
        assert_eq!(
            repo_inspect.manifest.output_schema["title"],
            "RepoInspection"
        );
        assert_eq!(
            attempt_start.manifest.input_schema["title"],
            "AttemptStartInput"
        );
        assert_eq!(
            attempt_start.manifest.output_schema["title"],
            "AttemptStartReport"
        );
        assert_eq!(
            attempt_list.manifest.input_schema["title"],
            "AttemptListInput"
        );
        assert_eq!(
            attempt_list.manifest.output_schema["title"],
            "AttemptListReport"
        );
        assert_eq!(
            attempt_show.manifest.input_schema["title"],
            "AttemptShowInput"
        );
        assert_eq!(
            attempt_show.manifest.output_schema["title"],
            "AttemptShowReport"
        );
        assert_eq!(run_exec.manifest.input_schema["title"], "RunExecInput");
        assert_eq!(run_exec.manifest.output_schema["title"], "RunExecReport");
        assert_eq!(run_list.manifest.input_schema["title"], "RunListInput");
        assert_eq!(run_list.manifest.output_schema["title"], "RunListReport");
        assert_eq!(run_show.manifest.input_schema["title"], "RunShowInput");
        assert_eq!(run_show.manifest.output_schema["title"], "RunShowReport");
        assert_eq!(
            change_detect.manifest.input_schema["title"],
            "ChangeDetectInput"
        );
        assert_eq!(
            change_detect.manifest.output_schema["title"],
            "ChangeDetectReport"
        );
        assert_eq!(
            change_list.manifest.input_schema["title"],
            "ChangeListInput"
        );
        assert_eq!(
            change_list.manifest.output_schema["title"],
            "ChangeListReport"
        );
        assert_eq!(
            change_show.manifest.input_schema["title"],
            "ChangeShowInput"
        );
        assert_eq!(
            change_show.manifest.output_schema["title"],
            "ChangeShowReport"
        );
        assert_eq!(repo_init.manifest.input_schema["title"], "RepoInitInput");
        assert_eq!(
            repo_init.manifest.output_schema["title"],
            "RickydataInitReport"
        );
        assert_eq!(
            repo_status.manifest.input_schema["title"],
            "RepoStatusInput"
        );
        assert_eq!(
            repo_status.manifest.output_schema["title"],
            "RepoStatusReport"
        );
        assert_eq!(
            intent_hash.manifest.input_schema["title"],
            "IntentFileInput"
        );
        assert_eq!(
            intent_hash.manifest.output_schema["title"],
            "IntentHashReport"
        );
        assert_eq!(
            intent_write.manifest.input_schema["title"],
            "IntentWriteInput"
        );
        assert_eq!(
            intent_write.manifest.output_schema["title"],
            "IntentWriteReport"
        );
        assert_eq!(
            intent_list.manifest.input_schema["title"],
            "IntentListInput"
        );
        assert_eq!(
            intent_list.manifest.output_schema["title"],
            "IntentListReport"
        );
        assert_eq!(
            intent_show.manifest.input_schema["title"],
            "IntentShowInput"
        );
        assert_eq!(
            intent_show.manifest.output_schema["title"],
            "IntentShowReport"
        );
        assert_eq!(
            object_write.manifest.input_schema["title"],
            "ObjectWriteInput"
        );
        assert_eq!(
            object_write.manifest.output_schema["title"],
            "ObjectWriteReport"
        );
        assert_eq!(
            patch_prepare.manifest.input_schema["title"],
            "PatchPrepareInput"
        );
        assert_eq!(
            patch_prepare.manifest.output_schema["title"],
            "PatchPrepareReport"
        );
        assert_eq!(
            patch_apply.manifest.input_schema["title"],
            "PatchApplyInput"
        );
        assert_eq!(
            patch_apply.manifest.output_schema["title"],
            "PatchApplyReport"
        );
        assert_eq!(
            patch_export.manifest.input_schema["title"],
            "PatchExportInput"
        );
        assert_eq!(
            patch_export.manifest.output_schema["title"],
            "PatchExportReport"
        );
        assert_eq!(
            patch_checkout.manifest.input_schema["title"],
            "PatchCheckoutInput"
        );
        assert_eq!(
            patch_checkout.manifest.output_schema["title"],
            "PatchCheckoutReport"
        );
        assert_eq!(
            patch_retire.manifest.input_schema["title"],
            "PatchRetireInput"
        );
        assert_eq!(
            patch_retire.manifest.output_schema["title"],
            "PatchRetireReport"
        );
        assert_eq!(patch_list.manifest.input_schema["title"], "PatchListInput");
        assert_eq!(
            patch_list.manifest.output_schema["title"],
            "PatchListReport"
        );
        assert_eq!(patch_show.manifest.input_schema["title"], "PatchShowInput");
        assert_eq!(
            patch_show.manifest.output_schema["title"],
            "PatchShowReport"
        );
        assert_eq!(sync_pull.manifest.input_schema["title"], "SyncInput");
        assert_eq!(sync_pull.manifest.output_schema["title"], "SyncReport");
        assert_eq!(sync_status.manifest.input_schema["title"], "SyncInput");
        assert_eq!(
            sync_status.manifest.output_schema["title"],
            "SyncStatusReport"
        );
        assert_eq!(
            sync_verify.manifest.input_schema["title"],
            "SyncVerifyInput"
        );
        assert_eq!(
            sync_verify.manifest.output_schema["title"],
            "SyncVerifyReport"
        );
        assert_eq!(sync_push.manifest.input_schema["title"], "SyncInput");
        assert_eq!(sync_push.manifest.output_schema["title"], "SyncReport");
    }

    #[test]
    fn rdl_discovery_object_exports_contract_manifests() {
        let discovery = rdl_discovery_object("rickydata_git", Some("abc123".to_string()), None);

        assert_eq!(discovery.repository, "rickydata_git");
        assert_eq!(
            discovery.language_adapters[0].adapter_name,
            "rust-rdl".to_string()
        );
        assert_eq!(
            discovery.contract_manifests.len(),
            command_manifest_exports().len()
        );
        assert!(
            discovery
                .contract_manifests
                .iter()
                .any(|contract| contract.name == "discovery_emit")
        );
    }
}
