# Test strategy

Status: **Proposed product verification design**. Product tests described here have not run. Completed research and package checks are recorded separately in [the quality review](PLANNING-PACKAGE-QUALITY-REVIEW.md). A test file found upstream is evidence of a test's existence, not a passing execution result.

## Evidence model and acceptance rules

Use four independent evidence kinds: source/render preview, static validation, simulated validation and actual game observation. None implies the others. A successful parser may coexist with an untested game behavior. A human observation is scoped to the game build, DLC, load order, save/scenario and exact mod hashes observed.

Every test result records test ID, requirement and stage, implementation revision, environment, fixture hash, invocation or UI action, expected oracle, actual outcome, logs/artifact hashes, duration, cost when known and limitations. Outcomes are pass, fail, unavailable or not-run. “Unavailable” cannot satisfy a mandatory gate. A reviewed narrower supported scope may remove the capability from release claims, with the reason preserved.

The initial test runner should use deterministic fake provider transports for fault injection and separately use real configured endpoints for model behavior. A fake transport may drive the production adapter, but its result remains a deterministic protocol test. Do not substitute it for an end-user model result.

## Fixture design and independent oracles

Fixtures should be small original synthetic mod fragments without redistributed game assets. Store a manifest of input bytes, expected untouched paths, permitted mutations, semantic assertions and expected diagnostics. Test oracles must be authored independently of the transformation being tested. A serializer comparing its output with its own parser cannot alone prove preservation.

| Fixture | Contents and fault variants | Independent oracle |
|---|---|---|
| F-SETUP-NEW | Empty owned directory, mod identity, missing optional tools, expected generated files | Exact generated closure and required descriptor fields, no paths outside root |
| F-SETUP-IMPORT | Existing source, custom assets, project guidance, generated files modified by a human, dirty index and untracked files | Before inventory, approved change set, Git index snapshot and exact untouched hashes |
| F-SETUP-PARTIAL | Disposable copies interrupted before staging, within apply and during finalization | Recorded baseline and the actual Setup recovery state machine, including rollback-required partial apply |
| F-FOCUS-FIDELITY | BOM and no-BOM UTF-8, CRLF/LF, comments, unknown syntax, relative/shared focus, duplicate IDs and malformed input | Byte comparison of untouched spans plus independently specified focus relationships |
| F-FOCUS-SMALL | Three-to-five-node branch, one supported effect set, one localization language, synthetic icons | Approved design assertions, required native paths and human-reviewable semantic diff |
| F-FOCUS-SCALE | 500-node and 2,000-node original graphs with deterministic identifiers and layout cases | Node/edge inventory and measured interaction traces, no screenshot-only correctness |
| F-RUNTIME-FAULT | Fake stream truncation, malformed calls, hook mutation, transient errors, tool ignoring cancellation, process death and lost responses | Durable state/event sequence, effect counter and expected allowed replay set |
| F-AUTHORITY | Disallowed roots, traversal, symlinks or Windows reparse points, revoked grants, expired leases, secret canary and hostile source text | Zero disallowed effects or canary disclosure, explicit refusal receipt |
| F-CONTEXT | Approved canon, contradictory suggestion, superseded lesson, stale source pointer, smaller context budget and cancelled index generation | Human-authored question-to-source key, required/forbidden facts and review version |
| F-COLLAB | Two owned workspaces, disjoint edits, same focus changed differently, shared IDs, localization and binary collisions | Both contributions retained, expected conflict set, current HEAD and index preservation |
| F-ASSET | Original licensed or synthetic files, one unsupported format, transformed output, missing provenance and duplicate target | Rights metadata, decoded properties, file hash and exact consuming reference |
| F-LOCALIZATION | Known keys, missing language, duplicates, BOM, malformed bytes and script references | Required key set, byte preservation and diagnostics without lossy decoding |
| F-MAP-STRUCTURED | Small complete original province raster and supporting definitions, states, regions and selected layers | Hand-verified ID/color/reference inventory and profile-specific topology expectations |
| F-MAP-BROKEN | Duplicate colors/IDs, missing memberships, dimension mismatch, orphan references, invalid adjacency and stale map input | Named failures and zero apply outside the approved closure |
| F-MAP-REFERENCE | Attributed original sketch with scale and fictional design brief | Classified as design reference, no automatic structured map or game acceptance |
| F-MAP-SCALE | Synthetic 5,632 by 2,048 raster for initial stress measurement, plus a 2-times-linear stress variant if memory permits | Dimensions are benchmark choices, not a universal HOI4 rule. Record complete dependency counts and peak memory |
| F-DEVELOPER | Starter files, current client/version, positive/negative skill tasks, one verified lesson and one false lesson | Schema/parse checks, instruction loading observed in client and reviewed promotion diff |
| F-RELEASE | Packaged candidate, backup copies, unresolved operation, migration from previous schema and interrupted update | Source and accepted-state inventory before/after, reproducible dependency/license manifest |
| F-TENANT | Two or more accounts with projects, secrets, budgets and queued runs, hostile identifiers and cross-tenant references | Per-account inventory of visible and mutable objects, zero cross-tenant reads or effects |
| F-GITHUB-REPO | Disposable GitHub repository with branches, history, binary assets and instruction files, plus a ZIP upload of the same tree | Exact tree, branch and commit inventory before and after, GitHub-side query of published refs |
| F-GAME-REF | Mounted read-only game reference with a recorded build and DLC manifest, and a mod targeting a different version | Build and DLC manifest hash, read-only mount evidence, and scans of outputs for game file bytes |

Do not generate large fixtures inside the planning archive. Future implementation creates and versions them under its approved stage. The S01 recovery reproduction records its relationship to the inspected failing CI log. If the original failure cannot be reproduced, preserve that uncertainty and still test the selected pin's recovery invariants.

## Test layers and failure injection

Unit checks cover parser span ownership, dependency graphs, policy decisions, capability normalization and state transitions. Integration tests use actual host, database, Git and adapter boundaries against disposable fixtures. Native packaged tests remove coding CLIs from the test environment and confirm the application does not invoke one.

Inject hard process termination at meaningful durable boundaries, not only a thrown exception. Cover intent persisted before effect, effect completed before receipt, receipt stored before UI delivery and commit created before response. Keep a separate observer to establish whether an effect occurred. A fixture should prove that an uncertain effect remains blocked or is reconciled rather than recreated with a new operation ID.

For Git, inspect the real index and unrelated dirty work before and after. Test expected-HEAD compare-and-swap, merge/review duplicate requests, branch conflicts and an interrupted commit reply. For multi-file edits, exercise the declared journal and rollback boundaries. PostgreSQL transactions do not make filesystem, Git, object store or GitHub changes atomic. Record the database, volume and storage durability settings each recovery test depends on.

For visual changes, test the actual write path in addition to screenshots and DOM assertions. Compare unknown byte spans, verify source locations, provoke an external edit and confirm that stale preview gestures cannot overwrite it. Undo is a new authorized proposal against current state when intervening edits exist.

For security, use nonsecret canaries and disposable owned directories. Treat malicious source comments and retrieved instructions as data. A trusted bundled worker process or a plain container is not proof of containment against hostile executable code. Untrusted extension acceptance and model-directed tools on shared servers require the boundary in SPEC-011, independently tested on the deployed host configuration.

## Performance and resource budgets

All targets below are proposed experiment budgets, not observed results or universal release claims.

| Measure | Initial target | Rationale and revision rule |
|---|---|---|
| Cached 500-node useful focus preview | Within 3 seconds | Keeps a small contributor task interactive, revise only with benchmark and usability evidence |
| Focus drag feedback | At most 100 ms p95 after warm-up | Interaction feedback target from SPEC-007, full validation may complete separately |
| Cached map viewport | Within 500 ms p95 on reference hardware | Proposed inspection goal, measure decoding, index lookup and rendering separately |
| Local map edit preview | Within 2 seconds p95 for declared affected region | Bound the operation instead of promising a full-map redraw budget |
| Default map test working set | Target under 2 GiB additional process memory for F-MAP-SCALE baseline | Initial desktop engineering budget, record actual components and no universal guarantee |
| Source invalidation feedback | Within 1 second after a received filesystem event | Event delivery itself is platform-dependent, periodic reconciliation covers missed events |
| Retained operational data | Report growth per accepted task across 100 cycles | No arbitrary deletion of pinned evidence or unresolved journals |
| Model scenario budget | Defined by the evaluation matrix | Cost is measured separately from tool CPU and human review time |

Reference hardware, display scale, OS, cold/warm cache, fixture size, trial count and p50/p95 must accompany results. Use at least 30 interaction samples after a declared warm-up for exploratory percentiles. Do not compare two builds measured under incompatible hardware or fixture conditions. Larger-scale claims require more representative measurements.

## Game and usability verification

Actual game checks run only with an authorized local installation and a declared scenario. Keep copyrighted game files outside the test repository. Record the supported game executable/build, DLC set, mod order and exact inputs. The inspected native Live Tools implementation is not a general Windows game-test mechanism. A user-operated launch with logs and a recorded scenario can provide separate observed game evidence.

The contributor pilot includes at least a non-scripter, a technical mod contributor and a maintainer. Ask each to find the accepted design, understand an unsupported preview warning, complete or review a bounded change and locate restore. Record observed assistance, wrong turns and whether evidence labels were understood. Do not infer usability from a visually attractive screenshot.

## Traceability and completion

The catalog below defines stable test IDs. The [acceptance matrix](REQUIREMENTS-ACCEPTANCE-MATRIX.md) maps requirements and every stage gate to them. The [evaluation matrix](EVALUATION-MATRIX.md) supplies model scenario runs. A stage handoff reports gate results against the final revision and records engineering learning and configuration reviews even when no guidance change is justified.

## Test catalog

| Test | Purpose and fixture | Observable success oracle |
|---|---|---|
| T-SETUP-01 | **New project through reused Setup.** F-SETUP-NEW | Real Setup analysis, plan, dry-run and apply yield a descriptor with exact resolved closure and native files. No duplicate importer is used. |
| T-SETUP-02 | **Import and idempotent rerun.** F-SETUP-IMPORT | All unapproved source, instructions, assets, dirty index and untracked files remain byte-identical. Rerun has no unexpected changes. |
| T-SETUP-03 | **Interrupted setup and recovery.** F-SETUP-PARTIAL | Kill at staging, each apply boundary and finalization. Resume only supported preapply states. Partial apply requires rollback. Before/after inventory agrees or apply remains blocked. |
| T-SETUP-04 | **Version and capability closure.** F-SETUP-IMPORT | Manifest, components, tool versions and unsupported capabilities are recorded. Upgrade is explicit and unsupported platform capabilities fail closed. |
| T-RUN-01 | **Independent packaged agent.** F-RUNTIME-FAULT | On a clean installation the product performs a native request without spawning or requiring Claude Code, Codex or another coding CLI. |
| T-RUN-02 | **Final schema and tool input validation.** F-RUNTIME-FAULT, F-AUTHORITY | Malformed schemas, hook-mutated arguments and invalid results are rejected at the host after all hooks. Zero fixture effects occur for rejected calls. |
| T-RUN-03 | **Pause, revocation and stale ownership.** F-RUNTIME-FAULT, F-AUTHORITY | Pause survives restart, dispatch stops at revocation, stale fence results cannot publish, and UI pause waits for settlement. |
| T-RUN-04 | **Unknown effects and replay.** F-RUNTIME-FAULT | Crash before intent, during effect and after success before reply. Unsafe uncertain operations remain blocked until reconciled. Changed workspace or tool identity invalidates replay. |
| T-RUN-05 | **Provider capabilities and graceful limits.** F-RUNTIME-FAULT | Three distinct profiles report actual tool, schema, stream and context behavior. Unsupported features produce explicit narrowing or rejection, never silent equivalence. |
| T-RUN-06 | **Cost, timeout and destination limits.** F-AUTHORITY | Budget exhaustion stops dispatch, retries are finite, unknown usage stays unknown and fallback cannot cross an unapproved provider destination. |
| T-VIS-01 | **Lossless bounded edit.** F-FOCUS-FIDELITY | No-op is byte-identical. Supported edits change only approved spans. Comments, BOM, CRLF, unknown constructs and unrelated values survive. Unsupported edits fail without mutation. |
| T-VIS-02 | **Visual, hybrid and autonomous equivalence.** F-FOCUS-SMALL | All three paths produce reviewable native artifacts with comparable accepted semantics, source provenance and checks. Autonomous execution uses no canvas action. |
| T-VIS-03 | **External edit, stale preview and undo.** F-FOCUS-FIDELITY, F-COLLAB | An external change invalidates the projection. Stale gestures are rejected or explicitly rebased. Undo preserves intervening human work and updates agent context. |
| T-GIT-01 | **Owned worktree and index discipline.** F-COLLAB | Only owned paths are committed under expected HEAD. Real index and unrelated dirty or untracked work remain intact. |
| T-GIT-02 | **Semantic collision and review race.** F-COLLAB | Detect overlapping focus logic, duplicate symbols, localization keys and binary destinations even when Git merges text. Concurrent approval and integration have one authoritative winner. |
| T-GIT-03 | **Commit reconciliation and restore.** F-COLLAB | Lost commit reply does not duplicate the commit. Restore or reviewed revert recovers the intended snapshot and preserves subsequent unrelated work. |
| T-HOI-01 | **Layered domain validation.** F-FOCUS-FIDELITY, F-LOCALIZATION | Parser, rule, reference and dependency failures are separately recorded with exact source and tool profiles. Known invalid fixtures block completion. |
| T-HOI-02 | **Evidence fidelity and game scope.** F-FOCUS-SMALL, F-MAP-STRUCTURED | Source preview, static check, simulation and actual game observations retain distinct labels. A game result records build, DLC, load order and scenario, with no unsupported generalization. |
| T-CTX-01 | **Canon, inference and context budget.** F-CONTEXT | Only approved canon is authoritative. Bundle records omissions and source versions, fits the selected profile or requests narrowing, and contains no private cross-provider reasoning. |
| T-CTX-02 | **Retrieval and stale-index rebuilding.** F-CONTEXT | Correct sources and stale facts are evaluated against a human-authored answer key. Cancellation cannot publish a partial generation. Changed dependencies invalidate affected recall. |
| T-CTX-03 | **Cross-model interruption and resume.** F-RUNTIME-FAULT, F-CONTEXT | New provider resumes the same task and receipts from canonical context, rechecks destination and capabilities, and repeats no unsafe effect. |
| T-CTX-04 | **Runtime lesson review and rollback.** F-CONTEXT | Candidate review has explicit owner and evidence. Concurrent decisions have one winner, rejection is retained, supersession removes active recall and rollback does not alter approved canon. |
| T-MAP-01 | **Structured import dependency closure.** F-MAP-STRUCTURED, F-MAP-BROKEN | Report exact import closure, remapping, missing inputs and conflicts before apply. Invalid dimensions, duplicate colors/IDs and broken references reject mutation. |
| T-MAP-02 | **Bounded map edit and recovery.** F-MAP-STRUCTURED, F-MAP-BROKEN | Supported edit preserves unrelated files and passes named topology checks. Stale input and interrupted apply cannot silently publish a partial map. |
| T-MAP-03 | **Reference and deferred synthesis boundary.** F-MAP-REFERENCE | Reference image remains attributed design input. Unsupported geography synthesis is disabled and the UI makes no game-ready output claim. |
| T-CONTENT-01 | **Asset provenance and wiring.** F-ASSET | Track original and transformed hashes, rights, recipe and actual consumer. Missing rights, collision, bad format or unwired output is visible and blocks the affected acceptance. |
| T-CONTENT-02 | **Localization preservation and coverage.** F-LOCALIZATION | BOM/encoding policy, duplicate keys, language coverage and references are checked. Untouched bytes survive and invalid encoding is not discarded. |
| T-DAG-01 | **Dependencies, leases and bounded retries.** F-COLLAB, F-RUNTIME-FAULT | Reject cycles, stale claim release and invalid dependency completion. Retry budget stops repeated failure. At most the current fenced owner can publish. |
| T-SEC-01 | **Filesystem, IPC and injection boundary.** F-AUTHORITY | Path traversal, symlink/reparse escape, malicious repo instructions, credential exfiltration and stale grants cause no unauthorized write or disclosure. |
| T-SEC-02 | **Extension and protocol negotiation.** F-AUTHORITY | Unknown MCP/ACP capabilities fail explicitly, read-only grants reject writes, skill metadata grants no authority, and untrusted executables remain unavailable without a proven containment mode. |
| T-ACCT-01 | **Tenant isolation.** F-TENANT | Two accounts with hostile inputs cannot read, list or change each other's workspaces, runs, secrets, budgets, logs or artifacts through the browser API, agent tools or GitHub tokens. |
| T-ACCT-02 | **Sign-in and project roles.** F-TENANT, F-COLLAB | GitHub and email sign-in, account linking, invitation, role change and removal each grant exactly the documented project actions. A removed collaborator's queued work cannot publish. |
| T-GH-01 | **GitHub import preservation.** F-GITHUB-REPO | Importing a disposable repository through the GitHub App keeps every byte, branch and commit, and an uploaded ZIP keeps every byte with a recorded baseline commit. |
| T-GH-02 | **Publication reconciliation.** F-GITHUB-REPO | Push, pull request and repository creation each need their own authorization. A lost reply is reconciled against GitHub before retry, so no duplicate branch, pull request or repository appears. |
| T-GAME-01 | **Game reference containment.** F-GAME-REF | Tools read the reference read-only, evidence names its build and DLC set, and images, exports, pull requests, logs and API responses contain no raw game files. |
| T-DEPLOY-01 | **Reproducible container images.** F-RELEASE | Images rebuild from pinned base digests and lockfiles, contain no coding CLI, game file, credential or user data, and start with Compose on a clean Linux host. |
| T-DEV-01 | **Clean-session guidance loading.** F-DEVELOPER | Actual target Claude version loads shared AGENTS through CLAUDE. A fresh session finds the authorized stage and known workflow without inventing missing rules. Optional Codex discovery is recorded separately. |
| T-DEV-02 | **Skills and trigger behavior.** F-DEVELOPER | Frontmatter and references validate, mirrors agree, positive and negative tasks invoke the appropriate workflow in an actual client. Static fixtures alone do not prove invocation. |
| T-DEV-03 | **Configuration migration and drift.** F-DEVELOPER | All 196 source leaves map to a disposition. Starter TOML/JSON parse, current schemas support active keys, disabled MCP stays disabled, secrets/global paths are absent and unavailable servers are reported truthfully. |
| T-DEV-04 | **Engineering learning lifecycle.** F-DEVELOPER | An evidenced scoped lesson is proposed, tested, reviewed, promoted and reverted. A duplicate or unsupported lesson is rejected. A no-change review is accepted and no skill can enlarge authority. |
| T-RELEASE-01 | **Install, update and native export.** F-RELEASE | Each advertised platform installs and performs the slice. Native files remain usable outside the app, update rollback preserves source and unsupported platforms remain explicit. |
| T-RELEASE-02 | **Backup, restore and store reconciliation.** F-RELEASE, F-RUNTIME-FAULT | Restore accepted source, canon and operation records after interruption. Reconcile database, artifact store and Git without treating them as one atomic transaction. |
| T-RELEASE-03 | **Dependency, license and asset inventory.** F-RELEASE | Every bundled dependency and asset has an owner, version and distribution basis. Restricted comparison code and game files are excluded from packages. |
| T-RELEASE-04 | **Contributor usability pilot.** F-FOCUS-SMALL, F-COLLAB | Observe non-scripter, technical contributor and maintainer completing bounded tasks, finding evidence limits and using restore. Record assistance and failures. |
| T-PERF-01 | **Focus responsiveness.** F-FOCUS-SCALE | Measure cached 500-node preview against 3 s and drag feedback against 100 ms p95 on declared hardware. Record 2,000-node scaling and justify any target revision. |
| T-PERF-02 | **Map and index responsiveness.** F-MAP-SCALE | Measure full import, cached viewport, local update and peak memory on fixed map fixtures. Compare incremental work with full rebuild and record bottlenecks. |
| T-PERF-03 | **Operational growth.** F-RELEASE | Run 100 synthetic task cycles, measure journal/artifact growth and retention, and prove cleanup preserves unresolved effects and evidence pinned by accepted results. |
| T-PKG-01 | **Planning archive integrity.** Planning, package, itself | Validate links, manifests, traceability, skills, configs and ZIP members with reproducible tooling. This check does not validate product behavior. |
| T-RESEARCH-01 | **Research coverage and provenance.** Source, inventory, and, registries | Account for supplied sources and requested references, distinguish retrieval from inspection, record exact pins, limitations and actual investigation results. |

