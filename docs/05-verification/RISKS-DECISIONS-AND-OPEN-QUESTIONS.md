# Risks, decisions and open questions

Status: **Proposed decisions with explicit unresolved implementation evidence**. This is a working risk register, not a list of reasons to stop planning. The planning package is complete when its artifacts pass QA. Product release decisions depend on the future experiments below.

## Critical risks and ownership

| Risk | Evidence and implication | Owner and decision point | Required response or fallback |
|---|---|---|---|
| RISK-01, Setup restoration | SRC-A09 records a failed exact-HEAD Windows recovery CI case reporting that the project was not restored exactly. Cause is unknown | Onboarding owner, S01 before production apply reliance | Reproduce and diagnose, then verify a corrected or separately selected pin. Do not assume an older release is safe |
| RISK-02, callable Setup boundary | SRC-A01 through A08 distinguish portable planning from desktop scan/transaction services and actual Tauri commands | Onboarding owner, EXP-01 | Prove direct embedding or an extracted facade while preserving provider-analysis and evidence-confirmation gates |
| RISK-03, source fidelity | Agent Tools has preserving paths, while comparison writers normalize or discard source details. Supported operations still need execution evidence | Domain owner, EXP-02 | Use a tested writable subset, refuse unsupported edits and keep raw source accessible |
| RISK-04, replay and ownership | Pi and OpenCode have materially different hook, replay and persistence behavior in SRC-B01/B02 | Runtime owner, EXP-03 | Host validates final arguments, records reconstructable intent, fences ownership and reconciles unknown effects |
| RISK-05, native packaging | No packaged Candidate A or B was executed in this assignment | Runtime/release owner, EXP-04 | Compare actual packaged requests and dependency burden before selecting the runtime |
| RISK-06, process containment | A same-user Node or MCP process can access resources outside a logical tool broker. Tauri frontend capabilities are not a universal sidecar sandbox | Security owner, S03 and EXP-07 | Initial trusted bundled adapters only, reviewed third-party execution needs demonstrated containment. Trusted-code exception remains deferred |
| RISK-07, game support | SRC-A17 shows a narrowly qualified native Linux runtime, with unavailable native Windows/macOS and mod-QA paths | Domain owner, S07/S08 | Declare build/platform limits, use separately scoped user-operated game observation where available, never relabel a preview |
| RISK-08, map generation | SRC-A13 shows unavailable geography-model synthesis despite useful previews | Map owner, EXP-06 | Build a separately tested bounded importer of structured files or remain read-only. Full world generation stays deferred |
| RISK-09, provider portability | Current libraries normalize many features but do not make every model equivalent. Credentials and live profiles were unavailable here | Runtime owner, EXP-04 and S08 | Probe two APIs and one local model, pin actual profile and preserve unsupported capability outcomes |
| RISK-10, provider cost uncertainty | Interrupted requests can have billing effects even without a final response | Runtime owner, S03 | Keep original unknown outcome and conservative budget reservation. Retry only under explicit bounded policy |
| RISK-11, configuration drift | Actual Agentic config has 196 scalar leaves and no model-context override. Its manifest selects Tools 4.0.3 while the separate tool audit is 4.3.0 | Developer workflow owner, S01/S08 | Use the full field map, explicit tool migrations, current client checks and disabled optional connections |
| RISK-12, distributed state | Git, SQLite and artifact bytes have different commit boundaries | Reliability owner, EXP-03/08 | Journal intent and reconciliation, test hard kills, restore consistent snapshots and preserve unresolved journals |
| RISK-13, stale knowledge | Retrieved or learned suggestions can conflict with approved design or changed source | Context owner, EXP-05 | Separate canon, runtime lessons, engineering guidance and caches, with source/version invalidation and reviewed promotion |
| RISK-14, concurrency | Text merges miss shared-symbol, binary and cross-file semantic conflicts | Collaboration owner, EXP-07 | One writer first, explicit semantic collision fixtures and fresh merged-result evidence |
| RISK-15, license scope | Permissive code licenses do not cover every asset or game right. Nova is restrictive and comparison only. Mastra template's declared license lacks a root license text | Maintainer, EXP-07 and S08 | Inventory transitive code and assets before distribution, exclude restricted material, resolve specific uncertainty before bundling |
| RISK-16, performance | No actual focus/map benchmark ran | UI/map owner, EXP-06 | Benchmark fixed fixtures/hardware, narrow affected regions and justify revised targets with evidence |
| RISK-17, user comprehension | An attractive preview may be misunderstood as game verification | Product owner, S08 pilot | Observe users interpreting evidence labels and correcting a broken result, revise the surface based on failures |
| RISK-18, instruction authority | Skill metadata, historical instructions or model-written lessons can be mistaken for new permission | Developer/runtime owners, S01 onward | Shared rules and scoped grants remain authoritative, tested learning review cannot silently broaden policy |

Source IDs resolve in [the source registry](../00-research/01-research-sources.md). The detailed audited code and license observations are in [existing assets](../00-research/02-existing-assets-audit.md), [candidate comparison](../00-research/03-candidate-comparison.md) and [the research log](../00-research/04-evidence-and-research-log.md).

## Smallest experiments that resolve architecture uncertainty

| Experiment | Bounded method | Required evidence and decision | Stage / checks |
|---|---|---|---|
| EXP-01, onboarding binding and recovery | In an isolated future fixture, call actual Setup planning and desktop services through a candidate binding. Exercise new/import and interruption boundaries, including the observed CI case | Exact service binding, preserved gates, before/after inventory, root cause or honest non-reproduction, selected pin and supported recovery variants | S01/S02, T-SETUP-01 through 04 |
| EXP-02, source-preserving focus subset | Run no-op, supported node creation/linking, move and scalar edits over original synthetic difficult syntax/encoding cases | Byte oracle, semantic oracle, explicit unsupported operations and a go/no-go decision for each editor gesture | S01/S04, T-VIS-01/03 |
| EXP-03, durable runtime and authority | Drive real runtime adapters with fake streams and effect observers, inject hard process death and stale ownership, then reopen under a new host | No unsafe duplicate effects, immutable reconstructable intent, settled versus unknown outcomes and explicit subordinate runtime state | S01/S03/S05, T-RUN-02/03/04 |
| EXP-04, packaging and model capabilities | Package a minimal native request for Candidate A and B, then probe available approved API/local profiles with a finite budget | No external coding CLI, exact binary/dependency closure, capability receipts and measured adaptation burden | S01/S03, T-RUN-01/05/06 |
| EXP-05, context and reviewed learning | Use a fixed question/source answer key with approved canon, stale facts, smaller context and cancelled indexing. Compare exact/domain/full-text baseline before optional semantic retrieval | Recall misses, stale-answer rate, budget behavior, review race results and a justified storage/retrieval decision | S05, T-CTX-01 through 04 |
| EXP-06, visual/map feasibility | Benchmark 500/2,000-node focus and synthetic structured map fixtures, test bounded map edits and dependency closure, keep reference images distinct | Source preservation, named topology checks, measured latency/memory and a supported operation subset | S04/S07, T-PERF-01/02, T-MAP-01/02/03 |
| EXP-07, collaboration and protection | Run owned-worktree collisions, malicious input, stale leases, extension isolation and dependency/license inventory on disposable artifacts | No unauthorized effect, complete collision report, reviewed containment proof and distributable closure | S06/S08, T-GIT-01/02, T-SEC-01/02, T-RELEASE-03 |
| EXP-08, restore and release evidence | Back up and restore a candidate with unresolved journals, interrupt schema update, export native source and observe a declared game scenario where authorized | Source/canon/state recovery, no silent loss, clean native continuation, actual support matrix and scoped game result | S05/S08, T-RELEASE-01/02, T-HOI-02 |

An experiment that fails should return a narrow proposed correction or an alternative. It should not silently create a full replacement subsystem. Findings update the ADR, affected spec, stage and test oracle through the normal review process.

## Decision record index

[ADR-001 through ADR-010](../02-architecture/options-and-decisions.md#provisional-decisions) are provisional. They cover Setup reuse, Candidate A preference, host authority, storage, focus-first editing, maps, one-writer sequencing, Claude-first engineering, retrieval and license constraints.

Each accepted ADR must record the approving owner, date, evidence revision, alternatives, constraints, costs, unresolved conditions and reconsideration trigger. S01 is allowed to reject the leading runtime and choose Candidate B if the experiments support that result. A documented implementation defect can justify a small upstream contribution or adapter correction before a fork is considered.

## Questions that remain genuinely open

| Question | Why it is not resolved by this research | Smallest next decision |
|---|---|---|
| Which Setup revision is safe for the selected recovery envelope? | Source and CI observation reveal a concrete failure without a reproduced cause | EXP-01 result on the actual selected pin |
| Should the native services be directly embedded or extracted into a shared facade? | Existing desktop boundaries are verified, reusable library ergonomics are not | Compare a bounded binding and its upstream maintenance burden |
| Is Pi core sufficient, or does subordinate pi-durable reduce recovery complexity? | Current source supports both approaches, neither was exercised in this host | EXP-03 with explicit record ownership and hard-kill evidence |
| Does OpenCode produce a simpler total product after persistent database and ownership controls? | Its genuine embedded SDK is verified, integration cost remains unmeasured | Candidate B packaged probe compared with A |
| Which OS is the first supported release target? | Windows is a proposal based on supplied workflow emphasis, not an observed user environment | Maintainer chooses a target, S08 tests it rather than promising all platforms |
| Which exact model IDs and local weights are supported? | Accounts, hardware and provider choices were not available for trials | Record actual permitted profiles and run capability/episode tests |
| How much visual editing is source-safe at the chosen tool closure? | Existing tests and implementation suggest useful operations without proving this deployment | EXP-02 writable subset and explicit unsupported gestures |
| What game behavior can be verified automatically on the target platform? | Audited Live Tools is constrained by OS and executable identity | Record qualified integration or a user-operated scenario procedure |
| What commercial distribution model is permissible? | The supplied Paradox agreement and individual code licenses do not establish every product-specific right | Resolve applicable agreement, assets, APIs and business model before distribution |
| Do users need remote collaboration or vectors early? | No measured local workflow or retrieval shortfall exists | Complete single-user/pilot baseline and measure the actual gap |

The [supplied Paradox agreement](https://store.steampowered.com/eula/3493540_eula_0) contains noncommercial-service and user-content terms as well as third-party rights constraints. This is a source-specific observation, not a conclusion that every standalone modding tool has the same legal position. Keep game/DLC assets local, do not redistribute them in this package, and resolve applicability before a commercial distribution decision.

## Unknowns that do not block the planning deliverable

Live client loading, MCP connectivity, runtime execution, paid model trials, sandbox enforcement, actual game checks and usability outcomes are all explicitly unexecuted here. They are implementation gates with owners and fixtures. A complete plan can contain those unknowns because it explains how to resolve them and what must remain unavailable meanwhile.

No unresolved question authorizes an agent to change approved canon, broaden permissions, run a game, purchase API usage or publish the product. Future stage authorization and existing project policy determine those actions. Within an authorized stage, routine reversible engineering should proceed without repeated permission requests.

