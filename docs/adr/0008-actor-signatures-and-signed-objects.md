# ADR 0008: Actor Signatures And Signed Objects

## Decision

Every canonical object MAY carry one or more detached actor signatures.
Signatures are transport metadata and MUST NOT participate in the object
identity computation. The signing message is the same canonical JSON byte
sequence that determines `object_id`.

## Algorithm

- Default algorithm: `ed25519` via `ed25519-dalek` v2.
- Key material: raw 32-byte ed25519 seed.
- Signing key file format: 32 raw bytes, or 64 hex characters (with optional
  trailing whitespace). The file is read via the environment variable
  `RICKYGIT_SIGNING_KEY_FILE` by callers in the CLI layer.

## Signing Message

```text
canonical_json({
  "body": <body>,
  "kind": <kind>,
  "schema_version": <schema_version>,
})
```

This is byte-identical to the input fed into the SHA-256 step that produces
`object_id`. Verifiers can therefore reconstruct the signing message from any
`CanonicalObject<T>` without needing the original wire bytes.

## Object Shape

`CanonicalObject<T>` gains a `signatures: Vec<ActorSignature>` field. The field
uses `#[serde(default, skip_serializing_if = "Vec::is_empty")]` so existing
unsigned wire payloads remain byte-compatible and existing golden vectors stay
unchanged.

`ActorSignature` carries:

| Field           | Type             | Notes                                          |
|-----------------|------------------|------------------------------------------------|
| `algorithm`     | `String`         | `"ed25519"` for the default.                   |
| `public_key`    | `String`         | Hex-encoded 32-byte verifying key.             |
| `signature`     | `String`         | Hex-encoded 64-byte signature.                 |
| `signed_at_ms`  | `Option<u64>`    | Optional wall-clock for the signing event.     |
| `signer_label`  | `Option<String>` | Optional human-readable label.                 |

Both optional fields use `#[serde(default, skip_serializing_if = "Option::is_none")]`.

## Backward Compatibility

- Objects produced before this ADR have no `signatures` field on the wire. They
  deserialize into `CanonicalObject<T>` with an empty `signatures` vector.
- The `object_id` of an unsigned object is identical before and after this
  change. The existing canonical hash vector fixture continues to pass.
- Adding, reordering, or removing signatures does not change `object_id`. A
  relying party that requires signatures enforces that policy outside of the
  identity computation.

## Verification

`verify_signature` recomputes the canonical signing message from `kind`,
`schema_version`, and `body`, decodes the hex public key and signature, and
calls `VerifyingKey::verify`. Tampered bodies produce a different signing
message and fail verification.

## Out Of Scope For Phase 1

- Wire-format signing of git refs or pack contents (Phase 2 / Phase 5).
- TEE-issued signatures and receipt binding (Phase 8 and ADR 0006).
- Key rotation, revocation lists, and trust bundles.
