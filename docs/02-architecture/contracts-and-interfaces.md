# Contracts and interfaces

Status: **Proposed contract specification**, version 0.2, revised for the hosted service in [maintainer decisions of 2026-10-24](maintainer-decisions-2026-10-24.md). These are design shapes for future implementation. They are not claims that outside projects expose these exact APIs.

Read [architecture](recommended-architecture.md), [data ownership](data-context-and-state.md) and [acceptance matrix](../05-verification/REQUIREMENTS-ACCEPTANCE-MATRIX.md). A contract change updates its consumers and negative tests in the same stage.

## Common envelope

Every internal request carries schema version, request ID, account ID, project ID and workspace ID where applicable, actor, actor role, expected state version and cancellation token. The server derives account and role from the authenticated session, never from client-supplied fields. A request whose workspace belongs to another tenant fails as not found, without revealing that it exists. Mutating requests additionally carry grant ID, revocation epoch, idempotency key and intended effect class. Receivers reject unknown required versions and unexpected fields in protected operation payloads.

Errors have a stable code, user-facing explanation, retry class, affected IDs and evidence references. Retry classes are `safe_same_inputs`, `refresh_preconditions`, `reconcile_first`, `requires_authorization` and `terminal`. A transport timeout is not itself a safe retry classification.

## C-01: WorkspaceDescriptor

| Field | Meaning and constraint |
|---|---|
| workspace_id | Stable opaque identity, verified against host registration |
| tenant | Owning account, project and member roles at descriptor creation |
| root_handles | Host-owned handles for the server mod workspace, read-only game reference, tools and artifact roots |
| source_snapshot | Git revision plus dirty/untracked owned file hashes |
| git_state | Branch, worktree, expected HEAD and staged/unstaged inventory |
| game_profile | Server reference build and DLC set, content order, the mod's declared supported version, any compatibility warning and explicit unknowns |
| remote_link | Optional C-10 repository link, or none for an uploaded or new mod |
| component_closure | Setup revision, manifest hash, resolved assets and tool versions |
| capabilities | Available, unavailable and unverified operations with reasons |
| setup_receipt | Plan identity, final transaction state and generated-file manifest |
| unresolved_checks | Precise remaining checks, owner and impact |

The runtime gets logical locators and authorized content, not unrestricted root paths or credential values. The browser client receives display names and logical locators. It never receives server filesystem paths or raw game reference files.

## C-02: ChangeProposal

A proposal has ID, workspace, objective and requirement IDs, base snapshot, per-path expected hashes, declared owned paths, typed operations, semantic before/after summary, source-span or binary replacements, dependency closure, expected postconditions, policy hash, grant reference and evidence requirements. Its immutable canonical representation has a hash.

Operations include supported source-span replacement, new file, guarded delete, binary replacement and native semantic adapter operations. A delete or path move cannot be smuggled through an ordinary scalar edit. Unknown operation variants fail closed. Final output must still be valid native mod data.

The apply response returns a receipt, new snapshot and invalidated evidence set. `stale_base` returns current hashes without applying. `ambiguous_source` returns diagnostic locations. `scope_exceeded` identifies the first unauthorized operation. `unknown_effect` requires reconciliation.

## C-03: RunEvent

Fields are event ID, host sequence, task/run/attempt IDs, schema version, actor, timestamp, workspace snapshot, policy version, event type, payload hash and evidence refs. Persist before announcing durable transitions. Events distinguish intent, started, progress, settled, cancelled, interrupted and reconciled outcomes.

Clients reconnect with an after-sequence cursor. A gap triggers a snapshot refresh. A client retry cannot duplicate a completed mutation solely because it missed a stream event. Raw model token streaming is optional diagnostic data, not the durable event authority.

## C-04: ToolReceipt

A receipt binds a persisted intent to its observed outcome. The intent stores operation and request IDs, idempotency key, tool and implementation version, negotiated schema version, normalized final argument hash and an immutable request artifact reference. That artifact contains the final normalized nonsecret arguments and credential references needed to reconstruct the operation. Persist source and root identity, grant ID and revocation epoch, host/worker fencing generation, retry class, reconciliation method and its exact observation inputs before the effect starts. A hash alone is insufficient recovery input.

The outcome adds start/finish data, observed before/after hashes, result artifact references and settlement status. Missing or corrupt intent material cannot be guessed from the current model conversation. Preserve an explicit integrity failure and any unknown effect, and block replay until reconciliation establishes what happened. Reconstructing intent does not renew its grant or make the effect safe to repeat.

Validate final arguments after all hooks. Validate returned data after postprocessing before it can become evidence. Mark partial or truncated outputs explicitly. Default unknown effects to non-replayable. A tool's `readOnly` annotation is a hint until the installed adapter's effect contract is reviewed.

## C-05: EvidenceRecord

Fields are evidence ID, kind, producer/version, exact inputs, artifact hash, checks performed, findings, coverage, limitations, source freshness, execution environment and result. Kinds include source inspection, static validation, source render, fixture simulation, actual game observation and human review.

A check may be passed, failed, unavailable, skipped with reason or stale. “Verified” in research identifies what was inspected or executed. It does not collapse these test outcomes. An actual game record includes game build, DLC, scenario, time or save context and captured observation. It cannot validate an unrelated source revision.

## C-06: ContextBundle

Fields are bundle ID/hash, task scope, approved canon and rule references, current state, relevant source locators, retrieval explanations, unresolved effects, recent visible messages, capability profile, token budget and omissions. Each locator includes a version or content hash.

A model switch consumes a newly rendered bundle. A context cache cannot override current policy. `required_context_too_large` is explicit. The bundle excludes credential values and provider-private reasoning.

## C-07: KnowledgeCandidate

Fields are candidate ID, domain, statement, author, owner, source evidence, confidence basis, applicability, version bounds, counterexample, proposed destination, change diff, state, review and supersession links. Domains include runtime lesson and engineering lesson. Approved lore uses a separate canon proposal.

Approval checks the expected candidate hash. A newly generated candidate with the same evidence cannot reset a completed review. Rejected and superseded items remain attributable while active retrieval excludes them. Under ADR-023, an engineering lesson backed by a passing regression test that changes no authority, external effect, cost or scope is accepted automatically as a Git change with its test and changelog entry. Policy or authority changes require the maintainer.

Since ADR-022, C-07 also carries runtime memory. A **MemoryItem** has scope (personal, project or global), kind, a bounded statement, applicability, rationale, lane, status, author, evidence references, source bounds (files, identifiers, game build, DLC set, tool versions), validity and supersession times, usage counts, a content hash and a version for expected-version transitions. A **FeedbackEvent** is an append-only typed signal (thumbs, remember or forget commands, proposal edits, rejections, reverts, requested changes, validation failure and fix pairs, game reports, repeated corrections, stops) with actor, project, task, attempt and artifact hashes. Lane assignment is a deterministic host rule. A MemoryItem never carries authority and cannot create or change canon, grants, tools, endpoints or budgets. [SPEC-004](../03-subsystem-specs/004-context-memory-and-runtime-learning.md) defines the lanes, the verification gate and erasure.

## C-08: ApprovalGrant

Fields are grant ID, grantor and grantor role, workspace/run scope, operation classes, allowed roots or object sets, provider destinations, cost/time/attempt/change limits, expiry, policy version and revocation epoch. Some grants authorize a plan class, others a specific proposal hash. The UI states which one applies.

Publication classes are separate from editing classes. Local commit, push of an owned branch, pull request creation, repository creation and default-branch integration are each named explicitly. A grant cannot exceed the grantor's project role. A collaborator who may edit cannot thereby authorize publication they are not permitted to perform.

Check the grant at dispatch and immediately before each effect. Revocation changes the epoch durably. A stale worker cannot regain authority by replaying an earlier request. When an operation already started, cancellation and reconciliation records explain its outcome. Existing valid authorization is reused within scope rather than repeatedly requested.

## C-09: SetupAdapter

Proposed host operations include `inspectProject`, `analyzeSetup`, `confirmSetupEvidence`, `planSetup`, `resolveSetupConflicts`, `approveSetupPlan`, `applySetupPlan`, `inspectRecovery`, `recoverSetup` and `openWorkspace`. They adapt the portable planning core together with the desktop scanning, analysis and transaction services, now operating on server workspace paths. Launcher integration and the user's local Documents folder are out of scope. These are proposed host names, not claims of an existing public service API. Exact bindings are selected in S01 and documented in SPEC-001.

The native Setup service owns the plan. Planning requires confirmed scan evidence or the confirmed new-project brief, the required provider-analysis ID and their source/root bindings. Preserve the inspected provider-analysis-required gate and evidence confirmation checks. A factory provider bridge may satisfy the native contract, but cannot fabricate confirmation or skip analysis. Conflict decisions produce a new plan hash before approval. The host does not allow the model to fabricate an arbitrary plan ID/root pair. Approval binds root identity and final plan hash. Reruns compare generated-file manifests and existing content.

Recovery response uses source-backed variants such as resumable staged work, rollback-required partial apply and verifiable finalization. A generic resume flag is insufficient. The handoff returns C-01 only when required setup invariants are satisfied, with optional capabilities separately labelled.

## C-10: RepositoryLink

Fields are link ID, workspace ID, provider (GitHub in the first release), repository identity, installation reference, selected access scope, default branch, last observed remote refs with hashes, and link state. The installation reference names a secret store entry. It never contains a token value.

Operations are `connectRepository`, `importRepository`, `uploadArchive`, `pushBranch`, `openPullRequest`, `createRepository` and `inspectRemote`. Import and upload produce a C-01 only after the Setup adapter's preservation checks. Upload extraction rejects traversal, symlinks, oversized content and unexpected file types.

Every remote mutation carries an idempotency key and is recorded as intent before the call. A lost response is reconciled by querying the named ref, pull request or repository before any retry, as in SPEC-005 GIT-09. Push to a shared default branch requires its own grant class. A revoked GitHub installation turns the link to `access_revoked` without deleting the server workspace. T-GH-01 and T-GH-02 exercise these cases.

## Shared editing operations and the engine

Added on 2026-11-01 for FR-5, D-07 and Q-1. These are extensions of existing contracts, not a new subsystem. Stages S03, S04, S06 and S07 implement them, as planned in [the roadmap](../04-implementation-stages/ROADMAP.md#engine-editors-and-tools-across-stages).

**Editor projection.** An editor's data is the result of the same inspect tool the agent calls, from the pinned tool profile. The server stores it with the tool name and version, the file hashes it read and a projection hash. The browser receives logical locators and the projection, never server paths.

**C-02 additions.** A proposal names its actor kind (person, native agent or coding client), the tool operation that produced it with its version, and the projection hash it was based on. A person's gesture is translated into the same tool operation and arguments the agent would send. No operation variant exists only for the editor. A proposal based on an older projection fails with `stale_base` and the current hashes.

**C-03 addition.** `source_changed` carries the workspace snapshot, the changed object IDs (such as focus IDs or event IDs), the new file hashes and the proposal ID. Editors refresh from it. The host keeps, per agent attempt, the changed IDs since that attempt's last read and adds them to its next tool result and to the next C-06 bundle.

**C-05 additions.** New evidence kinds are an engine check report (resolved, unresolved and unknown counts per kind, verdict, findings and the baseline used), a finding delta (introduced, worsened, changed, improved, unchanged and resolved), an import preservation report (one row per path: parsed, kept byte-exact or not understood) and a benchmark result (surface, fixture set hash, comparator pins, hardware, measured values, baseline and unavailable surfaces). An `incomplete` engine verdict is never recorded as passed.

**Engine client.** The server links `hoi4-engine` as a library from S04 and uses its versioned report format (`hoi4-engine-report/1`) and baseline format (`hoi4-engine-baseline/1`). The MCP tools call the engine binary through the agent-tools engine client. A missing or failing engine returns a typed unavailable error. Neither side falls back silently to another implementation.

**Tool profile.** C-01 capabilities name the pinned hoi4-agent-tools release, the engine revision, `hoi4-imagegen-mcp` where enabled, and the exposed tool list. The profile Setup writes for a local coding client names the same releases. A mismatch is recorded in evidence for external changes.

**Commit check.** Before an agent commit, the host runs the engine check on the proposal worktree against the project baseline. A new finding at or above the project threshold blocks the commit and returns those findings to the agent as diagnostics. A person's commit shows the same report and is not blocked by it.

## Protocol and version adapters

MCP is a tool transport. The [current specification](https://modelcontextprotocol.io/specification/2026-07-28) differs from older server assumptions, so each adapter negotiates and records actual supported version and capability behavior. Long-running Tasks support is opt-in. The product can use its own durable job wrapper around a synchronous tool without claiming the remote server implements Tasks.

[Agent Skills](https://agentskills.io/specification) is an instruction packaging format. Its metadata does not grant authority. [ACP v1](https://agentclientprotocol.com/protocol/v1/initialization) is optional interoperability with external clients. The factory remains functional without it and does not use it to depend on another coding harness.

## Contract tests

Each contract needs positive serialization, unknown version, malformed field, stale expected version and replay tests. Mutation contracts also need cross-tenant, cross-workspace, insufficient role, revoked grant, path traversal, symlink change, hook mutation and lost response cases. Producer and consumer tests use shared canonical fixtures with independent expected outcomes. A test that only serializes an object and reads it back cannot prove authority or recovery behavior. Every editor operation also needs a parity test (T-EDIT-01) that runs the gesture, the native agent call and a coding-client call and compares bytes, projection and finding delta.

