# Rickydata Lang Feedback From rickydata_git

This repo uses `rickydata_lang` as the reference contract layer. The first
implementation surfaced these concrete upstream needs:

- Add `CommandManifest`, not only `ToolManifest`, so CLIs and repo-native
  commands can be described without pretending to be MCP tools.
- Add intent and attempt fields to `ToolExecutionTrace`: `intent_id`,
  `attempt_id`, `base_commit`, `patch_id`, and issue/task refs.
- Add Git/repo capabilities upstream: `GitRead`, `GitObjectWrite`,
  `GitRefUpdate`, `WorktreeCreate`, `IssueRead`, `IssueWrite`, `TraceWrite`,
  `SecretDecrypt`, `TeeAttest`, and `ReceiptVerify`.
- Add filesystem-scoped read capabilities such as `LocalFileRead` so commands
  that validate or hash local artifacts can declare read effects without
  overloading Git capabilities.
- Replace source-scanned `graph --json` with compiled or AST-backed metadata.
- Implement real `effects --json`; the current placeholder is not enough for
  repo-wide discoverability.
- Add privacy and redaction metadata to manifest and trace fields.
- Add SARIF export for diagnostics.
- Add OpenTelemetry-style span export for execution traces.
- Add SCIP, LSP, and Tree-sitter adapter guidance for non-Rust languages.

The priority for `rickydata_git` is to use compiled RDL manifests immediately,
then let real CLI and provenance needs drive upstream RDL changes.
