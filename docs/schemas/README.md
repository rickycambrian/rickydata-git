# Schema Notes

The Rust types in `crates/rickydata-git-core` and `crates/rickydata-git-agent`
are the source of truth for the initial schemas.

Emit the compiled schema catalog with:

```bash
cargo run -p rickydata-git-cli -- schema --json
```

Each emitted schema includes a stable `sha256:` hash in the catalog's
`schema_hashes` map so language adapters and RDL command manifests can refer to
schema contracts without depending on file paths.

Initial object families:

- `agent.discovery.v1`
- `agent.contract.v1`
- `agent.intent.v1`
- `agent.attempt.v1`
- `agent.run.v1`
- `agent.change.v1`
- `agent.attestation.v1`
- `agent.payment.v1`

Hashing rule:

```text
object_id = sha256(canonical_json(kind + schema_version + body))
body_hash = sha256(canonical_json(body))
```

Transport metadata, signatures, storage locations, and timestamps outside the
body are excluded from stable object identity.

Golden hash fixtures live in `fixtures/canonical-hash-vectors.json`. Other
language adapters should reproduce those canonical JSON strings, body hashes,
and object IDs before their outputs are trusted as protocol-compatible.
