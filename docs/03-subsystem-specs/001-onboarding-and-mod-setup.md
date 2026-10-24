# SPEC-001: Onboarding and HOI4-Mod-Setup integration

Status: **Proposed**. Primary requirements: REQ-004, REQ-005, REQ-006 and REQ-007. Shared requirements: REQ-021, REQ-027, REQ-038 and REQ-043. Probe in S01, implement and accept the workspace lifecycle in S02.

Read first: [existing assets audit](../00-research/02-existing-assets-audit.md), [contracts](../02-architecture/contracts-and-interfaces.md), [Git collaboration](005-git-collaboration-and-review.md), [configuration migration](013-config-reuse-and-developer-environment.md) and [user workflows](../01-product/users-and-workflows.md).

## User value and scope

The user can create a mod from a short description, attach an existing mod, or maintain a managed installation. All three routes end in a validated workspace with known capabilities and explicit remaining checks. They reuse HOI4-Mod-Setup's actual source resolution, scanning, planning and native transaction services. The factory owns the surrounding native experience and provider integration.

The setup assistant is independent of the coding client selected for generated developer guidance. Claude Code is the primary development workflow for building the factory, while the shipped product must also operate through its native model integration. Onboarding does not require a coding CLI to be the runtime agent. Optional image, 3D and live-game integrations remain optional capabilities.

## Evidence and integration boundary

The inspected Setup commit is `4488a418cad56a158273cb66c4ce7794696b7ed1`. Its React route and Rust implementation contain real create/import flows, bounded scanning, manifest-derived plans, conflict preservation and journaled application. The portable core exposes read-only adapter traits. The desktop scanner and transaction are still native services, so C-09 is a proposed integration contract, not an existing public HTTP API. [SRC-A01 to SRC-A08, source adapters](https://github.com/klimPaskov/HOI4-Mod-Setup/blob/4488a418cad56a158273cb66c4ce7794696b7ed1/crates/hoi4-setup-core/src/adapters.rs)

The exact-head Windows recovery candidate failed with a project-restoration inventory mismatch. S01 must reproduce and explain that failure, and S02 must demonstrate the preservation invariant through the adapter. Adoption cannot be accepted on source tests alone. [SRC-A09, failed recovery job](https://github.com/klimPaskov/HOI4-Mod-Setup/actions/runs/39281117181/job/118160257466)

Keep incoming installation bytes, prepared plans, root identity, journal paths and approval checks under the native service. The UI and model supply typed intent and decisions. A displayed plan ID is not authority. An adapter refactor must preserve every relevant refusal and hash precondition of the source service.

### Existing callable surface versus proposed refactor

The current callable boundary is the desktop's typed Tauri command map, backed by private command handlers. It includes `scan_project` and `cancel_scan`, `ai_analyze`, `approve_scan_evidence` and `confirm_codex_analysis`, `preview_source_manifest`, `preview_descriptors`, `build_installation_plan`, conflict preview/resolution, `approve_installation`, `apply_installation`, maintenance and recovery commands. The legacy confirmation type names do not mean the factory must depend on Codex. [SRC-A02, actual command map](https://github.com/klimPaskov/HOI4-Mod-Setup/blob/4488a418cad56a158273cb66c4ce7794696b7ed1/src/lib/tauri.ts)

S01 compares embedding these services in the chosen native host with extracting a stable Rust service facade from `src-tauri/src/commands.rs`, `scanner.rs` and `transaction.rs`. The portable crate alone is insufficient. Keep the provider-analysis-required gate and confirmed evidence bindings. A new provider bridge may translate the factory's native API/local model configuration into that gate, but cannot skip analysis or assert confirmation on behalf of an unreviewed proposal. Do not expose credential-storage commands or broad path strings directly to untrusted extensions.

## C-09 operations

Every operation takes a workspace or onboarding-session identity, schema version and current policy. All paths are capabilities resolved by the host. Secrets are credential references only.

| Operation | Required input | Output | Refusal or recovery behavior |
|---|---|---|---|
| Inspect | Selected root, allowed external descriptor scope, inspection limits | Scan ID, root identity, source hashes, facts, conflicts, coverage and progress | Cancellation and limits yield explicit incomplete coverage, with no project writes |
| Analyze | Exact scan or new-project brief, approved context and provider configuration | Schema-checked proposals and evidence bindings | Invalid provider output is rejected. Expired or wrong-root analysis cannot build a plan |
| Plan | Confirmed identity, source revision, manifest closure, selected components, Git policy and analysis ID | Immutable proposed operations, command previews, conflicts and required checks | Missing dependencies, unsupported platform or unverified source hashes block affected actions |
| Decide conflicts | Plan hash, per-path keep/replace/rename/merge choices | New plan hash and merge evidence | A merge requires a verified reconstructible base and validated output |
| Approve | Final plan hash, exact effect scope and C-08 grant | Approval bound to that plan and policy | Any material plan change invalidates narrow approval |
| Stage and apply | Approved plan and current root identity | Progress, journal binding, C-04 receipt and readiness results | Recheck authorization and hashes before effects. Never trust replacement bytes from the UI |
| Inspect recovery | Workspace identity and native-derived journal identity | Actual allowed actions and verified state | Wrong-root, malformed or incomplete journals require recovery handling, not a new installation |
| Recover | Allowed action and original binding | Resumed staging, rollback or finalization reconciliation receipt | Never offer resume after partial apply if the native transaction only permits rollback |
| Maintain | Managed lock, repair/update/remove intent and reviewed plan | Versioned maintenance receipt and preserved user files | Modified managed content remains a conflict or is released from management |

## Create route

Start with a name, description, target game profile and optional context documents or images. The assistant can propose clearer conventions and a description expansion. Proposed narrative is not automatically approved game canon. The user confirms display name, project ID, script prefix, primary namespace, descriptor tags, initial folders and destination. Advanced author, mod version, supported game version and branch controls are available without cluttering the basic path.

Resolve the source manifest to an immutable revision before downloading selected files. Show coding environments, image generation, components and workflows, followed by conditional Live Tools, Meshy or MCP setup. Git policy distinguishes initialization, local commit, adding a remote, pushing and creating a public repository. A local setup authorization does not imply publication authority.

The dry run shows created, changed, preserved and conflicted paths, including external launcher descriptors. It also explains reviewed post-install commands, working directory, required environment-variable names and expected writes. It never displays credential values. Creation of the project root occurs only at the native application boundary, matching the existing new-project invariant. [SRC-A02 and SRC-A05, routes and plan](https://github.com/klimPaskov/HOI4-Mod-Setup/blob/4488a418cad56a158273cb66c4ce7794696b7ed1/src-tauri/src/commands.rs)

## Import route and preservation

Select the actual mod root, then perform bounded read-only inspection. Record descriptor identities, launcher links, Git status, existing instructions, managed lock, selected environments, source files and limitations. An external launcher descriptor must be specifically in scope. A found absolute path is not an automatic grant to inspect or modify it.

Present deterministic findings separately from assistant proposals. Confirm an existing valid identity rather than renaming the project to fit a template. Record a full pre-apply inventory for affected paths and all preservation assertions. User gameplay files, unselected components, existing instructions and custom assets remain intact. The scanner's retained setup inventory is not assumed to be a complete gameplay index.

For each collision, show local and incoming provenance. Keep is a valid decision. Replace needs explicit scope. Rename checks references and destination collisions. Merge requires the actual old source hash and the same adaptation rules that produced the installed base. Unsupported binary or source forms remain preserved unless a separately authorized operation can handle them. A changed file after review makes the proposal stale.

| Artifact | Proposed policy |
|---|---|
| `descriptor.mod` and launcher `.mod` | Generate or update only the reviewed fields and destinations, retaining valid user content |
| Thumbnail | Show selected image or the current placeholder behavior, with provenance |
| README and context copies | Provider-neutral project guidance and only approved context copies |
| Setup state and install lock | Native-owned versioned state, integrity and installed component hashes |
| Coding guidance and projections | Preserve user modifications and reconcile shared versus local files through SPEC-013 |
| Ignore rules | Review the current upstream policy, which ignores some generated guidance, against team requirements |

This policy adapts the source's generated-file behavior. It does not assume every generated file has a usable merge base. [SRC-A05, project-file generation](https://github.com/klimPaskov/HOI4-Mod-Setup/blob/4488a418cad56a158273cb66c4ce7794696b7ed1/crates/hoi4-setup-core/src/project_files.rs)

## Recovery and maintenance states

The factory persists the adapter session and native transaction reference without creating a competing installation journal. Recovery reads the native authority, then emits a factory RunEvent and ToolReceipt projection.

| Native condition | Factory action |
|---|---|
| Inspection or planning incomplete | Resume inspection or build a fresh plan, with no installed-success claim |
| Staging incomplete before apply | Discard or replan according to verified native state |
| Verified staging before apply | Offer supported resume after root, hashes and grant are rechecked |
| Partial application | Offer the native rollback path and preserve conflict evidence |
| Finalization interrupted | Reconcile durable lock and rollback-record evidence before reporting success |
| Concurrent user changes during recovery | Preserve them and report a conflict, do not overwrite to force a clean status |

Source tests explicitly reject resume after partial apply. The flow document's more general wording does not expand this capability. Repair, update and removal also remain distinct. Fresh maintenance analysis is required by the inspected update path. Repair can restore missing managed files while retaining modified ones. Removal deletes only unchanged managed content and records released files. [SRC-A06 and SRC-A07, transactions](https://github.com/klimPaskov/HOI4-Mod-Setup/blob/4488a418cad56a158273cb66c4ce7794696b7ed1/src-tauri/src/transaction.rs)

## Workspace handoff

On success, produce C-01 WorkspaceDescriptor with stable ID, roots, root identity, final source snapshot, Git refs and dirty inventory, game/DLC profile, exact setup manifest closure, installed tool versions, capability results, setup receipt and unresolved checks. Re-read the final bytes before hashing. Credentials, provider hidden reasoning and arbitrary absolute access are excluded.

Readiness is capability-specific. A workspace may be usable for source editing while optional image generation or live observation is unavailable. Missing core integrity or an unresolved partial installation blocks handoff. An unrun game check remains unrun. Structural MCP initialization does not establish gameplay compatibility. Browser ZIP export reports remaining installation actions and cannot issue a native install-success receipt.

## Acceptance and evidence

| Case | Observable acceptance |
|---|---|
| New local fixture | Reviewed descriptors, state and selected files appear only after apply, with matching receipt hashes |
| Imported dirty fixture | User gameplay file, modified skill, existing instructions and unrelated Git changes remain byte-identical |
| Repeated maintenance | No duplicate descriptors or unintended source changes, missing managed file repaired and modified file preserved |
| Source or root swap | Stale plan refuses before unauthorized writes and gives a fresh-inspection route |
| Provider unavailable during recovery | Native recovery remains reachable and does not require a new model answer |
| Every transaction interruption | Only allowed recovery actions appear, with accurate pre/post inventories and no premature success lock |
| Invalid merge base or unsupported encoding | Conflict stays unresolved, original bytes preserved |
| Optional tool absent | Relevant capability is limited while unrelated source work remains available |
| Exact-head recovery regression | Reproduction explains SRC-A09 and the corrected candidate proves complete intended restoration |

S01 records extraction feasibility, exact pins, candidate build and recovery diagnosis. S02 retains fixture inventories, plan and grant hashes, native journal outcome, readiness coverage and screenshots of the resulting flow. No stage can substitute a green UI badge for those artifacts. Packaging and upgrade ownership continues in S08 with SPEC-013.
