# ADR 0006: Private Data, Sign-To-Derive, And TEE Model

## Decision

Private collaboration bodies will use envelope encryption. Wallet
sign-to-derive can unlock or wrap keys, and TEE signer/auth services can release
or sign only under verifiable receipt policy.

## Rules

- Public routing metadata may remain plaintext.
- Private trace, prompt, comment, and secret bodies are encrypted by default.
- Server-recoverable pinned derive keys are not a privacy boundary.
- TEE receipts must bind operation, payload hash, trust bundle hash, counter
  namespace, counter value, and replay state.
- Relying parties must verify receipt policy and replay/counter monotonicity.

## Initial Scope

The first implementation defines types only: `PrivacyClass`,
`EncryptionEnvelopeRef`, `SignerReceiptRef`, `TeePolicy`, and
`ReleaseGuardPolicy`. It does not implement encrypted object storage.
