# S07: Map inspection, structured import and bounded edits

Status: **Proposed future implementation stage**. Nothing in this packet was executed as product implementation during planning. Primary agent: Claude Code. Delegate only bounded work with explicit file ownership. Revised on 2026-10-24 for the [maintainer decisions](../02-architecture/maintainer-decisions-2026-10-24.md).

## Objective and non-goals

Deliver useful existing-map inspection, structured import planning and a carefully bounded edit. Treat a reference image as design input. Do not promise a playable generated world from an image.

The inspected Agent Tools geography-model synthesis path is unavailable. Reuse actual inspect/preview/validation features and implement only the independently verified missing import or edit operation authorized for this stage.

## Entry conditions

S05 accepted. A synthetic structured map fixture, the server game reference for vanilla map comparison and an explicit map scope are available. S06 is needed only for shared asset or collaboration features used by the chosen map scope.

Before editing, read the target repository's current instructions. Preserve unrelated work. Confirm authorization names S07. If the entry condition is not met, perform useful read-only diagnosis and report the concrete blocker. Do not silently start a different stage.

## Read-first sequence and reuse

Read [implementation master goal](../06-agent-handoffs/IMPLEMENTATION_MASTER_GOAL.md), [shared instructions](../../AGENTS.md), [Claude guidance](../../CLAUDE.md), [roadmap](ROADMAP.md), [contracts](../02-architecture/contracts-and-interfaces.md), then [SPEC-006](../03-subsystem-specs/006-hoi4-tools-and-validation.md), [SPEC-007](../03-subsystem-specs/007-visual-state-synchronization.md), [SPEC-008](../03-subsystem-specs/008-maps.md), [SPEC-010](../03-subsystem-specs/010-assets-localization-and-content.md). Consult the [source registry](../00-research/01-research-sources.md) for the exact inspected pins. Use current local repository instructions to resolve implementation details and record any conflict with this planning baseline.

## Implementation-sized work packets

Each packet should produce a reviewable change and evidence before another packet expands its scope. Split a packet further if it cannot be reviewed in one ordinary engineering session. Owners are responsibilities, not mandatory simultaneous agents.

| Packet | Owner | Deliverable | Work and contract boundary |
|---|---|---|---|
| S07-W1 | Map engineer | Read projection and dependencies | Load provinces, definitions, states, regions, terrain and related profile data from the workspace, falling back to the server game reference for inherited vanilla files. Expose unknown/missing inputs and exact source links. Build a tile or region index for bounded reads. |
| S07-W2 | Import engineer | Structured import planner | Validate dimensions, formats, color/ID uniqueness and reference closure. Preserve IDs where possible, otherwise produce explicit remapping and affected consumers. Generate a reviewable plan before writes. |
| S07-W3 | Map editor engineer | Bounded mutation | Select one operation such as a supported province assignment or terrain region change. Route it through C-02 and native preservation tools. Reject unsupported topology changes. |
| S07-W4 | Designer | Reference-image workflow | Store attribution, intended scale and design interpretation separately from exact map data. Show what remains to be modeled and validated. Keep image suggestions reversible. |
| S07-W5 | Performance engineer | EXP-06 | Measure representative full-map read, tile cache memory, local redraw and dependency validation. Compare bounded update cost to full rebuild before choosing indexing strategy. |
| S07-W6 | Reviewer | Game and topology verification | Run deterministic topology fixtures and, where a user operates the game, record a user-observed scenario on the declared build. Record unsupported builds and avoid transferring one scenario's result to all maps. |

## Packet dependencies and exclusions

A contract-only dependency permits parallel implementation after that interface is reviewed. Runtime integration still waits for the dependency's acceptance evidence. Packet owners may edit only their agreed modules and fixture area. A change to another owner's contract requires coordination before editing.

| Packet | Depends on | Explicitly excluded |
|---|---|---|
| S07-W1 | S05 | Generation of a new playable world |
| S07-W2 | S07-W1 | Treating a reference image as structured game input |
| S07-W3 | S07-W1, S07-W2 | Arbitrary topology mutation |
| S07-W4 | S07-W1 contract | Unsupported geography synthesis or new paid image calls |
| S07-W5 | S07-W1, S07-W2, S07-W3 | Choosing full-map rebuilds without measurement |
| S07-W6 | S07-W1, S07-W2, S07-W3, S07-W4, S07-W5 | Advertising support for untested game builds |

## Verification plan

Use F-MAP-STRUCTURED, F-MAP-BROKEN and F-MAP-REFERENCE. Include duplicate colors, missing definitions, out-of-range IDs, mismatched dimensions, broken state/region references, isolated invalid pixels, ambiguous image scale, asset collision and a changed source during import.

Inject interruption after staged map outputs and after one dependent file is written. A rollback must restore the entire affected closure or stop with explicit conflicts. Benchmark on declared server hardware and source dimensions, including browser rendering of tiles. Generated synthetic fixtures must not contain redistributed game assets, and map tiles derived from the game reference are not offered for download.

Use the [test strategy](../05-verification/TEST-STRATEGY.md) and [evaluation matrix](../05-verification/EVALUATION-MATRIX.md). Exercise production code for the behavior under test. Record exact revision, fixture hash, environment, command or action, result, evidence kind and limitation. A declared upstream test or successful source inspection does not replace running the required stage test.

## Measurable exit gates

1. Existing map data is inspectable with fidelity limits and source identity.
2. Structured import produces a complete dependency and conflict report, then applies only its approved closure.
3. The supported bounded edit preserves unrelated source and passes named topology checks.
4. Reference image upload never creates an unsupported game-ready claim.
5. Known unavailable geography synthesis remains disabled or is separately implemented and validated, with evidence of what changed.
6. Map recovery and performance results are recorded. Actual game evidence is scenario-specific and missing game evidence remains explicit.

All gates are evaluated against the final stage revision. Changed inputs invalidate affected evidence. A known defect in the selected critical path requires a fix or an explicit narrower scope decision. Optional unavailable checks remain named limitations.

## Risk and rollback

Restore map files and dependent metadata together from the proposal's journal, subject to current hash checks. Keep original imports and reference images. Disable a risky edit operation while preserving read-only inspection.

## Engineering learning checkpoint

Capture topology edge cases, performance measurements and source-format constraints in dedicated map references and tests. Do not generalize one game build to all versions.

Record the result in `evidence/S07/learning-review.md`. Follow [reusable knowledge policy](../07-developer-workflow/REUSABLE-KNOWLEDGE-POLICY.md). A no-change decision is valid when justified. No agent may enlarge its own authority by editing a skill or instruction.

## Configuration checkpoint

Record game reference build and DLC set, tool schema, map dimensions, memory budget and optional image-provider rights. Check that no game files or private reference assets enter images, exports or pull requests.

Record `evidence/S07/config-review.md`, including changed files, verified client or runtime versions, source provenance and any deferred live check. Update the [knowledge changelog](../07-developer-workflow/KNOWLEDGE-CHANGELOG.md) only for material accepted guidance changes.

## Standalone completion and continuation handoff

State the exact supported map operation and what remains deferred. Include import manifests, topology reports, before/after hashes, performance environment and game-observation limits.

Produce `evidence/S07/environment.json`, `scope.md`, `checks.json`, `handoff.md`, `learning-review.md` and `config-review.md`, with artifact hashes or references. Use the [handoff template](../06-agent-handoffs/STAGE-HANDOFF-TEMPLATE.md).

A new session must be able to identify the completed revision, exact scope, gate results, failed or unavailable checks, changes to accepted knowledge and the next decision without the prior chat. End after reporting S07. **Do not start the next stage automatically.**

