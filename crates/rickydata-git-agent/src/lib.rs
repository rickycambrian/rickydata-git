use rickydata_git_core::{
    CoreError, EncryptionEnvelopeRef, PrivacyClass, ReleaseGuardPolicy, SignerReceiptRef,
    SourceSpan, SymbolRef, TeePolicy, stable_json_hash,
};
use schemars::{JsonSchema, schema::RootSchema, schema_for};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct IssueRef {
    pub platform: String,
    pub repository: String,
    pub id: String,
    pub url: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct TaskRef {
    pub system: String,
    pub id: String,
    pub title: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct WorkIntent {
    pub objective: String,
    pub issue_refs: Vec<IssueRef>,
    pub task_refs: Vec<TaskRef>,
    pub base_commit: Option<String>,
    pub allowed_capabilities: Vec<String>,
    pub privacy: PrivacyClass,
    pub tee_policy: Option<TeePolicy>,
    pub release_guard: Option<ReleaseGuardPolicy>,
    pub created_by: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum AttemptStatus {
    Planned,
    Running,
    Submitted,
    Abandoned,
    Integrated,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct AgentAttempt {
    pub attempt_id: String,
    pub intent_id: String,
    pub base_commit: String,
    pub agent_id: String,
    pub lease_expires_at_ms: Option<u64>,
    pub status: AttemptStatus,
    /// When true, the attempt records provenance against the repository's main
    /// working tree instead of an isolated `.git/rickydata/worktrees/*` worktree.
    /// Omitted on the wire when false so existing worktree-backed attempt object
    /// ids stay byte-identical.
    #[serde(default, skip_serializing_if = "is_false")]
    pub in_place: bool,
}

fn is_false(value: &bool) -> bool {
    !*value
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct AttemptStatusTransition {
    pub attempt_id: String,
    pub status: AttemptStatus,
    pub reason: Option<String>,
    pub created_by: Option<String>,
    pub created_at_ms: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum AgentRunResult {
    Succeeded,
    Failed,
    Cancelled,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct AgentRun {
    pub run_id: String,
    pub attempt_id: String,
    pub trace_hash: Option<String>,
    pub command_hashes: Vec<String>,
    pub rdl_manifest_hashes: Vec<String>,
    pub started_at_ms: u64,
    pub finished_at_ms: Option<u64>,
    pub result: Option<AgentRunResult>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct AgentRunTrace {
    pub trace_id: String,
    pub attempt_id: String,
    pub command_hash: String,
    pub command_argv: Option<Vec<String>>,
    pub executable: Option<String>,
    pub arg_count: u64,
    pub exit_code: Option<i32>,
    pub stdout_hash: String,
    pub stderr_hash: String,
    pub stdout_bytes: u64,
    pub stderr_bytes: u64,
    pub started_at_ms: u64,
    pub finished_at_ms: u64,
    pub result: AgentRunResult,
    pub privacy: PrivacyClass,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub encrypted_body: Option<EncryptionEnvelopeRef>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ChangeEvidence {
    pub change_id: String,
    pub intent_id: String,
    pub attempt_id: String,
    pub run_ids: Vec<String>,
    pub base_commit: String,
    pub file_paths: Vec<String>,
    #[serde(default)]
    pub diff_summary: DiffSummary,
    pub symbols: Vec<SymbolRef>,
    pub diff_hash: String,
    pub related_contract_hashes: Vec<String>,
    pub diagnostics: Vec<DiagnosticEvidence>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct DiffSummary {
    pub file_count: u64,
    pub files_added: u64,
    pub files_modified: u64,
    pub files_deleted: u64,
    pub files_renamed: u64,
    pub binary_file_count: u64,
    pub insertions: u64,
    pub deletions: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct PreparedPatch {
    pub patch_id: String,
    pub intent_id: String,
    pub attempt_id: String,
    pub base_commit: String,
    pub change_ids: Vec<String>,
    pub run_ids: Vec<String>,
    pub file_paths: Vec<String>,
    pub diff_hashes: Vec<String>,
    #[serde(default)]
    pub diff_object_ids: Vec<String>,
    pub related_contract_hashes: Vec<String>,
    pub diagnostics: Vec<DiagnosticEvidence>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct PatchDiff {
    pub patch_id: String,
    pub attempt_id: String,
    pub base_commit: String,
    pub diff_hash: String,
    pub diff_bytes: u64,
    pub file_paths: Vec<String>,
    pub encoding: String,
    pub encoded_diff: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct PatchApplication {
    pub patch_id: String,
    pub attempt_id: String,
    pub base_commit: String,
    pub head_commit: String,
    pub diff_hash: String,
    pub diff_bytes: u64,
    pub file_paths: Vec<String>,
    pub applied_by: Option<String>,
    pub reason: Option<String>,
    pub idempotency_key: Option<String>,
    pub applied_at_ms: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct PatchRetirement {
    pub patch_id: String,
    pub reason: String,
    pub retired_by: Option<String>,
    #[serde(default)]
    pub idempotency_key: Option<String>,
    pub retired_at_ms: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct DiagnosticEvidence {
    pub code: String,
    pub severity: String,
    pub message: String,
    pub span: Option<SourceSpan>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct AttestationEvidence {
    pub evidence_id: String,
    pub signer_receipt: SignerReceiptRef,
    pub tee_evidence_ref: Option<String>,
    pub payload_hash: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct PaymentEvidence {
    pub evidence_id: String,
    pub payment_protocol: String,
    pub authorization_receipt_ref: Option<String>,
    pub settlement_receipt_ref: Option<String>,
    pub amount: Option<String>,
}

/// A short, agent-to-agent coordination message recorded as a repo-native
/// canonical object (`agent.note`). Notes are the fast-lane analog of a chat
/// line, but unlike ephemeral chat they are content-addressed, signable, and
/// distributed over the same `refs/rickydata/*` + relay rails as the rest of
/// the work ledger, so any agent can recover and verify them.
///
/// `to` is an agent identity, the literal `all` (broadcast), or `kai` (the
/// human reviewer), mirroring the wire convention proposed in PsyProxy#51.
/// `created_at_ms` is part of the canonical body so that two otherwise
/// identical notes (e.g. repeated "ack") remain distinct objects and order
/// deterministically.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct AgentNote {
    pub from: String,
    pub to: String,
    pub body: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub thread: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub in_reply_to: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub refs: Vec<String>,
    pub created_at_ms: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct LanguageAdapterManifest {
    pub language: String,
    pub adapter_name: String,
    pub adapter_version: String,
    pub source: String,
    pub supported_object_kinds: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ContractManifest {
    pub name: String,
    pub version: String,
    pub description: String,
    pub capabilities_required: Vec<String>,
    pub input_schema_hash: String,
    pub output_schema_hash: Option<String>,
    pub source_symbols: Vec<SymbolRef>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct DiscoveryObject {
    pub repository: String,
    pub commit: Option<String>,
    pub tree: Option<String>,
    pub language_adapters: Vec<LanguageAdapterManifest>,
    pub symbol_index_refs: Vec<String>,
    pub contract_manifests: Vec<ContractManifest>,
    pub privacy: PrivacyClass,
    pub encrypted_body: Option<EncryptionEnvelopeRef>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct IntentDiagnostic {
    pub code: String,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SchemaCatalog {
    pub schema_version: u32,
    pub generated_by: &'static str,
    pub schemas: BTreeMap<String, RootSchema>,
    pub schema_hashes: BTreeMap<String, String>,
}

pub fn schema_catalog() -> Result<SchemaCatalog, CoreError> {
    let mut schemas = BTreeMap::new();
    insert_schema(&mut schemas, "AgentAttempt", schema_for!(AgentAttempt));
    insert_schema(
        &mut schemas,
        "AttemptStatusTransition",
        schema_for!(AttemptStatusTransition),
    );
    insert_schema(&mut schemas, "AgentNote", schema_for!(AgentNote));
    insert_schema(&mut schemas, "AgentRun", schema_for!(AgentRun));
    insert_schema(&mut schemas, "AgentRunTrace", schema_for!(AgentRunTrace));
    insert_schema(
        &mut schemas,
        "AttestationEvidence",
        schema_for!(AttestationEvidence),
    );
    insert_schema(&mut schemas, "ChangeEvidence", schema_for!(ChangeEvidence));
    insert_schema(
        &mut schemas,
        "ContractManifest",
        schema_for!(ContractManifest),
    );
    insert_schema(
        &mut schemas,
        "DiscoveryObject",
        schema_for!(DiscoveryObject),
    );
    insert_schema(
        &mut schemas,
        "LanguageAdapterManifest",
        schema_for!(LanguageAdapterManifest),
    );
    insert_schema(
        &mut schemas,
        "PaymentEvidence",
        schema_for!(PaymentEvidence),
    );
    insert_schema(&mut schemas, "PatchDiff", schema_for!(PatchDiff));
    insert_schema(
        &mut schemas,
        "PatchApplication",
        schema_for!(PatchApplication),
    );
    insert_schema(
        &mut schemas,
        "PatchRetirement",
        schema_for!(PatchRetirement),
    );
    insert_schema(&mut schemas, "PreparedPatch", schema_for!(PreparedPatch));
    insert_schema(&mut schemas, "WorkIntent", schema_for!(WorkIntent));

    let mut schema_hashes = BTreeMap::new();
    for (name, schema) in &schemas {
        let schema_value = serde_json::to_value(schema)?;
        schema_hashes.insert(name.clone(), stable_json_hash(&schema_value)?);
    }

    Ok(SchemaCatalog {
        schema_version: 1,
        generated_by: "rickydata-git-agent",
        schemas,
        schema_hashes,
    })
}

fn insert_schema(schemas: &mut BTreeMap<String, RootSchema>, name: &str, schema: RootSchema) {
    schemas.insert(name.to_string(), schema);
}

pub fn validate_work_intent(intent: &WorkIntent) -> Vec<IntentDiagnostic> {
    let mut diagnostics = Vec::new();
    if intent.objective.trim().is_empty() {
        diagnostics.push(IntentDiagnostic {
            code: "INTENT001".to_string(),
            message: "objective must not be empty".to_string(),
        });
    }
    if intent.issue_refs.is_empty() && intent.task_refs.is_empty() {
        diagnostics.push(IntentDiagnostic {
            code: "INTENT002".to_string(),
            message: "at least one issue_refs or task_refs entry is required".to_string(),
        });
    }
    if let Some(base_commit) = &intent.base_commit
        && !looks_like_git_object_id(base_commit)
    {
        diagnostics.push(IntentDiagnostic {
            code: "INTENT003".to_string(),
            message: "base_commit must be a 40- or 64-character hex object id when present"
                .to_string(),
        });
    }
    diagnostics
}

pub fn validate_agent_note(note: &AgentNote) -> Vec<IntentDiagnostic> {
    let mut diagnostics = Vec::new();
    if note.from.trim().is_empty() {
        diagnostics.push(IntentDiagnostic {
            code: "NOTE001".to_string(),
            message: "from must not be empty".to_string(),
        });
    }
    if note.to.trim().is_empty() {
        diagnostics.push(IntentDiagnostic {
            code: "NOTE002".to_string(),
            message: "to must not be empty".to_string(),
        });
    }
    if note.body.trim().is_empty() {
        diagnostics.push(IntentDiagnostic {
            code: "NOTE003".to_string(),
            message: "body must not be empty".to_string(),
        });
    }
    diagnostics
}

pub fn looks_like_git_object_id(value: &str) -> bool {
    matches!(value.len(), 40 | 64) && value.chars().all(|ch| ch.is_ascii_hexdigit())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn attempt_in_place_defaults_false_and_omits_on_wire() {
        let attempt = AgentAttempt {
            attempt_id: "sha256:a".into(),
            intent_id: "sha256:b".into(),
            base_commit: "c".repeat(40),
            agent_id: "agent:x".into(),
            lease_expires_at_ms: None,
            status: AttemptStatus::Running,
            in_place: false,
        };
        let serialized = serde_json::to_string(&attempt).unwrap();
        assert!(
            !serialized.contains("in_place"),
            "in_place must be omitted when false to preserve legacy object ids: {serialized}"
        );
        // A legacy attempt object with no in_place field deserializes to false.
        let legacy = serde_json::json!({
            "attempt_id": "sha256:a",
            "intent_id": "sha256:b",
            "base_commit": "c".repeat(40),
            "agent_id": "agent:x",
            "lease_expires_at_ms": null,
            "status": "running"
        });
        let parsed: AgentAttempt = serde_json::from_value(legacy).unwrap();
        assert!(!parsed.in_place);
    }

    #[test]
    fn attempt_in_place_true_serializes() {
        let attempt = AgentAttempt {
            attempt_id: "sha256:a".into(),
            intent_id: "sha256:b".into(),
            base_commit: "c".repeat(40),
            agent_id: "agent:x".into(),
            lease_expires_at_ms: None,
            status: AttemptStatus::Running,
            in_place: true,
        };
        let serialized = serde_json::to_string(&attempt).unwrap();
        assert!(serialized.contains("\"in_place\":true"));
        let parsed: AgentAttempt = serde_json::from_str(&serialized).unwrap();
        assert!(parsed.in_place);
    }

    #[test]
    fn validates_required_issue_or_task_binding() {
        let intent = WorkIntent {
            objective: " ".to_string(),
            issue_refs: Vec::new(),
            task_refs: Vec::new(),
            base_commit: Some("not-a-commit".to_string()),
            allowed_capabilities: Vec::new(),
            privacy: PrivacyClass::PublicMetadata,
            tee_policy: None,
            release_guard: None,
            created_by: None,
        };

        let diagnostics = validate_work_intent(&intent);
        assert_eq!(diagnostics.len(), 3);
        assert_eq!(diagnostics[0].code, "INTENT001");
        assert_eq!(diagnostics[1].code, "INTENT002");
        assert_eq!(diagnostics[2].code, "INTENT003");
    }

    fn sample_trace() -> AgentRunTrace {
        AgentRunTrace {
            trace_id: "trace-1".into(),
            attempt_id: "attempt-1".into(),
            command_hash: "sha256:abc".into(),
            command_argv: None,
            executable: Some("echo".into()),
            arg_count: 0,
            exit_code: Some(0),
            stdout_hash: "sha256:00".into(),
            stderr_hash: "sha256:00".into(),
            stdout_bytes: 0,
            stderr_bytes: 0,
            started_at_ms: 1,
            finished_at_ms: 2,
            result: AgentRunResult::Succeeded,
            privacy: PrivacyClass::PublicMetadata,
            encrypted_body: None,
        }
    }

    #[test]
    fn agent_run_trace_omits_empty_encrypted_body_on_wire() {
        let trace = sample_trace();
        let serialized = serde_json::to_string(&trace).unwrap();
        assert!(
            !serialized.contains("encrypted_body"),
            "missing encrypted_body must not serialize: {serialized}"
        );
        let parsed: AgentRunTrace = serde_json::from_str(&serialized).unwrap();
        assert!(parsed.encrypted_body.is_none());
    }

    #[test]
    fn agent_run_trace_round_trips_encryption_envelope() {
        let mut trace = sample_trace();
        trace.privacy = PrivacyClass::Encrypted;
        trace.encrypted_body = Some(EncryptionEnvelopeRef {
            algorithm: "aes-256-gcm".into(),
            envelope_hash: "sha256:deadbeef".into(),
            key_ref: Some("local:dev".into()),
        });
        let serialized = serde_json::to_string(&trace).unwrap();
        assert!(serialized.contains("\"encrypted_body\""));
        let parsed: AgentRunTrace = serde_json::from_str(&serialized).unwrap();
        assert_eq!(parsed.encrypted_body, trace.encrypted_body);
    }

    #[test]
    fn schema_catalog_includes_foundational_agent_objects() {
        let catalog = schema_catalog().unwrap();

        assert_eq!(catalog.schema_version, 1);
        assert!(catalog.schemas.contains_key("WorkIntent"));
        assert!(catalog.schemas.contains_key("DiscoveryObject"));
        assert!(catalog.schemas.contains_key("AgentRun"));
        assert!(catalog.schemas.contains_key("AgentNote"));
        assert!(catalog.schema_hashes["WorkIntent"].starts_with("sha256:"));
        assert!(catalog.schema_hashes["AgentNote"].starts_with("sha256:"));
    }

    fn sample_note() -> AgentNote {
        AgentNote {
            from: "agent:hermes".into(),
            to: "claude-code".into(),
            body: "AllPsy factor-fit rerun done; artifacts at <path>".into(),
            thread: Some("allpsy-factor-fit".into()),
            in_reply_to: None,
            refs: vec!["sha256:abc".into()],
            created_at_ms: 1717113600000,
        }
    }

    #[test]
    fn valid_note_has_no_diagnostics() {
        assert!(validate_agent_note(&sample_note()).is_empty());
    }

    #[test]
    fn note_requires_from_to_and_body() {
        let note = AgentNote {
            from: " ".into(),
            to: String::new(),
            body: "\t".into(),
            thread: None,
            in_reply_to: None,
            refs: Vec::new(),
            created_at_ms: 0,
        };
        let diagnostics = validate_agent_note(&note);
        assert_eq!(diagnostics.len(), 3);
        assert_eq!(diagnostics[0].code, "NOTE001");
        assert_eq!(diagnostics[1].code, "NOTE002");
        assert_eq!(diagnostics[2].code, "NOTE003");
    }

    #[test]
    fn note_omits_empty_optional_fields_on_wire() {
        let note = AgentNote {
            from: "agent:hermes".into(),
            to: "all".into(),
            body: "rerun done".into(),
            thread: None,
            in_reply_to: None,
            refs: Vec::new(),
            created_at_ms: 1,
        };
        let serialized = serde_json::to_string(&note).unwrap();
        assert!(!serialized.contains("thread"));
        assert!(!serialized.contains("in_reply_to"));
        assert!(!serialized.contains("refs"));
        let parsed: AgentNote = serde_json::from_str(&serialized).unwrap();
        assert_eq!(parsed, note);
    }

    #[test]
    fn note_round_trips_with_optional_fields() {
        let note = sample_note();
        let serialized = serde_json::to_string(&note).unwrap();
        let parsed: AgentNote = serde_json::from_str(&serialized).unwrap();
        assert_eq!(parsed, note);
    }
}
