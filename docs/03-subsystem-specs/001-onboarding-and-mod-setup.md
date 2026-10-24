# SPEC-001: Onboarding and HOI4-Mod-Setup integration

Status: **Proposed**. Primary requirements: REQ-004, REQ-005, REQ-006 and REQ-007. Shared requirements: REQ-021, REQ-027, REQ-038, REQ-043, REQ-046, REQ-047 and REQ-048. Probe in S01, implement and accept the workspace lifecycle in S02.

Read first: [existing assets audit](../00-research/02-existing-assets-audit.md), [contracts](../02-architecture/contracts-and-interfaces.md), [Git collaboration](005-git-collaboration-and-review.md), [configuration migration](013-config-reuse-and-developer-environment.md), [user workflows](../01-product/users-and-workflows.md) and the [2026-10-24 maintainer decisions](../02-architecture/maintainer-decisions-2026-10-24.md).

## User value and scope

The factory is a hosted web application. Every project lives in a server-side workspace owned by one account, with invited collaborators under REQ-046. A signed-in user reaches that workspace through one of four entry routes.

| Route | Source of the mod | Result |
|---|---|---|
| Create | A short description and project conventions | New server workspace with server-side Git history, optionally published to a new GitHub repository under SPEC-005 |
| Import from GitHub | A repository and ref the user selected through the GitHub connection | Server-side clone of that ref, with its remote recorded for later push and pull requests |
| Upload | A ZIP archive or folder upload of an existing mod with no repository | Server workspace with a new server-side Git history, publishable to GitHub later |
| Maintain | An existing managed workspace | Repair, update or removal of managed Setup content |

All routes end in a validated workspace with known capabilities and explicit remaining checks. They reuse HOI4-Mod-Setup's actual source resolution, scanning, planning and transaction services, running on the server against server workspace paths. The factory owns the surrounding web experience, account boundary and provider integration.

The setup assistant is independent of the coding client selected for generated developer guidance. Claude Code is the primary development workflow for building the factory, while the shipped product must also operate through its native model integration. Onboarding does not require a coding CLI to be the runtime agent. Optional image, 3D and live-game integrations remain optional capabilities.

## Evidence and integration boundary

The inspected Setup commit is `4488a418cad56a158273cb66c4ce7794696b7ed1`. Its React route and Rust implementation contain real create/import flows, bounded scanning, manifest-derived plans, conflict preservation and journaled application. The portable core exposes read-only adapter traits. The desktop scanner and transaction are still native services, so C-09 is a proposed integration contract, not an existing public HTTP API. [SRC-A01 to SRC-A08, source adapters](https://github.com/klimPaskov/HOI4-Mod-Setup/blob/4488a418cad56a158273cb66c4ce7794696b7ed1/crates/hoi4-setup-core/src/adapters.rs)

The exact-head Windows recovery candidate failed with a project-restoration inventory mismatch. S01 must reproduce and explain that failure, and S02 must demonstrate the preservation invariant through the adapter. Adoption cannot be accepted on source tests alone. [SRC-A09, failed recovery job](https://github.com/klimPaskov/HOI4-Mod-Setup/actions/runs/39281117181/job/118160257466)

Keep incoming installation bytes, prepared plans, root identity, journal paths and approval checks under the server-side Setup service. The UI and model supply typed intent and decisions. A displayed plan ID is not authority. An adapter refactor must preserve every relevant refusal and hash precondition of the source service.

### Existing callable surface versus proposed refactor

The current callable boundary is the desktop's typed Tauri command map, backed by private command handlers. It includes `scan_project` and `cancel_scan`, `ai_analyze`, `approve_scan_evidence` and `confirm_codex_analysis`, `preview_source_manifest`, `preview_descriptors`, `build_installation_plan`, conflict preview/resolution, `approve_installation`, `apply_installation`, maintenance and recovery commands. The legacy confirmation type names do not mean the factory must depend on Codex. [SRC-A02, actual command map](https://github.com/klimPaskov/HOI4-Mod-Setup/blob/4488a418cad56a158273cb66c4ce7794696b7ed1/src/lib/tauri.ts)

The factory has no Tauri shell, so the command map is not the binding. S01 compares two server bindings. One links the existing Rust modules from `src-tauri/src/commands.rs`, `scanner.rs` and `transaction.rs` into the Rust server crate with the Tauri glue removed. The other extracts a stable Rust service facade from them, ideally upstream in HOI4-Mod-Setup. The portable crate alone is insufficient. Both bindings operate on server workspace paths only. Launcher descriptor discovery, the user's local Documents folder and other desktop-only paths are out of scope, and the adapted services must refuse any path outside the workspace root. Keep the provider-analysis-required gate and confirmed evidence bindings. A new provider bridge may translate the factory's native API/local model configuration into that gate, but cannot skip analysis or assert confirmation on behalf of an unreviewed proposal. Do not expose credential-storage commands or broad path strings to the browser client or to untrusted extensions.

## C-09 operations

Every operation takes an account identity, a workspace or onboarding-session identity, schema version and current policy. The server rejects an operation whose account lacks the required project role. All paths are capabilities resolved by the host inside that account's workspace storage. Secrets are credential references into the server secret store only.

| Operation | Required input | Output | Refusal or recovery behavior |
|---|---|---|---|
| Inspect | Workspace root after clone, upload extraction or creation, inspection limits | Scan ID, root identity, source hashes, facts, conflicts, coverage and progress | Cancellation and limits yield explicit incomplete coverage, with no project writes |
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

Resolve the source manifest to an immutable revision before downloading selected files. Show coding environments, image generation, components and workflows, followed by conditional Live Tools, Meshy or MCP setup. Git policy distinguishes server-side initialization, commit, connecting a GitHub remote, pushing and creating a GitHub repository. A setup authorization does not imply publication authority. Repository creation and push follow the separately authorized GitHub operations in SPEC-005.

The dry run shows created, changed, preserved and conflicted paths inside the workspace. It also explains reviewed post-install commands, working directory, required environment-variable names and expected writes. It never displays credential values. Creation of the project root occurs only at the server's Setup boundary, matching the existing new-project invariant. [SRC-A02 and SRC-A05, routes and plan](https://github.com/klimPaskov/HOI4-Mod-Setup/blob/4488a418cad56a158273cb66c4ce7794696b7ed1/src-tauri/src/commands.rs)

## Import route and preservation

Import starts from a GitHub repository or an upload. For GitHub, the user picks a repository the connection grants and a branch, tag or commit. The server clones that exact ref into a new workspace and records the resolved commit, remote identity and default branch. The clone keeps the history the user selected. For an upload, the server extracts the archive or folder into staging, rejects traversal, symlink escapes, device files and size or count limits, then initializes server-side Git history with one import commit that records the upload hash. An upload that already contains a `.git` directory keeps that history after the same safety checks, with hooks, filters and configured helpers disabled under SPEC-005.

Within the resulting workspace, select the actual mod root, then perform bounded read-only inspection. Record descriptor identities, Git status, existing instructions, managed lock, selected environments, source files and limitations. A descriptor path that points outside the workspace, such as a launcher path from the author's own machine, is recorded as a fact and never followed.

Present deterministic findings separately from assistant proposals. Confirm an existing valid identity rather than renaming the project to fit a template. Record a full pre-apply inventory for affected paths and all preservation assertions. User gameplay files, unselected components, existing instructions and custom assets remain intact. The scanner's retained setup inventory is not assumed to be a complete gameplay index.

The preservation rules apply equally to cloned and uploaded mods. The import commit or cloned ref is the before inventory. Setup changes land as a separate reviewed commit on an owned branch, so the original GitHub ref or upload is never rewritten.

For each collision, show existing and incoming provenance. Keep is a valid decision. Replace needs explicit scope. Rename checks references and destination collisions. Merge requires the actual old source hash and the same adaptation rules that produced the installed base. Unsupported binary or source forms remain preserved unless a separately authorized operation can handle them. A changed file after review makes the proposal stale.

| Artifact | Proposed policy |
|---|---|
| `descriptor.mod` | Generate or update only the reviewed fields, retaining valid user content. A launcher `.mod` file for the user's own machine is offered only as a downloadable artifact |
| Thumbnail | Show selected image or the current placeholder behavior, with provenance |
| README and context copies | Provider-neutral project guidance and only approved context copies |
| Setup state and install lock | Native-owned versioned state, integrity and installed component hashes |
| Coding guidance and projections | Preserve user modifications and reconcile shared versus local files through SPEC-013 |
| Ignore rules | Review the current upstream policy, which ignores some generated guidance, against team requirements |

This policy adapts the source's generated-file behavior. It does not assume every generated file has a usable merge base. [SRC-A05, project-file generation](https://github.com/klimPaskov/HOI4-Mod-Setup/blob/4488a418cad56a158273cb66c4ce7794696b7ed1/crates/hoi4-setup-core/src/project_files.rs)

## Recovery and maintenance states

The factory persists the adapter session and Setup transaction reference in PostgreSQL without creating a competing installation journal. The Setup journal itself stays in the workspace volume where the Setup service wrote it. Recovery reads the native authority, then emits a factory RunEvent and ToolReceipt projection.

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

On success, produce C-01 WorkspaceDescriptor with stable ID, owning account, roots, root identity, final source snapshot, Git refs and dirty inventory, GitHub remote binding if any, game/DLC profile against the server game reference in SPEC-006, exact setup manifest closure, installed tool versions, capability results, setup receipt and unresolved checks. Re-read the final bytes before hashing. Credentials, provider hidden reasoning and arbitrary absolute access are excluded.

Readiness is capability-specific. A workspace may be usable for source editing while optional image generation or live observation is unavailable. Missing core integrity or an unresolved partial installation blocks handoff. An unrun game check remains unrun. Structural MCP initialization does not establish gameplay compatibility. Playing the mod happens on the user's own machine after a ZIP download, clone or GitHub pull. The factory lists the remaining local installation steps and cannot issue an install-success receipt for the user's game.

## Acceptance and evidence

| Case | Observable acceptance |
|---|---|
| New project fixture | Reviewed descriptors, state and selected files appear only after apply, with matching receipt hashes |
| Imported dirty fixture | User gameplay file, modified skill, existing instructions and unrelated Git changes remain byte-identical |
| GitHub import fixture (T-GH-01) | The cloned ref, history and every unapproved byte match the remote, and Setup changes appear only on an owned branch |
| Hostile upload | Traversal, symlink, device and oversize members are rejected before any workspace write |
| Cross-account access (T-ACCT-01) | Another account cannot inspect, plan or apply against the workspace |
| Repeated maintenance | No duplicate descriptors or unintended source changes, missing managed file repaired and modified file preserved |
| Source or root swap | Stale plan refuses before unauthorized writes and gives a fresh-inspection route |
| Provider unavailable during recovery | Native recovery remains reachable and does not require a new model answer |
| Every transaction interruption | Only allowed recovery actions appear, with accurate pre/post inventories and no premature success lock |
| Invalid merge base or unsupported encoding | Conflict stays unresolved, original bytes preserved |
| Optional tool absent | Relevant capability is limited while unrelated source work remains available |
| Exact-head recovery regression | Reproduction explains SRC-A09 and the corrected candidate proves complete intended restoration |

S01 records extraction feasibility, exact pins, candidate build and recovery diagnosis. S02 retains fixture inventories, plan and grant hashes, native journal outcome, readiness coverage and screenshots of the resulting flow. No stage can substitute a green UI badge for those artifacts. Container image, deployment and upgrade ownership continues in S08 with SPEC-013.
