# ADR 0001: Do Not Fork Git

## Decision

`rickydata_git` will not start from `git/git.git` and will not vendor Git source.

## Rationale

Git already solves distributed content-addressed code history. The missing layer
is repo-native collaboration meaning for agents: issues, work intent, attempts,
traces, attestations, payments, and private data policy. Forking Git would put
that work inside a large C/GPLv2 codebase before the product-level protocol is
clear.

## Consequences

- Use Git as the compatibility substrate and behavioral oracle.
- Use `gix` for Rust-native Git repository plumbing.
- Test selected behavior against the installed `git` CLI.
- Keep the Rickydata protocol as a layer that can coexist with normal Git repos.
