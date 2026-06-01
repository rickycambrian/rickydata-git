# rickydata_git

Git stores code history.
rickydata_git stores verifiable collaboration meaning around code history.

This repository is the Rust foundation for an agent-native, Git-compatible
protocol. It does not fork Git. It uses Git as the compatibility substrate and
adds typed, signed, repo-native objects for work intent, isolated agent attempts,
execution traces, semantic discovery, private data policy, payment evidence, and
TEE-verifiable receipts.

> This is the public protocol + tooling distribution. It is built from a private
> development repository; deployment/operations configuration for the maintainers'
> own hosted relay is intentionally not included here.

## Current Status

Usable today as a Git-compatible sidecar for agent work:

- Rust workspace, edition 2024
- read-only Git inspection through `gix`
- canonical JSON object hashing
- typed schemas for discovery, work intent, attempts, runs, changes, notes,
  payments, and attestations
- JSON-first `rickygit` CLI
- RDL-powered command manifests
- compiled JSON Schema catalog with stable schema hashes
- explicit `rickygit init` local metadata-store creation
- ref-backed canonical object write/read/verify
- isolated **or in-place** agent attempts
- run/change/patch evidence
- signed agent-to-agent `agent.note` messages
- sync push/pull/verify for `refs/rickydata/*`
- HTTP relay push/pull/status for content-addressed object bundles

TEE, x402, encrypted object storage, and signer auth are planned interfaces, not
live integrations.

## Build

```bash
cargo build --workspace
alias rickygit="$PWD/target/debug/rickygit"
```

Use normal Git for normal code history; use `rickygit` for the agent-native
collaboration record.

## Quick start

```bash
rickygit init --repo . --json

# Start work in place (records against the current working tree, no worktree)
rickygit work start --repo . --in-place \
  --objective "Implement the issue" \
  --agent-id agent:local --idempotency-key task-1 --json

# Capture evidence
rickygit run exec --repo . --attempt-id sha256:... --json -- cargo test
rickygit change detect --repo . --attempt-id sha256:... --json
rickygit patch prepare --repo . --attempt-id sha256:... --json

# Distribute the metadata
rickygit sync push --repo . --remote origin --json
```

For an isolated attempt (a hidden worktree), omit `--in-place`.

## Agent notes (fast-lane comms)

`agent.note` objects are a signed, content-addressed agent-to-agent coordination
layer: acks, "blocked on X", "rerun done", routing nudges. Unlike an ephemeral
chat line, every note rides the same `refs/rickydata/*` + relay rails as the rest
of the work ledger, so it is recoverable into any clone and can link to the
intent/attempt/run it concerns. `to` is an agent name, `all` (broadcast), or
`kai` (a human reviewer):

```bash
rickygit note send --repo . --from agent:hermes --to claude-code \
  --text "factor-fit rerun done; artifacts at <path>" \
  --thread allpsy --ref sha256:<attempt-or-run-id> --json

rickygit note inbox --repo . --agent claude-code --json   # new since last read
rickygit note list  --repo . --to kai --json              # full history
```

See [`docs/agent-notes-comms.md`](docs/agent-notes-comms.md).

## Where data is stored

```text
.git/rickydata/cache/objects/sha256/*   local canonical object cache
.git/rickydata/cache/bundles/*          local bundle cache
.git/rickydata/worktrees/*              hidden attempt worktrees (non-in-place)
refs/rickydata/*                        Git-native recovery/distribution refs
```

The canonical recovery path is `refs/rickydata/*` plus normal Git history; the
local cache is rebuildable from those refs. A relay is an additional
distribution/indexing path, not the sole source of truth.

## Relay

Run your own relay:

```bash
RICKYDATA_RELAY_STORE_DIR=/var/lib/rickydata-git-relay \
RICKYDATA_RELAY_ADDR=0.0.0.0:8080 \
cargo run -p rickydata-git-relay
```

Optional KFDB indexing is enabled by setting `RICKYDATA_RELAY_KFDB_URL` (and the
associated auth/derive env vars). When unset, the relay is a plain Git
sidecar/object distribution service. For durable hosted bytes, set
`RICKYDATA_RELAY_GCS_BUCKET` (with bucket versioning) or put
`RICKYDATA_RELAY_STORE_DIR` on durable storage.

Sync a repository through a relay:

```bash
rickygit relay push   --repo . --url https://your-relay.example.com --repo-id myrepo --json
rickygit relay status --repo . --url https://your-relay.example.com --repo-id myrepo --json
rickygit relay pull   --repo . --url https://your-relay.example.com --repo-id myrepo --json
```

A maintainer-hosted relay is available at `https://git.rickydata.org`.

## Commands

```bash
rickygit doctor --json
rickygit init    --repo <path> --json
rickygit status  --repo <path> --remote origin --json
rickygit intent  write|list|show ... --json
rickygit attempt start|list|show|status|abandon|submit ... [--in-place] --json
rickygit work    start ... [--in-place] --json
rickygit run     exec|list|show ... --json
rickygit change  detect|list|show ... --json
rickygit patch   prepare|list|show|export|checkout|apply|retire ... --json
rickygit note    send|inbox|list ... --json
rickygit sync    push|pull|status|verify ... --json
rickygit relay   push|pull|status ... --json
rickygit object  write|read|verify ... --json
```

## Workspace

```text
crates/rickydata-git-core   canonical objects, hashes, shared security refs
crates/rickydata-git-git    Git inspection and compatibility helpers
crates/rickydata-git-agent  work intent, attempts, runs, notes, evidence schemas
crates/rickydata-git-rdl    compiled RDL command manifests
crates/rickydata-git-cli    JSON-first `rickygit` CLI
crates/rickydata-git-relay  HTTP relay and content-addressed bundle store
```

## Verification

```bash
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

## License

See [LICENSE](LICENSE).
