# ADR 0007: Deployment And Distribution Service Model

## Status

Accepted initial direction.

## Problem

The local protocol is not enough by itself. To use Rickydata Git across real
projects, agents, machines, and organizations, we need a deployed service model
that answers:

- where collaboration metadata lives
- how multiple clones synchronize without data loss
- which service is canonical
- how private bodies are protected
- how agent execution, indexing, and Git compatibility fit into the existing
  Rickydata stack

## Decision

Rickydata Git will be local-first and Git-compatible. The deployed service is a
relay, indexer, and policy layer, not the sole source of truth.

The source of truth for public collaboration meaning is:

```text
Git commits and trees
+ refs/rickydata/*
+ canonical signed Rickydata objects
+ optional content-addressed object cache
```

The deployed service should make this easier to sync, search, verify, and
protect, but normal Git remotes and local clones must remain sufficient to carry
the durable collaboration record.

## Deployment Shape

Initial deployed service name:

```text
rickydata-git-relay
```

Initial GCP target:

```text
GCP project: your-gcp-project
Region: us-central1

Clients
  - rickygit CLI
  - Agent Gateway workflows
  - future web UI

Public HTTPS ingress
  - rickydata-git-relay
  - wallet/JWT auth
  - x402/payment hooks when enabled
  - JSON-first APIs

Durable storage
  - Git remote refs: refs/rickydata/*
  - GCS versioned buckets for content-addressed object bundles and backups
  - KFDB for searchable/indexed projections, not canonical state
  - optional ScyllaDB/Cockroach/Postgres state only for service-local queues,
    leases, and idempotency

TEE/private services
  - rickydata_auth signer TEE for sign-to-derive, wallet signing, receipts
  - Agent Gateway TEE for agent execution and BYOK/tool orchestration
  - MCP Gateway TEE for tool execution and execution proofs
  - KFDB private data TEE where protected indexing or private metadata release
    is required
```

## Responsibilities

### `rickygit` CLI

Owns local protocol operations:

- create/read/verify canonical objects
- write/read `.git/rickydata/cache/*`
- update/read `refs/rickydata/*`
- start local intents and attempts
- inspect sync status
- verify signatures, hashes, and receipts

The CLI must work without the relay for local-only or Git-remote-only use.

### `rickydata-git-relay`

Owns distribution convenience and high-availability:

- accept signed object bundles from clients/agents
- validate canonical hashes and signatures before accepting
- mirror accepted objects into a versioned content-addressed store
- optionally update hosted `refs/rickydata/*` for repos it manages
- serve object bundles to other clones
- provide idempotent bulk APIs for agents
- emit ingestion events for KFDB indexing
- maintain lease/idempotency state for hosted agent attempts
- expose verification and audit endpoints

The relay must not be the only copy of collaboration state.

### KFDB

Owns query and discovery projections:

- index repos, commits, files, symbols, contracts, intents, attempts, runs, and
  change evidence
- power search, dashboards, semantic discovery, and analytics
- support agent planning over repo-native evidence

KFDB is not canonical for Rickydata Git objects. It must be rebuildable from
Git refs and object bundles.

### Agent Gateway

Owns hosted agent execution:

- authenticated agent sessions
- BYOK/model/tool orchestration
- budgets and x402-aware tool calls
- Canvas workflows for issue-to-attempt execution
- calling `rickygit` inside controlled workers
- producing AgentRun and ChangeEvidence objects

Agent Gateway should verify relay/auth receipts before trusted writes.

### `rickydata_auth` Signer TEE

Owns private key/signing trust:

- wallet signing
- sign-to-derive key operations
- policy-checked decrypt/sign receipts
- monotonic counters and replay protection
- attested signer identity

The relay, Agent Gateway, and MCP Gateway are relying parties. They do not host
the signer keys.

## Distribution Model

Rickydata Git should support three sync paths:

### Path 1: Plain Git Remote

Use ordinary Git remotes to push and fetch `refs/rickydata/*`.

This is the baseline compatibility path and must work even if no Rickydata
hosted service exists.

```text
git push origin refs/rickydata/*:refs/rickydata/*
git fetch origin 'refs/rickydata/*:refs/rickydata/*'
```

### Path 2: Rickydata Relay

Use `rickygit sync push/pull` to exchange signed object bundles with the relay.

The relay can then mirror refs, store bundles, and notify KFDB indexers. This is
the preferred path for agent-heavy usage because it can support bulk,
idempotent, JSON-first operations without forcing agents through GitHub APIs.

### Path 3: Peer/Archive Replication

Export/import canonical object bundles to another machine, object store, or
peer network.

This gives us disaster recovery and future Radicle-like or IPFS-like
replication without making that a requirement for the first usable version.

## Data Loss Avoidance

The service model must assume any single layer can fail.

Required protections:

- every object is content-addressed by canonical hash
- every mutation is append-only or tombstoned, never silently overwritten
- every accepted relay write has an idempotency key
- `refs/rickydata/*` updates are signed and verify expected previous state
- relay persists accepted bundles to versioned object storage before returning
  success
- KFDB projections are rebuildable from object bundles and Git refs
- local clones can export full Rickydata object bundles
- hosted relay has periodic object-store backup and restore tests
- monotonic counters for signer/TEE receipts are stored in rollback-resistant
  durable state
- private bodies remain ciphertext outside approved TEE/key-release paths

Minimum replication target:

```text
local clone
+ normal Git remote carrying refs/rickydata/*
+ relay content-addressed object store with versioning
+ KFDB searchable projection rebuilt from the above
```

## Private Data Model

The relay may store and route encrypted private bodies, but it is not a privacy
boundary.

Rules:

- public routing metadata can remain plaintext
- trace bodies, prompts, secrets, private issue text, and private review
  comments are encrypted by default
- decrypt/sign operations require `rickydata_auth` TEE receipts
- release policies bind operation, payload hash, actor, repo, object kind,
  expiry, and monotonic counter
- relying parties verify trust bundle hash, signer receipt, counter state, and
  replay status before accepting private data release

## Where This Fits In The Existing Stack

Best initial placement:

1. Keep protocol/client code in `rickydata_git`.
2. Add the hosted relay service in `rickydata_git` as a Rust crate once the
   local object store exists.
3. Expose relay operations as an MCP/Agent Gateway tool after the CLI contract
   is stable.
4. Use KFDB as the projection/index backend after object bundle formats are
   fixed.
5. Use `rickydata_auth` for sign-to-derive and receipts, not for relay storage.
6. Reuse Agent Gateway for hosted agent execution rather than embedding LLM
   execution in the relay.

This keeps trust boundaries clean:

- `rickydata_git`: protocol, CLI, relay, object distribution
- `knowledgeflow_db`: query/index/projection
- `mcp_deployments_registry`: agent/tool orchestration and payment gateway
- `rickydata_auth`: minimal TEE signer/auth boundary

## Service APIs

Initial relay APIs should be JSON-first:

```text
GET  /health
GET  /v1/repos/:repo_id/status
POST /v1/repos/:repo_id/bundles/validate
POST /v1/repos/:repo_id/bundles/push
POST /v1/repos/:repo_id/bundles/pull
GET  /v1/repos/:repo_id/objects/:object_id
POST /v1/repos/:repo_id/refs/compare-and-swap
POST /v1/repos/:repo_id/verify
POST /v1/repos/:repo_id/index/rebuild
```

The first implemented relay slice is `crates/rickydata-git-relay`: it validates
canonical object envelopes, stores objects by content address, supports
idempotent bundle pushes, serves missing-object pulls, and exposes those
semantics through a local Axum router/binary. The pending deployment slice is to
swap the local file store for versioned GCS storage and run the same API on
Cloud Run.

Agent-oriented APIs should support bulk/idempotent operations:

```text
POST /v1/repos/:repo_id/intents/bulk-create
POST /v1/repos/:repo_id/attempts/start
POST /v1/repos/:repo_id/runs/append
POST /v1/repos/:repo_id/change-evidence/append
```

All write APIs must accept:

- idempotency key
- actor identity
- base/ref precondition when relevant
- canonical object hash
- signature or signer receipt where required
- dry-run/validate-only mode

## Deployment Phases

### Phase A: Local + Plain Git Remote

No hosted relay. Prove that `refs/rickydata/*` can carry intents, attempts,
runs, discovery, and change evidence across clones.

### Phase B: Relay Alpha

Deploy a stateless-ish Cloud Run relay with GCS versioned object storage and
wallet/JWT auth. KFDB indexing can be asynchronous.

### Phase C: Agent Gateway Integration

Expose relay operations to Agent Gateway workflows. Hosted agents can start
attempts, run work, and push evidence through relay APIs.

### Phase D: TEE Private Data Integration

Integrate `rickydata_auth` receipts for encrypted traces and private object
release. Add replay/counter verification.

### Phase E: Dedicated Git-Compatible Remote

Add Git smart HTTP/SSH support only after the object/relay semantics are stable.
Until then, ordinary Git remotes plus `rickygit sync` are enough.

## Open Questions

- Should the first relay deploy to Cloud Run or GKE?
  - Current recommendation: Cloud Run for the stateless API, GCS for object
    bundles, KFDB for projection. Move to GKE only when long-lived workers,
    custom Git SSH, or heavier queue/index workloads require it.
- Should public repos use GitHub as the Git remote initially?
  - Yes. GitHub can remain the Git commit transport while Rickydata objects are
    distributed through `refs/rickydata/*` and/or relay bundles.
- Should KFDB store canonical objects?
  - No. KFDB can store copies and indexes, but canonical recovery must come
    from Git refs and content-addressed bundles.
- Should the relay run in a TEE?
  - Not initially required for public metadata. Private decrypt/sign operations
    must use `rickydata_auth` TEE. A future relay TEE can strengthen receipt
    production, but it should not mix with signer custody.
