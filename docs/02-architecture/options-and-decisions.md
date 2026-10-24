# Architecture options and decisions

Status: **Revised by maintainer decisions on 2026-10-24**. The [maintainer decision record](maintainer-decisions-2026-10-24.md) selects a native Rust agent runtime, a public hosted web application deployed as Docker images, native GitHub integration, a server-held game reference and an AGPL-3.0 license. ADR-002 is superseded. Candidates A and B below are kept as the research record and as behavioral reference designs. No product architecture has been implemented. S01 experiments still verify the selected direction before dependencies are committed.

Read [candidate comparison](../00-research/03-candidate-comparison.md), [existing assets](../00-research/02-existing-assets-audit.md), [risks and experiments](../05-verification/RISKS-DECISIONS-AND-OPEN-QUESTIONS.md) and [roadmap](../04-implementation-stages/ROADMAP.md).

## Decision criteria

The most important criteria are reuse of Mod Setup, independent model access, source integrity, recoverable effects, clear UI/source synchronization and a manageable deployment. Library API availability is necessary but insufficient. A runtime's transcript store, permission dialog or successful test file does not automatically meet the product's workspace requirements.

The comparison below is an engineering judgment based on inspected interfaces. “Lower” or “higher” describes relative integration scope. It is not a measured cost or benchmark.

## Selected direction: Rust server, Rust agent and browser client

The Rust server owns workspace identity, grants, receipts, Git, GitHub and recovery. A Rust agent crate in the same Cargo workspace runs the model loop and requests every effect through the host tool broker. A React and TypeScript browser client presents projections and submits typed requests. The service ships as Docker images, uses PostgreSQL for operational state, an object store or volume for artifacts, and persistent volumes for Git repositories and the read-only game reference. Mod Setup's portable core runs on the server, as ADR-001 requires. The [recommended architecture](recommended-architecture.md) describes the components.

This direction keeps the strongest property of Candidate A, direct Rust reuse of Setup with a narrow runtime boundary, and removes its Node worker. It inherits the costs of the small custom loop described below. Those costs are accepted by ADR-011 and measured in S01.

## Candidate A (research record): Tauri host with a narrow Pi adapter

Use a Tauri desktop shell and React/TypeScript UI, a Rust workspace service that reuses Mod Setup's portable planning core plus extracted or adapted desktop scanner and transaction services, and a packaged Node worker embedding Pi libraries. Keep the host's SQLite state, approvals and filesystem transaction journal authoritative. Reuse existing HOI4 Agent Tools behind versioned adapters.

Pi `pi-ai` and `pi-agent-core` are the initial runtime boundary. Evaluate `pi-durable` separately for conversation execution persistence. It must either be a subordinate projection with an explicit bridge or supply one well-defined authority boundary. Two independently authoritative task stores are unacceptable.

Source evidence: [Pi embedding and durable tool handling](https://github.com/earendil-works/pi/blob/8a2ebd38549c3521a02db0e5d758a24320e97f94/packages/durable/src/harness/tool.ts), [Mod Setup audited revision](https://github.com/klimPaskov/HOI4-Mod-Setup/tree/4488a418cad56a158273cb66c4ce7794696b7ed1) and [Tauri external-binary documentation](https://v2.tauri.app/develop/sidecar/). The packaging path is documented, with no packaged prototype run here.

Strengths are direct reuse of Rust onboarding, a narrow runtime API and a clear trusted mutation boundary. Costs are a Rust/Node process boundary, signed binaries per supported platform, protocol evolution and two-language debugging. Node worker isolation must be designed and tested. Tauri's frontend capabilities do not sandbox every arbitrary sidecar operation.

## Candidate B (research record): Electron application with embedded OpenCode

Use an Electron application with an isolated React renderer, an embedded OpenCode SDK in a controlled Node process, and the same Rust Setup/workspace service reached through a versioned local interface. Persist OpenCode's database explicitly and keep effect authorization and workspace receipts in the host contract.

Current source supports in-process embedding through [OpenCode's SDK host](https://github.com/anomalyco/opencode/blob/02671e66454260d77fe04d63cf5eb94e76921dee/packages/sdk/src/internal/host.ts). It is not limited to launching the OpenCode CLI. The host defaults to an in-memory database, so a durable configured path is required. Its [restart implementation](https://github.com/anomalyco/opencode/blob/02671e66454260d77fe04d63cf5eb94e76921dee/packages/core/src/session/execution/restart.ts) explicitly has at-least-once recovery concerns. A factory needs single-owner fencing and effect reconciliation around that behavior.

Strengths are a richer embedded session system and a broadly TypeScript application. Costs include a larger inherited surface, native dependency packaging, explicit permission overrides and a careful split between OpenCode session state and product state. Electron's documented [renderer security requirements](https://www.electronjs.org/docs/latest/tutorial/security) remain mandatory implementation work.

## Comparison

| Dimension | Candidate A | Candidate B |
|---|---|---|
| Mod Setup reuse | Portable core and existing desktop services in host | Same core and desktop services behind Rust interface |
| Agent integration | Small Pi library boundary | Richer OpenCode embedded host |
| Initial durable authority | Product host, optional subordinate Pi durability | Product host plus explicit OpenCode database |
| UI isolation | Tauri capabilities plus host checks | Electron context isolation, renderer sandbox and host checks |
| Provider portability | Pi adapters plus capability probes | OpenCode adapters plus capability probes |
| Biggest correctness risk | Hook mutation and replay identity across worker restarts | At-least-once recovery and competing session ownership |
| Packaging work | Rust host and Node worker for each platform | Electron plus native runtime dependencies and Rust service |
| Replacement cost | Smaller adapter surface | Broader session and plugin integration to disentangle |
| Measured end-user result | Unknown | Unknown |

## Other approaches considered

**OpenHands SDK in a Python service** is a credible native alternative. Its inspected Python code has durable event structures, cooperative cancellation and atomic file-write support. The source record is [SRC-B03](../00-research/01-research-sources.md). It adds Python packaging alongside the Rust onboarding core and UI stack. Choose it if experiments show a clear advantage in required tools or remote execution, rather than selecting it because its framework name suggests durability.

**A small custom loop** around a provider abstraction offers complete policy control and a minimal conceptual core. It also makes the team responsible for stream assembly, malformed tool calls, provider errors, context conditioning, cancellation, usage accounting and recovery. The first prototype may use a tiny loop as a baseline, but a production custom runtime needs evidence that its maintenance cost is lower than an adapted library.

**Forking a complete harness** gives control over every internal behavior, at the cost of permanent upstream merge work, security maintenance and coupling to UI or CLI assumptions. A fork is justified only by a documented blocking defect that a supported adapter or small upstream contribution cannot solve. The product must retain a migration path to another runtime even then.

**Embedding a factory platform** such as Fusion, Gas City or Mastra would supply more orchestration. Fusion already embeds Pi and has useful transaction and fencing tests. These projects are therefore substantive references. Their deployment and database boundaries are larger than the first local HOI4 task needs. Borrow specific invariants before importing whole platforms. Gas Town, Beads and the two factory articles contribute task, review and evidence patterns with the limitations recorded in the comparison.

## Provisional decisions

| ADR | Proposed decision | Why | Reversal trigger |
|---|---|---|---|
| ADR-001 | Reuse Mod Setup's portable planning core and adapt desktop scanner and transaction services | Existing new/import workflow is a required asset | Recovery experiment cannot preserve the fixture at a supportable pin |
| ADR-002 | Superseded by ADR-011 and ADR-012. Previously preferred Candidate A | Narrow runtime replacement boundary and direct onboarding reuse | Not applicable |
| ADR-003 | Host owns grants and mutation receipts | Runtime hooks and transcript stores do not enforce all product invariants | A tested runtime supplies the same contract without split authority |
| ADR-004 | Git source plus a transactional operations database plus hashed artifacts. PostgreSQL on the server since ADR-012 | Different authority and lifetime requirements | Measured needs require a different store, with an explicit migration design |
| ADR-005 | Focus tree is first visual editor | Small valuable vertical slice with source-level assertions | Existing parser cannot safely support even bounded fields |
| ADR-006 | Maps begin with inspection and structured import | Map dependency closure and topology are substantial | A separately validated existing editor meets preservation and licensing gates |
| ADR-007 | One writer before parallel agents | Reduces conflicting effects while core correctness is established | Controlled two-worker trials improve accepted output without more loss or rework |
| ADR-008 | Claude-first engineering, portable shared rules | User's implementation preference with handoff continuity | User revises the development-tool preference |
| ADR-009 | Exact/domain retrieval before vectors | Rebuildable low-complexity baseline | EXP-05 demonstrates a material recall gap after simpler improvements |
| ADR-010 | No unlicensed or restricted UI/editor code reuse | Distribution terms vary independently from visual appeal | Separate written permission or a verified compatible license |
| ADR-011 | Native Rust agent runtime, no Node worker | Maintainer decision, one language for host and runtime | Adapter maintenance or fault handling costs materially more than an adapted library |
| ADR-012 | Public hosted web application with GitHub or email accounts and invited collaborators | Maintainer decision | Hosting cost, isolation or terms make a public service infeasible |
| ADR-013 | Docker images and Compose, portable Linux hosting | Maintainer decision, reproducible deployment | A chosen platform cannot run the images or per-task containers are impractical |
| ADR-014 | Native GitHub integration for sign-in, import, publication and pull requests | Maintainer decision | GitHub terms or rate limits block the flow |
| ADR-015 | Server-held read-only game reference, latest build with all DLC | Maintainer decision, server-side validation and previews | The rights review does not permit it |
| ADR-016 | Free use, bring-your-own keys and an operator credit allowance | Maintainer decision | Abuse or cost makes the allowance unsustainable |
| ADR-017 | AGPL-3.0 with attribution and origin terms | Maintainer decision, prevents closed rebranded forks | Maintainer revises the license |

ADR-011 through ADR-017 are recorded maintainer decisions with full records in the [decision record](maintainer-decisions-2026-10-24.md). The others remain proposals. S01 produces decision receipts with evidence and asks for architecture/roadmap review at its end. Approval of S01 does not authorize S02 automatically.

## S01 experiment gate

EXP-01 must reproduce the observed Setup recovery mismatch or explain it with artifact evidence, then verify the selected pin's new/import/recovery fixtures. EXP-02 tests source preservation and supported focus edits. EXP-03 tests the Rust agent loop for unsafe replay, revocation and model handoff. EXP-04 runs a native provider request from a Docker image with no external coding CLI and probes a GitHub App connection on a disposable repository.

A failed probe is useful evidence. Preserve the alternative and record a bounded next decision. Do not turn the planning recommendation into an irreversible dependency commitment before these gates.

