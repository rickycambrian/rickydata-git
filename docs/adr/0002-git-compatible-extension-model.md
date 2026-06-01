# ADR 0002: Git-Compatible Extension Model

## Decision

Rickydata metadata must not break normal Git clients. The first write path was
explicit initialization through `rickygit init`, which creates the local store
layout and reserved ref directories.

Canonical object writes are now stored twice:

- `.git/rickydata/cache/objects/sha256/*` keeps a local deterministic cache.
- `refs/rickydata/objects/sha256/*` points at Git blob objects containing the
  same canonical bytes, allowing normal Git remotes to distribute the objects.

Future higher-level metadata will also be anchored under `refs/rickydata/*`, and
local-only indexes may live under `.git/rickydata/cache/*`.

## Reserved Future Namespaces

```text
.git/rickydata/cache/*
.git/rickydata/cache/objects/sha256/*
.git/rickydata/cache/bundles/*
refs/rickydata/objects/sha256/*
refs/rickydata/intents/*
refs/rickydata/attempts/*
refs/rickydata/runs/*
refs/rickydata/discovery/*
refs/rickydata/policies/*
```

## Consequences

- Only explicit write commands may mutate `.git` metadata. `rickygit init`
  creates directories and a store `VERSION` file; it must be idempotent and keep
  normal Git status clean.
- Object write/read/verify commands must keep normal Git status clean and pass
  `git fsck`.
- Deleting the local object cache must be repairable from fetched
  `refs/rickydata/objects/*` refs.
- Worktrees are implementation details for isolated attempts, not the public
  collaboration abstraction.
