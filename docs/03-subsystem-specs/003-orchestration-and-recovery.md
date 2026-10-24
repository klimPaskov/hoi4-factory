# SPEC-003: Orchestration and recovery

Status: **Proposed**. Primary requirements: REQ-014, REQ-016 and REQ-017. S03 establishes durable attempts, S04 proves bounded autonomy, S05 hardens recovery and S06 adds contributor task dependencies.

Read first: [authority](../02-architecture/data-context-and-state.md), [contracts](../02-architecture/contracts-and-interfaces.md), [runtime](002-native-runtime-and-models.md), [Git review](005-git-collaboration-and-review.md), [permissions](011-extensibility-sandbox-and-permissions.md) and [test strategy](../05-verification/TEST-STRATEGY.md).

## Purpose and scope

A modder can delegate a bounded outcome, see what the application is doing, interrupt it and return after a crash without losing the distinction between proposed work and accepted source. The first autonomous workflow is one small focus task with explicit scope, tool limits, budget and acceptance checks. It can work without opening a canvas. The same state machine serves manual and assisted workflows.

The initial scheduler has one mutating owner per workspace boundary. Independent reads may run concurrently. Many workers, remote fleets and automatic merge trains remain later decisions. Complexity is added when a measured bottleneck requires it.

## Evidence and selected patterns

[SRC-B01, Pi durable](https://github.com/earendil-works/pi/blob/8a2ebd38549c3521a02db0e5d758a24320e97f94/packages/durable/src/harness/tool.ts) records final tool intent and separates safe from unsafe replay. Its [recovery tests](https://github.com/earendil-works/pi/blob/8a2ebd38549c3521a02db0e5d758a24320e97f94/packages/durable/test/harness-tools-recovery.test.ts) demonstrate both useful recovery and the hazard of safe replay after a changed working directory. [SRC-B02, OpenCode](https://github.com/anomalyco/opencode/blob/02671e66454260d77fe04d63cf5eb94e76921dee/packages/core/src/session/execution/restart.ts) explicitly uses at-least-once recovery and assumes a former owner is dead.

[SRC-B04, Gas City](https://github.com/gastownhall/gascity/blob/87a47634d0cebe611874df321c723bba95dcffea/cmd/gc/reconcile_effect_lease_test.go) tests refusal after lease takeover. [SRC-B07, Fusion](https://github.com/Runfusion/Fusion/blob/5bdc9ea1132ba4d1e49d0dfc5ea084e0e01eb5c3/packages/core/src/__tests__/postgres/handoff-to-review-atomicity.pg.test.ts) tests review handoff, queue and audit changes in one transaction. Adopt these invariants in the host's smaller task journal. Their upstream tests were inspected, not executed.

## State and authority

A task records objective, requirement IDs, bounded artifact scope, dependencies, owner, acceptance policy, current version and selected mode. Its proposed states are `planned`, `ready`, `running`, `waiting_for_input`, `in_review`, `completed`, `blocked` and `cancelled`. A successful attempt can produce a draft without completing its task.

An attempt records model profile, C-01/C-06 snapshots, grant reference, worker generation, budget and outcome. Its states distinguish `running`, `interrupt_requested`, `reconciling`, `succeeded`, `failed`, `cancelled` and `unknown_effect`. An unknown effect blocks conflicting work while reconciliation is pending. It does not turn every unrelated workspace action into a global failure.

Rust owns transitions in a local transactional database. Each accepted transition writes its C-03 event and resulting state together. Updates carry an expected version, making competing commands deterministically succeed once or return `version_conflict`. UI events are projections of committed data. A host restart reloads state before admitting new work.

A logical owner has a process incarnation and increasing fencing generation. A timeout may allow replacement, but never authorizes the previous owner to continue. The effect boundary checks the current generation. Liveness failures remain unknown until independent evidence proves an owner stopped.

## Admission and scheduling operations

| Operation | Input and output | Rule |
|---|---|---|
| Create task | Objective, requirements, C-01 and scope return task ID and plan | Reject missing objective or contradictory owned-path constraints |
| Admit attempt | Expected task version, C-06, C-08 and budget return durable attempt | Dependencies, source freshness and capability requirements must hold |
| Advance task | Attempt result, C-02 and C-05 return proposed state transition | Completion is determined by acceptance policy, not a model's assertion |
| Request intervention | Specific unresolved decision returns visible question event | Preserve completed work and allow unrelated permitted tasks |
| Retry | Classified failure and remaining budget return new attempt | No replay of uncertain effects without reconciliation |
| Recover | Stored attempts, receipts and observed workspace return reconciliation report | Resume only from established facts |
| Cancel | Task or attempt scope returns persisted request | Stop admission, revoke further effects and settle running operations |

Dependency edges are typed as prerequisites, artifact inputs or review gates. Reject cycles before scheduling. A completed prerequisite is satisfied only at its recorded artifact revision. If that artifact changes, invalidate dependent evidence and return affected tasks to a reviewable state. An unavailable optional tool produces a capability limitation. A required unavailable validator blocks only the acceptance claim that needs it.

## Effect protocol, C-04

Before any side effect, the host resolves and validates the final arguments after all model and extension hooks. It checks C-08 immediately before execution, then records an operation ID, normalized argument hash, tool/version, workspace/root identity, input hashes, retry class and reconciliation method.

Execution follows persisted intent. A completion receipt records observed before/after hashes, outcome, evidence and timestamps. Local file operations use recoverable staging and an apply journal. An external process or remote service may finish before its receipt is persisted. That window is classified as `unknown_effect` until observation resolves it.

Read-only operations can retry within policy. An idempotent operation must document its key and observed postcondition. A mutating operation is not made replayable by a label alone. Recovery compares exact operation identity, tool version, grant and source snapshot. Changed working directory, configuration or tool implementation invalidates the prior replay decision.

For a known local staged transaction, recovery may finish or restore from the journal according to its actual phase. For an external command with no reliable reconciliation query, preserve logs and partial artifacts, isolate affected paths and request a concrete decision. Never report rollback merely because a process was killed.

## Pause, cancellation and steering

Pause is a durable scheduler condition, recorded with an expected task version and C-03 event. It prevents new steps and survives restart. An active operation settles at a safe boundary, after which the UI shows paused while preserving the task and attempt history. Resuming clears this condition only after grant, source and dependency checks. Pause does not imply cancellation or completion.

Interrupt requests immediate cancellation of inference and cooperative tools. Cancellation completion additionally requires observed process/tool settlement or an explicit unknown-effect record. The user sees these as distinct statuses.

The host revokes future effect capability before signalling the worker. A late tool request fails the revocation-epoch check. Process supervision targets the complete owned process tree where supported and records termination evidence. An MCP or remote job may continue after the transport closes. Its receipt stays unresolved until queried or explicitly adjudicated.

Steering is a visible event tied to the active task. An instruction that narrows scope can take effect at the next boundary. A scope expansion creates a revised plan and checks whether the existing grant covers it. The application should not ask again for actions already authorized within an unchanged bounded scope.

## Persistence and restart

The authoritative store contains tasks, attempts, operation intents, receipts, grants, event sequence and evidence references. Large logs and artifacts may live in content-addressed files with manifests. A database record cannot claim a required artifact exists before its durable write and hash check.

Startup obtains workspace ownership, scans nonterminal attempts, checks unfinished operations against source and process observations, and emits a recovery report. It does not indiscriminately rerun the last prompt. Event cursors prevent duplicate display after reconnect, while idempotency keys prevent duplicate state transitions.

A corrupted journal or missing artifact produces a bounded integrity failure and a preserved forensic copy. Recovery can use a verified backup and replayable host events where available. Rebuilding a search index cannot reconstruct lost authoritative approvals. Database schema migration must be backed up, versioned and restartable.

Pi durable, if selected, stores conversation tasks linked to host operation IDs. It cannot independently approve or repeat filesystem effects. The S01 decision must show how both stores reconcile when one commit succeeds and the other fails. If that protocol adds more complexity than it removes, use Pi core and the host journal.

## Verification and acceptance

| Test | Failure or normal case | Required observation |
|---|---|---|
| ORCH-01 | Complete bounded focus task | Proposal and exact validation evidence enter review, no unsupported game-verified claim |
| ORCH-02 | Kill before intent persistence | No effect starts and attempt can restart safely |
| ORCH-03 | Kill after intent, before execution | Recovery identifies never-started or preserves uncertainty, with no blind duplicate |
| ORCH-04 | Kill after effect, before receipt | Reconciliation detects after-hash or returns unknown effect |
| ORCH-05 | Replace worker, then old worker requests a write | Old generation is refused |
| ORCH-06 | Revoke grant while a tool waits in queue | Tool does not start after revocation |
| ORCH-07 | Cancellation-resistant subprocess or accepted remote job | UI stays reconciling or unknown until effect evidence is available |
| ORCH-08 | Failure during state/event or review-handoff commit | Neither half becomes independently visible |
| ORCH-09 | Change cwd, source hash or tool version before retry | Existing replay is rejected and new attempt requires reconciliation |
| ORCH-10 | Disk full during staged multi-file apply | No accepted mixed snapshot, recoverable phase remains visible |
| ORCH-11 | Dependency cycle or stale prerequisite artifact | Admission rejects affected task with actionable reason |
| ORCH-12 | Repeated transient failures exhaust persisted budget | Scheduler stops retrying across app restart |

AC-014 requires useful bounded autonomous behavior with intervention. AC-016 requires process-kill and storage-failure evidence, not only graceful-close tests. AC-017 requires dependency and retry behavior at exact revisions. Each test stores input fixture, injected failure point, journal events, resulting hashes and C-05 limitations. S03 gates native writes on these invariants. S05 expands long-session and model-switch cases, while S06 adds contributor scheduling without weakening ownership.

