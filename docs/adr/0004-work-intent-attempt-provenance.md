# ADR 0004: Work Intent And Attempt Provenance

## Decision

Agent work must bind to issue or task intent before meaningful edits happen.
The core flow is:

```text
Issue/Task -> WorkIntent -> AgentAttempt -> AgentRun -> ChangeEvidence -> Patch/Commit
```

## Rationale

Retrofitting issue links from commit messages or pull request text loses meaning.
Agents need durable provenance while work happens: base commit, objective,
capabilities, budget, privacy policy, trace ID, diagnostics, and test evidence.

## Consequences

- `WorkIntent` requires at least one issue or task reference.
- `AgentAttempt` records isolated work under an immutable intent.
- Branches and worktrees are implementation primitives, not the collaboration
  data model.
- Merge/integration policy can reason over attempts instead of long-lived shared
  branches.
