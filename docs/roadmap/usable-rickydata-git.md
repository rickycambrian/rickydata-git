# Roadmap To A Usable Rickydata Git

## Target

The goal is not to replace Git commands on day one. The goal is a Git-compatible
agent collaboration layer that we can use on real Rickydata projects while
normal `git clone`, `git status`, `git commit`, `git pull`, and `git push`
continue to work.

A legitimately usable first product means:

- a human or agent can bind work to an issue/task before editing starts
- each agent attempt runs in isolation without exposing worktree mechanics as
  the user-facing model
- traces, commands, tests, diffs, contracts, and evidence are recorded as
  repo-native objects
- the recorded meaning can be pushed, pulled, verified, and inspected locally
- private traces and secrets have a clear encryption boundary before remote
  exchange
- GitHub remains bridgeable, but not the source of truth for collaboration
  meaning

## Commit Discipline

This repo should use frequent durable checkpoints:

- commit and push every green vertical slice
- commit docs/plans separately when they materially change direction
- never leave a large tested batch unstaged overnight
- do not commit unrelated dirt from sibling repos
- when touching `rickydata_lang`, commit and push that repo independently

## Phase 0: Foundation

Status: implemented.

Delivered:

- Rust workspace on `main`
- Git inspection through `gix`
- canonical JSON object IDs and golden vectors
- typed WorkIntent, AgentAttempt, AgentRun, ChangeEvidence, DiscoveryObject,
  AttestationEvidence, and PaymentEvidence schemas
- JSON-first CLI
- schema catalog with stable hashes
- compiled RDL command manifests
- compiled `rust-rdl` discovery emitter
- `rickygit init` local store creation

## Phase 1: Local Rickydata Store

Purpose: make Rickydata objects persist locally without breaking Git.

Implement:

- `rickygit init --repo <path> --json` (implemented)
- `.git/rickydata/cache/objects/*` local object cache (implemented)
- `refs/rickydata/objects/sha256/*` Git blob-backed object distribution
  (implemented)
- `refs/rickydata/discovery/*`
- `refs/rickydata/intents/*`
- `refs/rickydata/attempts/*`
- `refs/rickydata/runs/*`
- append-only object writes through canonical envelopes (implemented for generic
  objects)
- object read/verify commands (implemented for generic objects)
- migration/version marker

Acceptance:

- normal Git clients ignore the metadata
- Rickydata object writes are deterministic and test-covered
- `rickygit verify --repo <path> --json` can prove refs and cached objects match
- deleting `.git/rickydata/cache` can be repaired from `refs/rickydata/*`
  (implemented for object read after fetching `refs/rickydata/objects/*`)

## Phase 2: Work Intent Workflow

Purpose: capture issue/task meaning before edits happen.

Implement:

- `rickygit intent write --repo <path> <intent-file> --json` (implemented)
- `rickygit intent create --issue ... --objective ... --json`
- `rickygit intent list --repo <path> --json` (implemented)
- `rickygit intent show --repo <path> --object-id <id> --json` (implemented)
- `rickygit intent close <id> --json`
- validation against active issue/task refs
- stable issue/task binding model for GitHub, local tasks, and future Rickydata
  issue objects

Acceptance:

- no agent attempt can start without an active WorkIntent
- intent IDs are content-addressed and reproducible
- intent creation writes repo-native objects, not commit-message conventions

## Phase 3: Isolated Agent Attempts

Purpose: let agents work concurrently without branch/worktree chaos leaking to
users.

Implement:

- `rickygit attempt start --repo <path> --intent-id <id> --agent-id <id> --json`
  (implemented)
- `rickygit attempt list --repo <path> --json` (implemented)
- `rickygit attempt show --repo <path> --attempt-id <id> --json` (implemented)
- `rickygit attempt status <id> --json`
- `rickygit attempt abandon <id> --json`
- `rickygit attempt submit <id> --json`
- internal worktree allocation and cleanup
- lease records so two agents do not claim the same attempt
- base commit pinning and drift detection
- rebase/refresh command with structured conflicts

Design rule:

Worktrees are an implementation primitive. Users and agents interact with
attempt IDs, not long-lived shared branches.

Acceptance:

- two agents can work on two issues in parallel
- attempts can be abandoned without dirtying the main worktree
- submitted attempts produce patch/change evidence tied to their WorkIntent

## Phase 4: Agent Run And Trace Capture

Purpose: record what actually happened during agent work.

Implement:

- `rickygit run exec --repo <path> --attempt-id <id> --json -- <command...>`
  (implemented)
- `rickygit run list --repo <path> --json`
  (implemented)
- `rickygit run show --repo <path> --run-id <id> --json`
  (implemented)
- `rickygit run start --attempt <id> --json`
- `rickygit run record-command --run <id> --json`
- `rickygit run finish --run <id> --json`
- command hashes, manifest hashes, test results, and diagnostics
- private-by-default trace body references
- OpenTelemetry-style span shape for future export

Acceptance:

- every agent-created patch can point to a run
- every run points to the command contracts used
- trace bodies can be omitted, encrypted, or redacted without losing public
  audit metadata

## Phase 5: Change Evidence And Patch Preparation

Purpose: tie code changes to symbols, contracts, diagnostics, and tests while
the work is happening.

Implement:

- `rickygit change detect --repo <path> --attempt-id <id> --json`
  (implemented)
- `rickygit change list --repo <path> --json`
  (implemented)
- `rickygit change show --repo <path> --change-id <id> --json`
  (implemented)
- diff hash calculation
  (implemented)
- file path and symbol refs
  (file paths implemented; symbol refs pending)
- related RDL contract hashes
  (implemented from linked run manifests)
- test/diagnostic evidence
- `rickygit patch prepare --repo <path> --attempt-id <id> --json`
  (implemented as a repo-native patch summary; no PR creation yet)
- `rickygit patch list --repo <path> --json`
  (implemented)
- `rickygit patch show --repo <path> --patch-id <id> --json`
  (implemented)
- `rickygit patch export --repo <path> --patch-id <id> --output <file> --json`
  (implemented as a Git patch export from repo-native patch diff objects, with
  a guarded worktree fallback for older patch summaries)
- `rickygit patch apply --repo <path> --patch-id <id> --json`
  (implemented with clean-worktree, base-commit, and `git apply --check`
  guards before mutation)

Acceptance:

- a code reviewer can ask why a file changed and get issue, intent, attempt,
  run, tests, and contract links
- evidence is generated before PR creation, not reconstructed afterward

## Phase 6: Repo-Native Sync

Purpose: make collaboration state portable across clones.

Implement:

- `rickygit sync push --repo <path> --remote origin --json`
  (implemented for refs/rickydata/* over ordinary Git remotes)
- `rickygit sync pull --repo <path> --remote origin --json`
  (implemented for refs/rickydata/* over ordinary Git remotes)
- `rickygit sync status --repo <path> --remote origin --json`
  (implemented for local/remote Rickydata ref parity reporting)
- ref namespace negotiation
- deterministic merge of append-only objects
- signed object support
- verification against expected delegates/policies

Acceptance:

- another clone can pull Rickydata metadata and inspect intents/attempts/runs
- normal Git history remains untouched
- corrupt or conflicting metadata produces structured diagnostics

## Phase 7: GitHub Bridge

Purpose: use existing projects immediately while moving source of truth into the
repo-native layer.

Implement:

- import GitHub issues into WorkIntent-compatible refs
- attach PR/patch metadata to attempts
- optional `gh`/API bridge for comments and PR creation
- durable linkage back to GitHub issue IDs and URLs

Acceptance:

- starting work from a GitHub issue creates a WorkIntent first
- PR descriptions can be generated from Rickydata evidence
- losing GitHub API access does not erase local intent/run/change history

## Phase 8: Private Data And TEE Auth

Purpose: support sensitive traces and private collaboration objects.

Implement:

- envelope encrypted private bodies
- wallet sign-to-derive key wrapping
- TEE signer/auth receipts
- release guard policies
- replay counters and trust bundle verification
- redaction metadata in traces and manifests

Acceptance:

- private traces are not plaintext by default
- a relying party can verify operation, payload hash, trust bundle hash, and
  monotonic counter state
- server-pinned recoverable keys are treated as recovery, not a privacy boundary

## Phase 9: Rickydata Lang Upstream Loop

Purpose: improve `rickydata_lang` based on concrete `rickydata_git` needs.

Implement upstream as needs become proven:

- `CommandManifest`
- repo/Git/filesystem capabilities
- compiled or AST-backed graph metadata
- real `effects --json`
- manifest privacy/redaction metadata
- intent/attempt fields in execution traces
- SARIF and OpenTelemetry export paths
- guidance for TypeScript, Python, Go, SCIP, LSP, and Tree-sitter adapters

Acceptance:

- `rickydata_git` no longer needs local workarounds for command manifests
- non-Rust adapters can emit the same discovery schema

## Phase 10: Dogfooding Across Rickydata Repos

Purpose: prove the system is useful before building a hosted forge.

Dogfood order:

1. `rickydata_git`
2. `rickydata_lang`
3. `rickydata_SDK`
4. `rickydata_docs`
5. `rickydata_auth`
6. `knowledgeflow_db`

Acceptance:

- every substantial change starts with a WorkIntent
- agent attempts are isolated and inspectable
- change evidence links files/symbols/contracts/tests to issues
- metadata can be cloned and inspected from another machine

## Phase 11: Hosted/Peer Forge Layer

Purpose: add collaboration UX without recreating GitHub lock-in.

Implement:

- `rickydata-git-relay` deployed as a relay/index service, not a sole source of
  truth
  (local HTTP router and binary implemented; Cloud Run/GCS deploy pending)
- versioned content-addressed object bundle storage
  (local `FileRelayStore` validation/push/pull core implemented)
- Git ref mirroring for `refs/rickydata/*`
- KFDB projection/index rebuild jobs
- Agent Gateway workflow integration for hosted attempts
- local daemon and web UI
- search over repo-native objects
- issue/comment/review objects
- policy and delegate management
- paid storage/compute hooks with x402
- optional Radicle-style peer replication

Acceptance:

- the hosted layer is a convenience, not the source of truth
- users can self-custody repo metadata and private data
- metadata survives any single clone, relay, index, or Git host failure

See [ADR 0007](../adr/0007-deployment-distribution-service-model.md) for the
deployment and distribution model.

## Immediate Next Build Order

1. Implement `rickygit init` with explicit `.git/rickydata` creation.
2. Add canonical object write/read/verify APIs.
3. Add `intent create/list/show` using repo-native objects.
4. Add `attempt start/status/abandon` with internal worktree allocation.
5. Add `run start/finish` and minimal command capture.
6. Dogfood on `rickydata_git` itself before touching other repos.

The first genuinely usable milestone is Phase 3 plus a thin GitHub bridge:
issue-bound intents, isolated attempts, patch preparation, and local inspection
of the complete evidence chain.
