# Native runtime and factory candidate comparison

Status: **Proposed selection, supported by source inspection**. Research date: 2026-10-23. This review inspected pinned implementation and test files. It did not install these projects, run their tests, call their model providers or establish packaged Windows behavior.

Read first: [architecture options](../02-architecture/options-and-decisions.md), [authority and contracts](../02-architecture/contracts-and-interfaces.md), [runtime specification](../03-subsystem-specs/002-native-runtime-and-models.md), [recovery specification](../03-subsystem-specs/003-orchestration-and-recovery.md) and [evaluation matrix](../05-verification/EVALUATION-MATRIX.md).

## Recommendation and decision boundary

The leading experiment is a Tauri/Rust host, a React interface and a packaged Node worker embedding Pi library components. Setup reuse combines its portable planning core with its existing desktop scanner and transaction services through the proposed boundary in [SPEC-001](../03-subsystem-specs/001-onboarding-and-mod-setup.md). The portable crate alone does not supply native installation and recovery. Rust owns workspace identity, approvals, task transitions, filesystem effects and receipts. The worker owns model transport and the model/tool conversation. It asks the host to execute a typed operation and cannot grant itself access by naming a path.

Start with `pi-ai` and `pi-agent-core`. Compare a host-owned task journal against adding `pi-durable` during S01. Pi durability should be selected only if it removes enough orchestration work to justify another persistence boundary. A Pi transcript, a durable Pi task and an accepted mod change are different records. There must be one authoritative decision about whether a workspace operation happened.

Keep a coherent alternative: Electron/TypeScript embedding the current OpenCode SDK with an explicit persistent database, plus the same Rust setup/workspace service. OpenCode now has a real in-process SDK. Describing it as a CLI wrapper would be incorrect at the inspected revision. Its richer session and restart infrastructure may win if Pi requires extensive lifecycle reinvention, but the host must still control effects and recovery ownership.

Choose between these after packaged worker startup, provider conformance, interruption, restart and source-integrity probes. If neither supports the host boundary without substantial patches, a small custom loop over a provider library remains preferable to a permanent broad fork. A custom implementation of every provider protocol is the highest ongoing compatibility burden.

## Revision and license register

| ID | Source | Pinned revision | License evidence |
|---|---|---|---|
| SRC-B01 | [Pi](https://github.com/earendil-works/pi/tree/8a2ebd38549c3521a02db0e5d758a24320e97f94) | `8a2ebd38549c3521a02db0e5d758a24320e97f94` | Root MIT text inspected |
| SRC-B02 | [OpenCode](https://github.com/anomalyco/opencode/tree/02671e66454260d77fe04d63cf5eb94e76921dee) | `02671e66454260d77fe04d63cf5eb94e76921dee` | Root MIT text inspected |
| SRC-B03 | [OpenHands SDK](https://github.com/OpenHands/software-agent-sdk/tree/87bec04de8e08117399d5cab68042cc794bc2199) | `87bec04de8e08117399d5cab68042cc794bc2199` | Root MIT text inspected |
| SRC-B04 | [Gas City](https://github.com/gastownhall/gascity/tree/87a47634d0cebe611874df321c723bba95dcffea) | `87a47634d0cebe611874df321c723bba95dcffea` | Root MIT text inspected |
| SRC-B05 | [Gas Town](https://github.com/gastownhall/gastown/tree/ef70b0bf9e588dce00708e36564bfc06b51fac2c) | `ef70b0bf9e588dce00708e36564bfc06b51fac2c` | Root MIT text inspected |
| SRC-B06 | [Beads](https://github.com/gastownhall/beads/tree/bc0cc87fd9fda412e2eaca669fa6ae3ec2c713fb) | `bc0cc87fd9fda412e2eaca669fa6ae3ec2c713fb` | MIT and Apache-2.0 third-party text inspected |
| SRC-B07 | [Fusion](https://github.com/Runfusion/Fusion/tree/5bdc9ea1132ba4d1e49d0dfc5ea084e0e01eb5c3) | `5bdc9ea1132ba4d1e49d0dfc5ea084e0e01eb5c3` | Root MIT text inspected |
| SRC-B08 | [Mastra factory template](https://github.com/mastra-ai/softwarefactory-template/tree/ebde794a8656278cf26bbb2b8764be7070b325d9) | `ebde794a8656278cf26bbb2b8764be7070b325d9` | Manifest declares Apache-2.0. No LICENSE file in complete tree |
| SRC-B09 | [Mastra beta article](https://mastra.ai/blog/announcing-mastra-factory-beta) | Published 2026-09-08, retrieved 2026-10-23 | Author report, no source-code license inferred |
| SRC-B10 | [Glass Town article](https://aaif.io/blog/welcome-to-glass-town-building-a-software-factory-with-a-paper-trail) | Published 2026-10-14, retrieved 2026-10-23 | Author report, no source-code license inferred |

The first seven root licenses permit broad reuse subject to retaining their notices. Dependency notices remain separate work. Beads includes Dolt-related Apache notices. The Mastra manifest declaration is evidence of stated licensing, but its missing full license file needs resolution before copying template code into a distribution. A repository license gives no permission to redistribute HOI4 assets, model weights or third-party credentials.

## Embed, fork or build

| Approach | Useful starting surface | Relative initial cost | Ongoing cost and limits | Decision |
|---|---|---|---|---|
| Embed Pi libraries | Provider adapters, streaming, tool loop, optional durable harness | Moderate. Host journal, process bridge and product tools remain necessary | Smallest selected runtime surface. Must own final validation, process control and workspace effects | Leading probe |
| Embed OpenCode SDK | In-process sessions, event contracts, tools, restart and MCP | Moderate to high. More useful lifecycle machinery, more native packaging and configuration to constrain | Effect runtime, plugins, native services and recovery assumptions become integration obligations | Serious alternative |
| Embed OpenHands Python SDK | Agent loop, typed actions, local event persistence and Python tools | Higher for Rust/React desktop packaging, lower if Python tools dominate | Python environment, LiteLLM/FastMCP compatibility and cooperative tool cancellation | Conditional alternative |
| Fork a full coding harness or factory | Existing product flows, role management, broad tools and services | Fast first demonstration, expensive product reshaping | Upstream merging, inherited write surfaces, CLI assumptions and service migration | Defer unless a specific blocker requires a narrow maintained patch |
| Build a small native runtime over a provider library | Only product-required turns, grants, receipts and events | More host work than direct SDK adoption, less inherited behavior | Must build streaming settlement, retries, budgets, state migrations and conformance fixtures | Fallback experiment |
| Build every provider protocol in Rust | One implementation language and direct host ownership | Highest | Continuous provider drift, transcript normalization, OAuth and streaming edge cases | Do not select without evidence that library embedding fails |

These are qualitative engineering comparisons. No staffing, latency, binary-size or token-cost measurements were performed. The same model and workload can dominate inference cost regardless of harness. Scheduling, retries, context size and verification loops determine avoidable spend.

## Pi: small integration surface with explicit host responsibilities

The inspected packages are version 1.4.0 and require Node at least 22.19.0. [Agent](https://github.com/earendil-works/pi/blob/8a2ebd38549c3521a02db0e5d758a24320e97f94/packages/agent/src/agent.ts) supports injected streaming, credential lookup, context preparation, turn hooks, steering, subscriptions, abort and idle waiting. This permits an independent application without a terminal UI. The [coding-agent SDK](https://github.com/earendil-works/pi/blob/8a2ebd38549c3521a02db0e5d758a24320e97f94/packages/coding-agent/src/core/sdk.ts) also embeds, but brings coding-tool and resource defaults that the product would have to constrain.

[OpenAI-completions transport](https://github.com/earendil-works/pi/blob/8a2ebd38549c3521a02db0e5d758a24320e97f94/packages/ai/src/api/openai-completions.ts) uses a configurable model base URL. This is a route to local or self-hosted compatible endpoints, not proof that every local model handles tools or structured output. The code deliberately does not infer strict JSON-schema support from OpenAI compatibility alone. [Constrained-sampling tests](https://github.com/earendil-works/pi/blob/8a2ebd38549c3521a02db0e5d758a24320e97f94/packages/ai/test/constrained-sampling.test.ts) distinguish a preferred constraint that may fall back from a required constraint that rejects an unsupported schema.

The agent-core [tool loop](https://github.com/earendil-works/pi/blob/8a2ebd38549c3521a02db0e5d758a24320e97f94/packages/agent/src/agent-loop.ts) validates before a mutable pre-tool hook. Its [test](https://github.com/earendil-works/pi/blob/8a2ebd38549c3521a02db0e5d758a24320e97f94/packages/agent/test/agent-loop.test.ts) intentionally changes a string argument to a number and expects execution without another validation. The product must revalidate the final normalized arguments at the host boundary. Parallel tool execution must also be narrowed to independent reads unless the host serializes mutations.

The [message transformer](https://github.com/earendil-works/pi/blob/8a2ebd38549c3521a02db0e5d758a24320e97f94/packages/ai/src/api/transform-messages.ts) removes incompatible thought/signature artifacts during model changes and creates error tool results for missing outcomes. Preserve the original event history and derive a compatible provider request. A synthesized error is never evidence that a tool succeeded. The upstream [handoff test](https://github.com/earendil-works/pi/blob/8a2ebd38549c3521a02db0e5d758a24320e97f94/packages/ai/test/cross-provider-handoff.test.ts) skips missing credentials and insufficient fixtures, so its presence is not a measured guarantee across the provider matrix.

Pi has two materially different persistence choices. The [coding-agent session manager](https://github.com/earendil-works/pi/blob/8a2ebd38549c3521a02db0e5d758a24320e97f94/packages/coding-agent/src/core/session-manager.ts) appends transcripts and directly rewrites a file in the inspected path. [Pi durable tool execution](https://github.com/earendil-works/pi/blob/8a2ebd38549c3521a02db0e5d758a24320e97f94/packages/durable/src/harness/tool.ts) revalidates after hooks, records exact final arguments before execution and defaults uncertain replay to unsafe. Reopening reruns a tool only when both recorded and current policies permit safe replay.

The [recovery tests](https://github.com/earendil-works/pi/blob/8a2ebd38549c3521a02db0e5d758a24320e97f94/packages/durable/test/harness-tools-recovery.test.ts) deliberately show a safe tool replaying after its working directory changes. That behavior is unacceptable for an unbound mod mutation. Bind workspace ID, root identity, source snapshot, tool version, policy version and operation ID before an effect. A changed environment requires reconciliation or a new attempt.

[SQLite storage](https://github.com/earendil-works/pi/blob/8a2ebd38549c3521a02db0e5d758a24320e97f94/packages/durable/src/storage/sqlite/storage.ts) commits document changes and sequence advancement together. [Rollback tests](https://github.com/earendil-works/pi/blob/8a2ebd38549c3521a02db0e5d758a24320e97f94/packages/durable/test/sqlite-storage.test.ts) cover storage errors. [Cancellation-barrier tests](https://github.com/earendil-works/pi/blob/8a2ebd38549c3521a02db0e5d758a24320e97f94/packages/durable/test/harness-cancellation-barrier.test.ts) cover scheduled commits in MemoryStorage. Neither makes an external process or a multi-file workspace update exactly once.

## OpenCode: genuine embedding, broader lifecycle ownership

[EmbeddedHost](https://github.com/anomalyco/opencode/blob/02671e66454260d77fe04d63cf5eb94e76921dee/packages/sdk/src/internal/host.ts) creates an in-process ManagedRuntime and route handler, registers plugins and starts suspended-session recovery. It defaults to an in-memory database. Production embedding must set a persistent database path and exclusive owner policy. [Embedded tests](https://github.com/anomalyco/opencode/blob/02671e66454260d77fe04d63cf5eb94e76921dee/packages/sdk/test/embedded.test.ts) exercise local session routes with a test LLM, including model switching. They do not establish provider interoperability.

[Compatible-provider configuration](https://github.com/anomalyco/opencode/blob/02671e66454260d77fe04d63cf5eb94e76921dee/packages/ai/src/providers/openai-compatible.ts) accepts base URLs and optional credentials. [Streaming fixtures](https://github.com/anomalyco/opencode/blob/02671e66454260d77fe04d63cf5eb94e76921dee/packages/ai/test/provider/openai-compatible-chat.test.ts) cover sparse parallel tool chunks and reject a missing required finish reason. Its [step runner](https://github.com/anomalyco/opencode/blob/02671e66454260d77fe04d63cf5eb94e76921dee/packages/core/src/session/runner/step.ts) publishes a tool call before starting local execution and protects terminal settlement from interruption.

Host validation remains essential. [Tool runtime](https://github.com/anomalyco/opencode/blob/02671e66454260d77fe04d63cf5eb94e76921dee/packages/core/src/tool/runtime.ts) validates recognized codecs, but returns raw input when JSON Schema conversion yields no codec. Raw JSON output schemas get JSON-value validation in the inspected branch. [Post-execution hooks](https://github.com/anomalyco/opencode/blob/02671e66454260d77fe04d63cf5eb94e76921dee/packages/core/src/tool.ts) may alter already-validated results. Closed host schemas and immutable receipts should be independent of these extension behaviors.

[Restart source](https://github.com/anomalyco/opencode/blob/02671e66454260d77fe04d63cf5eb94e76921dee/packages/core/src/session/execution/restart.ts) explicitly describes at-least-once recovery and assumes a claimed turn's former process is dead. Process-local coordination cannot fence a second embedding host. [Tests](https://github.com/anomalyco/opencode/blob/02671e66454260d77fe04d63cf5eb94e76921dee/packages/core/test/session-execution.test.ts) verify durable attempt counting and exhaustion, not exactly-once external effects. [Interrupt](https://github.com/anomalyco/opencode/blob/02671e66454260d77fe04d63cf5eb94e76921dee/packages/core/src/session/run-coordinator.ts) returns before owner cleanup, while idle waiting observes settlement. The UI must preserve that distinction.

MCP supports local stdio through an environment abstraction and remote Streamable HTTP. The [client](https://github.com/anomalyco/opencode/blob/02671e66454260d77fe04d63cf5eb94e76921dee/packages/core/src/mcp/client.ts) attempts explicit session termination during cleanup. This does not guarantee that a remote service stopped an already-accepted operation. Native dependencies in the [core manifest](https://github.com/anomalyco/opencode/blob/02671e66454260d77fe04d63cf5eb94e76921dee/packages/core/package.json) increase the packaging probe surface.

## OpenHands: useful Python ecosystem, different packaging tradeoff

The inspected Python SDK is 1.58.1 with Python at least 3.12 and LiteLLM/FastMCP dependencies. [LLM configuration](https://github.com/OpenHands/software-agent-sdk/blob/87bec04de8e08117399d5cab68042cc794bc2199/openhands-sdk/openhands/sdk/llm/llm.py) exposes custom endpoints, streaming and retry controls. A [provider parsing test](https://github.com/OpenHands/software-agent-sdk/blob/87bec04de8e08117399d5cab68042cc794bc2199/tests/sdk/llm/test_litellm_provider.py) includes a localhost compatible endpoint, but performs no inference. The default parameter-dropping behavior reinforces the need for explicit capability requirements.

[LocalConversation](https://github.com/OpenHands/software-agent-sdk/blob/87bec04de8e08117399d5cab68042cc794bc2199/openhands-sdk/openhands/sdk/conversation/impl/local_conversation.py) persists before notifying subscribers, supports model replacement and retains accounting across a reused usage identifier. [Action construction](https://github.com/OpenHands/software-agent-sdk/blob/87bec04de8e08117399d5cab68042cc794bc2199/openhands-sdk/openhands/sdk/agent/agent.py) normalizes and repairs arguments before typed action validation. The host should preserve both requested and final arguments for audit.

[Atomic text writes](https://github.com/OpenHands/software-agent-sdk/blob/87bec04de8e08117399d5cab68042cc794bc2199/openhands-sdk/openhands/sdk/utils/files.py) use a temporary file, file fsync and replace. [Event tests](https://github.com/OpenHands/software-agent-sdk/blob/87bec04de8e08117399d5cab68042cc794bc2199/tests/sdk/conversation/test_event_store.py) exercise local thread serialization and malformed data. The local cache assumes exclusive file access, and rebuilt indexes can omit corrupt event files. A product audit log needs explicit corruption diagnostics and recovery policy.

[Interruption tests](https://github.com/OpenHands/software-agent-sdk/blob/87bec04de8e08117399d5cab68042cc794bc2199/tests/sdk/conversation/test_interrupt.py) stop a mock async LLM and retain a cancellation token for tool threads that outlive the main run. Tool interruption remains cooperative. The repository also contains a [local TypeScript loop](https://github.com/OpenHands/software-agent-sdk/blob/87bec04de8e08117399d5cab68042cc794bc2199/clients/typescript/src/conversation/local-conversation.ts). Its inspected file declares and stores `persistenceDir` without further use. Do not assume feature parity with Python based on the shared package name.

## Factory patterns to select and defer

| Source | Concrete evidence | Selected pattern | Deferred dependency or behavior |
|---|---|---|---|
| Gas City | [Fresh effect fences](https://github.com/gastownhall/gascity/blob/87a47634d0cebe611874df321c723bba95dcffea/cmd/gc/reconcile_effect_fence.go) and [lease takeover tests](https://github.com/gastownhall/gascity/blob/87a47634d0cebe611874df321c723bba95dcffea/cmd/gc/reconcile_effect_lease_test.go) | Unknown observations cannot prove absence. A stale owner cannot commit after takeover | Go controller, backend fleet and deployment topology |
| Gas Town | [Checkpoint fields and writer](https://github.com/gastownhall/gastown/blob/ef70b0bf9e588dce00708e36564bfc06b51fac2c/internal/checkpoint/checkpoint.go), [checkpoint tests](https://github.com/gastownhall/gastown/blob/ef70b0bf9e588dce00708e36564bfc06b51fac2c/internal/checkpoint/checkpoint_test.go) | Task step, Git base, modified paths and visible handoff notes | Direct checkpoint write as durability primitive, tmux startup and agent-specific provisioning |
| Beads | [Conditional claims](https://github.com/gastownhall/beads/blob/bc0cc87fd9fda412e2eaca669fa6ae3ec2c713fb/backend/conformance/claim.go), [transactional dependency tests](https://github.com/gastownhall/beads/blob/bc0cc87fd9fda412e2eaca669fa6ae3ec2c713fb/internal/storage/embeddeddolt/transaction_wisp_lifecycle_persistence_test.go) | Dependency DAG, expected-owner release and guarded lifecycle transitions | Dolt/cgo backend and experimental full extension interface |
| Fusion | [Workspace leases](https://github.com/Runfusion/Fusion/blob/5bdc9ea1132ba4d1e49d0dfc5ea084e0e01eb5c3/packages/core/src/task-store/workspace-leases.ts), [handoff rollback test](https://github.com/Runfusion/Fusion/blob/5bdc9ea1132ba4d1e49d0dfc5ea084e0e01eb5c3/packages/core/src/__tests__/postgres/handoff-to-review-atomicity.pg.test.ts) | Increasing fence tokens, intent records and atomic review state plus audit transition | Postgres/Docker operational stack, automatic merge policy |
| Mastra template | [Application configuration](https://github.com/mastra-ai/softwarefactory-template/blob/ebde794a8656278cf26bbb2b8764be7070b325d9/src/mastra/index.ts) | Explicit auth, storage and sandbox selection before deployment | Server-centric factory dependency and Postgres/Redis requirements |
| Mastra article | Author describes per-stage manual/automatic settings and review pressure | Stage-specific grants, bounded queues and visible task sessions | Productivity percentages as product benchmarks |
| Glass Town article | Author describes finite worker runs, deterministic preview and commit-linked sessions | Concrete role outputs, budget stops and evidence attached to changes | Automatic per-file commit and push, mandatory remote VM |

Gas City has explicit runtime capability interfaces and multiple backend concepts. Gas Town's inspected local provider does not implement model message creation. Fusion already embeds Pi libraries and offers runtime adapters, so these projects should not all receive a generic CLI-only label.

Fusion's [child-process implementation](https://github.com/Runfusion/Fusion/blob/5bdc9ea1132ba4d1e49d0dfc5ea084e0e01eb5c3/packages/engine/src/runtimes/child-process-runtime.ts) provides an instructive audit case. The inspected `killChild` schedules a fallback that reads `this.child`, then clears that reference. This appears to prevent the fallback from targeting the original process. It is a source inference, not a reproduced failure. Borrow the generation-fencing idea and independently test process termination.

The Mastra template's complete tree contains no tests or license file. Its package manifest declares Apache-2.0, and its factory implementation is supplied by dependencies. Its configuration requires a database and explicit sandbox provider outside development. That is useful deployment evidence, not a durability audit of the factory engine.

## Experiments that decide adoption

| Probe | Required observation | Failure consequence |
|---|---|---|
| Provider capability | One selected API provider and one selected local endpoint complete text, tool and cancellation fixtures. Unsupported strict output is rejected or visibly downgraded before use | Disable unsupported operation for that profile, retain other working profiles |
| Stream settlement | Truncated JSON, missing finish, disconnect and duplicated chunks never execute an incomplete mutation | Block runtime adoption until repaired behind adapter |
| Final validation | Hook changes argument type, path or tool name. Host rejects unauthorized or invalid final call | Block write capability |
| Model handoff | Same C-06 bundle moves across two provider families with tool outcomes preserved and foreign thought artifacts omitted | Keep switching read-only until resolved |
| Crash windows | Kill before intent, after intent, after effect and before receipt. Recovery distinguishes never-started, completed and unknown effect | No automatic replay of ambiguous effects |
| Double ownership | Two host processes contend. Old generation cannot write after successor admission | Block concurrent workspace admission |
| Cancellation | Stop model stream, cooperative tool, resistant child and remote accepted job separately. UI state matches observed effects | Keep unknown effects isolated and require reconciliation |
| Database and files | Inject disk full, malformed record and failure between staged file writes. No accepted mixed snapshot appears | Block source application until integrity restored |
| Packaging | Bundled runtime starts offline on target Windows machine without developer Node/Python or terminal UI | Reconsider shell/runtime combination |
| Cost and usefulness | Same bounded HOI4 fixtures compare user intervention, validation failures, tokens and wall time | Revise policy or scope from measured results |

S01 should choose the runtime and persistence boundary after these architectural probes. S03 establishes scoped tools and durable attempts. S04 proves useful focus editing in manual, assisted and bounded autonomous modes. S05 hardens model switching and recovery. S08 repeats the selected support matrix on distributable builds. Missing game execution leaves game compatibility unknown while static checks and source review remain useful.


## Guidance and release identity closeout

The final research pass inspected all present named root guidance plus relevant runtime and authority boundary documents. It added 26 fully read guidance files to the evidence ledger and reassessed the recommendation. No repository guidance command, installer, test suite or generated operational context was executed. [Detailed coverage](evidence-b.json)

OpenCode's inspected SDK/core manifests are development version 2.1.4, while the observed latest stable release is v1.19.4. A future package must use and verify the selected exact source closure, not assume that stable installation exposes the audited development API. Fusion's inspected core and engine are private beta packages at 0.78.0-beta.9, with observed stable release v0.77.0. These facts increase packaging and support uncertainty.

Gas City and Gas Town describe experimental boundaries. Gas City's current packages are private to its binary. Gas Town points to a root CLAUDE file absent from the pinned tree, and its generated gt prime context was not obtained. The tracked town-root template was inspected as such. OpenHands' TypeScript guidance calls LocalWorkspace a stub and directs readers to the package manifest, whose 1.58.1 pin supersedes stale 1.51 examples. Mastra's complete pinned tree has no named root guidance or root LICENSE text. Its release collection is empty, while exhaustive tag history remains unknown.

The recommendation remains a narrow Pi experiment with a substantive OpenCode alternative. Runtime/framework guidance informs reuse boundaries, while the new product's source, grant and effect contracts remain its own responsibility.
