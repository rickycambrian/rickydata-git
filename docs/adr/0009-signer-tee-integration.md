# ADR 0009: Signer TEE Integration

## Status

Proposed

## Context

The signer TEE VM runs at `10.73.0.2:8787` (AMD SEV-SNP, production mode) in the
`your-gcp-project` GCP project. It is accessible from the VPC but not the public
internet. This ADR records the integration contract between `rickygit` CLI commands
and the signer TEE service.

## Decision

### Network Topology

- Internal IP: `10.73.0.2:8787`
- Access: VPC-only (Cloud Run services and GCE VMs in the same VPC can reach it)
- Protocol: HTTP/1.1 JSON-RPC style
- TLS: not required within VPC (trust boundary is the VPC firewall)

### API Contract

Rust integrations must use the shared `rickydata-auth-client` crate from
`https://github.com/rickycambrian/rickydata-auth` instead of hand-rolling
endpoint paths. `rickygit` uses the blocking client variant because the CLI is
currently synchronous.

| Endpoint | Method | Purpose |
|----------|--------|---------|
| `/health` | GET | Liveness probe |
| `/security` | GET | Signer posture summary |
| `/security/production-readiness` | GET | Production readiness blockers |
| `/security/storage-summary` | GET | Durable signer-owned state summary |
| `/.well-known/jwks.json` | GET | Receipt verification public keys |
| `/attestation` | GET | Current signer attestation summary |
| `/attestation/report` | GET | SEV-SNP report bundle |
| `/attestation/challenge` | POST | Nonce-bound attestation challenge |
| `/rpc/wallet.create` | POST | Create wallet through signer policy |
| `/rpc/sign` | POST | Sign through signer authorization and policy |
| `/rpc/policy.update` | POST | Update signer policy through policy quorum |

### Integration Points in rickygit

1. `rickygit doctor --tee-url <url>` — probe `/health` (Phase 3, implemented)
2. `rickygit receipt verify --object-id <id> --tee-url <url> --json` — probe signer health through `rickydata-auth-client`
3. `rickygit sync verify --tee-url <url>` — optionally validate receipts live
4. `rickygit proof --tee-url <url>` — include TEE validation in proof report

### Offline Fallback

When `--tee-url` is not provided or the TEE is unreachable, commands fall back to
the existing stub behavior (receipts are stored but not validated live). This
preserves local-first operation for development and CI.

### Environment Variables

| Variable | Purpose |
|----------|---------|
| `RICKYGIT_TEE_URL` | Default TEE endpoint when `--tee-url` is not passed |
| `RICKYDATA_AUTH_SIGNER_URL` | Shared signer endpoint env used by `rickydata-auth-client` |
| `RICKYDATA_AUTH_REQUIRED` | Must remain unset/false for optional `rickygit` diagnostics; set true only for commands that explicitly require Steward |

## Consequences

- TEE integration remains optional for diagnostics — all commands work without it
- The signer VM must be reachable from wherever `rickygit` runs for live validation
- Receipts stored in objects can be validated later when TEE access is available
- CI pipelines that don't run in the VPC skip live TEE validation
- Privy/default app auth remains independent of this diagnostic signer probe

## Open Questions

1. Which `rickygit` operations should become Steward-required rather than diagnostic-only
2. Counter namespace conventions for multi-repo deployments
3. How `rickygit` should store and verify full signer receipt evidence for private object operations
