# SPEC-007: Visual state and source synchronization

Status: **Proposed**. Primary requirements: REQ-009, REQ-010, REQ-011 and REQ-044. Shared requirements: REQ-013, REQ-014, REQ-022 and REQ-023. First implementation is S04.

Read first: [focus editor](009-focus-and-other-editors.md), [Git review](005-git-collaboration-and-review.md), [maps](008-maps.md), [contracts](../02-architecture/contracts-and-interfaces.md) and [user workflows](../01-product/users-and-workflows.md).

## Purpose and invariants

A visual contributor and an autonomous agent work on one real mod repository. The canvas is a projection with proposed edits. It is never an independent database of gameplay truth. The same ChangeProposal contract applies to a drag, an inspector edit, an agent task and a source-editor patch.

The user can remain in visual mode, inspect raw source at any time, give high-level feedback on a preview, or close the editor while a bounded task continues. Opening a canvas is not a required checkpoint. Review is required only at the meaningful boundary established by the existing grant and project policy.

Five invariants govern every editor:

1. Every rendered object identifies the source snapshot it represents.
2. Every proposed mutation identifies the exact base bytes or source object version it expects.
3. Unsupported syntax remains present and visible as a fidelity limitation.
4. A stale projection cannot overwrite newer source.
5. An applied proposal can be traced through validation and Git to its approval and original intent.

These are product requirements. No outside renderer is assumed to guarantee them.

## Evidence and reuse boundary

The supplied newer Chaos Redux source separates source previews from game observations and restricts reward-only focus repairs from changing layout. The source audit is recorded in [research coverage](../00-research/04-evidence-and-research-log.md). Actual tool integration capabilities are bounded by [the assets audit](../00-research/02-existing-assets-audit.md).

The design uses a projection adapter over the selected HOI4 parser and source lookup tools. It does not prescribe a new full Clausewitz grammar. S01 must measure existing parser fidelity, source locations and unsupported constructs before selecting the write strategy. A parser that normalizes away comments may support inspection, while writes require a separate verified source-span adapter.

## Interaction surfaces

The default workspace shows the current task, its progress and an artifact preview. A contributor can switch among Canvas, Changes and Source without losing the task. The status strip states source revision, unsaved proposal count and evidence level. Advanced provider, tool, budget and parser controls live in a details panel.

Visual mode produces explicit edit gestures. Hybrid mode accepts agent proposals and allows local adjustments before apply. Autonomous mode executes the accepted task within its grant and reports at policy checkpoints. The product stores the mode as an interaction preference. Authorization is a separate grant with enforceable limits.

An inaccessible tool produces a limited preview or diagnostic where possible. It does not blank the whole project or create a false success badge. Keyboard navigation and a searchable node list must provide access to graph actions. Large canvases use viewport culling and selection summaries, with the source escape hatch available even when a visual projection fails.

## Projection and edit contracts

A proposed `ArtifactProjection` has artifact type, workspace ID, source snapshot hash, parser/renderer version, supported operations, nodes or layers, diagnostics and a fidelity declaration. Each object carries its source path, identifier, source location and any inherited or computed fields.

A `VisualEditIntent` records object identity, gesture, desired property values and the projection hash. The host resolves this into C-02, a ChangeProposal. Coordinates are proposals in the source's layout units, not raw screen pixels. Zoom and panning are local UI preferences. The proposal contains changed source spans, semantic before/after values, affected dependency closure and evidence requirements.

For the first focus editor, only supported scalar fields and unambiguous coordinate relationships are writable. Shared or relative positioning, scripted expressions, duplicates and macros remain inspectable, with explicit limitations where a safe round trip is unavailable. The user can open source or request a reviewed source patch. The editor must never silently flatten relative layouts or delete unknown keys to make its own model convenient.

## Apply protocol

The host receives an immutable proposal and checks workspace identity, policy version, grant revocation epoch, source hashes and allowed operation category. It validates the final arguments after all extensions and model hooks. The edit runs in the task's owned workspace, then source is re-read and validated. The preview is regenerated from those final bytes.

If a proposal changes after review, its hash changes and any approval bound to the old hash is invalidated. A wider task grant may still authorize it if the final operation remains within that grant, but the host must record which rule authorized it. A previous screenshot or pre-edit validation cannot be attached as evidence for new bytes.

Multi-file source application needs a journal and recoverable staging. Renaming one file at a time is not an atomic transaction across a mod. Readers must see either a complete accepted snapshot or an explicit applying state. A crash after some writes requires journal reconciliation before the app advertises a current projection.

## External changes and concurrency

Filesystem events are hints. On activation, before apply and before commit, the host verifies hashes and Git state. If an external editor changes a projected file, the UI marks the projection stale and offers refresh, review the external change, or rebase the pending gesture against a new base.

A human moving a node while an agent changes that node's prerequisite is not automatically safe because the text spans differ. Semantic conflict analysis checks object identity, graph invariants and downstream effects. Nonoverlapping, independent changes can form a new combined proposal with fresh evidence. Overlapping identifiers or uncertain semantics require explicit conflict resolution.

The first version uses Git worktrees and reviewed proposals. It does not require a real-time CRDT or central collaboration server. Later shared cursors and live previews may be added without making their transient UI state authoritative for source writes.

## Undo and redo against current source

Proposed edit history links each owned accepted operation to its C-02 proposal, C-04 receipt, actor, workspace, before/after hashes and retained source or binary objects. Pending gestures can be discarded before apply. Undo of an applied edit creates an inverse proposal against current source. Redo creates a fresh proposal for the original semantic intent after the undo has settled. Each applied action has a new operation identity and receipt, with links to its predecessors. History records describe completed effects and never grant permission themselves.

Both actions recheck current workspace identity, source hashes, dependency semantics, tool capability and C-08 scope, expiry and revocation epoch. Reuse an existing valid grant within its limits. An expired grant, conflicting human edit, changed object identity or unsupported inverse produces a specific refusal before writes. Independent changes may be rebased into a new proposal only after semantic checks. Retain the original history when a subsequent edit makes redo unavailable, and explain the conflict instead of silently replacing that edit.

After restart, reconstruct history from settled receipts and reconcile unfinished operations before enabling redo. Re-read resulting bytes, regenerate the projection and produce fresh evidence. Prior previews remain historical. Redo never repeats a provider generation, upload or game action as a side effect of restoring content. A retained generated asset can be referenced by hash when the new proposal authorizes its use. These rules apply equally to visual, hybrid and agent-authored changes in the owned workspace.

## Fidelity levels shown to users

| Evidence kind | Meaning | UI wording example |
|---|---|---|
| Source projection | Renderer interpreted a particular source snapshot | Preview from source |
| Static validation | Named syntax and semantic checks ran | Static checks passed, game execution untested |
| Fixture simulation | Named simulator and scenario ran | Simulated for this fixture |
| Actual game observation | A specific build, DLC profile and scenario ran | Observed in HOI4 for the recorded scenario |
| Unknown or stale | Required input or current evidence absent | Preview is stale, refresh before applying |

These labels are independent of whether the proposal was written by a person or an agent. Actual game evidence is scenario-specific. It does not certify every branch, language, resolution or DLC combination.

## Acceptance tests and performance probes

| Test | Input or interruption | Observable acceptance |
|---|---|---|
| VIS-01 | No-op parse and render on a fixture with comments, unknown blocks, BOM and CRLF | Original bytes unchanged |
| VIS-02 | Drag one supported focus node | Only intended coordinates change, IDs and unknown spans remain identical |
| VIS-03 | Open a file with unsupported expression-based position | View shows limitation, attempted unsafe drag yields a clear refusal and no write |
| VIS-04 | External edit arrives after proposal creation | Apply returns stale-base conflict, preserves both versions and offers rebase |
| VIS-05 | Agent updates the selected focus while human changes an overlapping property | No silent overwrite, conflict contains both intents and base hashes |
| VIS-06 | Close canvas during an autonomous task | Task continues within its grant and produces the same source/evidence contract |
| VIS-07 | Pause and revoke while a preview conversion runs | Further writes stop at the next enforceable boundary, partial artifacts stay non-current |
| VIS-08 | Kill the app between two file writes | Recovery journal detects partial state, no complete-preview badge appears prematurely |
| VIS-09 | Open the resulting mod outside the app | Native files remain editable, required content is not hidden in a proprietary project file |
| VIS-10 | Keyboard-only select, inspect, move and review | Supported operation can be completed without a pointer |
| VIS-11 | 500-node and 2,000-node synthetic focus graphs | Record load, interaction p95 and memory on the declared machine, compare with proposed budgets |
| VIS-12 | Malformed localization or missing icon | Preview identifies the missing dependency and source path without hiding other valid nodes |
| VIS-13 | Apply one supported edit, undo it, then redo it on unchanged source | Undo restores the prior owned bytes, redo restores intended semantics, unrelated bytes remain identical and all three operations have linked distinct receipts |
| VIS-14 | After undo, a human edits the same field while an agent proposes a related graph change | Redo refuses stale or semantically conflicting input, preserves both contributions and offers a fresh combined proposal only after explicit conflict resolution |
| VIS-15 | Restart after undo, then request redo with a revoked grant or an unsettled prior receipt | No automatic write or external job replay occurs. Reconciliation and current authorization are required before one fresh redo operation can settle |

Initial proposed usability targets are a useful preview within 3 seconds for a 500-node cached fixture and visible drag feedback within 100 ms p95 on the declared reference machine. These are experiment targets, not measured performance. S04 may revise them with a recorded benchmark and user impact. A screenshot comparison alone is insufficient to prove write fidelity.

## Open decisions

EXP-02 determines whether the existing parser exposes lossless spans and how inherited/shared focus structures should be edited. EXP-06 measures canvas interaction and map tile costs. Whole-tree automatic relayout, round-trip editing of every script form and live multiplayer editing stay deferred until specific tests justify expansion. Each new writable property adds source-preservation fixtures and conflict rules before it appears as an enabled UI control.

