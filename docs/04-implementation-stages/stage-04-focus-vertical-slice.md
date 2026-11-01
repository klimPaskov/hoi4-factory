# S04: Focus authoring vertical slice in all three modes

Status: **Proposed future implementation stage**. Nothing in this packet was executed as product implementation during planning. Primary agent: Claude Code. Delegate only bounded work with explicit file ownership. Revised on 2026-10-24 for the [maintainer decisions](../02-architecture/maintainer-decisions-2026-10-24.md).

## Objective and non-goals

Deliver the smallest useful end-to-end product in the browser. Create, upload or import a mod from GitHub, request a small focus branch, preview and adjust it, run the task without the canvas, review source and evidence, commit an owned branch, push it to the linked GitHub repository and open a pull request when authorized, and restore.

The agent-tools work behind this stage's MCP items (Q-1, M-1 to M-14, the engine moves and the coverage gaps) is planned in the [MCP improvement plan](../08-expansion-research/12-mcp-improvement-plan.md).

The focus canvas is the first two-way editor under FR-5. People drag, snap, link and edit fields, and the agent edits through the same `hoi4.focus_rewrite` operation, both saved to the native files. Each side sees the other's change at once. The shared Rust engine is linked into the server as a library, and its reference closure, lints, override audit and findings baseline (P1 to P4) run before every agent commit. Imports gain the preservation report.

Support a deliberately bounded focus syntax subset. Do not build general visual editors, full maps, unrestricted autonomous plans, a many-agent factory or pushes to a shared default branch. Do not reuse Hagane Studio's visual design or its preview cards. Other editors follow in S06 and S07.

## Entry conditions

S03 accepted, including native provider path, scoped mutation, durable recovery and truthful capability profiles. S03 completed before the cross-stage plan, so the shared tool profile and the first Q-1 baseline are done at the start of S04 (gates 11 and 12): the native loop reaches hoi4-agent-tools through one pinned tool profile, the same one Setup installs for a local Claude Code or Codex session, the engine binary ships in the image, and a missing engine is reported as unavailable, never replaced silently by the TypeScript path. The UI design track (ADR-025) has produced the design system the focus screens use.

Upstream prerequisites, each merged and released in hoi4-agent-tools: W3 (layout verdict, in-game connector model, inline renders), `hoi4.mod_index` and the closure option of `hoi4.script_validate` on the engine. W7 (the Rust layout solver) is expected. If it has not landed, snapping uses the plain grid through `hoi4.focus_rewrite`, solver snapping is recorded as unavailable and the gate wording is narrowed.

Before editing, read the target repository's current instructions. Preserve unrelated work. Confirm authorization names S04. If the entry condition is not met, perform useful read-only diagnosis and report the concrete blocker. Do not silently start a different stage.

## Read-first sequence and reuse

Read [implementation master goal](../06-agent-handoffs/IMPLEMENTATION_MASTER_GOAL.md), [shared instructions](../../AGENTS.md), [Claude guidance](../../CLAUDE.md), [roadmap](ROADMAP.md), [contracts](../02-architecture/contracts-and-interfaces.md), then [SPEC-001](../03-subsystem-specs/001-onboarding-and-mod-setup.md), [SPEC-002](../03-subsystem-specs/002-native-runtime-and-models.md), [SPEC-003](../03-subsystem-specs/003-orchestration-and-recovery.md), [SPEC-005](../03-subsystem-specs/005-git-collaboration-and-review.md), [SPEC-006](../03-subsystem-specs/006-hoi4-tools-and-validation.md), [SPEC-007](../03-subsystem-specs/007-visual-state-synchronization.md), [SPEC-009](../03-subsystem-specs/009-focus-and-other-editors.md). Consult the [source registry](../00-research/01-research-sources.md) for the exact inspected pins. Use current local repository instructions to resolve implementation details and record any conflict with this planning baseline.

## Engine, editors and tool quality

The cross-stage plan is in [engine, editors and tools across stages](ROADMAP.md#engine-editors-and-tools-across-stages).

| Topic | In S04 |
|---|---|
| Shared Rust engine | Link `hoi4-engine` into the server. Run closure, lints, override audit and findings baseline on the proposal worktree before every agent commit. Unknown counts are shown and never count as a pass. Focus routes move onto the engine if W3 and W7 have settled, otherwise in S05 |
| Focus editor (FR-5) | Projection from `hoi4.focus_inspect`, review images and verdict from `hoi4.focus_render`, writes through `hoi4.focus_rewrite`, IDs from `hoi4.mod_index`, references from `hoi4.reference_search` and `hoi4.source_lookup`. Snapping uses the W7 geometry so a person's move and the solver agree |
| MCP tools exposed | The S03 read tools plus `hoi4.focus_rewrite`. No other rewrite tool |
| Hagane harvest | Span repair plans as C-02 proposals, the per-edit finding delta on the review card, the dead-flag and DLC lints in the commit check. Decide whether the xyflow focus-graph wiring saves real effort and record the decision. Build the canvas on `@xyflow/react` either way |
| Import | Preservation report and findings baseline in the Setup import flow for uploads and GitHub imports (P4) |
| Q-1 | No regression on T-BENCH-01, plus the editor round trip measured against Hagane Studio on the same focus tree |
| Coding-agent integration | T-EDIT-01 runs the coding-client path on a clone with the same tool profile |

## Implementation-sized work packets

Each packet should produce a reviewable change and evidence before another packet expands its scope. Split a packet further if it cannot be reviewed in one ordinary engineering session. Owners are responsibilities, not mandatory simultaneous agents.

| Packet | Owner | Deliverable | Work and contract boundary |
|---|---|---|---|
| S04-W1 | Domain engineer | Focus read projection | Build the projection from `hoi4.focus_inspect` through the broker, the call the agent makes, bound to file hashes. Map current source IDs, prerequisite groups, exclusion, relative/shared layout and localization. Resolve icons and vanilla references against the server game reference. Render unsupported constructs with diagnostics in the browser canvas, and show the W3 layout verdict. Record parser, renderer, tool and game reference versions. |
| S04-W2 | Editor engineer | Two-way visual mutation | Implement node creation, prerequisite linking, node moves, snapping and scalar-property edits only within the source-preserving subset established by EXP-02. Each gesture becomes the same `hoi4.focus_rewrite` request the agent sends, with expected hashes, routed through C-02 with the person as actor. Applied changes emit `source_changed` and redraw every open editor. Preserve comments, encoding and unknown spans. Keep viewport state separate from gameplay coordinates. If creation or linking remains unsupported, narrow the advertised fixture and record the missing first-slice capability before claiming three-mode equivalence. |
| S04-W3 | Runtime engineer | Autonomous focus task and commit check | Accept approved intent for three-to-five nodes, supported effects and localization. Let the native Rust agent plan, edit and validate within limits while the browser tab is closed, through the same tools as the editor. Tell the agent which IDs a person changed since its last read. Link the engine and run closure, lints, override audit and the findings baseline before every agent commit. |
| S04-W4 | Review engineer | Unified change card | Show semantic summary, raw diff, source-linked preview, the finding delta, the closure verdict with unknown counts, the layout verdict, checks actually performed, limits and restore point. Show a pending agent proposal on the canvas with apply or discard per node. Bind final approval to proposal hash or a valid wider grant. |
| S04-W5 | Git engineer | Commit, GitHub publication, restore and import report | Commit only owned files under expected HEAD, after the engine check passes. Under a separate publication grant, push the owned branch to the linked GitHub repository and open a pull request through C-10 RepositoryLink. Reconcile lost commit, push and pull request replies by querying actual state. Restore the earlier accepted snapshot or produce a reviewed revert while preserving subsequent human work. Add the preservation report and findings baseline to the Setup import flow. |
| S04-W6 | Product reviewer | Three-mode equivalence, editor parity and Q-1 | Run equivalent intent in visual, hybrid and autonomous modes. Compare resulting native semantics and preservation, not identical model wording. Run T-EDIT-01 across the gesture, native agent and coding-client paths, and run T-BENCH-01 with the editor round trip. |

## Packet dependencies and exclusions

A contract-only dependency permits parallel implementation after that interface is reviewed. Runtime integration still waits for the dependency's acceptance evidence. Packet owners may edit only their agreed modules and fixture area. A change to another owner's contract requires coordination before editing.

| Packet | Depends on | Explicitly excluded |
|---|---|---|
| S04-W1 | S03 | Full event, map or technology editors, Hagane Studio visual design |
| S04-W2 | S04-W1 | Writing unsupported syntax or normalizing entire files, an editor-only write path |
| S04-W3 | S04-W1, S04-W2 contract | Arbitrary scripts or a large generated tree, committing with a new finding |
| S04-W4 | S04-W1, S04-W2, S04-W3 | Claiming preview fidelity proves game behavior, event or decision preview cards |
| S04-W5 | S04-W4 | Shared default-branch publication, merging pull requests |
| S04-W6 | S04-W2, S04-W3, S04-W4, S04-W5 | Expanding scope to rescue an unsuccessful scenario |

## Verification plan

Use F-FOCUS-SMALL and F-FOCUS-FIDELITY. Include missing icon, localization gap, duplicate focus ID, unknown effect, cycle, unsupported relative edit, external source edit after preview and collision between a drag and agent change. Verify no-op bytes independently from the parser's own serialized output.

Run T-EDIT-01 for each supported gesture: the same intent as a gesture, a native agent call and a coding-client MCP call on a clone must produce identical bytes, projection and finding delta. Include a person moving a node while the agent's proposal on that node is pending, and an agent rewrite sent after a person's edit with stale hashes. Run T-ENG-01 with a proposal that adds an unknown scripted trigger and confirm the commit is blocked, and one that touches a file with baseline findings and confirm only new findings block. Run T-IMPORT-01 on a GitHub import and a ZIP upload. Run T-BENCH-01 and compare with the baseline of gate 12.

Each of M-API-A, M-API-B and M-LOCAL-C attempts the same bounded focus task. Record unsupported capability of the self-hosted endpoint rather than silently replacing it with another provider. Simulate a lost push reply and a lost pull request reply on a disposable GitHub repository. Repeat one successful task after process death at each critical mutation boundary. Actual game observation is a separate user-operated test where available. The server does not run the game.

Use the [test strategy](../05-verification/TEST-STRATEGY.md) and [evaluation matrix](../05-verification/EVALUATION-MATRIX.md). Exercise production code for the behavior under test. Record exact revision, fixture hash, environment, command or action, result, evidence kind and limitation. A declared upstream test or successful source inspection does not replace running the required stage test.

## Measurable exit gates

1. One newly created project and one project imported from GitHub complete the full slice through reused onboarding in the browser.
2. All three interaction modes produce inspectable native files through the same proposal and authorization contracts.
3. Supported drag/edit fixtures preserve all untouched bytes and reject unsupported or stale writes without damage.
4. Review links exact source, preview and validation hashes. No source render is labelled actual game verification.
5. Owned branch commit, authorized push and pull request creation, and restore succeed with dirty unrelated work preserved. Lost-response replay creates no duplicate commit, push or pull request.
6. The user can download or clone the mod, continue editing it outside the factory, push changes back and reopen it in the factory with the external changes detected.
7. Measured preview and interaction results are recorded against proposed budgets, with any revised target justified.
8. The server links the engine, and every agent commit passed closure, lints, override audit and the findings baseline. Unknown references are counted and never reported as a pass. Every import has a preservation report and a recorded baseline.
9. Every supported focus gesture and the agent's matching edit go through the same operation, with parity across the gesture, native agent and coding-client paths. An agent's applied edit appears in an open editor without reload, and a person's edit is visible to the agent's next inspect.
10. T-BENCH-01 shows no regression from the baseline of gate 12, and the focus editor round trip is measured against Hagane Studio on the same tree.
11. The native loop and the coding-client profile name the same hoi4-agent-tools release, engine revision and read tools. A missing or broken engine yields an explicit unavailable result, never a silent fallback. T-ENG-01 removes the engine binary from a test image and confirms the affected tool answers unavailable.
12. A Q-1 baseline from T-BENCH-01 is stored with fixture hashes, comparator pins and hardware, recorded before the focus work is measured, and each surface that could not run, such as vanilla without a licensed copy, is named with its reason.

All gates are evaluated against the final stage revision. Changed inputs invalidate affected evidence. A known defect in the selected critical path requires a fix or an explicit narrower scope decision. Optional unavailable checks remain named limitations.

## Risk and rollback

Disable the affected writable editor operation and retain source inspection. Revert the owned product change or mod proposal through recorded history. Preserve broken fixtures as regressions.

## Engineering learning checkpoint

Review repeated human corrections, ambiguous editor gestures and unsupported syntax. Add a narrow regression or helper reference instead of bloating global guidance.

Record the result in `evidence/S04/learning-review.md`. Follow [reusable knowledge policy](../07-developer-workflow/REUSABLE-KNOWLEDGE-POLICY.md). A no-change decision is valid when justified. No agent may enlarge its own authority by editing a skill or instruction.

## Configuration checkpoint

Verify only selected read/write HOI4 tools are exposed, with `hoi4.focus_rewrite` the only rewrite tool, the engine revision linked into the server matches the MCP tool profile, model budgets match the task, the GitHub App holds only the selected repository permissions and optional live-game tools remain disabled.

Record `evidence/S04/config-review.md`, including changed files, verified client or runtime versions, source provenance and any deferred live check. Update the [knowledge changelog](../07-developer-workflow/KNOWLEDGE-CHANGELOG.md) only for material accepted guidance changes.

## Standalone completion and continuation handoff

S04 demonstrates value, but does not claim release readiness. Provide exact fixtures, native output, preview artifacts, provider results, pull request receipts, known syntax limits and restore evidence. Add the parity results, the engine check reports, the preservation reports, the benchmark comparison and the xyflow wiring decision. Ask for review before S05.

Produce `evidence/S04/environment.json`, `scope.md`, `checks.json`, `handoff.md`, `learning-review.md` and `config-review.md`, with artifact hashes or references. Use the [handoff template](../06-agent-handoffs/STAGE-HANDOFF-TEMPLATE.md).

A new session must be able to identify the completed revision, exact scope, gate results, failed or unavailable checks, changes to accepted knowledge and the next decision without the prior chat. End after reporting S04. **Do not start the next stage automatically.**

