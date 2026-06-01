# ADR 0003: Agentic Discovery Schema

## Decision

`rickydata_git` will define a language-agnostic discovery layer for code,
symbols, callable contracts, diagnostics, and evidence. RDL is the first
high-fidelity producer, but the object model is not Rust-specific.

## Initial Object Families

- `agent.discovery.v1`: repository, commit/tree, adapters, symbols, contracts.
- `agent.contract.v1`: callable contract compatible with RDL `ToolManifest`.
- `agent.diagnostic.v1`: structured compiler or checker diagnostics.
- `agent.evidence.v1`: tests, receipts, TEE evidence, and payment proofs.

## Adapter Model

The first adapter is `rust-rdl`. Future adapters should include
`typescript-tsserver`, `python-pyright`, `go-gopls`, `scip`, and `tree-sitter`.
All adapters emit the same Rickydata discovery objects.

The initial `rust-rdl` adapter is compiled into `rickydata-git-rdl`. It emits a
`DiscoveryObject` containing:

- a `LanguageAdapterManifest` for `rust-rdl`
- one `ContractManifest` per compiled `rickygit` command contract
- source-symbol references back to the manifest factory functions
- stable input and output schema hashes for each command contract

It is exposed through `rickygit discovery --repo <path> --json`, which returns a
canonical `agent.discovery` object ID plus the discovery body. The command
remains read-only: it does not write `refs/rickydata/*` or `.git/rickydata/*`.

## Standards To Borrow From

Use LSP, SCIP, SARIF, OpenTelemetry semantic conventions, W3C PROV, and MCP as
input designs. The repo-native Rickydata schema remains the authoritative model.
