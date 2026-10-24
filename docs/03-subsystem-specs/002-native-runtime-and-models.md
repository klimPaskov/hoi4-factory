# SPEC-002: Native runtime and models

Status: **Proposed**. Primary requirements: REQ-001, REQ-002, REQ-003 and REQ-041. Runtime selection belongs to S01, native execution to S03, handoff hardening to S05 and distributable evaluation to S08.

Read first: [candidate comparison](../00-research/03-candidate-comparison.md), [architecture decision](../02-architecture/options-and-decisions.md), [contracts](../02-architecture/contracts-and-interfaces.md), [recovery](003-orchestration-and-recovery.md), [context](004-context-memory-and-runtime-learning.md) and [evaluation matrix](../05-verification/EVALUATION-MATRIX.md).

## Purpose and boundaries

The application must work as an independent desktop product. A modder chooses an API or a configured local model, opens a validated workspace and starts a bounded task without installing Claude Code, Codex or a terminal agent. Those products may help implement the application during development. They are not the product's required execution engine.

The first supported surface is text generation, streamed visible output, typed tool calls, cancellation, durable attempt identity and recoverable model switching. Image input, constrained generation and other provider features are capabilities, not universal requirements. A local endpoint can support text while remaining unavailable for autonomous source modification. Running a local inference server and packaging a local model are separate product decisions. Initial support connects to an existing endpoint.

## Evidence and reuse decision

[SRC-B01, Pi Agent](https://github.com/earendil-works/pi/blob/8a2ebd38549c3521a02db0e5d758a24320e97f94/packages/agent/src/agent.ts) exposes a library loop with injected transport, credentials, context transformation, subscriptions and abort. Its [transport](https://github.com/earendil-works/pi/blob/8a2ebd38549c3521a02db0e5d758a24320e97f94/packages/ai/src/api/openai-completions.ts) uses custom model base URLs and does not infer strict-schema capability from compatibility alone. These source observations support the leading Pi worker experiment.

[SRC-B02, OpenCode EmbeddedHost](https://github.com/anomalyco/opencode/blob/02671e66454260d77fe04d63cf5eb94e76921dee/packages/sdk/src/internal/host.ts) is genuinely in process and defaults to an in-memory database. It is the principal alternative when embedded with explicit persistence and exclusive recovery ownership. [SRC-B03, OpenHands configuration](https://github.com/OpenHands/software-agent-sdk/blob/87bec04de8e08117399d5cab68042cc794bc2199/openhands-sdk/pyproject.toml) supports the Python alternative, with a different deployment burden. None was executed in this research.

## Ownership and runtime boundary

The leading design packages Node with the application and embeds selected Pi libraries in a supervised worker. Rust remains authoritative for C-01 workspace descriptors, C-02 proposals, C-03 events, C-04 receipts, C-05 evidence and C-08 grants. The worker receives immutable references and redacted context. Its request to perform an operation carries no authority by itself.

The worker protocol is versioned and framed, with explicit maximum message size, request ID, attempt ID, worker generation and schema version. A startup handshake reports protocol version, runtime build and available provider adapters. Incompatible versions fail before a task starts. The host rejects messages from a superseded generation and keeps stderr separate from structured frames. Unsolicited output cannot masquerade as a host event.

The host assigns the durable C-03 sequence after validation and persistence. UI streaming reads that sequence and can reconnect from a cursor. Token deltas may be transient projections, but accepted messages, tool intent, terminal outcomes and budget changes are durable. The UI never treats a completed stream as an accepted mod change.

## Model profile and capability contract

A model profile includes provider adapter and version, endpoint identity, model identifier, credential reference, context budget, output budget, timeout, retry policy and an optional estimated price record with source and retrieval date. It never embeds the secret in C-01 or C-06.

Capabilities distinguish advertised support, probe evidence, unsupported behavior and unknown behavior. Each capability identifies its evidence and model/endpoint version. Track text, tool calls, parallel calls, streaming, strict JSON Schema, image input, usage accounting, cancellation and continuation separately. A failed image probe need not disable text work.

A task declares required and preferred capabilities. Missing required capabilities return `capability_unsatisfied` before execution. Missing preferred features produce a visible adaptation such as validated JSON through a tool schema. The adaptation must preserve semantic checks. A provider ignoring a schema is not a successful structured-output result.

Output is parsed and validated by the host against the operation's closed schema. Unknown fields, missing required fields and invalid bounds are rejected where the contract forbids them. Every hook or repair runs before this final validation. Pi's [agent-loop test](https://github.com/earendil-works/pi/blob/8a2ebd38549c3521a02db0e5d758a24320e97f94/packages/agent/test/agent-loop.test.ts) explicitly permits mutable pre-tool hooks without another check. OpenCode's [raw-schema fallback](https://github.com/anomalyco/opencode/blob/02671e66454260d77fe04d63cf5eb94e76921dee/packages/core/src/tool/runtime.ts) also requires an independent host boundary.

## Operations

| Operation | Input and output | Rejection and recovery |
|---|---|---|
| Inspect provider | Profile without exposed secret returns capabilities, auth status and probe history | Invalid endpoint or unavailable service is a diagnostic, no workspace mutation |
| Start attempt | C-01, C-06, task scope, budget and C-08 reference return durable attempt ID | Stale workspace, expired grant or unsupported capability prevents admission |
| Send user steering | Attempt ID and visible instruction return acknowledged event ID | Closed attempt rejects or creates an explicitly new attempt |
| Request tool | Final typed arguments, operation ID and attempt generation return C-04 | Invalid schema, stale base or revoked grant returns structured refusal |
| Interrupt | Attempt ID returns durable interruption request | Settlement and effect reconciliation continue under SPEC-003 |
| Switch model | Target profile and new C-06 return a new attempt after a safe boundary | Unsettled effects or incompatible required capability prevent admission |
| Export conversation | Selected visible messages and evidence references return redacted portable data | Secrets and provider-private reasoning are excluded |

## Authentication, fallback and spend

Store credentials in the selected operating-system credential service. A provider adapter may implement an official supported OAuth flow or API-key configuration. Authentication support must be measured for each shipped adapter. A subscription to a coding tool does not automatically authorize arbitrary API use. Authentication errors are distinct from quota, transport and model errors.

The user can set a task budget, elapsed-time limit, maximum turns and retry allowance. The host reserves budget before a request and records actual provider usage afterward. Missing usage becomes `usage_unknown`, not zero cost. Estimates are labelled estimates, including local compute where there is no per-token invoice.

Fallback uses an already-authorized policy specifying providers, data exposure and spend bounds. A change to an unapproved provider requires the relevant authorization. Fallback is recorded as a new attempt using the same approved task scope and a fresh capability check. It never repeats an uncertain mutating operation merely because another model is available.

A provider request known not to have been accepted may retry within a bounded policy. Inference is not a pure read when acceptance can incur cost. An accepted or uncertain request is not automatically repeated after transport loss. Retain its request ID, budget reservation and unknown outcome, then reconcile or obtain explicit authority for a new request. Authentication failures, malformed outputs and ambiguous effects have separate classifications. Retry counters survive restart. A global stop prevents new admission and revokes further effect capability even if a remote request remains in flight.

## Context and switching

C-06 owns the portable handoff. Switching occurs after a tool boundary or after SPEC-003 has reconciled interruption. Include approved intent, current source references, visible decisions, completed receipts and unresolved questions. Exclude private thought artifacts and provider-specific signatures. Pi's [transformer](https://github.com/earendil-works/pi/blob/8a2ebd38549c3521a02db0e5d758a24320e97f94/packages/ai/src/api/transform-messages.ts) is a useful normalization reference, but canonical task state stays in the host.

Model selection affects the rendered request and capability policy, not the authority of lore, accepted requirements or source files. A new model cannot reinterpret a missing tool result as successful work.

## Verification and evidence

| Test | Scenario | Observable acceptance |
|---|---|---|
| RT-01 | Launch packaged app offline with no developer Node or agent CLI installed | Workspace opens and native worker starts. A configured local endpoint can be selected |
| RT-02 | Selected API and local endpoint execute the same bounded fixture | Profile-specific evidence records text, tools, usage and limitations |
| RT-03 | Provider streams truncated JSON or omits required completion marker | No mutation executes and an incomplete-output error is persisted |
| RT-04 | Hook changes a path or argument type after SDK validation | Host rejects the final call with no filesystem effect |
| RT-05 | Switch between two provider families after completed tool work | C-06 and receipts preserve task facts, foreign thought artifacts are omitted |
| RT-06 | Provider loses credentials or exceeds quota | Actionable auth/quota state appears without unrelated repeated requests |
| RT-07 | Stream disconnects after a tool was accepted | Recovery consults C-04, never infers failure from missing text |
| RT-08 | Worker crashes, restarts and sends a late old-generation message | Old frame is rejected and durable event order remains unchanged |
| RT-09 | Fallback would exceed budget or use an unapproved provider | New attempt is refused while completed work remains reviewable |
| RT-10 | Provider reports no usage or loses its response after acceptance | Cost stays unknown or estimated with explicit provenance, and uncertain acceptance does not trigger an automatic duplicate request |

Acceptance AC-001 requires independence from terminal agents. AC-002 requires capability-specific behavior. AC-003 requires explicit authentication, budget and fallback outcomes. AC-041 requires the selected provider and platform matrix on distributable builds, with failed, skipped and unavailable cases visible. Store test inputs, pinned runtime build, provider/profile version, host event log and C-05 results. Passing a mocked transport test does not replace a live selected-provider probe.

S01 resolves Pi core versus Pi durable and compares embedded OpenCode. S03 delivers the first safe native loop. S05 adds robust switching and long-session handling. S08 decides the supported release matrix from measurements, including native dependency packaging and process termination.

