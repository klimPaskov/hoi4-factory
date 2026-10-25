# S03: Native agent, durable attempts and scoped tools

Status: **Executed 2026-10-25, complete (G6 partial as planned)**. The result is in [the S03 handoff](../../evidence/S03/handoff.md) and the authorized scope in [the S03 scope](../../evidence/S03/scope.md). Primary agent: Claude Code. Delegate only bounded work with explicit file ownership. Revised on 2026-10-24 for the [maintainer decisions](../02-architecture/maintainer-decisions-2026-10-24.md), especially ADR-011 and ADR-016.

## Objective and non-goals

Implement the native Rust agent loop and durable host state in the server. The loop runs where S01 placed it, in process or as a supervised worker. The model can inspect source and generate a bounded proposal through typed tools. Add controlled application in an owned fixture only after grants and receipts work. Model use is paid either by the user's own API key or by an operator credit allowance, metered per account and per task.

Do not introduce a fleet, unrestricted shell, broad visual editors, credit purchasing or a payment provider. End-user execution must be independent of developer coding clients, and no coding CLI is present in the server image.

## Entry conditions

S02 accepted with reliable per-account workspace identity, owned Git isolation and setup recovery. The S01 provider adapter approach and execution placement are accepted.

Before editing, read the target repository's current instructions. Preserve unrelated work. Confirm authorization names S03. If the entry condition is not met, perform useful read-only diagnosis and report the concrete blocker. Do not silently start a different stage.

## Read-first sequence and reuse

Read [implementation master goal](../06-agent-handoffs/IMPLEMENTATION_MASTER_GOAL.md), [shared instructions](../../AGENTS.md), [Claude guidance](../../CLAUDE.md), [roadmap](ROADMAP.md), [contracts](../02-architecture/contracts-and-interfaces.md), then [maintainer decisions](../02-architecture/maintainer-decisions-2026-10-24.md), [SPEC-002](../03-subsystem-specs/002-native-runtime-and-models.md), [SPEC-003](../03-subsystem-specs/003-orchestration-and-recovery.md), [SPEC-004](../03-subsystem-specs/004-context-memory-and-runtime-learning.md), [SPEC-006](../03-subsystem-specs/006-hoi4-tools-and-validation.md), [SPEC-011](../03-subsystem-specs/011-extensibility-sandbox-and-permissions.md). Consult the [source registry](../00-research/01-research-sources.md) for the exact inspected pins. Use current local repository instructions to resolve implementation details and record any conflict with this planning baseline.

## Implementation-sized work packets

Each packet should produce a reviewable change and evidence before another packet expands its scope. Split a packet further if it cannot be reviewed in one ordinary engineering session. Owners are responsibilities, not mandatory simultaneous agents.

| Packet | Owner | Deliverable | Work and contract boundary |
|---|---|---|---|
| S03-W1 | Runtime engineer | Rust provider adapters | Implement streaming, final-message assembly, structured-output capability handling, normalized errors, usage accounting and auth references in Rust, using the approach S01 selected. Probe each configured endpoint instead of inferring capabilities from OpenAI-compatible naming. Custom endpoint URLs pass an outbound destination check that blocks private, loopback and link-local addresses unless the operator allows them. |
| S03-W2 | State engineer | Task ledger and initial C-06 | Implement C-03 ordered events, expected-version transitions, one host incarnation, lease fencing and durable pause condition. Build host-owned immutable C-06 bundles from approved task scope, exact source locators, selected canon/rule references, completed receipts, unresolved effects, capability profile and token budget. Record omissions explicitly. Distinguish task completion from attempt success. |
| S03-W3 | Security engineer | C-08, broker and payment source | Enforce operation class, paths, provider destination, budget, expiry and revocation. Revalidate final tool inputs and outputs after hooks or repair steps. Reject malformed schemas rather than continuing without validation. Store bring-your-own API keys encrypted in the server secret store, scoped to their owner. Meter operator credit allowance and user-key usage separately, and never use one user's key for another user. |
| S03-W4 | Integrity engineer | C-04 and effect reconciliation | Persist exact intent before effects and observed results afterward. Classify pure reads, derived jobs, idempotent writes and unknown effects. Bind replay to workspace, root, source and tool version. |
| S03-W5 | Integration engineer | HOI4 adapter negotiation | Connect only configured tool profiles, discover schema/version and report health separately from feature presence. Test authenticated read-only mode and bounded timeout/retry handling. |
| S03-W6 | Runtime reviewer | Restart and intervention | Exercise process death, disconnect, worker that ignores cancellation and stale result arrival. Reopen the same task without repeating writes, then hand off its context and uncertainty. |

## Packet dependencies and exclusions

A contract-only dependency permits parallel implementation after that interface is reviewed. Runtime integration still waits for the dependency's acceptance evidence. Packet owners may edit only their agreed modules and fixture area. A change to another owner's contract requires coordination before editing.

| Packet | Depends on | Explicitly excluded |
|---|---|---|
| S03-W1 | S02 | Universal model capability assumptions |
| S03-W2 | S02, S03-W1 capability contract | Distributed multi-node scheduling and the full S05 retrieval/learning system |
| S03-W3 | S03-W1, S03-W2 contracts | Treating skill text as authorization, credit purchasing |
| S03-W4 | S03-W2, S03-W3 | Blind replay of uncertain effects |
| S03-W5 | S03-W3, S03-W4 | Enabling every discovered MCP command |
| S03-W6 | S03-W1, S03-W2, S03-W3, S03-W4, S03-W5 | Calling an accepted cancel request settled cancellation |

## Verification plan

Use F-RUNTIME-FAULT and F-AUTHORITY with real state storage and host mutation code. Inject missing finish reasons, sparse stream fragments, malformed JSON, hook-mutated arguments, tool timeout, duplicate request IDs, lost replies and death after intent or effect. Distinguish accepted cancel from confirmed stop.

Capture an actual C-06 fixture, its immutable manifest hash and referenced source/receipt identities before and after restart. Required context that exceeds the profile budget returns a typed narrowing error. S03 provides this initial builder before S04 uses it. S05 extends it with full canon review, retrieval and runtime learning.

M-API-A, M-API-B and M-LOCAL-C each run read, structured proposal and tool-error recovery cases. M-LOCAL-C is a self-hosted OpenAI-compatible endpoint reachable from the server. Test one run paid by a user key and one by the operator allowance, a revoked user key and an exhausted allowance. Attempt a custom endpoint pointing at a private address and confirm it is refused. For a provider without strict JSON, validate locally and cap repair attempts at one for the fixture. A failed capability probe disables the corresponding operation. Live credentials and local model prerequisites are recorded.

Use the [test strategy](../05-verification/TEST-STRATEGY.md) and [evaluation matrix](../05-verification/EVALUATION-MATRIX.md). Exercise production code for the behavior under test. Record exact revision, fixture hash, environment, command or action, result, evidence kind and limitation. A declared upstream test or successful source inspection does not replace running the required stage test.

## Measurable exit gates

1. A containerized native run consumes a persisted immutable C-06, uses the selected provider directly through the Rust loop and does not spawn or contain a coding CLI.
2. All final tool payloads pass a closed host schema and current grant checks. Unauthorized fixture paths have zero writes.
3. An interrupted uncertain effect is reconciled or remains blocked. It is never silently rerun as a new operation.
4. Pause and revocation persist across restart. No later effect uses an obsolete grant epoch or host fence.
5. Events replay in order with gap detection, cost unknowns remain unknown, and budget exhaustion stops new dispatch per task and per account, with user-key and operator-allowance usage attributed separately.
6. Three configured provider profiles have truthful capability outcomes. Unsupported features are explicit rather than treated as equivalent. Custom endpoints to blocked destinations are refused.

All gates are evaluated against the final stage revision. Changed inputs invalidate affected evidence. A known defect in the selected critical path requires a fix or an explicit narrower scope decision. Optional unavailable checks remain named limitations.

## Risk and rollback

Disable dispatch, preserve journals and redeploy the previous server image. Database migrations need an accepted rollback or forward-recovery path. Do not delete unresolved receipts to make a restart look clean.

## Engineering learning checkpoint

Promote only reproduced adapter or recovery lessons. Record provider-specific limitations in versioned profile references rather than universal rules.

Record the result in `evidence/S03/learning-review.md`. Follow [reusable knowledge policy](../07-developer-workflow/REUSABLE-KNOWLEDGE-POLICY.md). A no-change decision is valid when justified. No agent may enlarge its own authority by editing a skill or instruction.

## Configuration checkpoint

Verify secrets and user API keys are absent from logs, evidence and tracked config, provider destinations match grants, the outbound destination policy is active and absent MCP servers remain disabled. Check runtime config is separate from Claude/Codex development config.

Record `evidence/S03/config-review.md`, including changed files, verified client or runtime versions, source provenance and any deferred live check. Update the [knowledge changelog](../07-developer-workflow/KNOWLEDGE-CHANGELOG.md) only for material accepted guidance changes.

## Standalone completion and continuation handoff

Record provider profile IDs, actual model versions, grant semantics, replay classes, storage ownership, metering and payment-source rules and remaining cancellation limits. Include a serialized C-06 fixture with manifest hash, source and receipt references, budget calculation and omissions. S04 receives the working initial context builder and a reliable single-writer proposal path.

Produce `evidence/S03/environment.json`, `scope.md`, `checks.json`, `handoff.md`, `learning-review.md` and `config-review.md`, with artifact hashes or references. Use the [handoff template](../06-agent-handoffs/STAGE-HANDOFF-TEMPLATE.md).

A new session must be able to identify the completed revision, exact scope, gate results, failed or unavailable checks, changes to accepted knowledge and the next decision without the prior chat. End after reporting S03. **Do not start the next stage automatically.**

