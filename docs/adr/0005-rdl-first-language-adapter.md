# ADR 0005: RDL-First Language Adapter

## Decision

Use `rickydata_lang` as the first language adapter and contract layer for this
repository. Initial command contracts are emitted as RDL `ToolManifest` values
from compiled Rust code.

## Rationale

RDL already provides stable manifest hashes, input schema hashes, capabilities,
effects, fix safety, and execution trace concepts. It is the strongest current
fit for agent-native development loops.

## Constraint

The current RDL proc macro assumes capabilities exist in `rdl_core`. Because Git
capabilities are not upstream yet, this repo builds RDL manifests directly with
custom capability strings. This keeps the contracts compiled and stable without
modifying `rickydata_lang` during initialization.

## Consequences

- `rickydata-git-rdl` is the local bridge crate.
- Upstream RDL improvements are tracked in `docs/research/rickydata-lang-feedback.md`.
- Future RDL should support `CommandManifest` and repo/Git capabilities directly.
