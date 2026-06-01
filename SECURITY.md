# Security & Threat Model

This document covers the **runtime trust model** of rickydata_git's agent-comms
layer — what the signing/relay/sync machinery protects and what it does not. For
**secret hygiene in the repository itself** (history scrubbing, the obfuscation
guard, the publish allowlist), see [`docs/SECRET_HYGIENE.md`](docs/SECRET_HYGIENE.md).

## What the comms layer protects

Notes (`agent.note`) and the other canonical objects are **ed25519-signed**.
Signing provides:

- **Authenticity** — a reader can confirm which agent authored an object via
  `object verify` / the `signature_count` surfaced by `note inbox` and
  `note list`. Each fleet signs with its own key.
- **Tamper-evidence** — any modification to a signed body invalidates the
  signature, so altered objects are detectable on read and on relay ingress (when
  signature enforcement is enabled).

## What it does **not** protect

- **Confidentiality.** Signing is not encryption. A note's `body` is stored as
  **plaintext** in the canonical object and is readable by anyone who holds the
  bytes — the `refs/rickydata/*` in a shared repo, or the relay's object store.
  **Do not put secrets (tokens, keys, credentials) in note bodies.**

Envelope encryption (AES-256-GCM `encrypt_body` / `decrypt_body`) is implemented
in `rickydata-git-core` but **not wired into any write path**, and is **deferred
for v1**. It becomes relevant when a third party hosts the relay or sensitive
payloads must flow cross-org; the key-distribution model is designed then.

## Access control is perimeter-based

There are two cross-fleet channels, each with its own access boundary:

| Channel | Status | Access control |
|---|---|---|
| Shared private repo via `sync push`/`sync pull` | **Primary** | **GitHub repo permissions** — only collaborators on the private repo can read/write notes |
| Relay (`relay push`/`pull`) | Secondary | **`RICKYDATA_RELAY_AUTH_TOKEN`** bearer token |

**Sync path (primary).** `sync push` shells out to `git push refs/rickydata/*` and
`sync pull` fetches those refs and rebuilds the local object cache. No relay is
involved; the GitHub collaborator list is the confidentiality boundary. Keep the
shared repo **private** and the collaborator list tight.

**Relay path (secondary).** Start the relay with `RICKYDATA_RELAY_AUTH_TOKEN` set;
every route except `/health` then requires `Authorization: Bearer <token>`, and
clients pass the matching token via `--auth-token` (or the same env var) on
`relay push/pull/status`. **If the token is unset the relay is open** (it logs a
warning at startup) — only acceptable on a trusted local network.

## Key storage

Signing keys are long-lived secrets and are stored owner-only:

- `~/.rickydata/signing-keys/` is created `0700`.
- Each `<agent>.key` file is written `0600`.

Do not commit signing keys. The default key location is outside any repo; if you
point `--signing-key-file` at a path inside a repo, add it to `.gitignore`.

## Reporting a vulnerability

Please report security issues privately rather than opening a public issue.
Open a GitHub security advisory on this repository, or contact the maintainer
listed in the repository metadata. Include reproduction steps and the affected
component (core / relay / CLI). We aim to acknowledge within a few days.
