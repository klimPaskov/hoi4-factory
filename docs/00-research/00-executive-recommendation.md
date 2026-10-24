# Executive recommendation

Status: **Research recommendation, partly superseded**. This page records the v02 research recommendation. On 2026-10-24 the maintainer selected a public hosted web application with a Rust server, a native Rust agent loop, a React browser client, Docker deployment and GitHub integration. See the [maintainer decision record](../02-architecture/maintainer-decisions-2026-10-24.md). The findings below still apply. The desktop and Pi worker parts of the original recommendation do not.

Original recommendation: build a local desktop workspace around reused HOI4 Mod Setup services, a small replaceable native model adapter and host-owned integrity. The leading candidate was Tauri/React with a Rust workspace host and packaged Node worker embedding Pi, with Electron and embedded OpenCode as the alternative.

This is a research-backed recommendation, not a completed implementation or measured runtime winner. The [architecture comparison](03-candidate-comparison.md) and [ADR table](../02-architecture/options-and-decisions.md) retain the tradeoffs and reversal triggers.

## Why this is the leading direction

Mod Setup is already a substantive onboarding asset. Its portable Rust planning core and existing desktop scanner/transaction services cover real new/import behavior, conflict review, source closure and native apply/recovery. A shared facade needs extraction or adaptation. A public onboarding HTTP API was not verified. Reuse those boundaries rather than building a second importer. [SRC-A01 through A08](02-existing-assets-audit.md)

Pi exposes embeddable libraries with a comparatively narrow adaptation surface. Current source also includes pi-durable, which deserves a separate experiment instead of being overlooked. Its recovery identity does not automatically match this product's workspace/source requirements. OpenCode now has a real in-process SDK, so dismissing it as CLI-only would be inaccurate. Its persistence and at-least-once restart assumptions require explicit host ownership. [SRC-B01/B02](03-candidate-comparison.md)

The product should own source preconditions, grants, receipts, task transitions, Git publication and reconciliation. That keeps the runtime replaceable and makes visual edits and autonomous edits converge on the same authority. It also prevents a library's session transcript from being mistaken for proof that an external effect completed.

## What to build first

S01 establishes the future Claude-first developer workspace and runs bounded architecture probes. S02 reuses Setup for real native workspaces. S03 adds the independent model loop and durable effect broker. S04 delivers a small focus branch through visual, hybrid and autonomous modes with source evidence, review, owned commit and restore.

S05 strengthens context and model handoff before a narrow pilot. S06 expands contributor collaboration and content. S07 adds map inspection, structured import and bounded editing. S08 tests distribution, restore, security and actual supported profiles. The [roadmap](../04-implementation-stages/ROADMAP.md) makes the dependencies and optional narrower pilot explicit.

A full map suite, many-agent swarm, hosted service and general world generator would delay evidence for the essential workflow. Their intended contracts and promotion tests remain in the specs and backlog.

## Findings that change the plan

| Finding | Architectural consequence |
|---|---|
| Setup's exact audited HEAD has a failed Windows recovery candidate reporting unequal restoration inventories | Reproduce and diagnose in S01 before trusting production apply |
| Desktop scanner and native transactions sit outside the portable Setup crate | Reuse the actual services through an explicit binding or extraction |
| Agent Tools offers preserving focus operations, while comparison printers normalize source | Use verified span-preserving writes and refuse unsupported edits |
| Geography-model synthesis is explicitly unavailable despite map previews | S07 imports structured files through a tested adapter, full generation remains deferred |
| Native Live Tools is qualified for a narrow Linux executable profile | Separate platform/build capability from optional user-observed game checks |
| Pi core hooks can mutate validated inputs, and runtime recovery assumptions vary | Revalidate final arguments in the host and reconcile effects with exact identity |
| Current OpenCode embeds in-process and defaults to an in-memory database | Keep it as a serious alternative, configure persistence and ownership explicitly |
| Actual Agentic TOML has no model-context-window override | Do not import historical context numbers or confuse instruction bytes with model tokens |
| Codex's documented repository skill path is .agents/skills | Ship required .codex mirrors plus supported discovery copies with deterministic drift checks |
| Nova's public license is restrictive and its implementation is absent | Use it only as a UX comparison, with no code or asset reuse |

The exact primary paths, revisions and qualifications are in [the source registry](01-research-sources.md). A source concern is not a reproduced runtime defect unless a specific execution is recorded. The Setup CI failure is an observed upstream result, with its cause still unknown.

## Context, learning and collaboration

Use native Git files for source, accepted canon and engineering guidance. Use a transactional local store for operational state and hashes for artifacts. Derived indexes are rebuildable. Beacon's reviewed-memory and failure-test patterns, Entire's Git ref publication, agentglass's read projections and MFS's cancellation reconciliation inform distinct boundaries without requiring four additional services. Glass Town is the separate factory article, not agentglass. [Context research](06-context-protocols-and-evaluation-research.md)

The product's runtime learning and the development team's engineering learning are different systems. Both retain evidence and review, but runtime feedback cannot rewrite project safety policy or approved design. The shipped engineering workflow records a small useful update or a justified no-change decision after substantive work.

Start collaboration with owned worktrees, explicit semantic collisions and attributable reviews. Measure whether a second worker improves accepted output before scaling. A deterministic parser, validator or merge gate is preferable to another agent when the operation already has a precise algorithm.

## What the package enables next

A future Claude Code session can read the shared guidance, select S01, merge the safe starter files, run specified probes and report a concrete decision without inventing the product. The 196-field migration preserves real upstream provenance and distinguishes Codex, Claude and future runtime settings.

The research completed source inspection and package-level checks. It did not execute provider comparisons, game scenarios, runtime suites or live client behavior. The [evaluation matrix](../05-verification/EVALUATION-MATRIX.md) provides the required two-API-plus-local trials with transparent outcomes, cost limits and recovery tests. The immediate next step is review of this completed package, then bounded S01 authorization.

