# SPEC-002: Native runtime and models

Status: **Proposed**. Primary requirements: REQ-001, REQ-002, REQ-003 and REQ-041. Shared requirement: REQ-046. Runtime probes belong to S01, native execution to S03, handoff hardening to S05 and deployed evaluation to S08. The runtime decision is [ADR-011](../02-architecture/maintainer-decisions-2026-10-24.md#adr-011-native-rust-agent-runtime), and the key and credit model is [ADR-016](../02-architecture/maintainer-decisions-2026-10-24.md#adr-016-free-use-bring-your-own-keys-and-credits).

Read first: [candidate comparison](../00-research/03-candidate-comparison.md), [architecture decision](../02-architecture/options-and-decisions.md), [contracts](../02-architecture/contracts-and-interfaces.md), [recovery](003-orchestration-and-recovery.md), [context](004-context-memory-and-runtime-learning.md) and [evaluation matrix](../05-verification/EVALUATION-MATRIX.md).

## Purpose and boundaries

The application must work as an independent hosted web product. A signed-in modder chooses a hosted API provider, their own reachable endpoint or the operator's credit allowance, opens a validated workspace and starts a bounded task without installing anything. Claude Code, Codex and other coding CLIs may help implement the application during development. They are not the product's execution engine and are absent from the deployed images.

The first supported surface is text generation, streamed visible output, typed tool calls, cancellation, durable attempt identity and recoverable model switching. Image input, constrained generation and other provider features are capabilities, not universal requirements. A self-hosted OpenAI-compatible endpoint can support text while remaining unavailable for autonomous source modification. The factory does not run inference servers or package model weights. It connects to an endpoint the server can reach.

## Evidence and reuse decision

The runtime is a Rust agent crate in the server's Cargo workspace. No Node worker, Pi library or OpenCode host is packaged. The inspected runtimes stay useful as behavioral references for design and test oracles.

[SRC-B01, Pi Agent](https://github.com/earendil-works/pi/blob/8a2ebd38549c3521a02db0e5d758a24320e97f94/packages/agent/src/agent.ts) shows a library loop with injected transport, credentials, context transformation, subscriptions and abort. Its [transport](https://github.com/earendil-works/pi/blob/8a2ebd38549c3521a02db0e5d758a24320e97f94/packages/ai/src/api/openai-completions.ts) uses custom model base URLs and does not infer strict-schema capability from compatibility alone. The Rust adapters should keep both properties.

[SRC-B02, OpenCode EmbeddedHost](https://github.com/anomalyco/opencode/blob/02671e66454260d77fe04d63cf5eb94e76921dee/packages/sdk/src/internal/host.ts) defaults to an in-memory database and has at-least-once restart behavior. [SRC-B03, OpenHands configuration](https://github.com/OpenHands/software-agent-sdk/blob/87bec04de8e08117399d5cab68042cc794bc2199/openhands-sdk/pyproject.toml) shows durable event structures and cooperative cancellation. Their recovery cases become fixtures for the Rust loop. None of these runtimes was executed in this research.

Owning the loop means owning stream assembly, malformed tool calls, provider error classes, context conditioning, cancellation, usage accounting and retry classification. Prefer vetted Rust crates for HTTP, server-sent events, JSON Schema validation and serialization. S01 decides between a third-party Rust provider abstraction and thin project adapters from probe evidence.

## Ownership and runtime boundary

The agent crate holds no filesystem, Git, secret-store or grant authority. The server host remains authoritative for C-01 workspace descriptors, C-02 proposals, C-03 events, C-04 receipts, C-05 evidence and C-08 grants. The loop receives immutable references, redacted context and a short-lived credential handle for the selected provider. Its request to perform an operation carries no authority by itself. Every tool call passes through the host broker, which validates the final arguments after any repair step.

S01 measures whether the loop runs as a task inside the server process or in a supervised worker process or container. The fencing rules apply either way. Each attempt carries a request ID, attempt ID, worker generation and schema version. A worker process speaks a versioned framed protocol with explicit maximum message size and a startup handshake reporting protocol version, runtime build and available provider adapters. Incompatible versions fail before a task starts. The host rejects results from a superseded generation, including a late result from an in-process task that outlived a restart, and keeps diagnostic output separate from structured frames. Unsolicited output cannot masquerade as a host event.

The host assigns the durable C-03 sequence after validation and persistence. UI streaming reads that sequence and can reconnect from a cursor. Token deltas may be transient projections, but accepted messages, tool intent, terminal outcomes and budget changes are durable. The UI never treats a completed stream as an accepted mod change.

## Model profile and capability contract

A model profile includes owning account, provider adapter and version, endpoint identity, model identifier, funding source (the user's own key or the operator credit allowance), credential reference, context budget, output budget, timeout, retry policy and an optional estimated price record with source and retrieval date. It never embeds the secret in C-01 or C-06.

A custom endpoint is any OpenAI-compatible URL the server can reach, such as the user's own GPU server or a tunnel. The user's own localhost is not reachable from the hosted service and is not promised. Endpoint URLs are an outbound request risk. Resolve the host at request time, block private, loopback, link-local and metadata-service addresses by default, refuse redirects to blocked addresses and cap response size and duration. A self-hosting operator may allow private ranges explicitly in deployment configuration.

Capabilities distinguish advertised support, probe evidence, unsupported behavior and unknown behavior. Each capability identifies its evidence and model/endpoint version. Track text, tool calls, parallel calls, streaming, strict JSON Schema, image input, usage accounting, cancellation and continuation separately. A failed image probe need not disable text work.

A task declares required and preferred capabilities. Missing required capabilities return `capability_unsatisfied` before execution. Missing preferred features produce a visible adaptation such as validated JSON through a tool schema. The adaptation must preserve semantic checks. A provider ignoring a schema is not a successful structured-output result.

Output is parsed and validated by the host against the operation's closed schema. Unknown fields, missing required fields and invalid bounds are rejected where the contract forbids them. Every hook or repair runs before this final validation. Pi's [agent-loop test](https://github.com/earendil-works/pi/blob/8a2ebd38549c3521a02db0e5d758a24320e97f94/packages/agent/test/agent-loop.test.ts) explicitly permits mutable pre-tool hooks without another check, and OpenCode's [raw-schema fallback](https://github.com/anomalyco/opencode/blob/02671e66454260d77fe04d63cf5eb94e76921dee/packages/core/src/tool/runtime.ts) relies on an outer boundary. Those cases become negative fixtures for the Rust loop. Any repair or normalization step in the Rust loop runs before host validation, never after it.

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

Two funding sources exist. A user may bring their own provider API key. It is encrypted in the server secret store, scoped to that account, never returned to the browser after entry, never used for another account and deletable by the user. The operator may also fund requests through a credit allowance using operator-held keys. Buying additional credits is a later feature in the backlog. A provider adapter may implement an official supported OAuth flow or API-key configuration. Authentication support must be measured for each shipped adapter. A subscription to a chat or coding product does not automatically authorize API use. Authentication errors are distinct from quota, transport and model errors.

Budgets apply per account, per project and per task regardless of who pays. The user can set a task budget, elapsed-time limit, maximum turns and retry allowance. The operator sets allowance limits and rate limits per account under REQ-046. The host reserves budget before a request and records actual provider usage afterward. Missing usage becomes `usage_unknown`, not zero cost. Credit spending uses the conservative reserved amount until usage is known. Estimates are labelled estimates, including self-hosted compute where there is no per-token invoice.

Fallback uses an already-authorized policy specifying providers, funding source, data exposure and spend bounds. Falling back from the user's own key to operator credits, or the reverse, is a change of funding source and needs that policy. A change to an unapproved provider requires the relevant authorization. Fallback is recorded as a new attempt using the same approved task scope and a fresh capability check. It never repeats an uncertain mutating operation merely because another model is available.

A provider request known not to have been accepted may retry within a bounded policy. Inference is not a pure read when acceptance can incur cost. An accepted or uncertain request is not automatically repeated after transport loss. Retain its request ID, budget reservation and unknown outcome, then reconcile or obtain explicit authority for a new request. Authentication failures, malformed outputs and ambiguous effects have separate classifications. Retry counters survive restart. A global stop prevents new admission and revokes further effect capability even if a remote request remains in flight.

## Context and switching

C-06 owns the portable handoff. Switching occurs after a tool boundary or after SPEC-003 has reconciled interruption. Include approved intent, current source references, visible decisions, completed receipts and unresolved questions. Exclude private thought artifacts and provider-specific signatures. Pi's [transformer](https://github.com/earendil-works/pi/blob/8a2ebd38549c3521a02db0e5d758a24320e97f94/packages/ai/src/api/transform-messages.ts) is a useful normalization reference, but canonical task state stays in the host.

Model selection affects the rendered request and capability policy, not the authority of lore, accepted requirements or source files. A new model cannot reinterpret a missing tool result as successful work.

## Verification and evidence

| Test | Scenario | Observable acceptance |
|---|---|---|
| RT-01 | Run a hosted task from the deployed Docker images, whose dependency inventory contains no coding CLI or Node agent runtime | The task completes through the Rust loop. A configured custom endpoint can be selected |
| RT-02 | Selected APIs and a self-hosted OpenAI-compatible endpoint execute the same bounded fixture | Profile-specific evidence records text, tools, usage and limitations |
| RT-03 | Provider streams truncated JSON or omits required completion marker | No mutation executes and an incomplete-output error is persisted |
| RT-04 | A repair step changes a path or argument type after adapter-level parsing | Host rejects the final call with no filesystem effect |
| RT-05 | Switch between two provider families after completed tool work | C-06 and receipts preserve task facts, foreign thought artifacts are omitted |
| RT-06 | Provider loses credentials or exceeds quota | Actionable auth/quota state appears without unrelated repeated requests |
| RT-07 | Stream disconnects after a tool was accepted | Recovery consults C-04, never infers failure from missing text |
| RT-08 | Worker or server restarts and a late old-generation result arrives | Old result is rejected and durable event order remains unchanged |
| RT-09 | Fallback would exceed budget or use an unapproved provider | New attempt is refused while completed work remains reviewable |
| RT-10 | Provider reports no usage or loses its response after acceptance | Cost stays unknown or estimated with explicit provenance, and uncertain acceptance does not trigger an automatic duplicate request |
| RT-11 | Custom endpoint URL resolves or redirects to a private, loopback or metadata address | Request is refused before connection with a specific diagnostic |
| RT-12 | One account's stored key, budget or attempt is referenced from another account | Refused, with no provider request and no disclosure (T-ACCT-02) |

Acceptance AC-001 requires independence from terminal agents. AC-002 requires capability-specific behavior. AC-003 requires explicit authentication, budget and fallback outcomes. AC-041 requires the selected provider matrix on the deployed images, with failed, skipped and unavailable cases visible. Store test inputs, pinned image digests and runtime build, provider/profile version, host event log and C-05 results. Passing a mocked transport test does not replace a live selected-provider probe.

S01 probes the Rust loop against deterministic fault fixtures, chooses in-process or worker execution and decides the provider-crate question. S03 delivers the first safe native loop. S05 adds robust switching and long-session handling. S08 decides the supported provider matrix from measurements on the deployed images, including worker termination and per-account limits.

