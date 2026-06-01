use anyhow::{Context, Result};
use clap::error::ErrorKind;
use clap::{Args, Parser, Subcommand};
use ed25519_dalek::SigningKey;
#[cfg(feature = "tee")]
use rickydata_auth_client::{
    BlockingSignerHttpClient, DEFAULT_SIGNER_TIMEOUT, SignerClientConfig, SignerHealth,
};
use rickydata_git_agent::{
    AgentAttempt, AgentNote, AgentRun, AgentRunResult, AgentRunTrace, AttemptStatus,
    AttemptStatusTransition, ChangeEvidence, DiffSummary, IssueRef, PatchApplication, PatchDiff,
    PatchRetirement, PreparedPatch, WorkIntent, schema_catalog, validate_agent_note,
    validate_work_intent,
};
use rickydata_git_core::{
    CanonicalObject, DEFAULT_SCHEMA_VERSION, PrivacyClass, SIGNATURE_ALGORITHM_ED25519,
    generate_signing_keypair, load_signing_key_from_file, save_signing_key_to_file, sign_object,
    stable_json_hash,
};
use rickydata_git_git::{ObjectWriteReport, ObjectWriteStatus};
use rickydata_git_rdl::{
    AttemptListEntry, AttemptListInput, AttemptListReport, AttemptShowInput, AttemptShowReport,
    AttemptStartInput, AttemptStartReport, ChangeDetectInput, ChangeDetectReport, ChangeListEntry,
    ChangeListInput, ChangeListReport, ChangeShowInput, ChangeShowReport, CommandErrorReport,
    DiscoveryEmitInput, DiscoveryEmitReport, DoctorInput, DoctorReport, IntentFileInput,
    IntentHashReport, IntentListInput, IntentListReport, IntentShowInput, IntentShowReport,
    IntentValidationReport, IntentWriteInput, IntentWriteReport, KeyGenerateInput,
    KeyGenerateReport, KeyInitInput, KeyInitReport, KeyShowInput, KeyShowReport, ManifestEmitInput,
    ObjectReadInput, ObjectVerifyInput, ObjectWriteInput, PatchApplyInput, PatchApplyReport,
    PatchCheckoutInput, PatchCheckoutReport, PatchExportInput, PatchExportReport, PatchListEntry,
    PatchListInput, PatchListReport, PatchPrepareInput, PatchPrepareReport, PatchRetireInput,
    PatchRetireReport, PatchShowInput, PatchShowReport, ReceiptVerifyInput, ReceiptVerifyReport,
    RepoDataLocations, RepoInitInput, RepoStatusInput, RepoStatusReport, RepoStoreStatus,
    RunExecInput, RunExecReport, RunListEntry, RunListInput, RunListReport, RunShowInput,
    RunShowReport, SchemaEmitInput, SyncDivergentRef, SyncInput, SyncReport, SyncStatusReport,
    SyncVerifyInput, SyncVerifyObjectDiagnostic, SyncVerifyPatchDiagnostic, SyncVerifyReport,
};
use rickydata_git_relay::{
    BundlePullReport, BundlePullRequest, BundlePushReport, BundlePushRequest, RepoRelayStatusReport,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::process::{Command as StdCommand, ExitCode};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

#[derive(Debug, Parser)]
#[command(name = "rickygit")]
#[command(about = "Agent-native Git-compatible protocol tooling")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    Doctor(DoctorArgs),
    Init(InspectArgs),
    Inspect(InspectArgs),
    Status(StatusArgs),
    Discovery(InspectArgs),
    Issue(IssueCommand),
    Manifest(JsonFlag),
    Schema(JsonFlag),
    Attempt(AttemptCommand),
    Intent(IntentCommand),
    Object(ObjectCommand),
    Run(RunCommand),
    Change(ChangeCommand),
    Patch(PatchCommand),
    Proof(ProofArgs),
    Receipt(ReceiptCommand),
    Sync(SyncCommand),
    Relay(RelayCommand),
    Graph(GraphCommand),
    Impact(ImpactArgs),
    Context(ContextArgs),
    ProjectKfdb(ProjectKfdbArgs),
    Work(WorkCommand),
    Note(NoteCommand),
    Key(KeyCommand),
}

#[derive(Debug, Args)]
struct JsonFlag {
    #[arg(long)]
    json: bool,
}

#[derive(Debug, Args)]
struct DoctorArgs {
    #[arg(long)]
    json: bool,
    #[arg(long = "relay-url")]
    relay_url: Option<String>,
    #[arg(long = "tee-url")]
    tee_url: Option<String>,
    #[arg(long = "agent-id")]
    agent_id: Option<String>,
}

#[derive(Debug, Args)]
struct InspectArgs {
    #[arg(long)]
    repo: PathBuf,
    #[arg(long)]
    json: bool,
}

#[derive(Debug, Args)]
struct StatusArgs {
    #[arg(long)]
    repo: PathBuf,
    #[arg(long)]
    remote: Option<String>,
    #[arg(long)]
    json: bool,
}

#[derive(Debug, Args)]
struct IntentCommand {
    #[command(subcommand)]
    command: IntentSubcommand,
}

#[derive(Debug, Args)]
struct IssueCommand {
    #[command(subcommand)]
    command: IssueSubcommand,
}

#[derive(Debug, Args)]
struct AttemptCommand {
    #[command(subcommand)]
    command: AttemptSubcommand,
}

#[derive(Debug, Args)]
struct ObjectCommand {
    #[command(subcommand)]
    command: ObjectSubcommand,
}

#[derive(Debug, Args)]
struct RunCommand {
    #[command(subcommand)]
    command: RunSubcommand,
}

#[derive(Debug, Args)]
struct ChangeCommand {
    #[command(subcommand)]
    command: ChangeSubcommand,
}

#[derive(Debug, Args)]
struct PatchCommand {
    #[command(subcommand)]
    command: PatchSubcommand,
}

#[derive(Debug, Args)]
struct SyncCommand {
    #[command(subcommand)]
    command: SyncSubcommand,
}

#[derive(Debug, Args)]
struct RelayCommand {
    #[command(subcommand)]
    command: RelaySubcommand,
}

#[derive(Debug, Args)]
struct GraphCommand {
    #[command(subcommand)]
    command: GraphSubcommand,
}

#[derive(Debug, Args)]
struct ImpactArgs {
    #[arg(long)]
    repo: PathBuf,
    #[arg(long = "attempt-id")]
    attempt_id: Option<String>,
    #[arg(long = "changed-file")]
    changed_file: Vec<String>,
    #[arg(long)]
    base: Option<String>,
    #[arg(long)]
    head: Option<String>,
    #[arg(long)]
    json: bool,
}

#[derive(Debug, Args)]
struct ContextArgs {
    #[arg(long)]
    repo: PathBuf,
    #[arg(long)]
    query: Option<String>,
    #[arg(long)]
    path: Option<String>,
    #[arg(long = "attempt-id")]
    attempt_id: Option<String>,
    #[arg(long, default_value_t = 10)]
    limit: usize,
    #[arg(long)]
    json: bool,
}

#[derive(Debug, Args)]
struct ProjectKfdbArgs {
    #[arg(long)]
    repo: PathBuf,
    #[arg(long = "kfdb-url")]
    kfdb_url: Option<String>,
    #[arg(long = "api-key-env", default_value = "RICKYDATA_GIT_KFDB_API_KEY")]
    api_key_env: String,
    #[arg(long)]
    scope: Option<String>,
    /// Allow live KFDB projection without sign-to-derive headers.
    ///
    /// rickydata repo/execution data is private by default; use only for
    /// intentionally public demo data. Dry-runs do not require this flag.
    #[arg(long = "allow-public-kfdb")]
    allow_public_kfdb: bool,
    #[arg(
        long = "derive-session-id-env",
        default_value = "RICKYDATA_KFDB_DERIVE_SESSION_ID"
    )]
    derive_session_id_env: String,
    #[arg(long = "derive-key-env", default_value = "RICKYDATA_KFDB_DERIVE_KEY")]
    derive_key_env: String,
    #[arg(
        long = "wallet-address-env",
        default_value = "RICKYDATA_KFDB_WALLET_ADDRESS"
    )]
    wallet_address_env: String,
    #[arg(long)]
    dry_run: bool,
    #[arg(long = "include-code-structure")]
    include_code_structure: bool,
    #[arg(long = "batch-size", default_value_t = 100)]
    batch_size: usize,
    #[arg(long)]
    json: bool,
}

#[derive(Debug, Args)]
struct WorkCommand {
    #[command(subcommand)]
    command: WorkSubcommand,
}

#[derive(Debug, Args)]
struct NoteCommand {
    #[command(subcommand)]
    command: NoteSubcommand,
}

#[derive(Debug, Args)]
struct KeyCommand {
    #[command(subcommand)]
    command: KeySubcommand,
}

#[derive(Debug, Subcommand)]
enum IntentSubcommand {
    Validate(IntentFileArgs),
    Hash(IntentFileArgs),
    List(IntentListArgs),
    Show(ObjectIdArgs),
    Write(IntentWriteArgs),
}

#[derive(Debug, Subcommand)]
enum IssueSubcommand {
    Import(IssueImportArgs),
}

#[derive(Debug, Subcommand)]
enum AttemptSubcommand {
    Abandon(AttemptTransitionArgs),
    List(AttemptListArgs),
    Show(AttemptShowArgs),
    Start(AttemptStartArgs),
    Status(AttemptShowArgs),
    Submit(AttemptTransitionArgs),
}

#[derive(Debug, Subcommand)]
enum ObjectSubcommand {
    Write(ObjectWriteArgs),
    Read(ObjectIdArgs),
    Verify(ObjectIdArgs),
}

#[derive(Debug, Subcommand)]
enum RunSubcommand {
    Exec(RunExecArgs),
    List(RunListArgs),
    Show(RunShowArgs),
}

#[derive(Debug, Subcommand)]
enum ChangeSubcommand {
    Detect(ChangeDetectArgs),
    List(ChangeListArgs),
    Show(ChangeShowArgs),
}

#[derive(Debug, Subcommand)]
enum PatchSubcommand {
    Apply(PatchApplyArgs),
    Checkout(PatchCheckoutArgs),
    Export(PatchExportArgs),
    List(PatchListArgs),
    Prepare(PatchPrepareArgs),
    Retire(PatchRetireArgs),
    ReviewQueue(PatchListArgs),
    Show(PatchShowArgs),
}

#[derive(Debug, Subcommand)]
enum SyncSubcommand {
    Pull(SyncArgs),
    Push(SyncArgs),
    Status(SyncArgs),
    Verify(SyncVerifyArgs),
}

#[derive(Debug, Subcommand)]
enum RelaySubcommand {
    Pull(RelayPullArgs),
    Push(RelayPushArgs),
    Status(RelayStatusArgs),
}

#[derive(Debug, Subcommand)]
enum GraphSubcommand {
    Scan(GraphScanArgs),
}

#[derive(Debug, Args)]
struct GraphScanArgs {
    #[arg(long)]
    repo: PathBuf,
    #[arg(long)]
    commit: Option<String>,
    #[arg(long = "include-code-structure")]
    include_code_structure: bool,
    #[arg(long)]
    json: bool,
}

#[derive(Debug, Subcommand)]
enum WorkSubcommand {
    Start(WorkStartArgs),
}

#[derive(Debug, Subcommand)]
enum NoteSubcommand {
    Send(NoteSendArgs),
    Inbox(NoteInboxArgs),
    List(NoteListArgs),
}

#[derive(Debug, Args)]
struct NoteSendArgs {
    #[arg(long)]
    repo: PathBuf,
    #[arg(long)]
    from: String,
    #[arg(long)]
    to: String,
    #[arg(long)]
    text: String,
    #[arg(long)]
    thread: Option<String>,
    #[arg(long = "in-reply-to")]
    in_reply_to: Option<String>,
    /// Link this note to one or more rickydata object ids (intent/attempt/run/patch).
    #[arg(long = "ref")]
    refs: Vec<String>,
    #[command(flatten)]
    signing: SigningKeyArgs,
    #[arg(long)]
    json: bool,
}

#[derive(Debug, Args)]
struct NoteInboxArgs {
    #[arg(long)]
    repo: PathBuf,
    /// The agent reading its inbox; matches notes addressed to it or `all`.
    #[arg(long)]
    agent: String,
    /// Only show notes newer than this Unix-ms timestamp (overrides the read marker).
    #[arg(long = "since-ms")]
    since_ms: Option<u64>,
    /// Include every matching note, ignoring the per-agent read marker.
    #[arg(long = "all-history")]
    all_history: bool,
    /// Include notes the reading agent sent itself.
    #[arg(long = "include-self")]
    include_self: bool,
    /// Read without advancing the per-agent read marker.
    #[arg(long)]
    peek: bool,
    #[arg(long)]
    json: bool,
}

#[derive(Debug, Args)]
struct NoteListArgs {
    #[arg(long)]
    repo: PathBuf,
    #[arg(long)]
    from: Option<String>,
    #[arg(long)]
    to: Option<String>,
    #[arg(long)]
    thread: Option<String>,
    #[arg(long)]
    json: bool,
}

#[derive(Debug, Args)]
struct ReceiptCommand {
    #[command(subcommand)]
    command: ReceiptSubcommand,
}

#[derive(Debug, Subcommand)]
enum ReceiptSubcommand {
    Verify(ReceiptVerifyCliArgs),
}

#[derive(Debug, Args)]
struct ReceiptVerifyCliArgs {
    #[arg(long)]
    repo: PathBuf,
    #[arg(long = "object-id")]
    object_id: String,
    #[arg(long = "tee-url")]
    tee_url: Option<String>,
    #[arg(long)]
    json: bool,
}

#[derive(Debug, Subcommand)]
enum KeySubcommand {
    Generate(KeyGenerateArgs),
    Init(KeyInitArgs),
    Show(KeyShowArgs),
}

#[derive(Debug, Args)]
struct KeyGenerateArgs {
    #[arg(long)]
    output: PathBuf,
    #[arg(long)]
    json: bool,
}

#[derive(Debug, Args)]
struct KeyInitArgs {
    #[arg(long = "agent-id")]
    agent_id: String,
    #[arg(long)]
    force: bool,
    #[arg(long)]
    json: bool,
}

#[derive(Debug, Args)]
struct KeyShowArgs {
    #[arg(long = "signing-key-file")]
    signing_key_file: PathBuf,
    #[arg(long)]
    json: bool,
}

#[derive(Debug, Clone, Args, Default)]
struct SigningKeyArgs {
    #[arg(long = "signing-key-file")]
    signing_key_file: Option<PathBuf>,
    #[arg(long = "signing-key")]
    signing_key: Option<String>,
    #[arg(long = "signer-label")]
    signer_label: Option<String>,
}

#[derive(Debug, Args)]
struct IntentFileArgs {
    intent_file: PathBuf,
    #[arg(long)]
    json: bool,
}

#[derive(Debug, Args)]
struct ObjectWriteArgs {
    #[arg(long)]
    repo: PathBuf,
    #[arg(long)]
    kind: String,
    #[arg(long)]
    body_file: PathBuf,
    #[arg(long)]
    json: bool,
    #[command(flatten)]
    signing: SigningKeyArgs,
}

#[derive(Debug, Args)]
struct IntentWriteArgs {
    #[arg(long)]
    repo: PathBuf,
    intent_file: PathBuf,
    #[arg(long)]
    json: bool,
    #[command(flatten)]
    signing: SigningKeyArgs,
}

#[derive(Debug, Args)]
struct IntentListArgs {
    #[arg(long)]
    repo: PathBuf,
    #[arg(long)]
    json: bool,
}

#[derive(Debug, Args)]
struct IssueImportArgs {
    #[arg(long)]
    repo: PathBuf,
    #[arg(long, default_value = "github")]
    platform: String,
    #[arg(long = "issue-repository")]
    issue_repository: String,
    #[arg(long = "issue-id")]
    issue_id: String,
    #[arg(long)]
    objective: String,
    #[arg(long)]
    url: Option<String>,
    #[arg(long)]
    base_commit: Option<String>,
    #[arg(long)]
    created_by: Option<String>,
    #[arg(long, default_value = "public_metadata")]
    privacy: String,
    #[arg(long)]
    json: bool,
    #[command(flatten)]
    signing: SigningKeyArgs,
}

#[derive(Debug, Args)]
struct AttemptStartArgs {
    #[arg(long)]
    repo: PathBuf,
    #[arg(long)]
    intent_id: String,
    #[arg(long)]
    agent_id: String,
    #[arg(long)]
    idempotency_key: Option<String>,
    #[arg(long)]
    base_commit: Option<String>,
    #[arg(long)]
    lease_expires_at_ms: Option<u64>,
    /// Record provenance against the main working tree instead of an isolated
    /// worktree. Lower friction; no isolation between concurrent attempts.
    #[arg(long = "in-place")]
    in_place: bool,
    #[arg(long)]
    json: bool,
    #[command(flatten)]
    signing: SigningKeyArgs,
}

#[derive(Debug, Args)]
struct AttemptListArgs {
    #[arg(long)]
    repo: PathBuf,
    #[arg(long)]
    json: bool,
}

#[derive(Debug, Args)]
struct AttemptShowArgs {
    #[arg(long)]
    repo: PathBuf,
    #[arg(long)]
    attempt_id: String,
    #[arg(long)]
    json: bool,
}

#[derive(Debug, Args)]
struct AttemptTransitionArgs {
    #[arg(long)]
    repo: PathBuf,
    #[arg(long)]
    attempt_id: String,
    #[arg(long)]
    reason: Option<String>,
    #[arg(long)]
    by: Option<String>,
    #[arg(long)]
    json: bool,
    #[command(flatten)]
    signing: SigningKeyArgs,
}

#[derive(Debug, Args)]
struct ObjectIdArgs {
    #[arg(long)]
    repo: PathBuf,
    #[arg(long)]
    object_id: String,
    #[arg(long)]
    json: bool,
}

#[derive(Debug, Args)]
struct RunExecArgs {
    #[arg(long)]
    repo: PathBuf,
    #[arg(long)]
    attempt_id: String,
    #[arg(long)]
    record_command_argv: bool,
    #[arg(long)]
    json: bool,
    #[command(flatten)]
    signing: SigningKeyArgs,
    #[arg(required = true, last = true)]
    command: Vec<String>,
}

#[derive(Debug, Args)]
struct RunListArgs {
    #[arg(long)]
    repo: PathBuf,
    #[arg(long)]
    json: bool,
}

#[derive(Debug, Args)]
struct RunShowArgs {
    #[arg(long)]
    repo: PathBuf,
    #[arg(long)]
    run_id: String,
    #[arg(long)]
    json: bool,
}

#[derive(Debug, Args)]
struct ChangeDetectArgs {
    #[arg(long)]
    repo: PathBuf,
    #[arg(long)]
    attempt_id: String,
    #[arg(long = "run-id")]
    run_ids: Vec<String>,
    #[arg(long)]
    json: bool,
    #[command(flatten)]
    signing: SigningKeyArgs,
}

#[derive(Debug, Args)]
struct ChangeListArgs {
    #[arg(long)]
    repo: PathBuf,
    #[arg(long)]
    json: bool,
}

#[derive(Debug, Args)]
struct ChangeShowArgs {
    #[arg(long)]
    repo: PathBuf,
    #[arg(long)]
    change_id: String,
    #[arg(long)]
    json: bool,
}

#[derive(Debug, Args)]
struct PatchPrepareArgs {
    #[arg(long)]
    repo: PathBuf,
    #[arg(long)]
    attempt_id: String,
    #[arg(long)]
    json: bool,
    #[command(flatten)]
    signing: SigningKeyArgs,
}

#[derive(Debug, Args)]
struct PatchListArgs {
    #[arg(long)]
    repo: PathBuf,
    #[arg(long)]
    json: bool,
}

#[derive(Debug, Args)]
struct PatchShowArgs {
    #[arg(long)]
    repo: PathBuf,
    #[arg(long)]
    patch_id: String,
    #[arg(long)]
    json: bool,
}

#[derive(Debug, Args)]
struct PatchExportArgs {
    #[arg(long)]
    repo: PathBuf,
    #[arg(long)]
    patch_id: String,
    #[arg(long)]
    output: PathBuf,
    #[arg(long)]
    force: bool,
    #[arg(long)]
    json: bool,
}

#[derive(Debug, Args)]
struct PatchApplyArgs {
    #[arg(long)]
    repo: PathBuf,
    #[arg(long)]
    patch_id: String,
    #[arg(long)]
    allow_dirty: bool,
    #[arg(long)]
    allow_base_drift: bool,
    #[arg(long)]
    applied_by: Option<String>,
    #[arg(long)]
    reason: Option<String>,
    #[arg(long)]
    idempotency_key: Option<String>,
    #[arg(long)]
    json: bool,
}

#[derive(Debug, Args)]
struct PatchCheckoutArgs {
    #[arg(long)]
    repo: PathBuf,
    #[arg(long)]
    patch_id: String,
    #[arg(long)]
    path: Option<PathBuf>,
    #[arg(long)]
    force: bool,
    #[arg(long)]
    allow_base_drift: bool,
    #[arg(long)]
    json: bool,
}

#[derive(Debug, Args)]
struct PatchRetireArgs {
    #[arg(long)]
    repo: PathBuf,
    #[arg(long)]
    patch_id: String,
    #[arg(long)]
    reason: String,
    #[arg(long)]
    retired_by: Option<String>,
    #[arg(long)]
    idempotency_key: Option<String>,
    #[arg(long)]
    json: bool,
}

#[derive(Debug, Args)]
struct SyncArgs {
    #[arg(long)]
    repo: PathBuf,
    #[arg(long, default_value = "origin")]
    remote: String,
    #[arg(long)]
    json: bool,
    #[command(flatten)]
    signing: SigningKeyArgs,
}

#[derive(Debug, Args)]
struct SyncVerifyArgs {
    #[arg(long)]
    repo: PathBuf,
    #[arg(long = "tee-url")]
    tee_url: Option<String>,
    #[arg(long)]
    json: bool,
}

#[derive(Debug, Args)]
struct RelayPushArgs {
    #[arg(long)]
    repo: PathBuf,
    #[arg(long)]
    url: String,
    #[arg(long)]
    repo_id: Option<String>,
    #[arg(long)]
    idempotency_key: Option<String>,
    #[arg(long, default_value_t = 20)]
    chunk_size: usize,
    /// Bearer token for an auth-gated relay (or set RICKYDATA_RELAY_AUTH_TOKEN).
    #[arg(long = "auth-token")]
    auth_token: Option<String>,
    #[arg(long)]
    json: bool,
}

#[derive(Debug, Args)]
struct RelayPullArgs {
    #[arg(long)]
    repo: PathBuf,
    #[arg(long)]
    url: String,
    #[arg(long)]
    repo_id: Option<String>,
    #[arg(long)]
    limit: Option<usize>,
    /// Bearer token for an auth-gated relay (or set RICKYDATA_RELAY_AUTH_TOKEN).
    #[arg(long = "auth-token")]
    auth_token: Option<String>,
    #[arg(long)]
    json: bool,
}

#[derive(Debug, Args)]
struct RelayStatusArgs {
    #[arg(long)]
    repo: PathBuf,
    #[arg(long)]
    url: String,
    #[arg(long)]
    repo_id: Option<String>,
    /// Bearer token for an auth-gated relay (or set RICKYDATA_RELAY_AUTH_TOKEN).
    #[arg(long = "auth-token")]
    auth_token: Option<String>,
    #[arg(long)]
    json: bool,
}

#[derive(Debug, Args)]
struct ProofArgs {
    #[arg(long)]
    repo: PathBuf,
    #[arg(long)]
    remote: Option<String>,
    #[arg(long = "relay-url")]
    relay_url: Option<String>,
    #[arg(long)]
    repo_id: Option<String>,
    #[arg(long = "kfdb-url")]
    kfdb_url: Option<String>,
    #[arg(long = "kfdb-bearer-token")]
    kfdb_bearer_token: Option<String>,
    #[arg(long = "kfdb-bearer-token-env")]
    kfdb_bearer_token_env: Option<String>,
    #[arg(long = "tee-url")]
    tee_url: Option<String>,
    #[arg(long)]
    json: bool,
}

#[derive(Debug, Args)]
struct WorkStartArgs {
    #[arg(long)]
    repo: PathBuf,
    #[arg(long, default_value = "github")]
    platform: String,
    #[arg(long = "issue-repository")]
    issue_repository: Option<String>,
    #[arg(long = "issue-id")]
    issue_id: Option<String>,
    #[arg(long)]
    objective: String,
    #[arg(long)]
    url: Option<String>,
    #[arg(long)]
    agent_id: String,
    #[arg(long)]
    idempotency_key: Option<String>,
    #[arg(long)]
    base_commit: Option<String>,
    #[arg(long)]
    lease_expires_at_ms: Option<u64>,
    #[arg(long)]
    created_by: Option<String>,
    #[arg(long, default_value = "public_metadata")]
    privacy: String,
    /// Record provenance against the main working tree instead of an isolated
    /// worktree. Lower friction; no isolation between concurrent attempts.
    #[arg(long = "in-place")]
    in_place: bool,
    #[arg(long)]
    json: bool,
    #[command(flatten)]
    signing: SigningKeyArgs,
}

fn main() -> ExitCode {
    let cli = match Cli::try_parse() {
        Ok(cli) => cli,
        Err(error) => return handle_parse_error(error),
    };

    match run(cli) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            let payload = CommandErrorReport {
                status: "error".to_string(),
                kind: "command".to_string(),
                message: error.to_string(),
            };
            println!("{}", serde_json::to_string_pretty(&payload).unwrap());
            ExitCode::FAILURE
        }
    }
}

fn handle_parse_error(error: clap::Error) -> ExitCode {
    if matches!(
        error.kind(),
        ErrorKind::DisplayHelp | ErrorKind::DisplayVersion
    ) {
        if error.print().is_err() {
            return ExitCode::FAILURE;
        }
        return ExitCode::SUCCESS;
    }

    let payload = CommandErrorReport {
        status: "error".to_string(),
        kind: "argument_parse".to_string(),
        message: error.to_string(),
    };
    println!("{}", serde_json::to_string_pretty(&payload).unwrap());
    ExitCode::FAILURE
}

fn run(cli: Cli) -> Result<()> {
    match cli.command {
        Command::Doctor(args) => {
            let _input = DoctorInput {
                json: args.json,
                relay_url: args.relay_url.clone(),
                tee_url: args.tee_url.clone(),
                agent_id: args.agent_id.clone(),
            };
            let signing_key_configured =
                args.agent_id.as_ref().map(|id| agent_key_path(id).exists());
            let relay_health = args.relay_url.as_ref().map(|url| {
                match relay_client().and_then(|c| {
                    c.get(format!("{}/health", url.trim_end_matches('/')))
                        .send()
                        .map_err(|e| e.into())
                }) {
                    Ok(resp) if resp.status().is_success() => "ok".to_string(),
                    Ok(resp) => format!("unhealthy ({})", resp.status()),
                    Err(err) => format!("unreachable ({})", err),
                }
            });
            let signer_tee_reachable = args.tee_url.as_ref().map(|url| tee_signer_status_ok(url));
            print_json(&DoctorReport {
                status: "ok".to_string(),
                workspace: "rickydata_git".to_string(),
                git_backend: "gix".to_string(),
                rdl_contracts: rickydata_git_rdl::command_manifest_exports().len(),
                mutating_commands_enabled: true,
                signing_key_configured,
                relay_health,
                signer_tee_reachable,
            })
        }
        Command::Init(args) => {
            let input = init_input(args);
            let report = rickydata_git_git::init_rickydata_repository(input.repo)?;
            print_json(&report)
        }
        Command::Inspect(args) => {
            let input = inspect_input(args);
            let inspection = rickydata_git_git::inspect_repository(input.repo)?;
            print_json(&inspection)
        }
        Command::Status(args) => {
            let input = status_input(args);
            repo_status(input)
        }
        Command::Discovery(args) => {
            let input = discovery_input(args);
            let inspection = rickydata_git_git::inspect_repository(&input.repo)?;
            let repository = inspection
                .root_path
                .as_ref()
                .map(|path| path.display().to_string())
                .unwrap_or_else(|| input.repo.clone());
            let discovery =
                rickydata_git_rdl::rdl_discovery_object(repository, inspection.head_commit, None);
            let object =
                CanonicalObject::new("agent.discovery", DEFAULT_SCHEMA_VERSION, 0, discovery)?;
            print_json(&DiscoveryEmitReport {
                object_id: object.object_id,
                body_hash: object.body_hash,
                discovery: object.body,
            })
        }
        Command::Issue(issue) => run_issue(issue.command),
        Command::Manifest(args) => {
            let _input = ManifestEmitInput { json: args.json };
            print_json(&rickydata_git_rdl::command_manifest_exports())
        }
        Command::Schema(args) => {
            let _input = SchemaEmitInput { json: args.json };
            print_json(&schema_catalog()?)
        }
        Command::Attempt(attempt) => run_attempt(attempt.command),
        Command::Intent(intent) => run_intent(intent.command),
        Command::Object(object) => run_object(object.command),
        Command::Run(run) => run_run(run.command),
        Command::Change(change) => run_change(change.command),
        Command::Patch(patch) => run_patch(patch.command),
        Command::Proof(args) => run_proof(args),
        Command::Receipt(receipt) => run_receipt(receipt.command),
        Command::Sync(sync) => run_sync(sync.command),
        Command::Relay(relay) => run_relay(relay.command),
        Command::Graph(graph) => run_graph(graph.command),
        Command::Impact(args) => run_impact(args),
        Command::Context(args) => run_context(args),
        Command::ProjectKfdb(args) => run_project_kfdb(args),
        Command::Work(work) => run_work(work.command),
        Command::Note(note) => run_note(note.command),
        Command::Key(key) => run_key(key.command),
    }
}

fn run_key(command: KeySubcommand) -> Result<()> {
    match command {
        KeySubcommand::Generate(args) => {
            let _input = KeyGenerateInput {
                output: args.output.display().to_string(),
                json: args.json,
            };
            if args.output.exists() {
                anyhow::bail!(
                    "signing key output path already exists: {}",
                    args.output.display()
                );
            }
            let key = generate_signing_keypair();
            save_signing_key_to_file(&key, &args.output).with_context(|| {
                format!("failed to save signing key to {}", args.output.display())
            })?;
            let public_key = hex::encode(key.verifying_key().to_bytes());
            print_json(&KeyGenerateReport {
                status: "ok".to_string(),
                algorithm: SIGNATURE_ALGORITHM_ED25519.to_string(),
                public_key,
                output_path: args.output.display().to_string(),
            })
        }
        KeySubcommand::Init(args) => {
            let _input = KeyInitInput {
                agent_id: args.agent_id.clone(),
                force: args.force,
                json: args.json,
            };
            let key_path = agent_key_path(&args.agent_id);
            if key_path.exists() && !args.force {
                anyhow::bail!(
                    "key already exists at {}; use --force to overwrite",
                    key_path.display()
                );
            }
            if let Some(parent) = key_path.parent() {
                std::fs::create_dir_all(parent).with_context(|| {
                    format!("failed to create key directory {}", parent.display())
                })?;
                // The signing-keys directory holds long-lived secrets: owner-only (0700).
                #[cfg(unix)]
                {
                    use std::os::unix::fs::PermissionsExt;
                    std::fs::set_permissions(parent, std::fs::Permissions::from_mode(0o700))
                        .with_context(|| {
                            format!("failed to chmod 700 key directory {}", parent.display())
                        })?;
                }
            }
            let key = generate_signing_keypair();
            save_signing_key_to_file(&key, &key_path)
                .with_context(|| format!("failed to save signing key to {}", key_path.display()))?;
            let public_key = hex::encode(key.verifying_key().to_bytes());
            print_json(&KeyInitReport {
                status: "ok".to_string(),
                algorithm: SIGNATURE_ALGORITHM_ED25519.to_string(),
                public_key,
                agent_id: args.agent_id,
                key_path: key_path.display().to_string(),
            })
        }
        KeySubcommand::Show(args) => {
            let _input = KeyShowInput {
                signing_key_file: args.signing_key_file.display().to_string(),
                json: args.json,
            };
            let key = load_signing_key_from_file(&args.signing_key_file).with_context(|| {
                format!(
                    "failed to load signing key from {}",
                    args.signing_key_file.display()
                )
            })?;
            let public_key = hex::encode(key.verifying_key().to_bytes());
            print_json(&KeyShowReport {
                algorithm: SIGNATURE_ALGORITHM_ED25519.to_string(),
                public_key,
                signing_key_file: args.signing_key_file.display().to_string(),
            })
        }
    }
}

fn resolve_signing_key(
    args: &SigningKeyArgs,
    agent_id: Option<&str>,
) -> Result<Option<(SigningKey, Option<String>)>> {
    match (&args.signing_key_file, &args.signing_key) {
        (Some(_), Some(_)) => {
            anyhow::bail!("--signing-key-file and --signing-key are mutually exclusive; pick one")
        }
        (Some(path), None) => {
            let key = load_signing_key_from_file(path)
                .with_context(|| format!("failed to load signing key from {}", path.display()))?;
            Ok(Some((key, args.signer_label.clone())))
        }
        (None, Some(hex_text)) => {
            let raw = hex::decode(hex_text.trim())
                .with_context(|| "--signing-key must be hex-encoded ed25519 seed bytes")?;
            let bytes: [u8; 32] = raw.as_slice().try_into().map_err(|_| {
                anyhow::anyhow!("--signing-key must decode to 32 bytes, got {}", raw.len())
            })?;
            Ok(Some((
                SigningKey::from_bytes(&bytes),
                args.signer_label.clone(),
            )))
        }
        (None, None) => {
            if let Ok(env_path) = std::env::var("RICKYGIT_SIGNING_KEY_FILE") {
                let path = PathBuf::from(&env_path);
                if path.exists() {
                    let key = load_signing_key_from_file(&path).with_context(|| {
                        format!(
                            "failed to load signing key from RICKYGIT_SIGNING_KEY_FILE={}",
                            env_path
                        )
                    })?;
                    return Ok(Some((key, args.signer_label.clone())));
                }
            }
            let effective_agent_id = agent_id
                .map(|s| s.to_string())
                .or_else(|| std::env::var("RICKYGIT_AGENT_ID").ok());
            if let Some(id) = effective_agent_id {
                let key_path = agent_key_path(&id);
                if key_path.exists() {
                    let key = load_signing_key_from_file(&key_path).with_context(|| {
                        format!(
                            "failed to load signing key for agent `{}` from {}",
                            id,
                            key_path.display()
                        )
                    })?;
                    return Ok(Some((key, args.signer_label.clone().or(Some(id)))));
                }
            }
            Ok(None)
        }
    }
}

fn agent_key_path(agent_id: &str) -> PathBuf {
    let sanitized = agent_id.replace(':', "_");
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
    PathBuf::from(home)
        .join(".rickydata")
        .join("signing-keys")
        .join(format!("{sanitized}.key"))
}

fn write_signed_or_cached(
    repo: impl AsRef<Path>,
    kind: &str,
    body: serde_json::Value,
    signing: Option<&(SigningKey, Option<String>)>,
) -> Result<rickydata_git_git::ObjectWriteReport> {
    let Some((signing_key, signer_label)) = signing else {
        return Ok(rickydata_git_git::write_cached_object(repo, kind, body)?);
    };
    let canonical_body = rickydata_git_core::canonical_json(&body);
    let mut object: CanonicalObject<serde_json::Value> =
        CanonicalObject::new(kind, DEFAULT_SCHEMA_VERSION, 0, canonical_body)?;
    let signature = sign_object(&object, signing_key, signer_label.clone())?;
    object.signatures.push(signature);
    Ok(rickydata_git_git::write_canonical_object(repo, &object)?)
}

#[derive(Debug, Serialize)]
struct IssueImportCliReport {
    status: String,
    valid: bool,
    diagnostics: Vec<rickydata_git_agent::IntentDiagnostic>,
    intent: WorkIntent,
    object: Option<rickydata_git_git::ObjectWriteReport>,
}

#[derive(Debug, Serialize)]
struct WorkStartCliReport {
    status: String,
    intent: WorkIntent,
    intent_object: rickydata_git_git::ObjectWriteReport,
    attempt: AgentAttempt,
    attempt_object: rickydata_git_git::ObjectWriteReport,
    local_worktree_path: String,
    worktree_created: bool,
}

fn run_issue(command: IssueSubcommand) -> Result<()> {
    match command {
        IssueSubcommand::Import(args) => {
            let signing = resolve_signing_key(&args.signing, None)?;
            let intent = issue_import_intent(IssueIntentFields {
                platform: args.platform,
                repository: Some(args.issue_repository),
                issue_id: Some(args.issue_id),
                url: args.url,
                objective: args.objective,
                base_commit: args.base_commit,
                created_by: args.created_by,
                privacy: parse_privacy_class(&args.privacy)?,
            });
            let diagnostics = validate_work_intent(&intent);
            if !diagnostics.is_empty() {
                return print_json(&IssueImportCliReport {
                    status: "invalid".to_string(),
                    valid: false,
                    diagnostics,
                    intent,
                    object: None,
                });
            }
            let object = write_signed_or_cached(
                &args.repo,
                "agent.intent",
                serde_json::to_value(&intent)?,
                signing.as_ref(),
            )?;
            print_json(&IssueImportCliReport {
                status: "ok".to_string(),
                valid: true,
                diagnostics,
                intent,
                object: Some(object),
            })
        }
    }
}

fn run_work(command: WorkSubcommand) -> Result<()> {
    match command {
        WorkSubcommand::Start(args) => {
            let signing = resolve_signing_key(&args.signing, Some(&args.agent_id))?;
            let in_place = args.in_place;
            let repo = args.repo.display().to_string();
            let intent = issue_import_intent(IssueIntentFields {
                platform: args.platform,
                repository: args.issue_repository,
                issue_id: args.issue_id,
                url: args.url,
                objective: args.objective,
                base_commit: args.base_commit.clone(),
                created_by: args.created_by,
                privacy: parse_privacy_class(&args.privacy)?,
            });
            let diagnostics = validate_work_intent(&intent);
            if !diagnostics.is_empty() {
                anyhow::bail!("generated work intent is invalid");
            }
            let intent_object = write_signed_or_cached(
                &repo,
                "agent.intent",
                serde_json::to_value(&intent)?,
                signing.as_ref(),
            )?;
            let inspection = rickydata_git_git::inspect_repository(&repo)?;
            let base_commit = args
                .base_commit
                .or(inspection.head_commit)
                .context("repository HEAD is unborn; pass --base-commit explicitly")?;
            let attempt_input = AttemptStartInput {
                repo: repo.clone(),
                intent_id: intent_object.object_id.clone(),
                agent_id: args.agent_id,
                idempotency_key: args.idempotency_key,
                base_commit: Some(base_commit.clone()),
                lease_expires_at_ms: args.lease_expires_at_ms,
                json: true,
            };
            let attempt_id = compute_attempt_id(&attempt_input, &base_commit)?;
            let attempt = AgentAttempt {
                attempt_id: attempt_id.clone(),
                intent_id: intent_object.object_id.clone(),
                base_commit: base_commit.clone(),
                agent_id: attempt_input.agent_id,
                lease_expires_at_ms: attempt_input.lease_expires_at_ms,
                status: AttemptStatus::Running,
                in_place,
            };
            let (local_worktree_path, worktree_created) = if in_place {
                (PathBuf::from(&repo), false)
            } else {
                ensure_attempt_worktree(Path::new(&repo), &attempt_id, &base_commit)?
            };
            let attempt_object = write_signed_or_cached(
                &repo,
                "agent.attempt",
                serde_json::to_value(&attempt)?,
                signing.as_ref(),
            )?;
            print_json(&WorkStartCliReport {
                status: "ok".to_string(),
                intent,
                intent_object,
                attempt,
                attempt_object,
                local_worktree_path: local_worktree_path.display().to_string(),
                worktree_created,
            })
        }
    }
}

#[derive(Debug, Serialize)]
struct NoteEntry {
    object_id: String,
    ref_name: String,
    git_object_id: String,
    signature_count: usize,
    note: AgentNote,
}

#[derive(Debug, Serialize)]
struct NoteSendReport {
    status: String,
    valid: bool,
    diagnostics: Vec<rickydata_git_agent::IntentDiagnostic>,
    note: AgentNote,
    object: Option<rickydata_git_git::ObjectWriteReport>,
}

#[derive(Debug, Serialize)]
struct NoteInboxReport {
    agent: String,
    count: usize,
    read_marker_ms: u64,
    marker_advanced: bool,
    notes: Vec<NoteEntry>,
}

#[derive(Debug, Serialize)]
struct NoteListReport {
    count: usize,
    notes: Vec<NoteEntry>,
}

#[derive(Debug, Serialize, Deserialize, Default)]
struct NoteReadMarker {
    last_read_ms: u64,
}

fn sanitize_agent_filename(agent: &str) -> String {
    agent
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.') {
                c
            } else {
                '_'
            }
        })
        .collect()
}

fn note_state_path(repo: &Path, agent: &str) -> Result<PathBuf> {
    let inspection = rickydata_git_git::inspect_repository(repo)?;
    let git_dir = inspection
        .git_dir
        .context("repository has no .git directory; run `rickygit init` first")?;
    Ok(git_dir
        .join("rickydata")
        .join("notes")
        .join("state")
        .join(format!("{}.json", sanitize_agent_filename(agent))))
}

fn read_note_marker(path: &Path) -> u64 {
    std::fs::read(path)
        .ok()
        .and_then(|bytes| serde_json::from_slice::<NoteReadMarker>(&bytes).ok())
        .map(|marker| marker.last_read_ms)
        .unwrap_or(0)
}

fn write_note_marker(path: &Path, last_read_ms: u64) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("failed to create note state dir {}", parent.display()))?;
    }
    let bytes = serde_json::to_vec(&NoteReadMarker { last_read_ms })?;
    std::fs::write(path, bytes)
        .with_context(|| format!("failed to write note read marker {}", path.display()))?;
    Ok(())
}

fn list_notes(repo: &Path) -> Result<Vec<NoteEntry>> {
    let object_entries = rickydata_git_git::list_ref_backed_objects(repo, Some("agent.note"))?;
    let mut notes = Vec::new();
    for entry in object_entries {
        let report = rickydata_git_git::read_cached_object(repo, &entry.object_id)?;
        let signature_count = report.object.signatures.len();
        let note: AgentNote = serde_json::from_value(report.object.body)?;
        notes.push(NoteEntry {
            object_id: entry.object_id,
            ref_name: entry.ref_name,
            git_object_id: entry.git_object_id,
            signature_count,
            note,
        });
    }
    notes.sort_by(|left, right| {
        left.note
            .created_at_ms
            .cmp(&right.note.created_at_ms)
            .then_with(|| left.object_id.cmp(&right.object_id))
    });
    Ok(notes)
}

fn run_note(command: NoteSubcommand) -> Result<()> {
    match command {
        NoteSubcommand::Send(args) => {
            let signing = resolve_signing_key(&args.signing, Some(&args.from))?;
            let note = AgentNote {
                from: args.from,
                to: args.to,
                body: args.text,
                thread: args.thread,
                in_reply_to: args.in_reply_to,
                refs: args.refs,
                created_at_ms: now_ms()?,
            };
            let diagnostics = validate_agent_note(&note);
            if !diagnostics.is_empty() {
                return print_json(&NoteSendReport {
                    status: "invalid".to_string(),
                    valid: false,
                    diagnostics,
                    note,
                    object: None,
                });
            }
            let object = write_signed_or_cached(
                &args.repo,
                "agent.note",
                serde_json::to_value(&note)?,
                signing.as_ref(),
            )?;
            print_json(&NoteSendReport {
                status: "ok".to_string(),
                valid: true,
                diagnostics,
                note,
                object: Some(object),
            })
        }
        NoteSubcommand::Inbox(args) => {
            let all_notes = list_notes(&args.repo)?;
            let marker_path = note_state_path(&args.repo, &args.agent)?;
            let lower_bound = if args.all_history {
                None
            } else {
                Some(
                    args.since_ms
                        .unwrap_or_else(|| read_note_marker(&marker_path)),
                )
            };

            // Newest note addressed to this agent (regardless of the display
            // window), so the read marker never replays a note already seen.
            let mut newest_addressed = read_note_marker(&marker_path);
            let mut matched = Vec::new();
            for entry in all_notes {
                let addressed = entry.note.to == args.agent || entry.note.to == "all";
                if !addressed {
                    continue;
                }
                newest_addressed = newest_addressed.max(entry.note.created_at_ms);
                if !args.include_self && entry.note.from == args.agent {
                    continue;
                }
                if let Some(bound) = lower_bound
                    && entry.note.created_at_ms <= bound
                {
                    continue;
                }
                matched.push(entry);
            }

            let marker_advanced = !args.peek;
            if marker_advanced {
                write_note_marker(&marker_path, newest_addressed)?;
            }
            print_json(&NoteInboxReport {
                agent: args.agent,
                count: matched.len(),
                read_marker_ms: newest_addressed,
                marker_advanced,
                notes: matched,
            })
        }
        NoteSubcommand::List(args) => {
            let notes = list_notes(&args.repo)?
                .into_iter()
                .filter(|entry| {
                    args.from
                        .as_ref()
                        .is_none_or(|from| &entry.note.from == from)
                        && args.to.as_ref().is_none_or(|to| &entry.note.to == to)
                        && args
                            .thread
                            .as_ref()
                            .is_none_or(|thread| entry.note.thread.as_deref() == Some(thread))
                })
                .collect::<Vec<_>>();
            print_json(&NoteListReport {
                count: notes.len(),
                notes,
            })
        }
    }
}

struct IssueIntentFields {
    platform: String,
    repository: Option<String>,
    issue_id: Option<String>,
    url: Option<String>,
    objective: String,
    base_commit: Option<String>,
    created_by: Option<String>,
    privacy: PrivacyClass,
}

fn issue_import_intent(fields: IssueIntentFields) -> WorkIntent {
    let issue_refs = match (fields.repository, fields.issue_id) {
        (Some(repository), Some(id)) => vec![IssueRef {
            platform: fields.platform,
            repository,
            id,
            url: fields.url,
        }],
        _ => Vec::new(),
    };
    let task_refs = if issue_refs.is_empty() {
        vec![rickydata_git_agent::TaskRef {
            system: "agent".to_string(),
            id: format!("work-{}", now_ms().unwrap_or(0)),
            title: Some(fields.objective.clone()),
        }]
    } else {
        Vec::new()
    };
    WorkIntent {
        objective: fields.objective,
        issue_refs,
        task_refs,
        base_commit: fields.base_commit,
        allowed_capabilities: Vec::new(),
        privacy: fields.privacy,
        tee_policy: None,
        release_guard: None,
        created_by: fields.created_by,
    }
}

fn parse_privacy_class(value: &str) -> Result<PrivacyClass> {
    match value {
        "public_metadata" => Ok(PrivacyClass::PublicMetadata),
        "private_body" => Ok(PrivacyClass::PrivateBody),
        "secret" => Ok(PrivacyClass::Secret),
        "encrypted" => Ok(PrivacyClass::Encrypted),
        other => anyhow::bail!("unsupported privacy class `{other}`"),
    }
}

fn run_attempt(command: AttemptSubcommand) -> Result<()> {
    match command {
        AttemptSubcommand::Abandon(args) => {
            let signing = resolve_signing_key(&args.signing, args.by.as_deref())?;
            let input = attempt_transition_input(args);
            write_attempt_transition(input, AttemptStatus::Abandoned, signing.as_ref())
        }
        AttemptSubcommand::List(args) => {
            let input = attempt_list_input(args);
            let attempts = list_attempts(&input.repo)?;
            print_json(&AttemptListReport { attempts })
        }
        AttemptSubcommand::Show(args) => {
            let input = attempt_show_input(args);
            let attempts = list_attempts(&input.repo)?;
            let Some(entry) = attempts
                .into_iter()
                .find(|entry| entry.attempt.attempt_id == input.attempt_id)
            else {
                anyhow::bail!("attempt {} was not found", input.attempt_id);
            };
            let report = rickydata_git_git::read_cached_object(&input.repo, &entry.object_id)?;
            print_json(&AttemptShowReport {
                object_id: entry.object_id,
                source: report.source,
                attempt: entry.attempt,
            })
        }
        AttemptSubcommand::Start(args) => {
            let signing = resolve_signing_key(&args.signing, Some(&args.agent_id))?;
            let in_place = args.in_place;
            let input = attempt_start_input(args);
            let intent_report =
                rickydata_git_git::read_cached_object(&input.repo, &input.intent_id)?;
            if intent_report.object.kind != "agent.intent" {
                anyhow::bail!(
                    "object {} is kind `{}`, expected `agent.intent`",
                    intent_report.object_id,
                    intent_report.object.kind
                );
            }
            let intent: WorkIntent = serde_json::from_value(intent_report.object.body)?;
            let diagnostics = validate_work_intent(&intent);
            if !diagnostics.is_empty() {
                anyhow::bail!("intent {} is invalid", input.intent_id);
            }

            let inspection = rickydata_git_git::inspect_repository(&input.repo)?;
            let base_commit = input
                .base_commit
                .clone()
                .or(inspection.head_commit)
                .context("repository HEAD is unborn; pass --base-commit explicitly")?;
            let attempt_id = compute_attempt_id(&input, &base_commit)?;
            let attempt = AgentAttempt {
                attempt_id: attempt_id.clone(),
                intent_id: input.intent_id.clone(),
                base_commit: base_commit.clone(),
                agent_id: input.agent_id.clone(),
                lease_expires_at_ms: input.lease_expires_at_ms,
                status: AttemptStatus::Running,
                in_place,
            };
            let (local_worktree_path, worktree_created) = if in_place {
                (PathBuf::from(&input.repo), false)
            } else {
                ensure_attempt_worktree(Path::new(&input.repo), &attempt_id, &base_commit)?
            };
            let object = write_signed_or_cached(
                &input.repo,
                "agent.attempt",
                serde_json::to_value(&attempt)?,
                signing.as_ref(),
            )?;
            print_json(&AttemptStartReport {
                attempt,
                object,
                local_worktree_path: local_worktree_path.display().to_string(),
                worktree_created,
            })
        }
        AttemptSubcommand::Status(args) => {
            let input = attempt_show_input(args);
            let report = build_attempt_status_report(&input.repo, &input.attempt_id)?;
            print_json(&report)
        }
        AttemptSubcommand::Submit(args) => {
            let signing = resolve_signing_key(&args.signing, args.by.as_deref())?;
            let input = attempt_transition_input(args);
            let patches = list_patches(&input.repo)?;
            if !patches
                .iter()
                .any(|entry| entry.patch.attempt_id == input.attempt_id)
            {
                anyhow::bail!(
                    "attempt {} has no prepared patch; run `rickygit patch prepare` first",
                    input.attempt_id
                );
            }
            write_attempt_transition(input, AttemptStatus::Submitted, signing.as_ref())
        }
    }
}

fn list_attempts(repo: &str) -> Result<Vec<AttemptListEntry>> {
    let object_entries = rickydata_git_git::list_ref_backed_objects(repo, Some("agent.attempt"))?;
    let mut attempts = Vec::new();
    for entry in object_entries {
        let report = rickydata_git_git::read_cached_object(repo, &entry.object_id)?;
        let attempt: AgentAttempt = serde_json::from_value(report.object.body)?;
        attempts.push(AttemptListEntry {
            object_id: entry.object_id,
            ref_name: entry.ref_name,
            git_object_id: entry.git_object_id,
            attempt,
        });
    }
    attempts.sort_by(|left, right| left.attempt.attempt_id.cmp(&right.attempt.attempt_id));
    Ok(attempts)
}

#[derive(Debug, Serialize)]
struct AttemptStatusCliReport {
    attempt_id: String,
    intent_id: String,
    base_commit: String,
    agent_id: String,
    status: AttemptStatus,
    status_object_id: Option<String>,
    worktree_path: String,
    worktree_exists: bool,
    changed: Option<bool>,
    diff_hash: Option<String>,
    diff_bytes: Option<u64>,
    file_paths: Vec<String>,
}

#[derive(Debug, Serialize)]
struct AttemptTransitionCliReport {
    status: String,
    attempt_id: String,
    effective_status: AttemptStatus,
    transition: AttemptStatusTransition,
    object: rickydata_git_git::ObjectWriteReport,
}

#[derive(Debug)]
struct AttemptTransitionInput {
    repo: String,
    attempt_id: String,
    reason: Option<String>,
    by: Option<String>,
}

fn write_attempt_transition(
    input: AttemptTransitionInput,
    status: AttemptStatus,
    signing: Option<&(SigningKey, Option<String>)>,
) -> Result<()> {
    let attempt = find_attempt(&input.repo, &input.attempt_id)?;
    let current_status = effective_attempt_status(&input.repo, &attempt.attempt_id)?.0;
    if !matches!(
        current_status,
        AttemptStatus::Running | AttemptStatus::Planned
    ) {
        anyhow::bail!(
            "attempt {} is {:?}, expected running or planned",
            attempt.attempt_id,
            current_status
        );
    }
    let transition = AttemptStatusTransition {
        attempt_id: attempt.attempt_id,
        status: status.clone(),
        reason: input.reason,
        created_by: input.by,
        created_at_ms: now_ms()?,
    };
    let object = write_signed_or_cached(
        &input.repo,
        "agent.attempt_status",
        serde_json::to_value(&transition)?,
        signing,
    )?;
    print_json(&AttemptTransitionCliReport {
        status: "ok".to_string(),
        attempt_id: transition.attempt_id.clone(),
        effective_status: status,
        transition,
        object,
    })
}

fn build_attempt_status_report(repo: &str, attempt_id: &str) -> Result<AttemptStatusCliReport> {
    let attempt = find_attempt(repo, attempt_id)?;
    let (status, status_object_id) = effective_attempt_status(repo, attempt_id)?;
    let worktree_path = attempt_worktree_path(Path::new(repo), attempt_id)?;
    let worktree_exists = worktree_path.is_dir();
    let detected = if worktree_exists {
        Some(detect_attempt_diff(Path::new(repo), &attempt)?)
    } else {
        None
    };

    Ok(AttemptStatusCliReport {
        attempt_id: attempt.attempt_id,
        intent_id: attempt.intent_id,
        base_commit: attempt.base_commit,
        agent_id: attempt.agent_id,
        status,
        status_object_id,
        worktree_path: worktree_path.display().to_string(),
        worktree_exists,
        changed: detected.as_ref().map(|diff| diff.changed),
        diff_hash: detected.as_ref().map(|diff| diff.diff_hash.clone()),
        diff_bytes: detected.as_ref().map(|diff| diff.diff_bytes),
        file_paths: detected.map(|diff| diff.file_paths).unwrap_or_default(),
    })
}

fn effective_attempt_status(
    repo: &str,
    attempt_id: &str,
) -> Result<(AttemptStatus, Option<String>)> {
    let attempt = find_attempt(repo, attempt_id)?;
    let mut transitions = Vec::new();
    for entry in rickydata_git_git::list_ref_backed_objects(repo, Some("agent.attempt_status"))? {
        let report = rickydata_git_git::read_cached_object(repo, &entry.object_id)?;
        let transition: AttemptStatusTransition = serde_json::from_value(report.object.body)?;
        if transition.attempt_id == attempt_id {
            transitions.push((entry.object_id, transition));
        }
    }
    transitions.sort_by(|left, right| {
        left.1
            .created_at_ms
            .cmp(&right.1.created_at_ms)
            .then_with(|| left.0.cmp(&right.0))
    });
    Ok(transitions
        .last()
        .map(|(object_id, transition)| (transition.status.clone(), Some(object_id.clone())))
        .unwrap_or((attempt.status, None)))
}

fn run_run(command: RunSubcommand) -> Result<()> {
    match command {
        RunSubcommand::Exec(args) => {
            let signing = resolve_signing_key(&args.signing, None)?;
            let input = run_exec_input(args);
            let attempt = find_attempt(&input.repo, &input.attempt_id)?;
            let (effective_status, _) = effective_attempt_status(&input.repo, &input.attempt_id)?;
            if effective_status != AttemptStatus::Running {
                anyhow::bail!(
                    "attempt {} is {:?}, expected running",
                    attempt.attempt_id,
                    effective_status
                );
            }
            let worktree_path = if attempt.in_place {
                PathBuf::from(&input.repo)
            } else {
                ensure_attempt_worktree(
                    Path::new(&input.repo),
                    &attempt.attempt_id,
                    &attempt.base_commit,
                )?
                .0
            };
            let started_at_ms = now_ms()?;
            let output = StdCommand::new(&input.command[0])
                .args(&input.command[1..])
                .current_dir(&worktree_path)
                .output()
                .with_context(|| format!("failed to execute `{}`", input.command[0]))?;
            let finished_at_ms = now_ms()?;
            let command_hash = stable_json_hash(&serde_json::json!({ "argv": input.command }))?;
            let stdout_hash = bytes_hash(&output.stdout);
            let stderr_hash = bytes_hash(&output.stderr);
            let exit_code = output.status.code();
            let result = if output.status.success() {
                AgentRunResult::Succeeded
            } else {
                AgentRunResult::Failed
            };
            let trace_id = stable_json_hash(&serde_json::json!({
                "attempt_id": attempt.attempt_id,
                "command_hash": command_hash,
                "exit_code": exit_code,
                "stdout_hash": stdout_hash,
                "stderr_hash": stderr_hash,
                "started_at_ms": started_at_ms,
                "finished_at_ms": finished_at_ms,
            }))?;
            let trace = AgentRunTrace {
                trace_id: trace_id.clone(),
                attempt_id: attempt.attempt_id.clone(),
                command_hash: command_hash.clone(),
                command_argv: input.record_command_argv.then(|| input.command.clone()),
                executable: input.command.first().cloned(),
                arg_count: input.command.len() as u64,
                exit_code,
                stdout_hash: stdout_hash.clone(),
                stderr_hash: stderr_hash.clone(),
                stdout_bytes: output.stdout.len() as u64,
                stderr_bytes: output.stderr.len() as u64,
                started_at_ms,
                finished_at_ms,
                result: result.clone(),
                privacy: if input.record_command_argv {
                    PrivacyClass::PrivateBody
                } else {
                    PrivacyClass::PublicMetadata
                },
                encrypted_body: None,
            };
            let trace_object = write_signed_or_cached(
                &input.repo,
                "agent.run_trace",
                serde_json::to_value(&trace)?,
                signing.as_ref(),
            )?;
            let run_id = stable_json_hash(&serde_json::json!({
                "attempt_id": attempt.attempt_id,
                "trace_hash": trace_id.clone(),
            }))?;
            let rdl_manifest_hashes = rickydata_git_rdl::command_manifest_exports()
                .into_iter()
                .filter(|export| export.manifest.name == "run_exec")
                .map(|export| export.stable_hash)
                .collect::<Vec<_>>();
            let run = AgentRun {
                run_id,
                attempt_id: attempt.attempt_id,
                trace_hash: Some(trace_id),
                command_hashes: vec![command_hash.clone()],
                rdl_manifest_hashes,
                started_at_ms,
                finished_at_ms: Some(finished_at_ms),
                result: Some(result),
            };
            let object = write_signed_or_cached(
                &input.repo,
                "agent.run",
                serde_json::to_value(&run)?,
                signing.as_ref(),
            )?;
            print_json(&RunExecReport {
                run,
                object,
                trace_object,
                exit_code,
                command_hash,
                stdout_hash,
                stderr_hash,
                stdout_bytes: output.stdout.len() as u64,
                stderr_bytes: output.stderr.len() as u64,
            })
        }
        RunSubcommand::List(args) => {
            let input = run_list_input(args);
            let runs = list_runs(&input.repo)?;
            print_json(&RunListReport { runs })
        }
        RunSubcommand::Show(args) => {
            let input = run_show_input(args);
            let runs = list_runs(&input.repo)?;
            let Some(entry) = runs
                .into_iter()
                .find(|entry| entry.run.run_id == input.run_id)
            else {
                anyhow::bail!("run {} was not found", input.run_id);
            };
            let report = rickydata_git_git::read_cached_object(&input.repo, &entry.object_id)?;
            print_json(&RunShowReport {
                object_id: entry.object_id,
                source: report.source,
                run: entry.run,
            })
        }
    }
}

fn find_attempt(repo: &str, attempt_id: &str) -> Result<AgentAttempt> {
    list_attempts(repo)?
        .into_iter()
        .find(|entry| entry.attempt.attempt_id == attempt_id)
        .map(|entry| entry.attempt)
        .with_context(|| format!("attempt {attempt_id} was not found"))
}

fn list_runs(repo: &str) -> Result<Vec<RunListEntry>> {
    let object_entries = rickydata_git_git::list_ref_backed_objects(repo, Some("agent.run"))?;
    let mut runs = Vec::new();
    for entry in object_entries {
        let report = rickydata_git_git::read_cached_object(repo, &entry.object_id)?;
        let run: AgentRun = serde_json::from_value(report.object.body)?;
        runs.push(RunListEntry {
            object_id: entry.object_id,
            ref_name: entry.ref_name,
            git_object_id: entry.git_object_id,
            run,
        });
    }
    runs.sort_by(|left, right| left.run.run_id.cmp(&right.run.run_id));
    Ok(runs)
}

fn run_change(command: ChangeSubcommand) -> Result<()> {
    match command {
        ChangeSubcommand::Detect(args) => {
            let signing = resolve_signing_key(&args.signing, None)?;
            let input = change_detect_input(args);
            let attempt = find_attempt(&input.repo, &input.attempt_id)?;
            let detected = detect_attempt_diff(Path::new(&input.repo), &attempt)?;
            if !detected.changed {
                anyhow::bail!(
                    "no worktree changes detected for attempt {}; refusing to write empty agent.change evidence",
                    attempt.attempt_id
                );
            }
            let selected_runs =
                select_change_runs(&input.repo, &attempt.attempt_id, &input.run_ids)?;
            let run_ids = selected_runs
                .iter()
                .map(|run| run.run_id.clone())
                .collect::<Vec<_>>();
            let related_contract_hashes = related_contract_hashes(&selected_runs);
            let change_id = stable_json_hash(&serde_json::json!({
                "attempt_id": &attempt.attempt_id,
                "diff_hash": &detected.diff_hash,
                "run_ids": &run_ids,
            }))?;
            let change = ChangeEvidence {
                change_id,
                intent_id: attempt.intent_id,
                attempt_id: attempt.attempt_id,
                run_ids,
                base_commit: attempt.base_commit,
                file_paths: detected.file_paths,
                diff_summary: detected.diff_summary,
                symbols: Vec::new(),
                diff_hash: detected.diff_hash,
                related_contract_hashes,
                diagnostics: Vec::new(),
            };
            let object = write_signed_or_cached(
                &input.repo,
                "agent.change",
                serde_json::to_value(&change)?,
                signing.as_ref(),
            )?;
            print_json(&ChangeDetectReport {
                changed: detected.changed,
                diff_bytes: detected.diff_bytes,
                change,
                object,
            })
        }
        ChangeSubcommand::List(args) => {
            let input = change_list_input(args);
            let changes = list_changes(&input.repo)?;
            print_json(&ChangeListReport { changes })
        }
        ChangeSubcommand::Show(args) => {
            let input = change_show_input(args);
            let changes = list_changes(&input.repo)?;
            let Some(entry) = changes
                .into_iter()
                .find(|entry| entry.change.change_id == input.change_id)
            else {
                anyhow::bail!("change {} was not found", input.change_id);
            };
            let report = rickydata_git_git::read_cached_object(&input.repo, &entry.object_id)?;
            print_json(&ChangeShowReport {
                object_id: entry.object_id,
                source: report.source,
                change: entry.change,
            })
        }
    }
}

fn list_changes(repo: &str) -> Result<Vec<ChangeListEntry>> {
    let object_entries = rickydata_git_git::list_ref_backed_objects(repo, Some("agent.change"))?;
    let mut changes = Vec::new();
    for entry in object_entries {
        let report = rickydata_git_git::read_cached_object(repo, &entry.object_id)?;
        let change: ChangeEvidence = serde_json::from_value(report.object.body)?;
        changes.push(ChangeListEntry {
            object_id: entry.object_id,
            ref_name: entry.ref_name,
            git_object_id: entry.git_object_id,
            change,
        });
    }
    changes.sort_by(|left, right| left.change.change_id.cmp(&right.change.change_id));
    Ok(changes)
}

fn run_patch(command: PatchSubcommand) -> Result<()> {
    match command {
        PatchSubcommand::Apply(args) => {
            let input = patch_apply_input(args);
            let patches = list_patches(&input.repo)?;
            let Some(entry) = patches
                .into_iter()
                .find(|entry| entry.patch.patch_id == input.patch_id)
            else {
                anyhow::bail!("patch {} was not found", input.patch_id);
            };
            let attempt = find_attempt(&input.repo, &entry.patch.attempt_id)?;
            if attempt.base_commit != entry.patch.base_commit {
                anyhow::bail!(
                    "attempt {} base commit {} does not match prepared patch base commit {}",
                    attempt.attempt_id,
                    attempt.base_commit,
                    entry.patch.base_commit
                );
            }
            let inspection = rickydata_git_git::inspect_repository(&input.repo)?;
            let head_commit = inspection
                .head_commit
                .context("repository HEAD does not point to a commit")?;
            if !input.allow_base_drift && head_commit != entry.patch.base_commit {
                anyhow::bail!(
                    "repository HEAD {} does not match prepared patch base commit {}; pass --allow-base-drift to override",
                    head_commit,
                    entry.patch.base_commit
                );
            }
            let resolved = resolve_patch_diff(&input.repo, &entry.patch, &attempt)?;
            if resolved.raw_diff.is_empty() {
                anyhow::bail!("patch {} has no diff bytes to apply", entry.patch.patch_id);
            }
            let patch_file = TempPatchFile::new(
                Path::new(&input.repo),
                &entry.patch.patch_id,
                &resolved.raw_diff,
            )?;
            if let Some(idempotency_key) = input.idempotency_key.as_deref()
                && let Some((application, object)) = find_patch_application_by_idempotency(
                    &input.repo,
                    &entry.patch.patch_id,
                    idempotency_key,
                )?
            {
                print_json(&PatchApplyReport {
                    patch_id: entry.patch.patch_id,
                    attempt_id: attempt.attempt_id,
                    base_commit: attempt.base_commit,
                    head_commit,
                    applied: false,
                    diff_hash: resolved.diff_hash,
                    diff_bytes: resolved.diff_bytes,
                    file_count: resolved.file_paths.len(),
                    file_paths: resolved.file_paths,
                    application,
                    object,
                    replayed: true,
                })?;
                return Ok(());
            }
            if !input.allow_dirty {
                ensure_clean_worktree(Path::new(&input.repo))?;
            }
            git_apply_file(Path::new(&input.repo), patch_file.path(), true)?;
            git_apply_file(Path::new(&input.repo), patch_file.path(), false)?;
            let application = PatchApplication {
                patch_id: entry.patch.patch_id.clone(),
                attempt_id: attempt.attempt_id.clone(),
                base_commit: attempt.base_commit.clone(),
                head_commit: head_commit.clone(),
                diff_hash: resolved.diff_hash.clone(),
                diff_bytes: resolved.diff_bytes,
                file_paths: resolved.file_paths.clone(),
                applied_by: input.applied_by,
                reason: input.reason,
                idempotency_key: input.idempotency_key,
                applied_at_ms: now_ms()?,
            };
            let object = rickydata_git_git::write_cached_object(
                &input.repo,
                "agent.patch_application",
                serde_json::to_value(&application)?,
            )?;
            print_json(&PatchApplyReport {
                patch_id: entry.patch.patch_id,
                attempt_id: attempt.attempt_id,
                base_commit: attempt.base_commit,
                head_commit,
                applied: true,
                diff_hash: resolved.diff_hash,
                diff_bytes: resolved.diff_bytes,
                file_count: resolved.file_paths.len(),
                file_paths: resolved.file_paths,
                application,
                object,
                replayed: false,
            })
        }
        PatchSubcommand::Checkout(args) => {
            let input = patch_checkout_input(args);
            patch_checkout(input)
        }
        PatchSubcommand::Export(args) => {
            let input = patch_export_input(args);
            let patches = list_patches(&input.repo)?;
            let Some(entry) = patches
                .into_iter()
                .find(|entry| entry.patch.patch_id == input.patch_id)
            else {
                anyhow::bail!("patch {} was not found", input.patch_id);
            };
            let attempt = find_attempt(&input.repo, &entry.patch.attempt_id)?;
            if attempt.base_commit != entry.patch.base_commit {
                anyhow::bail!(
                    "attempt {} base commit {} does not match prepared patch base commit {}",
                    attempt.attempt_id,
                    attempt.base_commit,
                    entry.patch.base_commit
                );
            }
            let resolved = resolve_patch_diff(&input.repo, &entry.patch, &attempt)?;
            if resolved.raw_diff.is_empty() {
                anyhow::bail!("patch {} has no diff bytes to export", entry.patch.patch_id);
            }
            let output_path = PathBuf::from(&input.output);
            let overwritten = output_path.exists();
            if overwritten && !input.force {
                anyhow::bail!(
                    "output file {} already exists; pass --force to overwrite",
                    output_path.display()
                );
            }
            if let Some(parent) = output_path.parent()
                && !parent.as_os_str().is_empty()
            {
                std::fs::create_dir_all(parent).with_context(|| {
                    format!("failed to create output parent `{}`", parent.display())
                })?;
            }
            std::fs::write(&output_path, &resolved.raw_diff).with_context(|| {
                format!("failed to write patch file `{}`", output_path.display())
            })?;
            print_json(&PatchExportReport {
                patch_id: entry.patch.patch_id,
                attempt_id: attempt.attempt_id,
                base_commit: attempt.base_commit,
                output_path: output_path.display().to_string(),
                diff_hash: resolved.diff_hash,
                diff_bytes: resolved.diff_bytes,
                file_count: resolved.file_paths.len(),
                file_paths: resolved.file_paths,
                overwritten,
            })
        }
        PatchSubcommand::List(args) => {
            let input = patch_list_input(args);
            let patches = list_patches(&input.repo)?;
            print_json(&PatchListReport { patches })
        }
        PatchSubcommand::Prepare(args) => {
            let signing = resolve_signing_key(&args.signing, None)?;
            let input = patch_prepare_input(args);
            let attempt = find_attempt(&input.repo, &input.attempt_id)?;
            let changes = list_changes(&input.repo)?
                .into_iter()
                .map(|entry| entry.change)
                .filter(|change| change.attempt_id == attempt.attempt_id)
                .collect::<Vec<_>>();
            if changes.is_empty() {
                anyhow::bail!(
                    "no change evidence found for attempt {}",
                    attempt.attempt_id
                );
            }

            let change_ids = sorted_dedup(
                changes
                    .iter()
                    .map(|change| change.change_id.clone())
                    .collect(),
            );
            let run_ids = sorted_dedup(
                changes
                    .iter()
                    .flat_map(|change| change.run_ids.iter().cloned())
                    .collect(),
            );
            let file_paths = sorted_dedup(
                changes
                    .iter()
                    .flat_map(|change| change.file_paths.iter().cloned())
                    .collect(),
            );
            let diff_hashes = sorted_dedup(
                changes
                    .iter()
                    .map(|change| change.diff_hash.clone())
                    .collect(),
            );
            let related_contract_hashes = sorted_dedup(
                changes
                    .iter()
                    .flat_map(|change| change.related_contract_hashes.iter().cloned())
                    .collect(),
            );
            let detected = detect_attempt_diff(Path::new(&input.repo), &attempt)?;
            if !changes
                .iter()
                .any(|change| change.diff_hash == detected.diff_hash)
            {
                anyhow::bail!(
                    "current attempt worktree diff hash {} does not match change evidence for attempt {}",
                    detected.diff_hash,
                    attempt.attempt_id
                );
            }
            let patch_id = stable_json_hash(&serde_json::json!({
                "attempt_id": &attempt.attempt_id,
                "change_ids": &change_ids,
            }))?;
            let patch_diff = PatchDiff {
                patch_id: patch_id.clone(),
                attempt_id: attempt.attempt_id.clone(),
                base_commit: attempt.base_commit.clone(),
                diff_hash: detected.diff_hash.clone(),
                diff_bytes: detected.diff_bytes,
                file_paths: detected.file_paths,
                encoding: "hex".to_string(),
                encoded_diff: encode_hex(&detected.raw_diff),
            };
            let diff_object = write_signed_or_cached(
                &input.repo,
                "agent.patch_diff",
                serde_json::to_value(&patch_diff)?,
                signing.as_ref(),
            )?;
            let patch = PreparedPatch {
                patch_id,
                intent_id: attempt.intent_id,
                attempt_id: attempt.attempt_id,
                base_commit: attempt.base_commit,
                change_ids,
                run_ids,
                file_paths,
                diff_hashes,
                diff_object_ids: vec![diff_object.object_id],
                related_contract_hashes,
                diagnostics: Vec::new(),
            };
            let change_count = patch.change_ids.len();
            let file_count = patch.file_paths.len();
            let object = write_signed_or_cached(
                &input.repo,
                "agent.patch",
                serde_json::to_value(&patch)?,
                signing.as_ref(),
            )?;
            print_json(&PatchPrepareReport {
                patch,
                object,
                change_count,
                file_count,
            })
        }
        PatchSubcommand::Retire(args) => {
            let input = patch_retire_input(args);
            let patches = list_patches(&input.repo)?;
            if !patches
                .iter()
                .any(|entry| entry.patch.patch_id == input.patch_id)
            {
                anyhow::bail!("patch {} was not found", input.patch_id);
            }
            if input.reason.trim().is_empty() {
                anyhow::bail!("patch retirement reason must be non-empty");
            }
            let retirement = PatchRetirement {
                patch_id: input.patch_id,
                reason: input.reason,
                retired_by: input.retired_by,
                idempotency_key: input.idempotency_key,
                retired_at_ms: now_ms()?,
            };
            let object = rickydata_git_git::write_cached_object(
                &input.repo,
                "agent.patch_retirement",
                serde_json::to_value(&retirement)?,
            )?;
            print_json(&PatchRetireReport { retirement, object })
        }
        PatchSubcommand::ReviewQueue(args) => {
            let input = patch_list_input(args);
            print_json(&build_patch_review_queue(&input.repo)?)
        }
        PatchSubcommand::Show(args) => {
            let input = patch_show_input(args);
            let patches = list_patches(&input.repo)?;
            let Some(entry) = patches
                .into_iter()
                .find(|entry| entry.patch.patch_id == input.patch_id)
            else {
                anyhow::bail!("patch {} was not found", input.patch_id);
            };
            let report = rickydata_git_git::read_cached_object(&input.repo, &entry.object_id)?;
            print_json(&PatchShowReport {
                object_id: entry.object_id,
                source: report.source,
                patch: entry.patch,
            })
        }
    }
}

fn list_patches(repo: &str) -> Result<Vec<PatchListEntry>> {
    let object_entries = rickydata_git_git::list_ref_backed_objects(repo, Some("agent.patch"))?;
    let mut patches = Vec::new();
    for entry in object_entries {
        let report = rickydata_git_git::read_cached_object(repo, &entry.object_id)?;
        let patch: PreparedPatch = serde_json::from_value(report.object.body)?;
        patches.push(PatchListEntry {
            object_id: entry.object_id,
            ref_name: entry.ref_name,
            git_object_id: entry.git_object_id,
            patch,
        });
    }
    patches.sort_by(|left, right| left.patch.patch_id.cmp(&right.patch.patch_id));
    Ok(patches)
}

fn retired_patch_ids(repo: &str) -> Result<BTreeSet<String>> {
    let object_entries =
        rickydata_git_git::list_ref_backed_objects(repo, Some("agent.patch_retirement"))?;
    let mut patch_ids = BTreeSet::new();
    for entry in object_entries {
        let report = rickydata_git_git::read_cached_object(repo, &entry.object_id)?;
        let retirement: PatchRetirement = serde_json::from_value(report.object.body)?;
        patch_ids.insert(retirement.patch_id);
    }
    Ok(patch_ids)
}

fn find_patch_application_by_idempotency(
    repo: &str,
    patch_id: &str,
    idempotency_key: &str,
) -> Result<Option<(PatchApplication, ObjectWriteReport)>> {
    let object_entries =
        rickydata_git_git::list_ref_backed_objects(repo, Some("agent.patch_application"))?;
    for entry in object_entries {
        let report = rickydata_git_git::read_cached_object(repo, &entry.object_id)?;
        let application: PatchApplication = serde_json::from_value(report.object.body)?;
        if application.patch_id == patch_id
            && application.idempotency_key.as_deref() == Some(idempotency_key)
        {
            let object = ObjectWriteReport {
                status: ObjectWriteStatus::AlreadyExists,
                object_id: entry.object_id,
                body_hash: entry.body_hash,
                kind: entry.kind,
                schema_version: report.object.schema_version,
                cache_path: report.cache_path,
                ref_name: entry.ref_name,
                git_object_id: entry.git_object_id,
                bytes_written: 0,
            };
            return Ok(Some((application, object)));
        }
    }
    Ok(None)
}

#[derive(Debug, Serialize)]
struct PatchReviewQueueReport {
    status: String,
    patch_count: usize,
    ready_count: usize,
    patches: Vec<PatchReviewQueueEntry>,
}

#[derive(Debug, Serialize)]
struct PatchReviewQueueEntry {
    patch_id: String,
    intent_id: String,
    attempt_id: String,
    attempt_status: Option<AttemptStatus>,
    base_commit: String,
    file_count: usize,
    file_paths: Vec<String>,
    run_count: usize,
    change_count: usize,
    apply_ready: bool,
    diagnostics: Vec<String>,
}

fn build_patch_review_queue(repo: &str) -> Result<PatchReviewQueueReport> {
    let retired = retired_patch_ids(repo)?;
    let inspection = rickydata_git_git::inspect_repository(repo)?;
    let head_commit = inspection.head_commit;
    let clean = inspection.dirty == Some(false);
    let mut entries = Vec::new();
    for entry in list_patches(repo)? {
        let patch = entry.patch;
        if retired.contains(&patch.patch_id) {
            continue;
        }
        let mut diagnostics = Vec::new();
        let attempt = match find_attempt(repo, &patch.attempt_id) {
            Ok(attempt) => Some(attempt),
            Err(error) => {
                diagnostics.push(format!("attempt is not readable: {error}"));
                None
            }
        };
        let attempt_status = match effective_attempt_status(repo, &patch.attempt_id) {
            Ok((status, _)) => Some(status),
            Err(error) => {
                diagnostics.push(format!("attempt status is not readable: {error}"));
                None
            }
        };
        if head_commit.as_deref() != Some(patch.base_commit.as_str()) {
            diagnostics.push("repository HEAD does not match patch base commit".to_string());
        }
        if !clean {
            diagnostics.push("repository worktree is dirty".to_string());
        }
        if let Some(attempt) = attempt.as_ref()
            && let Err(error) = resolve_patch_diff(repo, &patch, attempt)
        {
            diagnostics.push(format!("patch diff evidence is invalid: {error}"));
        }
        let apply_ready = diagnostics.is_empty();
        entries.push(PatchReviewQueueEntry {
            patch_id: patch.patch_id,
            intent_id: patch.intent_id,
            attempt_id: patch.attempt_id,
            attempt_status,
            base_commit: patch.base_commit,
            file_count: patch.file_paths.len(),
            file_paths: patch.file_paths,
            run_count: patch.run_ids.len(),
            change_count: patch.change_ids.len(),
            apply_ready,
            diagnostics,
        });
    }
    let ready_count = entries.iter().filter(|entry| entry.apply_ready).count();
    Ok(PatchReviewQueueReport {
        status: "ok".to_string(),
        patch_count: entries.len(),
        ready_count,
        patches: entries,
    })
}

fn patch_checkout(input: PatchCheckoutInput) -> Result<()> {
    let patches = list_patches(&input.repo)?;
    let Some(entry) = patches
        .into_iter()
        .find(|entry| entry.patch.patch_id == input.patch_id)
    else {
        anyhow::bail!("patch {} was not found", input.patch_id);
    };
    let attempt = find_attempt(&input.repo, &entry.patch.attempt_id)?;
    if attempt.base_commit != entry.patch.base_commit {
        anyhow::bail!(
            "attempt {} base commit {} does not match prepared patch base commit {}",
            attempt.attempt_id,
            attempt.base_commit,
            entry.patch.base_commit
        );
    }
    let inspection = rickydata_git_git::inspect_repository(&input.repo)?;
    let head_commit = inspection
        .head_commit
        .context("repository HEAD does not point to a commit")?;
    if !input.allow_base_drift && head_commit != entry.patch.base_commit {
        anyhow::bail!(
            "repository HEAD {} does not match prepared patch base commit {}; pass --allow-base-drift to override",
            head_commit,
            entry.patch.base_commit
        );
    }

    let resolved = resolve_patch_diff(&input.repo, &entry.patch, &attempt)?;
    if resolved.raw_diff.is_empty() {
        anyhow::bail!(
            "patch {} has no diff bytes to check out",
            entry.patch.patch_id
        );
    }
    let checkout_path = review_checkout_path(&input.repo, &entry.patch.patch_id, input.path)?;
    let replaced = prepare_review_checkout_path(
        Path::new(&input.repo),
        &checkout_path,
        &entry.patch.patch_id,
        input.force,
    )?;
    add_review_worktree(
        Path::new(&input.repo),
        &checkout_path,
        &entry.patch.base_commit,
    )?;
    let patch_file = TempPatchFile::new(
        Path::new(&input.repo),
        &entry.patch.patch_id,
        &resolved.raw_diff,
    )?;
    if let Err(error) = git_apply_file(&checkout_path, patch_file.path(), true)
        .and_then(|_| git_apply_file(&checkout_path, patch_file.path(), false))
        .and_then(|_| {
            write_review_checkout_marker(
                &checkout_path,
                ReviewCheckoutMarker {
                    patch_id: entry.patch.patch_id.clone(),
                    attempt_id: attempt.attempt_id.clone(),
                    base_commit: attempt.base_commit.clone(),
                    diff_hash: resolved.diff_hash.clone(),
                    created_at_ms: now_ms()?,
                },
            )
        })
    {
        let _ = remove_review_worktree(Path::new(&input.repo), &checkout_path);
        return Err(error);
    }

    print_json(&PatchCheckoutReport {
        patch_id: entry.patch.patch_id,
        attempt_id: attempt.attempt_id,
        base_commit: attempt.base_commit,
        head_commit,
        checkout_path: checkout_path.display().to_string(),
        applied: true,
        diff_hash: resolved.diff_hash,
        diff_bytes: resolved.diff_bytes,
        file_count: resolved.file_paths.len(),
        file_paths: resolved.file_paths,
        replaced,
    })
}

struct ResolvedPatchDiff {
    diff_hash: String,
    diff_bytes: u64,
    file_paths: Vec<String>,
    raw_diff: Vec<u8>,
}

fn resolve_patch_diff(
    repo: &str,
    patch: &PreparedPatch,
    attempt: &AgentAttempt,
) -> Result<ResolvedPatchDiff> {
    if let Some(diff_object_id) = patch.diff_object_ids.first() {
        let report = rickydata_git_git::read_cached_object(repo, diff_object_id)?;
        if report.object.kind != "agent.patch_diff" {
            anyhow::bail!(
                "object {} is kind `{}`, expected `agent.patch_diff`",
                report.object_id,
                report.object.kind
            );
        }
        let patch_diff: PatchDiff = serde_json::from_value(report.object.body)?;
        if patch_diff.patch_id != patch.patch_id {
            anyhow::bail!(
                "patch diff object {} belongs to patch {}, expected {}",
                diff_object_id,
                patch_diff.patch_id,
                patch.patch_id
            );
        }
        if patch_diff.attempt_id != patch.attempt_id {
            anyhow::bail!(
                "patch diff object {} belongs to attempt {}, expected {}",
                diff_object_id,
                patch_diff.attempt_id,
                patch.attempt_id
            );
        }
        if patch_diff.base_commit != patch.base_commit {
            anyhow::bail!(
                "patch diff object {} has base commit {}, expected {}",
                diff_object_id,
                patch_diff.base_commit,
                patch.base_commit
            );
        }
        if patch_diff.encoding != "hex" {
            anyhow::bail!(
                "patch diff object {} uses unsupported encoding `{}`",
                diff_object_id,
                patch_diff.encoding
            );
        }
        if !patch.diff_hashes.contains(&patch_diff.diff_hash) {
            anyhow::bail!(
                "patch diff object {} hash {} is not listed in prepared patch hashes {:?}",
                diff_object_id,
                patch_diff.diff_hash,
                patch.diff_hashes
            );
        }
        let raw_diff = decode_hex(&patch_diff.encoded_diff)?;
        let computed_diff_hash = bytes_hash(&raw_diff);
        if computed_diff_hash != patch_diff.diff_hash {
            anyhow::bail!(
                "patch diff object {} decoded hash {} does not match recorded {}",
                diff_object_id,
                computed_diff_hash,
                patch_diff.diff_hash
            );
        }
        if raw_diff.len() as u64 != patch_diff.diff_bytes {
            anyhow::bail!(
                "patch diff object {} decoded byte length {} does not match recorded {}",
                diff_object_id,
                raw_diff.len(),
                patch_diff.diff_bytes
            );
        }
        return Ok(ResolvedPatchDiff {
            diff_hash: patch_diff.diff_hash,
            diff_bytes: patch_diff.diff_bytes,
            file_paths: patch_diff.file_paths,
            raw_diff,
        });
    }

    let detected = detect_attempt_diff(Path::new(repo), attempt)?;
    if !patch.diff_hashes.contains(&detected.diff_hash) {
        anyhow::bail!(
            "current attempt worktree diff hash {} does not match prepared patch hashes {:?}",
            detected.diff_hash,
            patch.diff_hashes
        );
    }
    Ok(ResolvedPatchDiff {
        diff_hash: detected.diff_hash,
        diff_bytes: detected.diff_bytes,
        file_paths: detected.file_paths,
        raw_diff: detected.raw_diff,
    })
}

const RICKYDATA_REFSPEC: &str = "refs/rickydata/*:refs/rickydata/*";

fn repo_status(input: RepoStatusInput) -> Result<()> {
    let inspection = rickydata_git_git::inspect_repository(&input.repo)?;
    let data_locations = repo_data_locations(&inspection);
    let store = repo_store_status(&inspection);
    let verify = if store.initialized {
        Some(build_sync_verify(SyncVerifyInput {
            repo: input.repo.clone(),
            json: true,
        })?)
    } else {
        None
    };
    let sync = if store.initialized {
        match input.remote {
            Some(remote) => Some(build_sync_status(SyncInput {
                repo: input.repo,
                remote,
                json: true,
            })?),
            None => None,
        }
    } else {
        None
    };
    let status = repo_status_value(&inspection, &store, verify.as_ref(), sync.as_ref());

    print_json(&RepoStatusReport {
        status,
        inspection,
        data_locations,
        store,
        verify,
        sync,
    })
}

fn repo_status_value(
    inspection: &rickydata_git_git::RepoInspection,
    store: &RepoStoreStatus,
    verify: Option<&SyncVerifyReport>,
    sync: Option<&SyncStatusReport>,
) -> String {
    if !inspection.is_git_repo {
        return "not_git".to_string();
    }
    if !store.initialized {
        return "not_initialized".to_string();
    }
    if verify.is_some_and(|report| report.status != "ok") {
        return "failed".to_string();
    }
    if sync.is_some_and(|report| {
        !report.local_only_refs.is_empty()
            || !report.remote_only_refs.is_empty()
            || !report.divergent_refs.is_empty()
    }) {
        return "out_of_sync".to_string();
    }
    "ok".to_string()
}

fn repo_data_locations(inspection: &rickydata_git_git::RepoInspection) -> RepoDataLocations {
    let Some(git_dir) = inspection.git_dir.as_ref() else {
        return RepoDataLocations {
            metadata_dir: None,
            object_cache_dir: None,
            bundle_dir: None,
            temp_dir: None,
            attempt_worktrees_dir: None,
            review_worktrees_dir: None,
            refs_dir: None,
            object_ref_prefix: "refs/rickydata/objects/sha256/".to_string(),
            refspec: RICKYDATA_REFSPEC.to_string(),
        };
    };

    let metadata_dir = git_dir.join("rickydata");
    RepoDataLocations {
        metadata_dir: Some(path_display(&metadata_dir)),
        object_cache_dir: Some(path_display(
            &metadata_dir.join("cache").join("objects").join("sha256"),
        )),
        bundle_dir: Some(path_display(&metadata_dir.join("cache").join("bundles"))),
        temp_dir: Some(path_display(&metadata_dir.join("tmp"))),
        attempt_worktrees_dir: Some(path_display(&metadata_dir.join("worktrees"))),
        review_worktrees_dir: Some(path_display(&metadata_dir.join("reviews"))),
        refs_dir: Some(path_display(&git_dir.join("refs").join("rickydata"))),
        object_ref_prefix: "refs/rickydata/objects/sha256/".to_string(),
        refspec: RICKYDATA_REFSPEC.to_string(),
    }
}

fn repo_store_status(inspection: &rickydata_git_git::RepoInspection) -> RepoStoreStatus {
    let Some(git_dir) = inspection.git_dir.as_ref() else {
        return RepoStoreStatus {
            initialized: false,
            store_version: None,
            version_path: None,
            diagnostics: vec!["path is not inside a Git repository".to_string()],
        };
    };

    let metadata_dir = git_dir.join("rickydata");
    let version_path = metadata_dir.join("VERSION");
    let version_path_string = path_display(&version_path);
    if !version_path.exists() {
        return RepoStoreStatus {
            initialized: false,
            store_version: None,
            version_path: Some(version_path_string),
            diagnostics: vec![
                "Rickydata store is not initialized; run `rickygit init --repo <path> --json`"
                    .to_string(),
            ],
        };
    }

    match std::fs::read_to_string(&version_path) {
        Ok(contents) => {
            let found = contents.trim().to_string();
            let mut diagnostics = Vec::new();
            let version_matches = found == rickydata_git_git::RICKYDATA_STORE_VERSION;
            if !version_matches {
                diagnostics.push(format!(
                    "unsupported store version `{found}`; expected `{}`",
                    rickydata_git_git::RICKYDATA_STORE_VERSION
                ));
            }
            let temp_dir = metadata_dir.join("tmp");
            let temp_dir_exists = temp_dir.is_dir();
            if !temp_dir_exists {
                diagnostics.push(format!(
                    "Rickydata temp directory is missing at {}",
                    temp_dir.display()
                ));
            }
            RepoStoreStatus {
                initialized: version_matches && temp_dir_exists,
                store_version: Some(found),
                version_path: Some(version_path_string),
                diagnostics,
            }
        }
        Err(error) => RepoStoreStatus {
            initialized: false,
            store_version: None,
            version_path: Some(version_path_string),
            diagnostics: vec![format!("failed to read store version: {error}")],
        },
    }
}

fn path_display(path: &Path) -> String {
    path.display().to_string()
}

fn run_sync(command: SyncSubcommand) -> Result<()> {
    match command {
        SyncSubcommand::Pull(args) => {
            let signing = resolve_signing_key(&args.signing, None)?;
            let input = sync_input(args);
            sync_refs("pull", "fetch", input, signing.as_ref(), Vec::new())
        }
        SyncSubcommand::Push(args) => {
            let signing = resolve_signing_key(&args.signing, None)?;
            let expectations = match signing.as_ref() {
                Some((key, label)) => {
                    build_signed_ref_expectations(&args.repo, key, label.as_deref())?
                }
                None => Vec::new(),
            };
            let input = sync_input(args);
            sync_refs("push", "push", input, signing.as_ref(), expectations)
        }
        SyncSubcommand::Status(args) => {
            let input = sync_input(args);
            sync_status(input)
        }
        SyncSubcommand::Verify(args) => {
            let input = sync_verify_input(args);
            sync_verify(input)
        }
    }
}

fn build_signed_ref_expectations(
    repo: &Path,
    signing_key: &SigningKey,
    signer_label: Option<&str>,
) -> Result<Vec<rickydata_git_core::SignedRefExpectation>> {
    let refs = local_rickydata_refs(repo)?;
    let mut expectations = Vec::with_capacity(refs.len());
    for (ref_name, oid) in refs {
        // sync push attests "after this push, ref X should still point to oid Y".
        // We model that as expected_previous_oid == new_oid == current oid: a no-op-on-match
        // expectation that the relay/server can verify before accepting subsequent updates.
        let expectation = rickydata_git_core::sign_ref_expectation(
            &ref_name,
            Some(&oid),
            &oid,
            signing_key,
            signer_label.map(str::to_string),
        )?;
        expectations.push(expectation);
    }
    Ok(expectations)
}

#[derive(Debug, Serialize)]
struct RelayPullCliReport {
    status: String,
    repo_id: String,
    object_count: usize,
    remaining_object_count: usize,
    written_object_count: usize,
    duplicate_object_count: usize,
    object_ids: Vec<String>,
}

#[derive(Debug, Serialize)]
struct RelayStatusCliReport {
    status: String,
    repo_id: String,
    local_object_count: usize,
    relay_object_count: usize,
    relay_object_ids_hash: String,
}

#[derive(Debug, Serialize)]
struct ProofReport {
    status: String,
    repo: String,
    repo_id: Option<String>,
    local: SyncVerifyReport,
    git_remote: Option<SyncStatusReport>,
    relay: Option<RelayStatusCliReport>,
    kfdb: Option<KfdbProofReport>,
    tee_reachable: Option<bool>,
    diagnostics: Vec<String>,
    signature_summary: ProofSignatureSummary,
}

#[derive(Debug, Serialize)]
struct ProofSignatureSummary {
    object_count: usize,
    signed_object_count: usize,
    valid_signature_count: usize,
}

#[derive(Debug, Serialize)]
struct KfdbProofReport {
    status: String,
    url: String,
    repo_id: String,
    object_mirror_count: i64,
    prepared_patch_count: i64,
    expected_object_count: usize,
    expected_patch_count: usize,
}

fn run_proof(args: ProofArgs) -> Result<()> {
    let repo = args.repo.display().to_string();
    let repo_id = match (args.relay_url.as_ref(), args.kfdb_url.as_ref()) {
        (Some(_), _) | (_, Some(_)) => Some(relay_repo_id(&args.repo, args.repo_id)?),
        _ => args.repo_id,
    };
    let local = build_sync_verify(SyncVerifyInput {
        repo: repo.clone(),
        json: true,
    })?;
    let git_remote = match args.remote {
        Some(remote) => Some(build_sync_status(SyncInput {
            repo: repo.clone(),
            remote,
            json: true,
        })?),
        None => None,
    };
    let relay = match (args.relay_url, repo_id.as_ref()) {
        (Some(url), Some(repo_id)) => {
            // Proof path resolves the relay token from RICKYDATA_RELAY_AUTH_TOKEN.
            Some(build_relay_status(&args.repo, &url, repo_id, None)?)
        }
        _ => None,
    };
    let kfdb = match (args.kfdb_url, repo_id.as_ref()) {
        (Some(url), Some(repo_id)) => Some(build_kfdb_proof(
            &url,
            repo_id,
            args.kfdb_bearer_token,
            args.kfdb_bearer_token_env,
            local.object_count,
            local.patch_count,
        )?),
        _ => None,
    };
    let tee_reachable =
        resolve_tee_url(args.tee_url.as_deref()).map(|url| tee_signer_reachable(&url));
    let mut diagnostics = Vec::new();
    if local.status != "ok" {
        diagnostics.push("local sync verification failed".to_string());
    }
    if let Some(sync) = git_remote.as_ref()
        && (!sync.local_only_refs.is_empty()
            || !sync.remote_only_refs.is_empty()
            || !sync.divergent_refs.is_empty())
    {
        diagnostics.push("Git remote refs/rickydata parity failed".to_string());
    }
    if let Some(relay) = relay.as_ref()
        && relay.status != "ok"
    {
        diagnostics.push(format!(
            "relay object count mismatch: local={}, relay={}",
            relay.local_object_count, relay.relay_object_count
        ));
    }
    if let Some(kfdb) = kfdb.as_ref()
        && kfdb.status != "ok"
    {
        diagnostics.push(format!(
            "KFDB projection mismatch: mirror={} expected {}, patches={} expected {}",
            kfdb.object_mirror_count,
            kfdb.expected_object_count,
            kfdb.prepared_patch_count,
            kfdb.expected_patch_count
        ));
    }
    let status = if diagnostics.is_empty() {
        "ok"
    } else {
        "failed"
    };
    let signature_summary = ProofSignatureSummary {
        object_count: local.object_count,
        signed_object_count: local.signed_object_count,
        valid_signature_count: local.valid_signature_count,
    };
    print_json(&ProofReport {
        status: status.to_string(),
        repo,
        repo_id,
        local,
        git_remote,
        relay,
        kfdb,
        tee_reachable,
        diagnostics,
        signature_summary,
    })
}

fn run_relay(command: RelaySubcommand) -> Result<()> {
    match command {
        RelaySubcommand::Push(args) => {
            let repo = args.repo.display().to_string();
            let repo_id = relay_repo_id(&args.repo, args.repo_id)?;
            let object_entries = rickydata_git_git::list_ref_backed_objects(&repo, None)?;
            let mut objects = Vec::new();
            for entry in &object_entries {
                let report = rickydata_git_git::read_cached_object(&repo, &entry.object_id)?;
                objects.push(report.object);
            }
            let object_ids = objects
                .iter()
                .map(|object| object.object_id.clone())
                .collect::<Vec<_>>();
            let idempotency_key = match args.idempotency_key {
                Some(value) => value,
                None => stable_json_hash(&serde_json::json!({
                    "repo_id": repo_id,
                    "object_ids": object_ids,
                }))?,
            };
            let chunk_size = args.chunk_size.max(1);
            let chunk_count = objects.len().div_ceil(chunk_size).max(1);
            let client = relay_client()?;
            let mut accepted_object_count = 0;
            let mut duplicate_object_count = 0;
            let mut pushed_object_ids = Vec::new();
            for (index, chunk) in objects.chunks(chunk_size).enumerate() {
                let chunk_idempotency_key = if chunk_count == 1 {
                    idempotency_key.clone()
                } else {
                    format!(
                        "{idempotency_key}:chunk:{:04}-of-{chunk_count:04}",
                        index + 1
                    )
                };
                let request = BundlePushRequest {
                    repo_id: repo_id.clone(),
                    idempotency_key: chunk_idempotency_key,
                    objects: chunk.to_vec(),
                };
                let report: BundlePushReport = with_relay_auth(
                    client.post(relay_url(&args.url, &repo_id, "bundles/push")?),
                    args.auth_token.as_deref(),
                )
                .json(&request)
                .send()
                .context("failed to push bundle to relay")?
                .error_for_status()
                .context("relay rejected bundle push")?
                .json()
                .context("failed to parse relay bundle push response")?;
                accepted_object_count += report.accepted_object_count;
                duplicate_object_count += report.duplicate_object_count;
                pushed_object_ids.extend(report.object_ids);
            }
            let bundle_hash = stable_json_hash(&serde_json::json!({ "object_ids": object_ids }))?;
            print_json(&BundlePushReport {
                status: "ok".to_string(),
                repo_id,
                idempotency_key,
                accepted_object_count,
                duplicate_object_count,
                object_ids: pushed_object_ids,
                bundle_hash,
            })
        }
        RelaySubcommand::Pull(args) => {
            let repo = args.repo.display().to_string();
            let repo_id = relay_repo_id(&args.repo, args.repo_id)?;
            let known_object_ids = rickydata_git_git::list_ref_backed_objects(&repo, None)?
                .into_iter()
                .map(|entry| entry.object_id)
                .collect::<Vec<_>>();
            let request = BundlePullRequest {
                repo_id: repo_id.clone(),
                known_object_ids,
                limit: args.limit,
            };
            let report: BundlePullReport = with_relay_auth(
                relay_client()?.post(relay_url(&args.url, &repo_id, "bundles/pull")?),
                args.auth_token.as_deref(),
            )
            .json(&request)
            .send()
            .context("failed to pull bundle from relay")?
            .error_for_status()
            .context("relay rejected bundle pull")?
            .json()
            .context("failed to parse relay bundle pull response")?;
            let mut written_object_count = 0;
            let mut duplicate_object_count = 0;
            let mut object_ids = Vec::new();
            for object in &report.objects {
                object_ids.push(object.object_id.clone());
                let write = rickydata_git_git::write_canonical_object(&repo, object)?;
                match write.status {
                    rickydata_git_git::ObjectWriteStatus::Written => written_object_count += 1,
                    rickydata_git_git::ObjectWriteStatus::AlreadyExists => {
                        duplicate_object_count += 1
                    }
                }
            }
            print_json(&RelayPullCliReport {
                status: report.status,
                repo_id: report.repo_id,
                object_count: report.object_count,
                remaining_object_count: report.remaining_object_count,
                written_object_count,
                duplicate_object_count,
                object_ids,
            })
        }
        RelaySubcommand::Status(args) => {
            let repo_id = relay_repo_id(&args.repo, args.repo_id)?;
            print_json(&build_relay_status(
                &args.repo,
                &args.url,
                &repo_id,
                args.auth_token.as_deref(),
            )?)
        }
    }
}

#[derive(Debug, Clone, Serialize)]
struct GraphNode {
    id: String,
    label: String,
    source_object_id: Option<String>,
    properties: BTreeMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, Serialize)]
struct GraphEdge {
    id: String,
    from: String,
    to: String,
    edge_type: String,
    properties: BTreeMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, Serialize)]
struct GraphScanReport {
    status: String,
    schema_version: String,
    repo: String,
    repo_id: String,
    commit: Option<String>,
    include_code_structure: bool,
    node_count: usize,
    edge_count: usize,
    graph_hash: String,
    nodes: Vec<GraphNode>,
    edges: Vec<GraphEdge>,
}

#[derive(Debug, Serialize)]
struct ImpactReport {
    status: String,
    repo: String,
    attempt_id: Option<String>,
    base: Option<String>,
    head: Option<String>,
    changed_files: Vec<String>,
    affected_entities: Vec<serde_json::Value>,
    suggested_tests: Vec<String>,
    suggested_proofs: Vec<String>,
    risk_class: String,
    summary: String,
}

#[derive(Debug, Serialize)]
struct GraphContextReport {
    status: String,
    repo: String,
    query: Option<String>,
    path: Option<String>,
    attempt_id: Option<String>,
    limit: usize,
    relevant_files: Vec<String>,
    relevant_entities: Vec<serde_json::Value>,
    related_work: Vec<serde_json::Value>,
    recent_runs: Vec<serde_json::Value>,
    command_suggestions: Vec<String>,
    proof_suggestions: Vec<String>,
}

#[derive(Debug, Serialize)]
struct ProjectKfdbReport {
    status: String,
    repo: String,
    kfdb_url: Option<String>,
    scope: Option<String>,
    dry_run: bool,
    include_code_structure: bool,
    nodes_written: usize,
    edges_written: usize,
    skipped_unchanged_count: usize,
    batch_size: usize,
    batches_written: usize,
    projection_id: String,
    projection_hash: String,
    response_status: Option<u16>,
}

fn run_graph(command: GraphSubcommand) -> Result<()> {
    match command {
        GraphSubcommand::Scan(args) => {
            let report = build_graph_scan(&args.repo, args.commit, args.include_code_structure)?;
            print_json(&report)
        }
    }
}

fn run_impact(args: ImpactArgs) -> Result<()> {
    let repo = args.repo.display().to_string();
    let graph = build_graph_scan(&args.repo, args.head.clone(), false)?;
    let mut changed_files: BTreeSet<String> = args.changed_file.into_iter().collect();
    if let Some(attempt_id) = args.attempt_id.as_ref() {
        for entry in list_changes(&repo)? {
            if &entry.change.attempt_id == attempt_id {
                changed_files.extend(entry.change.file_paths);
            }
        }
        for entry in list_patches(&repo)? {
            if &entry.patch.attempt_id == attempt_id {
                changed_files.extend(entry.patch.file_paths);
            }
        }
        if changed_files.is_empty()
            && let Ok(attempt) = find_attempt(&repo, attempt_id)
        {
            let worktree = attempt_worktree_path(Path::new(&repo), &attempt.attempt_id)?;
            if worktree.is_dir() {
                changed_files.extend(detect_attempt_diff(Path::new(&repo), &attempt)?.file_paths);
            }
        }
    }
    if changed_files.is_empty()
        && let (Some(base), Some(head)) = (args.base.as_ref(), args.head.as_ref())
    {
        let output = StdCommand::new("git")
            .arg("-C")
            .arg(&args.repo)
            .arg("diff")
            .arg("--name-only")
            .arg(base)
            .arg(head)
            .output()
            .context("failed to run git diff for impact analysis")?;
        if output.status.success() {
            for line in String::from_utf8_lossy(&output.stdout).lines() {
                if !line.trim().is_empty() {
                    changed_files.insert(line.trim().to_string());
                }
            }
        }
    }
    let changed_files = changed_files.into_iter().collect::<Vec<_>>();
    let mut affected_entities = Vec::new();
    for node in &graph.nodes {
        let node_path = node.properties.get("path").and_then(|v| v.as_str());
        if node_path.is_some_and(|path| changed_files.iter().any(|changed| changed == path)) {
            affected_entities.push(serde_json::to_value(node)?);
        }
    }
    for edge in &graph.edges {
        if edge.edge_type == "TOUCHES" && affected_entities.len() < 50 {
            affected_entities.push(serde_json::json!({"edge": edge}));
        }
    }
    let risk_class = classify_graph_risk(&changed_files);
    let suggested_tests = suggest_tests_for_files(&changed_files);
    let suggested_proofs = vec![
        "rickygit sync verify --repo <repo> --json".to_string(),
        "rickygit proof --repo <repo> --json".to_string(),
    ];
    let summary = format!(
        "{} changed file(s), {} affected graph item(s), risk={}",
        changed_files.len(),
        affected_entities.len(),
        risk_class
    );
    print_json(&ImpactReport {
        status: "ok".to_string(),
        repo,
        attempt_id: args.attempt_id,
        base: args.base,
        head: args.head,
        changed_files,
        affected_entities,
        suggested_tests,
        suggested_proofs,
        risk_class,
        summary,
    })
}

fn run_context(args: ContextArgs) -> Result<()> {
    let repo = args.repo.display().to_string();
    let graph = build_graph_scan(&args.repo, None, false)?;
    let query = args.query.clone().unwrap_or_default().to_lowercase();
    let path_filter = args.path.clone();
    let attempt_filter = args.attempt_id.clone();
    let mut relevant_files = BTreeSet::new();
    let mut relevant_entities = Vec::new();
    let mut related_work = Vec::new();
    for node in &graph.nodes {
        let serialized = serde_json::to_string(node)?.to_lowercase();
        let matches_query = query.is_empty() || serialized.contains(&query);
        let matches_path = path_filter
            .as_ref()
            .is_none_or(|path| serialized.contains(&path.to_lowercase()));
        let matches_attempt = attempt_filter
            .as_ref()
            .is_none_or(|attempt| serialized.contains(attempt));
        if matches_query && matches_path && matches_attempt {
            if let Some(path) = node.properties.get("path").and_then(|v| v.as_str()) {
                relevant_files.insert(path.to_string());
            }
            if relevant_entities.len() < args.limit {
                relevant_entities.push(serde_json::to_value(node)?);
            }
            if matches!(
                node.label.as_str(),
                "RickydataWorkIntent" | "RickydataAttempt" | "RickydataPatch" | "RickydataProof"
            ) && related_work.len() < args.limit
            {
                related_work.push(serde_json::to_value(node)?);
            }
        }
    }
    let mut recent_runs = Vec::new();
    for run in list_runs(&repo)?.into_iter().rev().take(args.limit) {
        recent_runs.push(serde_json::to_value(run)?);
    }
    print_json(&GraphContextReport {
        status: "ok".to_string(),
        repo,
        query: args.query,
        path: args.path,
        attempt_id: args.attempt_id,
        limit: args.limit,
        relevant_files: relevant_files.into_iter().take(args.limit).collect(),
        relevant_entities,
        related_work,
        recent_runs,
        command_suggestions: vec![
            "rickygit graph scan --repo <repo> --json".to_string(),
            "rickygit impact --repo <repo> --attempt-id <attempt> --json".to_string(),
        ],
        proof_suggestions: vec!["rickygit proof --repo <repo> --json".to_string()],
    })
}

fn run_project_kfdb(args: ProjectKfdbArgs) -> Result<()> {
    let graph = build_graph_scan(&args.repo, None, args.include_code_structure)?;
    let projection_hash = graph.graph_hash.clone();
    let projection_id = graph_id("kfdbProjection", &[&graph.repo_id, &projection_hash]);
    let operations = graph_kfdb_operations(&graph);
    let batch_size = args.batch_size.max(1);
    let mut response_status = None;
    let mut batches_written = 0usize;
    if !args.dry_run {
        let private_auth = if args.allow_public_kfdb {
            None
        } else {
            Some(project_kfdb_private_auth(&args)?)
        };
        let kfdb_url = args
            .kfdb_url
            .clone()
            .or_else(|| std::env::var("RICKYDATA_GIT_KFDB_URL").ok())
            .context("--kfdb-url or RICKYDATA_GIT_KFDB_URL is required unless --dry-run is set")?;
        let client = relay_client()?;
        let write_url = format!("{}/api/v1/write", kfdb_url.trim_end_matches('/'));
        let bearer_token = std::env::var(&args.api_key_env).ok();
        for chunk in operations.chunks(batch_size) {
            let payload = graph_kfdb_payload(chunk, args.scope.clone());
            let mut request = client.post(&write_url).json(&payload);
            if let Some(token) = bearer_token.as_ref() {
                request = request.bearer_auth(token);
            }
            if let Some(private_auth) = private_auth.as_ref() {
                request = request
                    .header("x-derive-session-id", &private_auth.derive_session_id)
                    .header("x-derive-key", &private_auth.derive_key);
                if let Some(wallet_address) = private_auth.wallet_address.as_ref() {
                    request = request.header("x-wallet-address", wallet_address);
                }
            }
            let response = request
                .send()
                .context("failed to write graph projection to KFDB")?;
            let status = response.status();
            if !status.is_success() {
                let body = response.text().unwrap_or_default();
                anyhow::bail!("KFDB rejected graph projection with {status}: {body}");
            }
            response_status = Some(status.as_u16());
            batches_written += 1;
        }
    }
    print_json(&ProjectKfdbReport {
        status: "ok".to_string(),
        repo: graph.repo,
        kfdb_url: args
            .kfdb_url
            .or_else(|| std::env::var("RICKYDATA_GIT_KFDB_URL").ok()),
        scope: args.scope,
        dry_run: args.dry_run,
        include_code_structure: args.include_code_structure,
        nodes_written: graph.nodes.len(),
        edges_written: graph.edges.len(),
        skipped_unchanged_count: 0,
        batch_size,
        batches_written,
        projection_id,
        projection_hash,
        response_status,
    })
}

#[derive(Debug)]
struct ProjectKfdbPrivateAuth {
    derive_session_id: String,
    derive_key: String,
    wallet_address: Option<String>,
}

fn project_kfdb_private_auth(args: &ProjectKfdbArgs) -> Result<ProjectKfdbPrivateAuth> {
    let derive_session_id = std::env::var(&args.derive_session_id_env).with_context(|| {
        format!(
            "live KFDB projection is private by default; set {} or pass --allow-public-kfdb for intentionally public/demo data",
            args.derive_session_id_env
        )
    })?;
    let derive_key = std::env::var(&args.derive_key_env).with_context(|| {
        format!(
            "live KFDB projection is private by default; set {} or pass --allow-public-kfdb for intentionally public/demo data",
            args.derive_key_env
        )
    })?;
    let wallet_address = std::env::var(&args.wallet_address_env).ok();
    Ok(ProjectKfdbPrivateAuth {
        derive_session_id,
        derive_key,
        wallet_address,
    })
}

fn build_graph_scan(
    repo_path: &Path,
    commit: Option<String>,
    include_code_structure: bool,
) -> Result<GraphScanReport> {
    let repo = repo_path.display().to_string();
    let inspection = rickydata_git_git::inspect_repository(&repo)?;
    let commit = commit.or(inspection.head_commit.clone());
    let repo_id = canonical_repo_id(repo_path);
    let mut nodes = Vec::new();
    let mut edges = Vec::new();
    let repo_node_id = graph_id("Repository", &[&repo_id]);
    nodes.push(graph_node(
        "Repository",
        repo_node_id.clone(),
        None,
        [
            ("repo", serde_json::Value::String(repo.clone())),
            ("repo_id", serde_json::Value::String(repo_id.clone())),
            (
                "head_commit",
                commit
                    .clone()
                    .map(serde_json::Value::String)
                    .unwrap_or(serde_json::Value::Null),
            ),
        ],
    ));
    if let Some(head) = commit.as_ref() {
        let commit_id = graph_id("Commit", &[&repo_node_id, head]);
        nodes.push(graph_node(
            "Commit",
            commit_id.clone(),
            None,
            [("commit_sha", serde_json::Value::String(head.clone()))],
        ));
        edges.push(graph_edge(&repo_node_id, "HAS_COMMIT", &commit_id));
    }
    if include_code_structure {
        append_code_structure_graph(
            repo_path,
            &repo_node_id,
            commit.as_deref().unwrap_or("unknown"),
            &mut nodes,
            &mut edges,
        )?;
    }
    for entry in rickydata_git_git::list_ref_backed_objects(&repo, Some("agent.intent"))? {
        let report = rickydata_git_git::read_cached_object(&repo, &entry.object_id)?;
        let intent: WorkIntent = serde_json::from_value(report.object.body)?;
        let node_id = graph_id("RickydataWorkIntent", &[&repo_node_id, &entry.object_id]);
        nodes.push(graph_node(
            "RickydataWorkIntent",
            node_id.clone(),
            Some(entry.object_id.clone()),
            [
                ("objective", serde_json::Value::String(intent.objective)),
                (
                    "object_id",
                    serde_json::Value::String(entry.object_id.clone()),
                ),
            ],
        ));
        edges.push(graph_edge(&repo_node_id, "CONTAINS", &node_id));
    }
    for entry in list_attempts(&repo)? {
        let node_id = graph_id(
            "RickydataAttempt",
            &[&repo_node_id, &entry.attempt.attempt_id],
        );
        nodes.push(graph_node(
            "RickydataAttempt",
            node_id.clone(),
            Some(entry.object_id.clone()),
            [
                (
                    "attempt_id",
                    serde_json::Value::String(entry.attempt.attempt_id.clone()),
                ),
                (
                    "intent_id",
                    serde_json::Value::String(entry.attempt.intent_id.clone()),
                ),
                (
                    "agent_id",
                    serde_json::Value::String(entry.attempt.agent_id.clone()),
                ),
                ("status", serde_json::to_value(entry.attempt.status)?),
            ],
        ));
        edges.push(graph_edge(&repo_node_id, "CONTAINS", &node_id));
        let intent_id = graph_id(
            "RickydataWorkIntent",
            &[&repo_node_id, &entry.attempt.intent_id],
        );
        edges.push(graph_edge(&node_id, "DERIVED_FROM_ISSUE", &intent_id));
    }
    for entry in list_runs(&repo)? {
        let node_id = graph_id("RickydataRun", &[&repo_node_id, &entry.run.run_id]);
        nodes.push(graph_node(
            "RickydataRun",
            node_id.clone(),
            Some(entry.object_id.clone()),
            [
                (
                    "run_id",
                    serde_json::Value::String(entry.run.run_id.clone()),
                ),
                (
                    "attempt_id",
                    serde_json::Value::String(entry.run.attempt_id.clone()),
                ),
                ("result", serde_json::to_value(entry.run.result)?),
            ],
        ));
        let attempt_id = graph_id("RickydataAttempt", &[&repo_node_id, &entry.run.attempt_id]);
        edges.push(graph_edge(&attempt_id, "HAS_RUN", &node_id));
    }
    for entry in list_changes(&repo)? {
        let change_node_id = graph_id("RickydataProof", &[&repo_node_id, &entry.change.change_id]);
        nodes.push(graph_node(
            "RickydataProof",
            change_node_id.clone(),
            Some(entry.object_id.clone()),
            [
                (
                    "change_id",
                    serde_json::Value::String(entry.change.change_id.clone()),
                ),
                (
                    "attempt_id",
                    serde_json::Value::String(entry.change.attempt_id.clone()),
                ),
                (
                    "diff_hash",
                    serde_json::Value::String(entry.change.diff_hash.clone()),
                ),
            ],
        ));
        let attempt_id = graph_id(
            "RickydataAttempt",
            &[&repo_node_id, &entry.change.attempt_id],
        );
        edges.push(graph_edge(&attempt_id, "PROVES", &change_node_id));
        for file in entry.change.file_paths {
            let file_id = graph_id(
                "File",
                &[
                    &repo_node_id,
                    commit.as_deref().unwrap_or("unknown"),
                    &file,
                    &entry.change.diff_hash,
                ],
            );
            nodes.push(graph_node(
                "File",
                file_id.clone(),
                None,
                [("path", serde_json::Value::String(file.clone()))],
            ));
            edges.push(graph_edge(&change_node_id, "TOUCHES", &file_id));
        }
    }
    for entry in list_patches(&repo)? {
        let node_id = graph_id("RickydataPatch", &[&repo_node_id, &entry.patch.patch_id]);
        nodes.push(graph_node(
            "RickydataPatch",
            node_id.clone(),
            Some(entry.object_id.clone()),
            [
                (
                    "patch_id",
                    serde_json::Value::String(entry.patch.patch_id.clone()),
                ),
                (
                    "attempt_id",
                    serde_json::Value::String(entry.patch.attempt_id.clone()),
                ),
            ],
        ));
        let attempt_id = graph_id(
            "RickydataAttempt",
            &[&repo_node_id, &entry.patch.attempt_id],
        );
        edges.push(graph_edge(&attempt_id, "PRODUCED_BY", &node_id));
        for file in entry.patch.file_paths {
            let file_id = graph_id(
                "File",
                &[
                    &repo_node_id,
                    commit.as_deref().unwrap_or("unknown"),
                    &file,
                    "patch",
                ],
            );
            nodes.push(graph_node(
                "File",
                file_id.clone(),
                None,
                [("path", serde_json::Value::String(file.clone()))],
            ));
            edges.push(graph_edge(&node_id, "TOUCHES", &file_id));
        }
    }
    dedupe_graph(&mut nodes, &mut edges);
    let graph_hash = stable_json_hash(&serde_json::json!({"nodes": nodes, "edges": edges}))?;
    Ok(GraphScanReport {
        status: "ok".to_string(),
        schema_version: "rickydata.repo_execution_graph.v1".to_string(),
        repo,
        repo_id,
        commit,
        include_code_structure,
        node_count: nodes.len(),
        edge_count: edges.len(),
        graph_hash,
        nodes,
        edges,
    })
}

fn append_code_structure_graph(
    repo_path: &Path,
    repo_node_id: &str,
    commit: &str,
    nodes: &mut Vec<GraphNode>,
    edges: &mut Vec<GraphEdge>,
) -> Result<()> {
    for source_path in collect_rust_source_files(repo_path)? {
        let relative_path = source_path
            .strip_prefix(repo_path)
            .unwrap_or(source_path.as_path())
            .to_string_lossy()
            .replace('\\', "/");
        let content = std::fs::read_to_string(&source_path).with_context(|| {
            format!("failed to read Rust source file {}", source_path.display())
        })?;
        let content_hash = stable_json_hash(&serde_json::json!({"content": content}))?;
        let file_id = graph_id(
            "File",
            &[repo_node_id, commit, &relative_path, &content_hash],
        );
        nodes.push(graph_node(
            "File",
            file_id.clone(),
            None,
            [
                ("path", serde_json::Value::String(relative_path.clone())),
                ("language", serde_json::Value::String("rust".to_string())),
                ("content_hash", serde_json::Value::String(content_hash)),
            ],
        ));
        edges.push(graph_edge(repo_node_id, "CONTAINS", &file_id));
        append_rust_symbols(
            &content,
            &relative_path,
            repo_node_id,
            &file_id,
            nodes,
            edges,
        );
    }
    Ok(())
}

fn collect_rust_source_files(repo_path: &Path) -> Result<Vec<PathBuf>> {
    fn visit(dir: &Path, files: &mut Vec<PathBuf>) -> Result<()> {
        for entry in std::fs::read_dir(dir)
            .with_context(|| format!("failed to read directory {}", dir.display()))?
        {
            let entry = entry?;
            let path = entry.path();
            let name = entry.file_name().to_string_lossy().to_string();
            if name == ".git" || name == "target" || name == ".rickygit-worktrees" {
                continue;
            }
            let file_type = entry.file_type()?;
            if file_type.is_dir() {
                visit(&path, files)?;
            } else if file_type.is_file()
                && path.extension().is_some_and(|extension| extension == "rs")
            {
                files.push(path);
            }
        }
        Ok(())
    }

    let mut files = Vec::new();
    visit(repo_path, &mut files)?;
    files.sort();
    Ok(files)
}

fn append_rust_symbols(
    content: &str,
    relative_path: &str,
    repo_node_id: &str,
    file_id: &str,
    nodes: &mut Vec<GraphNode>,
    edges: &mut Vec<GraphEdge>,
) {
    let mut next_fn_is_test = false;
    for (index, line) in content.lines().enumerate() {
        let line_number = index + 1;
        let trimmed = line.trim_start();
        if trimmed.starts_with("#[test]") {
            next_fn_is_test = true;
            continue;
        }
        if let Some(name) = rust_symbol_name(trimmed, "fn") {
            let label = if next_fn_is_test {
                "TestCase"
            } else {
                "Function"
            };
            let symbol_id = graph_id(
                label,
                &[repo_node_id, relative_path, &name, &line_number.to_string()],
            );
            nodes.push(graph_node(
                label,
                symbol_id.clone(),
                None,
                [
                    ("name", serde_json::Value::String(name)),
                    ("path", serde_json::Value::String(relative_path.to_string())),
                    ("language", serde_json::Value::String("rust".to_string())),
                    ("start_line", serde_json::json!(line_number)),
                ],
            ));
            edges.push(graph_edge(file_id, "DEFINES", &symbol_id));
            if next_fn_is_test {
                edges.push(graph_edge(file_id, "TESTS", &symbol_id));
            }
            next_fn_is_test = false;
            continue;
        }
        next_fn_is_test = false;
        for kind in ["struct", "enum", "trait"] {
            if let Some(name) = rust_symbol_name(trimmed, kind) {
                let symbol_id = graph_id(
                    "TypeDefinition",
                    &[repo_node_id, relative_path, &name, &line_number.to_string()],
                );
                nodes.push(graph_node(
                    "TypeDefinition",
                    symbol_id.clone(),
                    None,
                    [
                        ("name", serde_json::Value::String(name)),
                        ("kind", serde_json::Value::String(kind.to_string())),
                        ("path", serde_json::Value::String(relative_path.to_string())),
                        ("language", serde_json::Value::String("rust".to_string())),
                        ("start_line", serde_json::json!(line_number)),
                    ],
                ));
                edges.push(graph_edge(file_id, "DEFINES", &symbol_id));
                break;
            }
        }
    }
}

fn rust_symbol_name(line: &str, keyword: &str) -> Option<String> {
    let token = format!("{keyword} ");
    let start = line.find(&token)? + token.len();
    let name: String = line[start..]
        .chars()
        .skip_while(|character| character.is_whitespace())
        .take_while(|character| character.is_ascii_alphanumeric() || *character == '_')
        .collect();
    if name.is_empty() { None } else { Some(name) }
}

fn graph_node<const N: usize>(
    label: &str,
    id: String,
    source_object_id: Option<String>,
    props: [(&str, serde_json::Value); N],
) -> GraphNode {
    let mut properties = BTreeMap::new();
    properties.insert(
        "rickydata_graph_schema_version".to_string(),
        serde_json::Value::String("rickydata.repo_execution_graph.v1".to_string()),
    );
    properties.insert(
        "rickydata_graph_kind".to_string(),
        serde_json::Value::String(label.to_string()),
    );
    for (key, value) in props {
        properties.insert(key.to_string(), value);
    }
    GraphNode {
        id,
        label: label.to_string(),
        source_object_id,
        properties,
    }
}

fn graph_edge(from: &str, edge_type: &str, to: &str) -> GraphEdge {
    GraphEdge {
        id: graph_edge_id(from, edge_type, to),
        from: from.to_string(),
        to: to.to_string(),
        edge_type: edge_type.to_string(),
        properties: BTreeMap::from([(
            "rickydata_graph_schema_version".to_string(),
            serde_json::Value::String("rickydata.repo_execution_graph.v1".to_string()),
        )]),
    }
}

fn graph_id(kind: &str, parts: &[&str]) -> String {
    let name = format!(
        "rickydata.repo_execution_graph.v1:{}",
        std::iter::once(kind)
            .chain(parts.iter().copied())
            .collect::<Vec<_>>()
            .join("\u{1f}")
    );
    uuid::Uuid::new_v5(&graph_namespace(), name.as_bytes()).to_string()
}

fn graph_edge_id(from: &str, edge_type: &str, to: &str) -> String {
    let name = format!(
        "rickydata.repo_execution_graph.v1:edge:{}\u{1f}{}\u{1f}{}",
        from, edge_type, to
    );
    uuid::Uuid::new_v5(&graph_namespace(), name.as_bytes()).to_string()
}

fn graph_namespace() -> uuid::Uuid {
    uuid::Uuid::parse_str("2f3e8ab8-8684-5c6a-9fd2-c5467b94251d")
        .expect("static graph namespace uuid")
}

fn canonical_repo_id(repo: &Path) -> String {
    repo.canonicalize()
        .unwrap_or_else(|_| repo.to_path_buf())
        .display()
        .to_string()
        .trim_end_matches('/')
        .to_lowercase()
}

fn dedupe_graph(nodes: &mut Vec<GraphNode>, edges: &mut Vec<GraphEdge>) {
    let mut seen_nodes = BTreeSet::new();
    nodes.retain(|node| seen_nodes.insert(node.id.clone()));
    let mut seen_edges = BTreeSet::new();
    edges.retain(|edge| seen_edges.insert(edge.id.clone()));
    nodes.sort_by(|left, right| left.id.cmp(&right.id));
    edges.sort_by(|left, right| left.id.cmp(&right.id));
}

fn classify_graph_risk(files: &[String]) -> String {
    if files.iter().any(|file| {
        file.contains("Cargo.toml")
            || file.contains("package.json")
            || file.contains("deploy")
            || file.contains("infra")
    }) {
        "high".to_string()
    } else if files.len() > 10 {
        "medium".to_string()
    } else {
        "low".to_string()
    }
}

fn suggest_tests_for_files(files: &[String]) -> Vec<String> {
    let mut suggestions = BTreeSet::new();
    if files
        .iter()
        .any(|file| file.ends_with(".rs") || file.contains("Cargo.toml"))
    {
        suggestions.insert("cargo test".to_string());
    }
    if files.iter().any(|file| {
        file.ends_with(".ts") || file.ends_with(".tsx") || file.contains("package.json")
    }) {
        suggestions.insert("npm test".to_string());
    }
    if suggestions.is_empty() {
        suggestions.insert("run the repo's standard CI test suite".to_string());
    }
    suggestions.into_iter().collect()
}

fn graph_kfdb_operations(graph: &GraphScanReport) -> Vec<serde_json::Value> {
    let mut operations = Vec::new();
    for node in &graph.nodes {
        operations.push(serde_json::json!({
            "operation": "create_node",
            "id": node.id,
            "label": private_projection_label(&node.label),
            "properties": graph_properties(&node.properties),
            "mode": "merge"
        }));
    }
    for edge in &graph.edges {
        operations.push(serde_json::json!({
            "operation": "create_edge",
            "id": edge.id,
            "from": edge.from,
            "to": edge.to,
            "edge_type": private_projection_edge_type(&edge.edge_type),
            "properties": graph_properties(&edge.properties)
        }));
    }
    operations
}

fn graph_kfdb_payload(
    operations: &[serde_json::Value],
    scope: Option<String>,
) -> serde_json::Value {
    let mut payload = serde_json::json!({"operations": operations, "skip_embedding": true});
    if let Some(scope) = scope {
        payload["scope"] = serde_json::Value::String(scope);
    }
    payload
}

fn private_projection_label(label: &str) -> String {
    let unprefixed = label
        .strip_prefix("Rickydata")
        .or_else(|| label.strip_prefix("rickydata"))
        .unwrap_or(label);
    format!("rickydata{unprefixed}")
}

fn private_projection_edge_type(edge_type: &str) -> String {
    let unprefixed = edge_type
        .strip_prefix("RICKYDATA_")
        .or_else(|| edge_type.strip_prefix("rickydata_"))
        .unwrap_or(edge_type);
    format!("rickydata_{}", unprefixed.to_ascii_lowercase())
}

fn graph_properties(properties: &BTreeMap<String, serde_json::Value>) -> serde_json::Value {
    serde_json::Value::Object(
        properties
            .iter()
            .map(|(key, value)| (key.clone(), graph_property_value(value)))
            .collect(),
    )
}

fn graph_property_value(value: &serde_json::Value) -> serde_json::Value {
    match value {
        serde_json::Value::Null => serde_json::json!({"Null": null}),
        serde_json::Value::Bool(v) => serde_json::json!({"Boolean": v}),
        serde_json::Value::Number(v) if v.is_i64() || v.is_u64() => {
            serde_json::json!({"Integer": v})
        }
        serde_json::Value::Number(v) => serde_json::json!({"Float": v}),
        serde_json::Value::String(v) => serde_json::json!({"String": v}),
        serde_json::Value::Array(values) => {
            serde_json::json!({"Array": values.iter().map(graph_property_value).collect::<Vec<_>>() })
        }
        serde_json::Value::Object(map) => {
            serde_json::json!({"Object": map.iter().map(|(key, value)| (key.clone(), graph_property_value(value))).collect::<serde_json::Map<_, _>>() })
        }
    }
}

fn build_relay_status(
    repo: &Path,
    url: &str,
    repo_id: &str,
    auth_token: Option<&str>,
) -> Result<RelayStatusCliReport> {
    let local_object_count = rickydata_git_git::list_ref_backed_objects(repo, None)?.len();
    let report: RepoRelayStatusReport = with_relay_auth(
        relay_client()?.get(relay_url(url, repo_id, "status")?),
        auth_token,
    )
    .send()
    .context("failed to fetch relay status")?
    .error_for_status()
    .context("relay rejected status request")?
    .json()
    .context("failed to parse relay status response")?;
    let status = if local_object_count == report.object_count {
        "ok"
    } else {
        "out_of_sync"
    };
    Ok(RelayStatusCliReport {
        status: status.to_string(),
        repo_id: report.repo_id,
        local_object_count,
        relay_object_count: report.object_count,
        relay_object_ids_hash: report.object_ids_hash,
    })
}

fn build_kfdb_proof(
    url: &str,
    repo_id: &str,
    bearer_token: Option<String>,
    bearer_token_env: Option<String>,
    expected_object_count: usize,
    expected_patch_count: usize,
) -> Result<KfdbProofReport> {
    let token = match (bearer_token, bearer_token_env) {
        (Some(token), _) => Some(token),
        (None, Some(env_name)) => Some(
            std::env::var(&env_name)
                .with_context(|| format!("failed to read KFDB bearer token env `{env_name}`"))?,
        ),
        (None, None) => None,
    };
    let object_mirror_count = kfdb_count(
        url,
        token.as_deref(),
        &format!(
            "MATCH (n:RickydataObjectMirror) WHERE n.repo_id = '{repo_id}' RETURN COUNT(n) AS total"
        ),
    )?;
    let prepared_patch_count = kfdb_string_values(
        url,
        token.as_deref(),
        &format!(
            "MATCH (n:RickydataPreparedPatch) WHERE n.repo_id = '{repo_id}' RETURN n.object_id AS object_id"
        ),
        "object_id",
    )?
    .into_iter()
    .collect::<BTreeSet<_>>()
    .len() as i64;
    let status = if object_mirror_count == expected_object_count as i64
        && prepared_patch_count == expected_patch_count as i64
    {
        "ok"
    } else {
        "out_of_sync"
    };
    Ok(KfdbProofReport {
        status: status.to_string(),
        url: url.trim_end_matches('/').to_string(),
        repo_id: repo_id.to_string(),
        object_mirror_count,
        prepared_patch_count,
        expected_object_count,
        expected_patch_count,
    })
}

fn kfdb_count(url: &str, bearer_token: Option<&str>, query: &str) -> Result<i64> {
    let client = relay_client()?;
    let mut request = client
        .post(format!("{}/api/v1/query", url.trim_end_matches('/')))
        .json(&serde_json::json!({ "query": query }));
    if let Some(token) = bearer_token {
        request = request.bearer_auth(token);
    }
    let response: serde_json::Value = request
        .send()
        .context("failed to query KFDB")?
        .error_for_status()
        .context("KFDB rejected proof query")?
        .json()
        .context("failed to parse KFDB query response")?;
    response["data"][0]["count"]["Integer"]
        .as_i64()
        .or_else(|| response["data"][0]["total"]["Integer"].as_i64())
        .context("KFDB count response did not include data[0].count.Integer")
}

fn kfdb_string_values(
    url: &str,
    bearer_token: Option<&str>,
    query: &str,
    field: &str,
) -> Result<Vec<String>> {
    let client = relay_client()?;
    let mut request = client
        .post(format!("{}/api/v1/query", url.trim_end_matches('/')))
        .json(&serde_json::json!({ "query": query }));
    if let Some(token) = bearer_token {
        request = request.bearer_auth(token);
    }
    let response: serde_json::Value = request
        .send()
        .context("failed to query KFDB")?
        .error_for_status()
        .context("KFDB rejected proof query")?
        .json()
        .context("failed to parse KFDB query response")?;
    let rows = response["data"]
        .as_array()
        .context("KFDB value response did not include a data array")?;
    rows.iter()
        .map(|row| {
            row[field]["String"]
                .as_str()
                .map(str::to_string)
                .with_context(|| {
                    format!("KFDB value response did not include {field}.String in every row")
                })
        })
        .collect()
}

fn relay_client() -> Result<reqwest::blocking::Client> {
    reqwest::blocking::Client::builder()
        .http1_only()
        .timeout(Duration::from_secs(180))
        .build()
        .context("failed to build relay HTTP client")
}

/// Resolve the optional relay bearer token from an explicit `--auth-token` flag,
/// falling back to the `RICKYDATA_RELAY_AUTH_TOKEN` env var. Empty values are
/// treated as unset. The relay only enforces this when it was started with the
/// same token; otherwise it is ignored (open relay).
fn relay_auth_token(explicit: Option<&str>) -> Option<String> {
    explicit
        .map(|s| s.to_string())
        .or_else(|| std::env::var("RICKYDATA_RELAY_AUTH_TOKEN").ok())
        .filter(|token| !token.is_empty())
}

/// Attach `Authorization: Bearer <token>` to a relay request when a token is
/// configured (mirrors the `HttpKfdbIndexSink` bearer pattern).
fn with_relay_auth(
    builder: reqwest::blocking::RequestBuilder,
    explicit: Option<&str>,
) -> reqwest::blocking::RequestBuilder {
    match relay_auth_token(explicit) {
        Some(token) => builder.bearer_auth(token),
        None => builder,
    }
}

fn resolve_tee_url(explicit: Option<&str>) -> Option<String> {
    explicit
        .map(|s| s.to_string())
        .or_else(|| std::env::var("RICKYGIT_TEE_URL").ok())
}

#[cfg(feature = "tee")]
fn tee_client(tee_url: &str) -> Result<BlockingSignerHttpClient> {
    BlockingSignerHttpClient::new(SignerClientConfig {
        base_url: tee_url.to_string(),
        timeout: DEFAULT_SIGNER_TIMEOUT,
    })
    .context("failed to build rickydata_auth signer HTTP client")
}

#[cfg(feature = "tee")]
fn tee_health(tee_url: &str) -> Result<SignerHealth> {
    tee_client(tee_url)?
        .health()
        .context("failed to query rickydata_auth signer health")
}

// Signer reachability helpers. These exist in both build configs so the call
// sites stay config-agnostic; the public build (no `tee` feature) reports the
// signer as unreachable rather than linking the rickydata_auth client.
#[cfg(feature = "tee")]
fn tee_signer_status_ok(tee_url: &str) -> bool {
    tee_health(tee_url)
        .map(|health| health.status.as_deref() == Some("ok"))
        .unwrap_or(false)
}

#[cfg(not(feature = "tee"))]
fn tee_signer_status_ok(_tee_url: &str) -> bool {
    false
}

#[cfg(feature = "tee")]
fn tee_signer_reachable(tee_url: &str) -> bool {
    tee_health(tee_url).map(|_| true).unwrap_or(false)
}

#[cfg(not(feature = "tee"))]
fn tee_signer_reachable(_tee_url: &str) -> bool {
    false
}

#[cfg(feature = "tee")]
fn tee_receipt_status(tee_url: &str) -> (Option<bool>, Option<bool>) {
    match tee_health(tee_url) {
        Ok(health) => (Some(true), health.production_signing_enabled),
        Err(_) => (Some(false), None),
    }
}

#[cfg(not(feature = "tee"))]
fn tee_receipt_status(_tee_url: &str) -> (Option<bool>, Option<bool>) {
    (Some(false), None)
}

fn run_receipt(command: ReceiptSubcommand) -> Result<()> {
    match command {
        ReceiptSubcommand::Verify(args) => {
            let tee_url = resolve_tee_url(args.tee_url.as_deref());
            let input = ReceiptVerifyInput {
                repo: args.repo.display().to_string(),
                object_id: args.object_id.clone(),
                tee_url: tee_url.clone(),
                json: args.json,
            };
            run_receipt_verify(input)
        }
    }
}

fn run_receipt_verify(input: ReceiptVerifyInput) -> Result<()> {
    let verify = rickydata_git_git::verify_cached_object(&input.repo, &input.object_id)?;
    let has_signatures = verify.signature_count > 0;
    let signature_count = verify.signature_count as usize;

    let (tee_reachable, tee_production_signing) = match input.tee_url.as_deref() {
        Some(url) => tee_receipt_status(url),
        None => (None, None),
    };

    let status = if verify.valid { "ok" } else { "failed" };
    print_json(&ReceiptVerifyReport {
        status: status.to_string(),
        object_id: input.object_id,
        has_signatures,
        signature_count,
        tee_reachable,
        tee_production_signing,
    })
}

fn relay_url(base_url: &str, repo_id: &str, suffix: &str) -> Result<String> {
    ensure_safe_repo_id(repo_id)?;
    Ok(format!(
        "{}/v1/repos/{}/{}",
        base_url.trim_end_matches('/'),
        repo_id,
        suffix
    ))
}

fn relay_repo_id(repo: &Path, requested: Option<String>) -> Result<String> {
    let repo_id = match requested {
        Some(value) => value,
        None => {
            let inspection = rickydata_git_git::inspect_repository(repo)?;
            let root = inspection.root_path.as_deref().unwrap_or(repo);
            root.file_name()
                .and_then(|name| name.to_str())
                .context("failed to derive relay repo id from repository path")?
                .to_string()
        }
    };
    ensure_safe_repo_id(&repo_id)?;
    Ok(repo_id)
}

fn ensure_safe_repo_id(repo_id: &str) -> Result<()> {
    if repo_id.is_empty()
        || !repo_id
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '.' | '_' | '-'))
    {
        anyhow::bail!(
            "relay repo id `{}` is invalid; use only ASCII letters, numbers, dot, underscore, and dash",
            repo_id
        );
    }
    Ok(())
}

fn sync_status(input: SyncInput) -> Result<()> {
    print_json(&build_sync_status(input)?)
}

fn build_sync_status(input: SyncInput) -> Result<SyncStatusReport> {
    let local_refs = local_rickydata_refs(Path::new(&input.repo))?;
    let remote_refs = remote_rickydata_refs(Path::new(&input.repo), &input.remote)?;
    let mut matching_ref_count = 0;
    let mut local_only_refs = Vec::new();
    let mut remote_only_refs = Vec::new();
    let mut divergent_refs = Vec::new();

    for (ref_name, local_object_id) in &local_refs {
        match remote_refs.get(ref_name) {
            Some(remote_object_id) if remote_object_id == local_object_id => {
                matching_ref_count += 1;
            }
            Some(remote_object_id) => divergent_refs.push(SyncDivergentRef {
                ref_name: ref_name.clone(),
                local_object_id: local_object_id.clone(),
                remote_object_id: remote_object_id.clone(),
            }),
            None => local_only_refs.push(ref_name.clone()),
        }
    }

    for ref_name in remote_refs.keys() {
        if !local_refs.contains_key(ref_name) {
            remote_only_refs.push(ref_name.clone());
        }
    }

    Ok(SyncStatusReport {
        status: "ok".to_string(),
        remote: input.remote,
        refspec: RICKYDATA_REFSPEC.to_string(),
        local_ref_count: local_refs.len(),
        remote_ref_count: remote_refs.len(),
        matching_ref_count,
        local_only_refs,
        remote_only_refs,
        divergent_refs,
        local_refs_hash: refs_hash(&local_refs)?,
        remote_refs_hash: refs_hash(&remote_refs)?,
    })
}

fn sync_verify(input: SyncVerifyInput) -> Result<()> {
    print_json(&build_sync_verify(input)?)
}

fn build_sync_verify(input: SyncVerifyInput) -> Result<SyncVerifyReport> {
    let entries = rickydata_git_git::list_ref_backed_objects(&input.repo, None)?;
    let mut valid_object_count = 0;
    let mut recoverable_object_count = 0;
    let mut invalid_objects = Vec::new();
    let mut signed_object_count: usize = 0;
    let mut valid_signature_count: usize = 0;

    for entry in &entries {
        let mut diagnostics = Vec::new();
        let expected_ref = object_ref_name_from_id(&entry.object_id)?;
        if entry.ref_name != expected_ref {
            diagnostics.push(format!(
                "object ref {} does not match canonical ref {}",
                entry.ref_name, expected_ref
            ));
        }
        let verify = rickydata_git_git::verify_cached_object(&input.repo, &entry.object_id)?;
        if verify.signature_count > 0 {
            signed_object_count += 1;
        }
        valid_signature_count += verify.valid_signature_count as usize;
        // Signature diagnostics (OBJECT008/009) are warnings, not hard failures —
        // include them in the per-object diagnostics list but do not let them mark
        // the object as invalid in the sync verify summary.
        let hard_diagnostics = verify
            .diagnostics
            .iter()
            .filter(|diagnostic| diagnostic.code != "OBJECT008" && diagnostic.code != "OBJECT009")
            .map(|diagnostic| format!("{}: {}", diagnostic.code, diagnostic.message))
            .collect::<Vec<_>>();
        diagnostics.extend(hard_diagnostics);
        if verify.valid && diagnostics.is_empty() {
            valid_object_count += 1;
            recoverable_object_count += 1;
        } else {
            invalid_objects.push(SyncVerifyObjectDiagnostic {
                object_id: entry.object_id.clone(),
                ref_name: entry.ref_name.clone(),
                valid: false,
                source: object_read_source_name(verify.source)?,
                diagnostics,
            });
        }
    }

    let patches = list_patches(&input.repo)?;
    let retired_patch_ids = retired_patch_ids(&input.repo)?;
    let mut valid_patch_count = 0;
    let mut retired_patch_count = 0;
    let mut invalid_patches = Vec::new();
    for entry in patches {
        let patch = entry.patch;
        if retired_patch_ids.contains(&patch.patch_id) {
            retired_patch_count += 1;
            continue;
        }
        let mut diagnostics = Vec::new();
        if patch.diff_object_ids.is_empty() {
            diagnostics.push("prepared patch has no diff_object_ids".to_string());
        }
        for diff_object_id in &patch.diff_object_ids {
            let verify = rickydata_git_git::verify_cached_object(&input.repo, diff_object_id)?;
            diagnostics.extend(verify.diagnostics.iter().map(|diagnostic| {
                format!(
                    "diff object {} {}: {}",
                    diff_object_id, diagnostic.code, diagnostic.message
                )
            }));
            match rickydata_git_git::read_cached_object(&input.repo, diff_object_id) {
                Ok(report) if report.object.kind == "agent.patch_diff" => {}
                Ok(report) => diagnostics.push(format!(
                    "diff object {} is kind `{}`, expected `agent.patch_diff`",
                    diff_object_id, report.object.kind
                )),
                Err(error) => diagnostics.push(format!(
                    "diff object {} is not readable: {}",
                    diff_object_id, error
                )),
            }
        }
        match find_attempt(&input.repo, &patch.attempt_id) {
            Ok(attempt) => {
                if let Err(error) = resolve_patch_diff(&input.repo, &patch, &attempt) {
                    diagnostics.push(format!("patch diff evidence is invalid: {error}"));
                }
            }
            Err(error) => diagnostics.push(format!(
                "attempt {} is not readable: {}",
                patch.attempt_id, error
            )),
        }

        if diagnostics.is_empty() {
            valid_patch_count += 1;
        } else {
            invalid_patches.push(SyncVerifyPatchDiagnostic {
                patch_id: patch.patch_id,
                attempt_id: patch.attempt_id,
                valid: false,
                diff_object_ids: patch.diff_object_ids,
                diagnostics,
            });
        }
    }

    let status = if invalid_objects.is_empty() && invalid_patches.is_empty() {
        "ok"
    } else {
        "failed"
    };
    Ok(SyncVerifyReport {
        status: status.to_string(),
        object_count: entries.len(),
        valid_object_count,
        recoverable_object_count,
        invalid_objects,
        patch_count: valid_patch_count + retired_patch_count + invalid_patches.len(),
        valid_patch_count,
        retired_patch_count,
        invalid_patches,
        signed_object_count,
        valid_signature_count,
    })
}

fn local_rickydata_refs(repo: &Path) -> Result<BTreeMap<String, String>> {
    let output = StdCommand::new("git")
        .arg("-C")
        .arg(repo)
        .args([
            "for-each-ref",
            "--format=%(objectname)%09%(refname)",
            "refs/rickydata",
        ])
        .output()
        .with_context(|| "failed to execute git for-each-ref")?;
    if !output.status.success() {
        anyhow::bail!(
            "git for-each-ref refs/rickydata failed\nstdout:\n{}\nstderr:\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }
    parse_ref_lines(&output.stdout)
}

fn remote_rickydata_refs(repo: &Path, remote: &str) -> Result<BTreeMap<String, String>> {
    let output = StdCommand::new("git")
        .arg("-C")
        .arg(repo)
        .args(["ls-remote", remote, "refs/rickydata/*"])
        .output()
        .with_context(|| "failed to execute git ls-remote")?;
    if !output.status.success() {
        anyhow::bail!(
            "git ls-remote {remote} refs/rickydata/* failed\nstdout:\n{}\nstderr:\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }
    parse_ref_lines(&output.stdout)
}

fn parse_ref_lines(bytes: &[u8]) -> Result<BTreeMap<String, String>> {
    let mut refs = BTreeMap::new();
    for line in String::from_utf8_lossy(bytes).lines() {
        if line.trim().is_empty() {
            continue;
        }
        let Some((object_id, ref_name)) = line.split_once('\t') else {
            anyhow::bail!("invalid Git ref listing line `{line}`");
        };
        refs.insert(ref_name.to_string(), object_id.to_string());
    }
    Ok(refs)
}

fn refs_hash(refs: &BTreeMap<String, String>) -> Result<String> {
    stable_json_hash(&serde_json::to_value(refs)?).map_err(Into::into)
}

fn object_ref_name_from_id(object_id: &str) -> Result<String> {
    let hex = object_id
        .strip_prefix("sha256:")
        .context("object id should be a sha256 object id")?;
    if hex.len() != 64 || !hex.chars().all(|character| character.is_ascii_hexdigit()) {
        anyhow::bail!("object id should include 64 hex characters");
    }
    Ok(format!(
        "refs/rickydata/objects/sha256/{}/{}",
        &hex[0..2],
        hex
    ))
}

fn object_read_source_name(source: rickydata_git_git::ObjectReadSource) -> Result<String> {
    let value = serde_json::to_value(source)?;
    Ok(value.as_str().unwrap_or("unknown").to_string())
}

fn sync_refs(
    direction: &str,
    git_subcommand: &str,
    input: SyncInput,
    _signing: Option<&(SigningKey, Option<String>)>,
    signed_ref_expectations: Vec<rickydata_git_core::SignedRefExpectation>,
) -> Result<()> {
    let output = StdCommand::new("git")
        .arg("-C")
        .arg(&input.repo)
        .arg(git_subcommand)
        .arg(&input.remote)
        .arg(RICKYDATA_REFSPEC)
        .output()
        .with_context(|| format!("failed to execute git {git_subcommand}"))?;
    if !output.status.success() {
        anyhow::bail!(
            "git {git_subcommand} {} {RICKYDATA_REFSPEC} failed with exit code {:?}",
            input.remote,
            output.status.code()
        );
    }
    print_json(&SyncReport {
        status: "ok".to_string(),
        direction: direction.to_string(),
        remote: input.remote,
        refspec: RICKYDATA_REFSPEC.to_string(),
        stdout_hash: bytes_hash(&output.stdout),
        stderr_hash: bytes_hash(&output.stderr),
        stdout_bytes: output.stdout.len() as u64,
        stderr_bytes: output.stderr.len() as u64,
        signed_ref_expectations,
    })
}

fn run_intent(command: IntentSubcommand) -> Result<()> {
    match command {
        IntentSubcommand::Validate(args) => {
            let input = intent_file_input(args);
            let intent = read_intent(&input.intent_file)?;
            let diagnostics = validate_work_intent(&intent);
            let valid = diagnostics.is_empty();
            print_json(&IntentValidationReport { valid, diagnostics })
        }
        IntentSubcommand::Hash(args) => {
            let input = intent_file_input(args);
            let intent = read_intent(&input.intent_file)?;
            let diagnostics = validate_work_intent(&intent);
            let valid = diagnostics.is_empty();
            let body_value = serde_json::to_value(&intent)?;
            let object = CanonicalObject::new("agent.intent", DEFAULT_SCHEMA_VERSION, 0, intent)?;
            print_json(&IntentHashReport {
                object_id: object.object_id,
                body_hash: object.body_hash,
                canonical_hash: stable_json_hash(&body_value)?,
                valid,
                diagnostics,
            })
        }
        IntentSubcommand::Write(args) => {
            let signing = resolve_signing_key(&args.signing, None)?;
            let input = intent_write_input(args);
            let intent = read_intent(&input.intent_file)?;
            let diagnostics = validate_work_intent(&intent);
            if !diagnostics.is_empty() {
                return print_json(&IntentWriteReport {
                    valid: false,
                    diagnostics,
                    object: None,
                });
            }
            let body = serde_json::to_value(intent)?;
            let object =
                write_signed_or_cached(&input.repo, "agent.intent", body, signing.as_ref())?;
            print_json(&IntentWriteReport {
                valid: true,
                diagnostics,
                object: Some(object),
            })
        }
        IntentSubcommand::List(args) => {
            let input = intent_list_input(args);
            let intents =
                rickydata_git_git::list_ref_backed_objects(input.repo, Some("agent.intent"))?;
            print_json(&IntentListReport { intents })
        }
        IntentSubcommand::Show(args) => {
            let input = intent_show_input(args);
            let report = rickydata_git_git::read_cached_object(input.repo, &input.object_id)?;
            if report.object.kind != "agent.intent" {
                anyhow::bail!(
                    "object {} is kind `{}`, expected `agent.intent`",
                    report.object_id,
                    report.object.kind
                );
            }
            let intent: WorkIntent = serde_json::from_value(report.object.body)?;
            let diagnostics = validate_work_intent(&intent);
            let valid = diagnostics.is_empty();
            print_json(&IntentShowReport {
                object_id: report.object_id,
                source: report.source,
                intent,
                valid,
                diagnostics,
            })
        }
    }
}

fn run_object(command: ObjectSubcommand) -> Result<()> {
    match command {
        ObjectSubcommand::Write(args) => {
            let signing = resolve_signing_key(&args.signing, None)?;
            let input = object_write_input(args);
            let body = read_json_value(&input.body_file)?;
            let report = write_signed_or_cached(&input.repo, &input.kind, body, signing.as_ref())?;
            print_json(&report)
        }
        ObjectSubcommand::Read(args) => {
            let input = object_read_input(args);
            let report = rickydata_git_git::read_cached_object(input.repo, &input.object_id)?;
            print_json(&report)
        }
        ObjectSubcommand::Verify(args) => {
            let input = object_verify_input(args);
            let report = rickydata_git_git::verify_cached_object(input.repo, &input.object_id)?;
            print_json(&report)
        }
    }
}

fn inspect_input(args: InspectArgs) -> rickydata_git_rdl::RepoInspectInput {
    rickydata_git_rdl::RepoInspectInput {
        repo: args.repo.display().to_string(),
        json: args.json,
    }
}

fn init_input(args: InspectArgs) -> RepoInitInput {
    RepoInitInput {
        repo: args.repo.display().to_string(),
        json: args.json,
    }
}

fn status_input(args: StatusArgs) -> RepoStatusInput {
    RepoStatusInput {
        repo: args.repo.display().to_string(),
        remote: args.remote,
        json: args.json,
    }
}

fn discovery_input(args: InspectArgs) -> DiscoveryEmitInput {
    DiscoveryEmitInput {
        repo: args.repo.display().to_string(),
        json: args.json,
    }
}

fn intent_file_input(args: IntentFileArgs) -> IntentFileInput {
    IntentFileInput {
        intent_file: args.intent_file.display().to_string(),
        json: args.json,
    }
}

fn intent_write_input(args: IntentWriteArgs) -> IntentWriteInput {
    IntentWriteInput {
        repo: args.repo.display().to_string(),
        intent_file: args.intent_file.display().to_string(),
        json: args.json,
    }
}

fn intent_list_input(args: IntentListArgs) -> IntentListInput {
    IntentListInput {
        repo: args.repo.display().to_string(),
        json: args.json,
    }
}

fn intent_show_input(args: ObjectIdArgs) -> IntentShowInput {
    IntentShowInput {
        repo: args.repo.display().to_string(),
        object_id: args.object_id,
        json: args.json,
    }
}

fn attempt_start_input(args: AttemptStartArgs) -> AttemptStartInput {
    AttemptStartInput {
        repo: args.repo.display().to_string(),
        intent_id: args.intent_id,
        agent_id: args.agent_id,
        idempotency_key: args.idempotency_key,
        base_commit: args.base_commit,
        lease_expires_at_ms: args.lease_expires_at_ms,
        json: args.json,
    }
}

fn attempt_list_input(args: AttemptListArgs) -> AttemptListInput {
    AttemptListInput {
        repo: args.repo.display().to_string(),
        json: args.json,
    }
}

fn attempt_show_input(args: AttemptShowArgs) -> AttemptShowInput {
    AttemptShowInput {
        repo: args.repo.display().to_string(),
        attempt_id: args.attempt_id,
        json: args.json,
    }
}

fn attempt_transition_input(args: AttemptTransitionArgs) -> AttemptTransitionInput {
    AttemptTransitionInput {
        repo: args.repo.display().to_string(),
        attempt_id: args.attempt_id,
        reason: args.reason,
        by: args.by,
    }
}

fn object_write_input(args: ObjectWriteArgs) -> ObjectWriteInput {
    ObjectWriteInput {
        repo: args.repo.display().to_string(),
        kind: args.kind,
        body_file: args.body_file.display().to_string(),
        json: args.json,
    }
}

fn object_read_input(args: ObjectIdArgs) -> ObjectReadInput {
    ObjectReadInput {
        repo: args.repo.display().to_string(),
        object_id: args.object_id,
        json: args.json,
    }
}

fn object_verify_input(args: ObjectIdArgs) -> ObjectVerifyInput {
    ObjectVerifyInput {
        repo: args.repo.display().to_string(),
        object_id: args.object_id,
        json: args.json,
    }
}

fn run_exec_input(args: RunExecArgs) -> RunExecInput {
    RunExecInput {
        repo: args.repo.display().to_string(),
        attempt_id: args.attempt_id,
        command: args.command,
        record_command_argv: args.record_command_argv,
        json: args.json,
    }
}

fn run_list_input(args: RunListArgs) -> RunListInput {
    RunListInput {
        repo: args.repo.display().to_string(),
        json: args.json,
    }
}

fn run_show_input(args: RunShowArgs) -> RunShowInput {
    RunShowInput {
        repo: args.repo.display().to_string(),
        run_id: args.run_id,
        json: args.json,
    }
}

fn change_detect_input(args: ChangeDetectArgs) -> ChangeDetectInput {
    ChangeDetectInput {
        repo: args.repo.display().to_string(),
        attempt_id: args.attempt_id,
        run_ids: args.run_ids,
        json: args.json,
    }
}

fn change_list_input(args: ChangeListArgs) -> ChangeListInput {
    ChangeListInput {
        repo: args.repo.display().to_string(),
        json: args.json,
    }
}

fn change_show_input(args: ChangeShowArgs) -> ChangeShowInput {
    ChangeShowInput {
        repo: args.repo.display().to_string(),
        change_id: args.change_id,
        json: args.json,
    }
}

fn patch_prepare_input(args: PatchPrepareArgs) -> PatchPrepareInput {
    PatchPrepareInput {
        repo: args.repo.display().to_string(),
        attempt_id: args.attempt_id,
        json: args.json,
    }
}

fn patch_list_input(args: PatchListArgs) -> PatchListInput {
    PatchListInput {
        repo: args.repo.display().to_string(),
        json: args.json,
    }
}

fn patch_show_input(args: PatchShowArgs) -> PatchShowInput {
    PatchShowInput {
        repo: args.repo.display().to_string(),
        patch_id: args.patch_id,
        json: args.json,
    }
}

fn patch_export_input(args: PatchExportArgs) -> PatchExportInput {
    PatchExportInput {
        repo: args.repo.display().to_string(),
        patch_id: args.patch_id,
        output: args.output.display().to_string(),
        force: args.force,
        json: args.json,
    }
}

fn patch_apply_input(args: PatchApplyArgs) -> PatchApplyInput {
    PatchApplyInput {
        repo: args.repo.display().to_string(),
        patch_id: args.patch_id,
        allow_dirty: args.allow_dirty,
        allow_base_drift: args.allow_base_drift,
        applied_by: args.applied_by,
        reason: args.reason,
        idempotency_key: args.idempotency_key,
        json: args.json,
    }
}

fn patch_checkout_input(args: PatchCheckoutArgs) -> PatchCheckoutInput {
    PatchCheckoutInput {
        repo: args.repo.display().to_string(),
        patch_id: args.patch_id,
        path: args.path.map(|path| path.display().to_string()),
        force: args.force,
        allow_base_drift: args.allow_base_drift,
        json: args.json,
    }
}

fn patch_retire_input(args: PatchRetireArgs) -> PatchRetireInput {
    PatchRetireInput {
        repo: args.repo.display().to_string(),
        patch_id: args.patch_id,
        reason: args.reason,
        retired_by: args.retired_by,
        idempotency_key: args.idempotency_key,
        json: args.json,
    }
}

fn sync_input(args: SyncArgs) -> SyncInput {
    SyncInput {
        repo: args.repo.display().to_string(),
        remote: args.remote,
        json: args.json,
    }
}

fn sync_verify_input(args: SyncVerifyArgs) -> SyncVerifyInput {
    SyncVerifyInput {
        repo: args.repo.display().to_string(),
        json: args.json,
    }
}

fn read_intent(path: &str) -> Result<WorkIntent> {
    let contents = std::fs::read_to_string(path)
        .with_context(|| format!("failed to read intent file `{path}`"))?;
    serde_json::from_str(&contents)
        .with_context(|| format!("failed to parse intent file `{path}` as JSON"))
}

fn read_json_value(path: &str) -> Result<serde_json::Value> {
    let contents =
        std::fs::read_to_string(path).with_context(|| format!("failed to read file `{path}`"))?;
    serde_json::from_str(&contents).with_context(|| format!("failed to parse `{path}` as JSON"))
}

fn compute_attempt_id(input: &AttemptStartInput, base_commit: &str) -> Result<String> {
    let idempotency_key = input.idempotency_key.as_deref().unwrap_or("default");
    let identity = serde_json::json!({
        "intent_id": input.intent_id,
        "agent_id": input.agent_id,
        "base_commit": base_commit,
        "idempotency_key": idempotency_key,
    });
    stable_json_hash(&identity).map_err(Into::into)
}

struct DetectedDiff {
    changed: bool,
    diff_hash: String,
    diff_bytes: u64,
    file_paths: Vec<String>,
    diff_summary: DiffSummary,
    raw_diff: Vec<u8>,
}

fn detect_attempt_diff(repo: &Path, attempt: &AgentAttempt) -> Result<DetectedDiff> {
    let worktree_path = if attempt.in_place {
        repo.to_path_buf()
    } else {
        existing_attempt_worktree_path(repo, &attempt.attempt_id)?
    };
    let temp_index = TempGitIndex::new(repo, &attempt.attempt_id)?;

    git_with_temp_index(
        &worktree_path,
        temp_index.path(),
        &["read-tree", &attempt.base_commit],
    )?;
    git_with_temp_index(&worktree_path, temp_index.path(), &["add", "-A"])?;
    let diff = git_with_temp_index(
        &worktree_path,
        temp_index.path(),
        &[
            "diff",
            "--cached",
            "--binary",
            "--full-index",
            &attempt.base_commit,
            "--",
        ],
    )?;
    let numstat = git_with_temp_index(
        &worktree_path,
        temp_index.path(),
        &["diff", "--cached", "--numstat", &attempt.base_commit, "--"],
    )?;
    let name_status = git_with_temp_index(
        &worktree_path,
        temp_index.path(),
        &[
            "diff",
            "--cached",
            "--name-status",
            &attempt.base_commit,
            "--",
        ],
    )?;
    let names = git_with_temp_index(
        &worktree_path,
        temp_index.path(),
        &[
            "diff",
            "--cached",
            "--name-only",
            "-z",
            &attempt.base_commit,
            "--",
        ],
    )?;
    let mut file_paths = names
        .split(|byte| *byte == 0)
        .filter(|part| !part.is_empty())
        .map(|part| String::from_utf8_lossy(part).into_owned())
        .collect::<Vec<_>>();
    file_paths.sort();
    file_paths.dedup();
    let diff_summary = summarize_diff(&file_paths, &numstat, &name_status);

    Ok(DetectedDiff {
        changed: !diff.is_empty(),
        diff_hash: bytes_hash(&diff),
        diff_bytes: diff.len() as u64,
        file_paths,
        diff_summary,
        raw_diff: diff,
    })
}

fn summarize_diff(file_paths: &[String], numstat: &[u8], name_status: &[u8]) -> DiffSummary {
    let mut summary = DiffSummary {
        file_count: file_paths.len() as u64,
        ..DiffSummary::default()
    };

    for line in String::from_utf8_lossy(numstat).lines() {
        let mut parts = line.split('\t');
        let Some(insertions) = parts.next() else {
            continue;
        };
        let Some(deletions) = parts.next() else {
            continue;
        };
        if insertions == "-" || deletions == "-" {
            summary.binary_file_count += 1;
            continue;
        }
        summary.insertions += insertions.parse::<u64>().unwrap_or(0);
        summary.deletions += deletions.parse::<u64>().unwrap_or(0);
    }

    for line in String::from_utf8_lossy(name_status).lines() {
        let Some(status) = line.split('\t').next() else {
            continue;
        };
        match status.chars().next() {
            Some('A') => summary.files_added += 1,
            Some('D') => summary.files_deleted += 1,
            Some('R') => summary.files_renamed += 1,
            Some('M') | Some('T') | Some('C') => summary.files_modified += 1,
            _ => {}
        }
    }

    summary
}

fn select_change_runs(
    repo: &str,
    attempt_id: &str,
    requested_run_ids: &[String],
) -> Result<Vec<AgentRun>> {
    let all_runs = list_runs(repo)?
        .into_iter()
        .map(|entry| entry.run)
        .collect::<Vec<_>>();
    let mut selected = if requested_run_ids.is_empty() {
        all_runs
            .into_iter()
            .filter(|run| run.attempt_id == attempt_id)
            .collect::<Vec<_>>()
    } else {
        let mut runs = Vec::new();
        for run_id in requested_run_ids {
            let Some(run) = all_runs.iter().find(|run| run.run_id == *run_id) else {
                anyhow::bail!("run {run_id} was not found");
            };
            if run.attempt_id != attempt_id {
                anyhow::bail!("run {run_id} belongs to attempt {}", run.attempt_id);
            }
            runs.push(run.clone());
        }
        runs
    };

    selected.sort_by(|left, right| left.run_id.cmp(&right.run_id));
    selected.dedup_by(|left, right| left.run_id == right.run_id);
    Ok(selected)
}

fn related_contract_hashes(runs: &[AgentRun]) -> Vec<String> {
    sorted_dedup(
        runs.iter()
            .flat_map(|run| run.rdl_manifest_hashes.iter().cloned())
            .collect::<Vec<_>>(),
    )
}

fn sorted_dedup(mut values: Vec<String>) -> Vec<String> {
    values.sort();
    values.dedup();
    values
}

fn ensure_attempt_worktree(
    repo: &Path,
    attempt_id: &str,
    base_commit: &str,
) -> Result<(PathBuf, bool)> {
    let worktree_path = attempt_worktree_path(repo, attempt_id)?;
    if worktree_path.exists() {
        return Ok((worktree_path, false));
    }
    let parent = worktree_path
        .parent()
        .context("attempt worktree path should have a parent")?;
    std::fs::create_dir_all(parent)
        .with_context(|| format!("failed to create worktree parent `{}`", parent.display()))?;
    let output = StdCommand::new("git")
        .arg("-C")
        .arg(repo)
        .args(["worktree", "add", "--detach"])
        .arg(&worktree_path)
        .arg(base_commit)
        .output()
        .with_context(|| "failed to execute git worktree add")?;
    if !output.status.success() {
        anyhow::bail!(
            "git worktree add failed\nstdout:\n{}\nstderr:\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }
    Ok((worktree_path, true))
}

fn existing_attempt_worktree_path(repo: &Path, attempt_id: &str) -> Result<PathBuf> {
    let worktree_path = attempt_worktree_path(repo, attempt_id)?;
    if !worktree_path.is_dir() {
        anyhow::bail!(
            "attempt worktree for {attempt_id} does not exist at {}",
            worktree_path.display()
        );
    }
    Ok(worktree_path)
}

fn attempt_worktree_path(repo: &Path, attempt_id: &str) -> Result<PathBuf> {
    let inspection = rickydata_git_git::inspect_repository(repo)?;
    let git_dir = inspection
        .git_dir
        .context("repository has no .git directory")?;
    let attempt_hex = attempt_id
        .strip_prefix("sha256:")
        .context("attempt id should be a sha256 object id")?;
    if attempt_hex.len() < 12 {
        anyhow::bail!("attempt id should include at least 12 hex characters");
    }
    Ok(git_dir
        .join("rickydata")
        .join("worktrees")
        .join(&attempt_hex[0..12]))
}

const REVIEW_CHECKOUT_MARKER: &str = ".rickydata-review.json";

#[derive(Debug, Serialize, Deserialize)]
struct ReviewCheckoutMarker {
    patch_id: String,
    attempt_id: String,
    base_commit: String,
    diff_hash: String,
    created_at_ms: u64,
}

fn review_checkout_path(
    repo: &str,
    patch_id: &str,
    requested_path: Option<String>,
) -> Result<PathBuf> {
    if let Some(path) = requested_path {
        let path = PathBuf::from(path);
        if path.is_absolute() {
            return Ok(path);
        }
        return Ok(Path::new(repo).join(path));
    }

    let inspection = rickydata_git_git::inspect_repository(repo)?;
    let git_dir = inspection
        .git_dir
        .context("repository has no .git directory")?;
    let patch_hex = patch_id
        .strip_prefix("sha256:")
        .context("patch id should be a sha256 object id")?;
    if patch_hex.len() < 12 {
        anyhow::bail!("patch id should include at least 12 hex characters");
    }
    Ok(git_dir
        .join("rickydata")
        .join("reviews")
        .join(&patch_hex[0..12]))
}

fn prepare_review_checkout_path(
    repo: &Path,
    checkout_path: &Path,
    patch_id: &str,
    force: bool,
) -> Result<bool> {
    if !checkout_path.exists() {
        return Ok(false);
    }
    if !force {
        anyhow::bail!(
            "checkout path {} already exists; pass --force to replace a Rickydata-owned checkout",
            checkout_path.display()
        );
    }
    let marker = read_review_checkout_marker(checkout_path)?;
    if marker.patch_id != patch_id {
        anyhow::bail!(
            "checkout path {} belongs to patch {}, expected {}",
            checkout_path.display(),
            marker.patch_id,
            patch_id
        );
    }
    remove_review_worktree(repo, checkout_path)?;
    Ok(true)
}

fn read_review_checkout_marker(checkout_path: &Path) -> Result<ReviewCheckoutMarker> {
    let marker_path = checkout_path.join(REVIEW_CHECKOUT_MARKER);
    let contents = std::fs::read_to_string(&marker_path).with_context(|| {
        format!(
            "checkout path {} is not marked as Rickydata-owned",
            checkout_path.display()
        )
    })?;
    serde_json::from_str(&contents).with_context(|| {
        format!(
            "checkout marker `{}` is not valid Rickydata metadata",
            marker_path.display()
        )
    })
}

fn add_review_worktree(repo: &Path, checkout_path: &Path, base_commit: &str) -> Result<()> {
    if let Some(parent) = checkout_path.parent()
        && !parent.as_os_str().is_empty()
    {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("failed to create checkout parent `{}`", parent.display()))?;
    }
    let output = StdCommand::new("git")
        .arg("-C")
        .arg(repo)
        .args(["worktree", "add", "--detach"])
        .arg(checkout_path)
        .arg(base_commit)
        .output()
        .with_context(|| "failed to execute git worktree add for review checkout")?;
    if !output.status.success() {
        anyhow::bail!(
            "git worktree add failed\nstdout:\n{}\nstderr:\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }
    Ok(())
}

fn remove_review_worktree(repo: &Path, checkout_path: &Path) -> Result<()> {
    let output = StdCommand::new("git")
        .arg("-C")
        .arg(repo)
        .args(["worktree", "remove", "--force"])
        .arg(checkout_path)
        .output()
        .with_context(|| "failed to execute git worktree remove for review checkout")?;
    if output.status.success() || !checkout_path.exists() {
        return Ok(());
    }
    if checkout_path.is_dir() {
        std::fs::remove_dir_all(checkout_path).with_context(|| {
            format!(
                "failed to remove existing checkout `{}`",
                checkout_path.display()
            )
        })?;
    } else {
        std::fs::remove_file(checkout_path).with_context(|| {
            format!(
                "failed to remove existing checkout file `{}`",
                checkout_path.display()
            )
        })?;
    }
    Ok(())
}

fn write_review_checkout_marker(checkout_path: &Path, marker: ReviewCheckoutMarker) -> Result<()> {
    ignore_review_checkout_marker(checkout_path)?;
    let marker_path = checkout_path.join(REVIEW_CHECKOUT_MARKER);
    std::fs::write(&marker_path, serde_json::to_vec_pretty(&marker)?).with_context(|| {
        format!(
            "failed to write checkout marker `{}`",
            marker_path.display()
        )
    })
}

fn ignore_review_checkout_marker(checkout_path: &Path) -> Result<()> {
    let output = StdCommand::new("git")
        .arg("-C")
        .arg(checkout_path)
        .args(["rev-parse", "--git-path", "info/exclude"])
        .output()
        .with_context(|| "failed to execute git rev-parse for review checkout")?;
    if !output.status.success() {
        anyhow::bail!(
            "git rev-parse --git-path info/exclude failed\nstdout:\n{}\nstderr:\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }
    let exclude_path = PathBuf::from(String::from_utf8_lossy(&output.stdout).trim());
    let exclude_path = if exclude_path.is_absolute() {
        exclude_path
    } else {
        checkout_path.join(exclude_path)
    };
    let mut contents = if exclude_path.exists() {
        std::fs::read_to_string(&exclude_path).with_context(|| {
            format!(
                "failed to read checkout exclude `{}`",
                exclude_path.display()
            )
        })?
    } else {
        String::new()
    };
    if !contents
        .lines()
        .any(|line| line.trim() == REVIEW_CHECKOUT_MARKER)
    {
        if !contents.ends_with('\n') && !contents.is_empty() {
            contents.push('\n');
        }
        contents.push_str(REVIEW_CHECKOUT_MARKER);
        contents.push('\n');
        if let Some(parent) = exclude_path.parent() {
            std::fs::create_dir_all(parent).with_context(|| {
                format!("failed to create exclude parent `{}`", parent.display())
            })?;
        }
        std::fs::write(&exclude_path, contents).with_context(|| {
            format!(
                "failed to write checkout exclude `{}`",
                exclude_path.display()
            )
        })?;
    }
    Ok(())
}

struct TempGitIndex {
    path: PathBuf,
}

impl TempGitIndex {
    fn new(repo: &Path, attempt_id: &str) -> Result<Self> {
        let inspection = rickydata_git_git::inspect_repository(repo)?;
        let git_dir = inspection
            .git_dir
            .context("repository has no .git directory")?;
        let attempt_hex = attempt_id
            .strip_prefix("sha256:")
            .context("attempt id should be a sha256 object id")?;
        if attempt_hex.len() < 12 {
            anyhow::bail!("attempt id should include at least 12 hex characters");
        }
        let temp_dir = git_dir.join("rickydata").join("tmp");
        std::fs::create_dir_all(&temp_dir)
            .with_context(|| format!("failed to create temp dir `{}`", temp_dir.display()))?;
        Ok(Self {
            path: temp_dir.join(format!(
                "change-{}-{}.index",
                &attempt_hex[0..12],
                now_ms()?
            )),
        })
    }

    fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for TempGitIndex {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
        let _ = std::fs::remove_file(PathBuf::from(format!("{}.lock", self.path.display())));
    }
}

struct TempPatchFile {
    path: PathBuf,
}

impl TempPatchFile {
    fn new(repo: &Path, patch_id: &str, patch_bytes: &[u8]) -> Result<Self> {
        let inspection = rickydata_git_git::inspect_repository(repo)?;
        let git_dir = inspection
            .git_dir
            .context("repository has no .git directory")?;
        let patch_hex = patch_id
            .strip_prefix("sha256:")
            .context("patch id should be a sha256 object id")?;
        if patch_hex.len() < 12 {
            anyhow::bail!("patch id should include at least 12 hex characters");
        }
        let temp_dir = git_dir.join("rickydata").join("tmp");
        std::fs::create_dir_all(&temp_dir)
            .with_context(|| format!("failed to create temp dir `{}`", temp_dir.display()))?;
        let path = temp_dir.join(format!("apply-{}-{}.patch", &patch_hex[0..12], now_ms()?));
        std::fs::write(&path, patch_bytes)
            .with_context(|| format!("failed to write temp patch `{}`", path.display()))?;
        Ok(Self { path })
    }

    fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for TempPatchFile {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
    }
}

fn ensure_clean_worktree(repo: &Path) -> Result<()> {
    let output = StdCommand::new("git")
        .arg("-C")
        .arg(repo)
        .args(["status", "--porcelain", "-z"])
        .output()
        .with_context(|| "failed to execute git status")?;
    if !output.status.success() {
        anyhow::bail!(
            "git status failed\nstdout:\n{}\nstderr:\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }
    if !output.stdout.is_empty() {
        anyhow::bail!("worktree is dirty; pass --allow-dirty to override");
    }
    Ok(())
}

fn git_apply_file(repo: &Path, patch_file: &Path, check: bool) -> Result<()> {
    let mut command = StdCommand::new("git");
    command.arg("-C").arg(repo).arg("apply");
    if check {
        command.arg("--check");
    }
    let output = command
        .arg(patch_file)
        .output()
        .with_context(|| "failed to execute git apply")?;
    if !output.status.success() {
        anyhow::bail!(
            "git apply{} failed\nstdout:\n{}\nstderr:\n{}",
            if check { " --check" } else { "" },
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }
    Ok(())
}

fn git_with_temp_index(worktree_path: &Path, index_path: &Path, args: &[&str]) -> Result<Vec<u8>> {
    let output = StdCommand::new("git")
        .arg("-C")
        .arg(worktree_path)
        .args(args)
        .env("GIT_INDEX_FILE", index_path)
        .output()
        .with_context(|| "failed to execute git for change detection")?;
    if !output.status.success() {
        anyhow::bail!(
            "git {} failed\nstdout:\n{}\nstderr:\n{}",
            args.join(" "),
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }
    Ok(output.stdout)
}

fn now_ms() -> Result<u64> {
    Ok(SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .context("system time is before UNIX_EPOCH")?
        .as_millis() as u64)
}

fn bytes_hash(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    format!("sha256:{:x}", hasher.finalize())
}

fn encode_hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut encoded = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        encoded.push(HEX[(byte >> 4) as usize] as char);
        encoded.push(HEX[(byte & 0x0f) as usize] as char);
    }
    encoded
}

fn decode_hex(encoded: &str) -> Result<Vec<u8>> {
    if !encoded.len().is_multiple_of(2) {
        anyhow::bail!("hex-encoded patch diff has odd length");
    }
    let mut bytes = Vec::with_capacity(encoded.len() / 2);
    for index in (0..encoded.len()).step_by(2) {
        let byte = u8::from_str_radix(&encoded[index..index + 2], 16)
            .with_context(|| format!("invalid hex byte at offset {index}"))?;
        bytes.push(byte);
    }
    Ok(bytes)
}

fn print_json<T: Serialize>(value: &T) -> Result<()> {
    println!("{}", serde_json::to_string_pretty(value)?);
    Ok(())
}
