# Research sources and evidence registry

Inspection date: 2026-10-23. All requested starting sources are accounted for below. The machine-readable [source registry](source-registry.json) contains normalized evidence labels and links to full records. The detailed ledgers preserve exact inspected paths, coverage ranges, version pins, findings and limitations.

**Verified** means implementation inspection or a named reproducible check, within its stated scope. It never silently means a runtime test passed. **Documented only** includes official client/protocol descriptions and paper/vendor claims whose behavior was not executed here. All product designs remain **Proposed**, with execution and applicability gaps **Unknown**.

## Supplied Project Sources

The local protocol and authorized Chaos Redux examples were fully read before external research. [Coverage metadata](supplied-source-coverage.json) records 152 distinct text contents, 250 full chunks and exact duplicate accounting. The plan adopts bounded packets, ownership, explicit gates and handoff discipline. It does not import event quotas, game-specific story requirements or old environment assumptions. See [the research log](04-evidence-and-research-log.md).

## Existing HOI4 assets

The [detailed asset audit](02-existing-assets-audit.md) covers actual code, UI flow, transaction boundaries, source preservation, tests, licenses and maintenance. Exact inspected repository pins follow.

| Repository | Audited revision | License boundary | Detailed evidence |
|---|---|---|---|
| [klimPaskov/HOI4-Mod-Setup](https://github.com/klimPaskov/HOI4-Mod-Setup/tree/4488a418cad56a158273cb66c4ce7794696b7ed1) | 4488a418cad56a158273cb66c4ce7794696b7ed1 | Apache-2.0 | SRC-A01, SRC-A02, SRC-A03, SRC-A04, SRC-A05, SRC-A06, SRC-A07, SRC-A08, SRC-A09 |
| [klimPaskov/hoi4-agent-tools](https://github.com/klimPaskov/hoi4-agent-tools/tree/25f5641bbc07c65a822ef576ac6e92bd12666880) | 25f5641bbc07c65a822ef576ac6e92bd12666880 | Apache-2.0 | SRC-A10, SRC-A11, SRC-A12, SRC-A13, SRC-A14, SRC-A15, SRC-A16 |
| [klimPaskov/HOI4-Live-Tools](https://github.com/klimPaskov/HOI4-Live-Tools/tree/6b91a2a43edfeab60a415ec2ddd9b71d215b8a56) | 6b91a2a43edfeab60a415ec2ddd9b71d215b8a56 | Apache-2.0 | SRC-A17, SRC-A18, SRC-A19 |
| [cwtools/cwtools](https://github.com/cwtools/cwtools/tree/b377453dee803f9258be92cfc49896d09039702d) | b377453dee803f9258be92cfc49896d09039702d | MIT | SRC-A20, SRC-A21 |
| [cwtools/cwtools-hoi4-config](https://github.com/cwtools/cwtools-hoi4-config/tree/297dd69c42aabe10078a8b7ab7f2d07ee631d4ef) | 297dd69c42aabe10078a8b7ab7f2d07ee631d4ef | MIT | SRC-A22 |
| [Floxizdia/hoi4-modmaker](https://github.com/Floxizdia/hoi4-modmaker/tree/a5c138ffacf4ed471769558f864cbe55ec5163fa) | a5c138ffacf4ed471769558f864cbe55ec5163fa | MIT | SRC-A23 |
| [battleskorpion/HOIIVUtils](https://github.com/battleskorpion/HOIIVUtils/tree/0fe06450bf17c506485caf90fb5d52cf8a2e1b33) | 0fe06450bf17c506485caf90fb5d52cf8a2e1b33 | Apache-2.0 | SRC-A24 |
| [Ultra-XT/HOI4-Modding-Suite](https://github.com/Ultra-XT/HOI4-Modding-Suite/tree/5e44bc4f483e504310a0d4bad6e5ecc3c1cd08a6) | 5e44bc4f483e504310a0d4bad6e5ecc3c1cd08a6 | CC-BY-NC-ND-4.0 with repository-specific explanatory restrictions | SRC-A25 |
| [klimPaskov/Agentic-HOI4-Modding](https://github.com/klimPaskov/Agentic-HOI4-Modding/tree/856940f2a629e0e2f3e03fc484cb0c0309c96734) | 856940f2a629e0e2f3e03fc484cb0c0309c96734 | Apache-2.0 original content, NOTICE exclusions | SRC-C01 through SRC-C08, SRC-C26 |

## Native runtimes and factory systems

Read [the candidate comparison](03-candidate-comparison.md). Pins below identify inspected source, not necessarily the latest stable release. Package versions and release metadata are separate observations in the detailed record.

| Source | Audited revision or document | Decision role | Record |
|---|---|---|---|
| [earendil-works/pi](https://github.com/earendil-works/pi) | 8a2ebd38549c3521a02db0e5d758a24320e97f94 | Embed pi-ai plus pi-agent-core behind a bundled worker. Evaluate pi-durable in S01 without introducing a second authoritative task database. Borrow replay and cancellation test shapes. | SRC-B01 |
| [anomalyco/opencode](https://github.com/anomalyco/opencode) | 02671e66454260d77fe04d63cf5eb94e76921dee | Keep an Electron/TypeScript embedded-SDK alternative with explicit persistent database and Rust workspace authority. Reuse host-side schemas and effect controls even if choosing this SDK. | SRC-B02 |
| [OpenHands/software-agent-sdk](https://github.com/OpenHands/software-agent-sdk) | 87bec04de8e08117399d5cab68042cc794bc2199 | Retain as measured alternative if Python tool integration clearly wins. Do not assume its TypeScript local loop has persistence parity with Python. | SRC-B03 |
| [gastownhall/gascity](https://github.com/gastownhall/gascity) | 87a47634d0cebe611874df321c723bba95dcffea | Adapt observed-state uncertainty, owner incarnation and fencing test invariants. Defer whole Go controller and deployment topology. | SRC-B04 |
| [gastownhall/gastown](https://github.com/gastownhall/gastown) | ef70b0bf9e588dce00708e36564bfc06b51fac2c | Borrow checkpoint fields and human-readable recovery handoff. Do not transplant CLI, tmux role provisioning or checkpoint writer as native durability. | SRC-B05 |
| [gastownhall/beads](https://github.com/gastownhall/beads) | bc0cc87fd9fda412e2eaca669fa6ae3ec2c713fb | Implement a smaller local task model with these invariants. Defer wholesale Dolt backend, CLI and experimental extension interface. | SRC-B06 |
| [Runfusion/Fusion](https://github.com/Runfusion/Fusion) | 5bdc9ea1132ba4d1e49d0dfc5ea084e0e01eb5c3 | Adapt monotonic ownership fencing, intent-before-publish and transactional review handoff tests. Keep application-specific runtime and database authority independent. | SRC-B07 |
| [mastra-ai/softwarefactory-template](https://github.com/mastra-ai/softwarefactory-template) | ebde794a8656278cf26bbb2b8764be7070b325d9 | Borrow per-stage controls and visible task/session views. Defer Mastra server, Postgres/Redis and hosted-sandbox architecture for local MVP. Confirm distribution notices for exact dependencies before copying. | SRC-B08 |
| [Announcing Mastra Factory Beta](https://mastra.ai/blog/announcing-mastra-factory-beta) | 2026-09-08 | Adopt stage-specific policy and queue limits as design ideas. | SRC-B09 |
| [Welcome to Glass Town: building a software factory with a paper trail](https://aaif.io/blog/welcome-to-glass-town-building-a-software-factory-with-a-paper-trail) | 2026-10-14 | Use bounded roles and traceable evidence. Do not adopt automatic per-file push as product workflow. | SRC-B10 |

## Context, protocols, evaluation and deployment references

The [context and protocol synthesis](06-context-protocols-and-evaluation-research.md) distinguishes approved knowledge, observability, retrieval and workflow patterns. Glass Town in the prior section and agentglass below are separate sources.

| Source | Revision / inspection basis | Architectural use | Record |
|---|---|---|---|
| [Agent Beacon](https://github.com/Asymptote-Labs/agent-beacon) | cfbd7b1d721664e3b1b5ca2fca5eac4327532c07 | Adapt design and tests as patterns. Do not require Beacon Cloud or evaluator service. Native product should own canon and policy decisions. | SRC-R01 |
| [Entire CLI](https://github.com/entireio/cli) | ee57ff55142e5078b625dd963fc398d49a8ddda1 | Adapt checkpoint provenance and guarded Git update patterns. Keep full run database outside normal mod history, publish redacted evidence references. | SRC-R02 |
| [agentglass](https://github.com/BjoernSchotte/agentglass) | 3fdd14eafdc873f23b009a273580b4a0ff9b5ca0 | Adapt observability vocabulary and unknown-cost handling. Optional export integration after native traces are stable. | SRC-R03 |
| [MFS](https://github.com/zilliztech/mfs) | 78352894a0a826ee24db852411bf9a3c49884922 | Borrow fingerprint, locator and reconciliation concepts. Defer MFS service or vector stack until exact search and domain indexes fail measured retrieval tasks. | SRC-R04 |
| [MCP specification and Tasks extension](https://modelcontextprotocol.io/specification/2026-07-28) | Official documentation read | Negotiate actual server version and capabilities. Support existing HOI4 server versions through adapters. Do not assume a session-based handshake or task support universally. | SRC-R05 |
| [Agent Skills specification](https://agentskills.io/specification) | Official specification read | Portable workflow body and explicit client-specific discovery mirrors. Validate metadata, paths and cross-client drift. | SRC-R06 |
| [Agent Client Protocol](https://agentclientprotocol.com/protocol/v1/overview) | Official overview and initialization read | Optional external editor interoperability after native product works. Do not put ACP between every internal module or depend on an external agent executable. | SRC-R07 |
| [LiteLLM](https://docs.litellm.ai/docs/) | Official getting-started and routing docs read | Optional user-managed gateway or Python alternative. Avoid adding a gateway service solely to obtain a uniform API if the selected runtime already has tested adapters. | SRC-R08 |
| [Harness Engineering source-code study](https://arxiv.org/html/2609.00006v1) | Research paper sections 4, 15, 16 inspected | Use taxonomy as a research checklist. Recheck candidate implementations at current pins. Do not repeat its benchmark numbers as HOI4 evidence. | SRC-R09 |
| [AI Harness Engineering runtime substrate](https://arxiv.org/html/2605.13357v1) | Methods, evidence ladder, controlled case and outlook inspected | Adapt requirement-linked evidence and intervention metrics into proposed HOI4 evaluation. Separate behavioral correctness from evidence completeness. | SRC-R10 |
| [byProduct lifecycle](https://byproduct.ai/docs/how-it-works) | Vendor operating-model documentation | Use a compact goal-to-evidence trace for stage packets. No dependency on its service. | SRC-R11 |
| [Warp software factory article](https://www.warp.dev/articles/coding-agents-to-software-factory) | Vendor article dated 2026-08-25 | Adopt bounded workflow rollout as a hypothesis. Keep local-first deployment because the user's game and files are local. | SRC-R12 |
| [Paradox Interactive User Agreement](https://store.steampowered.com/eula/3493540_eula_0) | User-supplied publisher agreement read, legal applicability not adjudicated | Keep game/DLC bytes local and out of distribution, record asset provenance, and make commercial product/UGC licensing review a release gate. | SRC-R13 |
| [Tauri v2 capabilities and external binaries](https://v2.tauri.app/security/capabilities/) | Official capability and sidecar documentation inspected on 2026-10-23 | Candidate A uses documented packaging and host APIs, with explicit separate worker containment experiments. | SRC-R14 |
| [Electron security](https://www.electronjs.org/docs/latest/tutorial/security) | Official renderer security guidance inspected on 2026-10-23 | Candidate B keeps the renderer isolated and puts privileged operations behind the same host authority contract. | SRC-R15 |
| [SQLite atomic commit](https://www.sqlite.org/atomiccommit.html) | Official transaction and filesystem-assumption discussion inspected on 2026-10-23 | Use a transactional operational store with explicit multi-store intent, receipts, reconciliation and backup tests. | SRC-R16 |

## Developer instructions, skills and configuration

[Developer research](05-developer-guidance-research.md) and [the full migration report](../07-developer-workflow/CONFIG-MIGRATION-REPORT.md) explain current client behavior, exact source config and supported starter decisions. Official documentation is time-sensitive. Its inspected date and schema pins are recorded, and S01 must check the installed client.

| Record | Primary source | Evidence status and scope |
|---|---|---|
| SRC-C01 | [Agentic HOI4 Modding revision](https://github.com/klimPaskov/Agentic-HOI4-Modding/commit/856940f2a629e0e2f3e03fc484cb0c0309c96734) | Verified. repository_source |
| SRC-C02 | [Actual Agentic root Codex configuration](https://github.com/klimPaskov/Agentic-HOI4-Modding/blob/856940f2a629e0e2f3e03fc484cb0c0309c96734/.codex/config.toml) | Verified. repository_source |
| SRC-C03 | [Actual Agentic Codex role layers](https://github.com/klimPaskov/Agentic-HOI4-Modding/tree/856940f2a629e0e2f3e03fc484cb0c0309c96734/.codex/agents) | Verified. repository_source |
| SRC-C04 | [Canonical Claude roles and settings](https://github.com/klimPaskov/Agentic-HOI4-Modding/tree/856940f2a629e0e2f3e03fc484cb0c0309c96734/.claude) | Verified. repository_source |
| SRC-C05 | [Agentic projection and synchronization](https://github.com/klimPaskov/Agentic-HOI4-Modding/blob/856940f2a629e0e2f3e03fc484cb0c0309c96734/.tools/sync/README.md) | Verified. repository_source |
| SRC-C06 | [Agentic setup manifest](https://github.com/klimPaskov/Agentic-HOI4-Modding/blob/856940f2a629e0e2f3e03fc484cb0c0309c96734/hoi4-mod-setup.manifest.json) | Verified. repository_source |
| SRC-C07 | [Agentic license and attribution](https://github.com/klimPaskov/Agentic-HOI4-Modding/blob/856940f2a629e0e2f3e03fc484cb0c0309c96734/NOTICE) | Verified. repository_source |
| SRC-C08 | [Agentic test and CI contracts](https://github.com/klimPaskov/Agentic-HOI4-Modding/blob/856940f2a629e0e2f3e03fc484cb0c0309c96734/scripts/test_agent_sync_contract.py) | Verified. repository_source |
| SRC-C09 | [Codex basic configuration](https://learn.chatgpt.com/docs/config-file/config-basic) | Documented only. official_documentation |
| SRC-C10 | [Codex configuration reference](https://learn.chatgpt.com/docs/config-file/config-reference) | Documented only. official_documentation |
| SRC-C11 | [Codex exact configuration schema](https://github.com/openai/codex/blob/f71c6afc30031011fe0b4d757b981fd0301b2536/codex-rs/core/config.schema.json) | Verified. schema_source |
| SRC-C12 | [Codex skill discovery and metadata](https://learn.chatgpt.com/docs/build-skills) | Documented only. official_documentation |
| SRC-C13 | [Codex AGENTS discovery](https://learn.chatgpt.com/docs/agent-configuration/agents-md) | Documented only. official_documentation |
| SRC-C14 | [Evaluating skills](https://developers.openai.com/blog/eval-skills) | Documented only. official_documentation |
| SRC-C15 | [Iterating engineering workflows](https://developers.openai.com/cookbook/examples/codex/iterating-development-workflows-with-codex) | Documented only. official_documentation |
| SRC-C16 | [Claude project memory and imports](https://code.claude.com/docs/en/memory) | Documented only. official_documentation |
| SRC-C17 | [Claude skills](https://code.claude.com/docs/en/skills) | Documented only. official_documentation |
| SRC-C18 | [Claude MCP](https://code.claude.com/docs/en/mcp) | Documented only. official_documentation |
| SRC-C19 | [Claude settings](https://code.claude.com/docs/en/settings) | Documented only. official_documentation |
| SRC-C20 | [Claude settings schema](https://github.com/SchemaStore/schemastore/blob/bf7c6745505ccace5568dc0d7f4ae29647e9b41a/src/schemas/json/claude-code-settings.json) | Verified. schema_source |
| SRC-C21 | [Claude subagents](https://code.claude.com/docs/en/sub-agents) | Documented only. official_documentation |
| SRC-C22 | [Claude sessions](https://code.claude.com/docs/en/sessions) | Documented only. official_documentation |
| SRC-C23 | [Claude worktrees](https://code.claude.com/docs/en/worktrees) | Documented only. official_documentation |
| SRC-C24 | [Claude permissions](https://code.claude.com/docs/en/permissions) | Documented only. official_documentation |
| SRC-C25 | [Agent Skills standard](https://agentskills.io/specification) | Documented only. official_documentation |
| SRC-C26 | [Agentic HOI4 Agent Tools bootstrap implementation and contract tests](https://github.com/klimPaskov/Agentic-HOI4-Modding/blob/856940f2a629e0e2f3e03fc484cb0c0309c96734/.tools/mcp/bootstrap_hoi4_agent_tools.py) | Verified. repository_source |

## Detailed evidence entry index

Each entry below has an exact source link and a full record in its track ledger. These are source assertions, not invented product test results. Source files fetched but not fully read are distinguished from inspected sections in the detailed coverage.

| ID | Topic | Status | Full record file |
|---|---|---|---|
| SRC-A01 | [Setup architecture and license](https://github.com/klimPaskov/HOI4-Mod-Setup/blob/4488a418cad56a158273cb66c4ce7794696b7ed1/AGENTS.md) | Verified | [evidence-a.json](evidence-a.json) |
| SRC-A02 | [Actual wizard routes and controls](https://github.com/klimPaskov/HOI4-Mod-Setup/blob/4488a418cad56a158273cb66c4ce7794696b7ed1/src/App.tsx) | Verified | [evidence-a.json](evidence-a.json) |
| SRC-A03 | [Read-only import scanner](https://github.com/klimPaskov/HOI4-Mod-Setup/blob/4488a418cad56a158273cb66c4ce7794696b7ed1/src-tauri/src/scanner.rs) | Verified | [evidence-a.json](evidence-a.json) |
| SRC-A04 | [Assistant analysis and source closure](https://github.com/klimPaskov/HOI4-Mod-Setup/blob/4488a418cad56a158273cb66c4ce7794696b7ed1/AGENTS.md) | Verified | [evidence-a.json](evidence-a.json) |
| SRC-A05 | [Plan outputs and conflict ownership](https://github.com/klimPaskov/HOI4-Mod-Setup/blob/4488a418cad56a158273cb66c4ce7794696b7ed1/src-tauri/src/commands.rs) | Verified | [evidence-a.json](evidence-a.json) |
| SRC-A06 | [Desktop transaction and recovery states](https://github.com/klimPaskov/HOI4-Mod-Setup/blob/4488a418cad56a158273cb66c4ce7794696b7ed1/src-tauri/src/transaction.rs) | Verified | [evidence-a.json](evidence-a.json) |
| SRC-A07 | [Lifecycle tests and maintenance preservation](https://github.com/klimPaskov/HOI4-Mod-Setup/blob/4488a418cad56a158273cb66c4ce7794696b7ed1/src-tauri/src/commands_lifecycle_tests.rs) | Verified | [evidence-a.json](evidence-a.json) |
| SRC-A08 | [Browser export differs from desktop transaction](https://github.com/klimPaskov/HOI4-Mod-Setup/blob/4488a418cad56a158273cb66c4ce7794696b7ed1/crates/hoi4-setup-web/src/export.rs) | Verified | [evidence-a.json](evidence-a.json) |
| SRC-A09 | [Exact-head CI recovery failure](https://github.com/klimPaskov/HOI4-Mod-Setup/blob/4488a418cad56a158273cb66c4ce7794696b7ed1/scripts/run_released_e2e.mjs) | Verified | [evidence-a.json](evidence-a.json) |
| SRC-A10 | [MCP product boundary and tool surface](https://github.com/klimPaskov/hoi4-agent-tools/blob/25f5641bbc07c65a822ef576ac6e92bd12666880/AGENTS.md) | Verified | [evidence-a.json](evidence-a.json) |
| SRC-A11 | [Source preservation and internal transaction execution](https://github.com/klimPaskov/hoi4-agent-tools/blob/25f5641bbc07c65a822ef576ac6e92bd12666880/src/hoi4_agent_tools/core/source/encoding.ts) | Verified | [evidence-a.json](evidence-a.json) |
| SRC-A12 | [Focus authoring and preview evidence](https://github.com/klimPaskov/hoi4-agent-tools/blob/25f5641bbc07c65a822ef576ac6e92bd12666880/src/hoi4_agent_tools/mcp/tools/focus.ts) | Verified | [evidence-a.json](evidence-a.json) |
| SRC-A13 | [Map capabilities and structured geography limitation](https://github.com/klimPaskov/hoi4-agent-tools/blob/25f5641bbc07c65a822ef576ac6e92bd12666880/src/hoi4_agent_tools/mcp/tools/map.ts) | Verified | [evidence-a.json](evidence-a.json) |
| SRC-A14 | [Source interpretation is not game simulation](https://github.com/klimPaskov/hoi4-agent-tools/blob/25f5641bbc07c65a822ef576ac6e92bd12666880/docs/mechanics.md) | Verified | [evidence-a.json](evidence-a.json) |
| SRC-A15 | [HTTP authorization and session boundaries](https://github.com/klimPaskov/hoi4-agent-tools/blob/25f5641bbc07c65a822ef576ac6e92bd12666880/docs/http.md) | Verified | [evidence-a.json](evidence-a.json) |
| SRC-A16 | [Agent Tools license, dependencies and CI](https://github.com/klimPaskov/hoi4-agent-tools/blob/25f5641bbc07c65a822ef576ac6e92bd12666880/LICENSE) | Verified | [evidence-a.json](evidence-a.json) |
| SRC-A17 | [Native Live Tools capability limits](https://github.com/klimPaskov/HOI4-Live-Tools/blob/6b91a2a43edfeab60a415ec2ddd9b71d215b8a56/README.md) | Verified | [evidence-a.json](evidence-a.json) |
| SRC-A18 | [Live action journal and uncertain effects](https://github.com/klimPaskov/HOI4-Live-Tools/blob/6b91a2a43edfeab60a415ec2ddd9b71d215b8a56/native/src/actions.rs) | Verified | [evidence-a.json](evidence-a.json) |
| SRC-A19 | [Live licensing, campaign policy and CI](https://github.com/klimPaskov/HOI4-Live-Tools/blob/6b91a2a43edfeab60a415ec2ddd9b71d215b8a56/LICENSE) | Verified | [evidence-a.json](evidence-a.json) |
| SRC-A20 | [CWTools HOI4 validation services](https://github.com/cwtools/cwtools/blob/b377453dee803f9258be92cfc49896d09039702d/README.md) | Verified | [evidence-a.json](evidence-a.json) |
| SRC-A21 | [CWTools printer normalizes output](https://github.com/cwtools/cwtools/blob/b377453dee803f9258be92cfc49896d09039702d/CWTools/Parser/Printer.fs) | Verified | [evidence-a.json](evidence-a.json) |
| SRC-A22 | [HOI4 rule pack and limits](https://github.com/cwtools/cwtools-hoi4-config/blob/297dd69c42aabe10078a8b7ab7f2d07ee631d4ef/README.md) | Verified | [evidence-a.json](evidence-a.json) |
| SRC-A23 | [Modmaker editing and map comparison](https://github.com/Floxizdia/hoi4-modmaker/blob/a5c138ffacf4ed471769558f864cbe55ec5163fa/README.md) | Verified | [evidence-a.json](evidence-a.json) |
| SRC-A24 | [HOIIVUtils parser and province generation comparison](https://github.com/battleskorpion/HOIIVUtils/blob/0fe06450bf17c506485caf90fb5d52cf8a2e1b33/README.md) | Verified | [evidence-a.json](evidence-a.json) |
| SRC-A25 | [Nova comparison-only license and absent public implementation](https://github.com/Ultra-XT/HOI4-Modding-Suite/blob/5e44bc4f483e504310a0d4bad6e5ecc3c1cd08a6/LICENSE) | Verified | [evidence-a.json](evidence-a.json) |
| SRC-B01 | [earendil-works/pi](https://github.com/earendil-works/pi) | Verified | [evidence-b.json](evidence-b.json) |
| SRC-B02 | [anomalyco/opencode](https://github.com/anomalyco/opencode) | Verified | [evidence-b.json](evidence-b.json) |
| SRC-B03 | [OpenHands/software-agent-sdk](https://github.com/OpenHands/software-agent-sdk) | Verified | [evidence-b.json](evidence-b.json) |
| SRC-B04 | [gastownhall/gascity](https://github.com/gastownhall/gascity) | Verified | [evidence-b.json](evidence-b.json) |
| SRC-B05 | [gastownhall/gastown](https://github.com/gastownhall/gastown) | Verified | [evidence-b.json](evidence-b.json) |
| SRC-B06 | [gastownhall/beads](https://github.com/gastownhall/beads) | Verified | [evidence-b.json](evidence-b.json) |
| SRC-B07 | [Runfusion/Fusion](https://github.com/Runfusion/Fusion) | Verified | [evidence-b.json](evidence-b.json) |
| SRC-B08 | [mastra-ai/softwarefactory-template](https://github.com/mastra-ai/softwarefactory-template) | Verified | [evidence-b.json](evidence-b.json) |
| SRC-B09 | [Announcing Mastra Factory Beta](https://mastra.ai/blog/announcing-mastra-factory-beta) | Documented only | [evidence-b.json](evidence-b.json) |
| SRC-B10 | [Welcome to Glass Town: building a software factory with a paper trail](https://aaif.io/blog/welcome-to-glass-town-building-a-software-factory-with-a-paper-trail) | Documented only | [evidence-b.json](evidence-b.json) |
| SRC-C01 | [Agentic HOI4 Modding revision](https://github.com/klimPaskov/Agentic-HOI4-Modding/commit/856940f2a629e0e2f3e03fc484cb0c0309c96734) | Verified | [evidence-c.json](evidence-c.json) |
| SRC-C02 | [Actual Agentic root Codex configuration](https://github.com/klimPaskov/Agentic-HOI4-Modding/blob/856940f2a629e0e2f3e03fc484cb0c0309c96734/.codex/config.toml) | Verified | [evidence-c.json](evidence-c.json) |
| SRC-C03 | [Actual Agentic Codex role layers](https://github.com/klimPaskov/Agentic-HOI4-Modding/tree/856940f2a629e0e2f3e03fc484cb0c0309c96734/.codex/agents) | Verified | [evidence-c.json](evidence-c.json) |
| SRC-C04 | [Canonical Claude roles and settings](https://github.com/klimPaskov/Agentic-HOI4-Modding/tree/856940f2a629e0e2f3e03fc484cb0c0309c96734/.claude) | Verified | [evidence-c.json](evidence-c.json) |
| SRC-C05 | [Agentic projection and synchronization](https://github.com/klimPaskov/Agentic-HOI4-Modding/blob/856940f2a629e0e2f3e03fc484cb0c0309c96734/.tools/sync/README.md) | Verified | [evidence-c.json](evidence-c.json) |
| SRC-C06 | [Agentic setup manifest](https://github.com/klimPaskov/Agentic-HOI4-Modding/blob/856940f2a629e0e2f3e03fc484cb0c0309c96734/hoi4-mod-setup.manifest.json) | Verified | [evidence-c.json](evidence-c.json) |
| SRC-C07 | [Agentic license and attribution](https://github.com/klimPaskov/Agentic-HOI4-Modding/blob/856940f2a629e0e2f3e03fc484cb0c0309c96734/NOTICE) | Verified | [evidence-c.json](evidence-c.json) |
| SRC-C08 | [Agentic test and CI contracts](https://github.com/klimPaskov/Agentic-HOI4-Modding/blob/856940f2a629e0e2f3e03fc484cb0c0309c96734/scripts/test_agent_sync_contract.py) | Verified | [evidence-c.json](evidence-c.json) |
| SRC-C09 | [Codex basic configuration](https://learn.chatgpt.com/docs/config-file/config-basic) | Documented only | [evidence-c.json](evidence-c.json) |
| SRC-C10 | [Codex configuration reference](https://learn.chatgpt.com/docs/config-file/config-reference) | Documented only | [evidence-c.json](evidence-c.json) |
| SRC-C11 | [Codex exact configuration schema](https://github.com/openai/codex/blob/f71c6afc30031011fe0b4d757b981fd0301b2536/codex-rs/core/config.schema.json) | Verified | [evidence-c.json](evidence-c.json) |
| SRC-C12 | [Codex skill discovery and metadata](https://learn.chatgpt.com/docs/build-skills) | Documented only | [evidence-c.json](evidence-c.json) |
| SRC-C13 | [Codex AGENTS discovery](https://learn.chatgpt.com/docs/agent-configuration/agents-md) | Documented only | [evidence-c.json](evidence-c.json) |
| SRC-C14 | [Evaluating skills](https://developers.openai.com/blog/eval-skills) | Documented only | [evidence-c.json](evidence-c.json) |
| SRC-C15 | [Iterating engineering workflows](https://developers.openai.com/cookbook/examples/codex/iterating-development-workflows-with-codex) | Documented only | [evidence-c.json](evidence-c.json) |
| SRC-C16 | [Claude project memory and imports](https://code.claude.com/docs/en/memory) | Documented only | [evidence-c.json](evidence-c.json) |
| SRC-C17 | [Claude skills](https://code.claude.com/docs/en/skills) | Documented only | [evidence-c.json](evidence-c.json) |
| SRC-C18 | [Claude MCP](https://code.claude.com/docs/en/mcp) | Documented only | [evidence-c.json](evidence-c.json) |
| SRC-C19 | [Claude settings](https://code.claude.com/docs/en/settings) | Documented only | [evidence-c.json](evidence-c.json) |
| SRC-C20 | [Claude settings schema](https://github.com/SchemaStore/schemastore/blob/bf7c6745505ccace5568dc0d7f4ae29647e9b41a/src/schemas/json/claude-code-settings.json) | Verified | [evidence-c.json](evidence-c.json) |
| SRC-C21 | [Claude subagents](https://code.claude.com/docs/en/sub-agents) | Documented only | [evidence-c.json](evidence-c.json) |
| SRC-C22 | [Claude sessions](https://code.claude.com/docs/en/sessions) | Documented only | [evidence-c.json](evidence-c.json) |
| SRC-C23 | [Claude worktrees](https://code.claude.com/docs/en/worktrees) | Documented only | [evidence-c.json](evidence-c.json) |
| SRC-C24 | [Claude permissions](https://code.claude.com/docs/en/permissions) | Documented only | [evidence-c.json](evidence-c.json) |
| SRC-C25 | [Agent Skills standard](https://agentskills.io/specification) | Documented only | [evidence-c.json](evidence-c.json) |
| SRC-C26 | [Agentic HOI4 Agent Tools bootstrap implementation and contract tests](https://github.com/klimPaskov/Agentic-HOI4-Modding/blob/856940f2a629e0e2f3e03fc484cb0c0309c96734/.tools/mcp/bootstrap_hoi4_agent_tools.py) | Verified | [evidence-c.json](evidence-c.json) |
| SRC-R01 | [Agent Beacon](https://github.com/Asymptote-Labs/agent-beacon) | Verified | [evidence-root.json](evidence-root.json) |
| SRC-R02 | [Entire CLI](https://github.com/entireio/cli) | Verified | [evidence-root.json](evidence-root.json) |
| SRC-R03 | [agentglass](https://github.com/BjoernSchotte/agentglass) | Verified | [evidence-root.json](evidence-root.json) |
| SRC-R04 | [MFS](https://github.com/zilliztech/mfs) | Verified | [evidence-root.json](evidence-root.json) |
| SRC-R05 | [MCP specification and Tasks extension](https://modelcontextprotocol.io/specification/2026-07-28) | Documented only | [evidence-root.json](evidence-root.json) |
| SRC-R06 | [Agent Skills specification](https://agentskills.io/specification) | Documented only | [evidence-root.json](evidence-root.json) |
| SRC-R07 | [Agent Client Protocol](https://agentclientprotocol.com/protocol/v1/overview) | Documented only | [evidence-root.json](evidence-root.json) |
| SRC-R08 | [LiteLLM](https://docs.litellm.ai/docs/) | Documented only | [evidence-root.json](evidence-root.json) |
| SRC-R09 | [Harness Engineering source-code study](https://arxiv.org/html/2609.00006v1) | Documented only | [evidence-root.json](evidence-root.json) |
| SRC-R10 | [AI Harness Engineering runtime substrate](https://arxiv.org/html/2605.13357v1) | Documented only | [evidence-root.json](evidence-root.json) |
| SRC-R11 | [byProduct lifecycle](https://byproduct.ai/docs/how-it-works) | Documented only | [evidence-root.json](evidence-root.json) |
| SRC-R12 | [Warp software factory article](https://www.warp.dev/articles/coding-agents-to-software-factory) | Documented only | [evidence-root.json](evidence-root.json) |
| SRC-R13 | [Paradox Interactive User Agreement](https://store.steampowered.com/eula/3493540_eula_0) | Documented only | [evidence-root.json](evidence-root.json) |
| SRC-R14 | [Tauri v2 capabilities and external binaries](https://v2.tauri.app/security/capabilities/) | Documented only | [evidence-root.json](evidence-root.json) |
| SRC-R15 | [Electron security](https://www.electronjs.org/docs/latest/tutorial/security) | Documented only | [evidence-root.json](evidence-root.json) |
| SRC-R16 | [SQLite atomic commit](https://www.sqlite.org/atomiccommit.html) | Documented only | [evidence-root.json](evidence-root.json) |

## Inspection limits

Repository metadata reports activity, branch and observed releases. It does not establish maintainer capacity, future support or production maturity. No popularity score was used to rank candidates. Missing release metadata or absent guidance is explicitly stated where observed.

The two papers informed taxonomy and evaluation structure. Neither validates HOI4 performance. LiteLLM was assessed through official documentation, not an internal source audit. Nova had no public implementation to audit. The supplied publisher agreement has an applicability limit for the intended product/business model. No inaccessible feature is described as fully verified.

No copied upstream implementation, copyrighted game asset, secret or personal machine path is distributed in these ledgers. Source URLs and hashes let a future authorized engineer recheck the evidence.
