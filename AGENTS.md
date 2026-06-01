# rickydata_git — Agent Instructions

This repository is the Rust foundation for an agent-native, Git-compatible
collaboration protocol. Git remains the compatibility substrate; rickydata_git
adds repo-native meaning around work intents, attempts, execution traces,
discovery, private objects, signed agent notes, payment evidence, and
attestations.

This is the public protocol + tooling distribution. It is built from a private
development repository; deployment/operations configuration for the maintainers'
own hosted relay is intentionally not included here.

## Non-negotiable rules

- Do not fork or vendor `git/git.git`. Git is the substrate, not a dependency to
  rewrite.
- Only write rickydata metadata through explicit, tested write commands. New
  object/ref writes need ADR + test coverage before implementation.
- Keep every command JSON-first and machine-readable.
- Prefer compiled manifests and typed schemas over source scanning.
- Treat agent traces as private by default.

## Build & verify

```bash
cargo build --workspace
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo run -p rickydata-git-cli -- doctor --json
cargo run -p rickydata-git-cli -- manifest --json
cargo run -p rickydata-git-cli -- schema --json
```

Or run the bundled gate:

```bash
./scripts/verify.sh
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

The `rdl-core` contract layer is consumed as a git dependency on the public
[`rickydata-lang`](https://github.com/rickycambrian/rickydata-lang) mirror.

## Signer / TEE

TEE-backed receipt verification is provided by a separate signer client that
lives outside this public distribution. The CLI builds and runs fully without
it; the signer-dependent paths (`receipt verify --tee-url`, `doctor`'s
`signer_tee_reachable`) report the signer as unreachable in this build.

## Secret hygiene

This repository ships with an agentic secret-obfuscation setup. Run
`scripts/setup-secret-scrub.sh` after cloning to install the local git filters
and pre-commit guard. Never commit real credentials; placeholders live in
`.git-secrets-map.example.json`. See [docs/SECRET_HYGIENE.md](docs/SECRET_HYGIENE.md).

## Reference

- Architecture decisions: [docs/adr/](docs/adr/)
- Schemas: [docs/schemas/](docs/schemas/)
- Roadmap: [docs/roadmap/usable-rickydata-git.md](docs/roadmap/usable-rickydata-git.md)
- Agent notes comms: [docs/agent-notes-comms.md](docs/agent-notes-comms.md)
