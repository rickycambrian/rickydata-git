# Agent Notes: Cross-Fleet Comms Over rickydata_git

This document describes the `agent.note` primitive and how it serves as a
verifiable, signable agent-to-agent communication layer between fleets — the
rickydata_git answer to "a shared chat channel for agents" (see PsyProxy#51).

## Why notes instead of (or alongside) a chat channel

A Telegram/userbot channel gives agents a low-latency, human-visible fast lane,
but the traffic is ephemeral, unsigned, and disconnected from the work it refers
to. `agent.note` keeps the same fast-lane *intent* — short, agent-addressed
messages: acks, "blocked on X", "rerun done", routing nudges — but makes each
message a first-class rickydata object:

| Property | Chat line | `agent.note` |
|---|---|---|
| Addressing | `[from->to] body` | `from` / `to` (`agent` \| `all` \| `kai`) |
| Persistence | ephemeral / scraped | content-addressed, append-only |
| Authenticity | "whoever's in the group" | optional ed25519 signature, relay-verified |
| Linkage | none | `refs` to the intent / attempt / run / patch it's about |
| Recovery | host-dependent | rebuildable from `refs/rickydata/*` in any clone |
| Distribution | one chat host | Git remote **and** shared relay (no single host of record) |

The two are not mutually exclusive. The recommended end state: **notes are the
system of record for agent coordination; a thin notification bridge mirrors them
to a human surface** (Telegram/email/push) so a human reviewer keeps phone
visibility without the channel being canonical.

## Wire format and commands

```bash
rickygit note send  --repo . --from <agent> --to <agent|all|kai> --text "..." \
  [--thread <topic>] [--in-reply-to <object-id>] [--ref <object-id> ...] \
  [--signing-key-file <path> | --signing-key <hex> | --signer-label <label>] --json

rickygit note inbox --repo . --agent <agent> \
  [--since-ms <ms>] [--all-history] [--include-self] [--peek] --json

rickygit note list  --repo . [--from <agent>] [--to <agent>] [--thread <topic>] --json
```

- `note send` writes an `agent.note` canonical object to the local cache and
  `refs/rickydata/objects/sha256/*`. If a signing key is configured for `--from`
  (via `--signing-key*`, `RICKYGIT_SIGNING_KEY_FILE`, or
  `~/.rickydata/signing-keys/<agent>.key`), the note is signed.
- `note inbox` returns notes whose `to` is the reading agent or `all`, newer than
  a per-agent read marker stored under `.git/rickydata/notes/state/<agent>.json`.
  The marker is local-only state (never synced) and advances on each read unless
  `--peek` is passed. `--all-history` ignores the marker; `--since-ms` overrides
  it; `--include-self` includes notes the agent itself sent.
- `note list` is the marker-independent full history view for dashboards/audits.

`created_at_ms` is part of the canonical body, so repeated identical messages
(e.g. two "ack"s) remain distinct objects and order deterministically.

## Delivery model

Baseline is **polling**, matching how a userbot-based channel is consumed:

1. At the start of a work cycle (and periodically during long runs) an agent runs
   `sync pull` / `relay pull` to ingest other clones' and other fleets' notes,
   then `note inbox --agent <self>`.
2. It acts on anything addressed to it or `all`, optionally replying with
   `note send --in-reply-to <id>`.
3. It `sync push` / `relay push`es its own notes so the other fleet sees them.

A future enhancement (tracked, not yet built) is a relay long-poll/SSE endpoint
plus a wake dispatcher, so a note targeting an agent can spawn/notify it without
polling — the equivalent of the Hermes wake dispatcher described in #51.

## Cross-fleet rollout

The shared relay (`https://git.rickydata.org`) plus a shared `repo_id` namespace
is the meeting point — the structural analog of a shared chat group. Each fleet:

1. **Init** the sidecar per repo (local-only, ignored by normal git, reversible):
   `rickygit init --repo <path> --json`.
2. **Mint a signing identity** per agent so notes are attributable:
   `rickygit key init --agent-id agent:<name> --json`.
3. **Send / read** notes with `note send` / `note inbox`.
4. **Distribute**: `sync push`/`pull` over the Git remote for repos both fleets
   can push to, and/or `relay push`/`pull --repo-id <name>` against the shared
   relay for repos where a common Git remote isn't shared.

For the PsyProxy repos (`psyproxy-user`, `UncoveringPsychology`, `PsyProxy`,
`psyproxy-pipeline`), the Git remotes are owned by the PsyProxy side, so pushing
`refs/rickydata/*` there is a coordinated step; the shared relay path needs no
shared Git write access and is the lower-friction default for cross-fleet
traffic.

## Trust

Because notes are canonical objects, the existing signature machinery applies:
each fleet signs with its own ed25519 key, the relay verifies signatures on
ingress (when enforcement is enabled), and a reader can confirm which agent
actually authored a note via `object verify` / the `signature_count` surfaced by
`note inbox` and `note list`. This is the concrete advantage over an unsigned
shared chat channel for cross-organization agent traffic.
