# SPEC-009: Focus authoring and other source-linked editors

Status: **Proposed**. Primary requirements: REQ-012 and REQ-013. Shared requirements: REQ-009, REQ-011, REQ-014, REQ-023, REQ-025 and REQ-044. S04 delivers a small focus task end to end in visual, hybrid and autonomous modes. Later editor families expand only after their write contracts pass.

Read first: [visual synchronization](007-visual-state-synchronization.md), [HOI4 tools](006-hoi4-tools-and-validation.md), [assets and localization](010-assets-localization-and-content.md), [Git collaboration](005-git-collaboration-and-review.md) and [contracts](../02-architecture/contracts-and-interfaces.md).

## User value and first vertical slice

A user describes a small focus branch, sees a useful preview, changes an allowed property or gives high-level feedback, and obtains reviewable source with localization and evidence. An experienced modder can open raw source at any point. An autonomous run can perform the same bounded task without requiring the canvas to stay open. All modes operate on one workspace and produce C-02 proposals.

The S04 demonstration creates or edits a small branch in a synthetic mod fixture, resolves references, renders the exact candidate, validates it and presents a Git review. The initial writable surface is deliberately narrower than the full HOI4 grammar. Complex shared, relative, scripted or dynamically positioned content remains inspectable and source-editable until the corresponding structured operation is proven safe.

## Reuse and explicit limits

Agent Tools `25f5641bbc07c65a822ef576ac6e92bd12666880` exposes focus inspect, render and rewrite operations with typed outputs and linked artifacts. Its render path includes HTML, SVG, JSON, source maps and default PNG output. An inspected integration test exercises creation, compaction, repeat-compaction no-op and rendering of a 1,024-focus fixture. That is useful test coverage evidence, not an independently measured performance claim. [SRC-A12, actual workflow test](https://github.com/klimPaskov/hoi4-agent-tools/blob/25f5641bbc07c65a822ef576ac6e92bd12666880/tests/integration/mcp-large-focus-workflow.test.ts)

Reuse the shared source model and preservation layer rather than introduce another complete parser. Event and technology tools currently provide inspect/render surfaces. Their presence does not establish public editing APIs. Modmaker provides useful canvas, undo and prerequisite-group examples, but its UTF-8 decoding/rewrite path is unsuitable as the preservation-critical writer. CWTools can independently validate references and scopes, while its printer remains outside this write path. [SRC-A10, tool surface](https://github.com/klimPaskov/hoi4-agent-tools/blob/25f5641bbc07c65a822ef576ac6e92bd12666880/docs/tools.md) [SRC-A23, comparison writer](https://github.com/Floxizdia/hoi4-modmaker/blob/a5c138ffacf4ed471769558f864cbe55ec5163fa/app/focus_surgery.py)

## Focus projection and identity

A FocusProjection contains workspace and snapshot IDs, tree identity, source layer, source paths, parser/renderer versions, scenario assumptions, fidelity, diagnostics and supported operations. Each node carries its exact source location, ID, label/localization references, icon reference, declared coordinates, resolved display coordinates and dependencies. Distinguish a literal source value from a computed position or unresolved expression.

Prerequisites are groups, not a flattened edge list. Keep separate prerequisite groups and their alternatives, mutual exclusions, shared-focus references and relative-position relationships. Preserve source ordering and unknown blocks. A visually missing node may be filtered, conditional, inherited, unresolved or outside the viewport. The interface must explain which, rather than deleting it from a replacement file.

Use a source-bound object identity composed of path, defining scope, symbol and source revision. A user-visible ID alone may collide across layers or malformed files. The planner checks namespace and global definition collisions before proposing a new ID. A renamed ID invalidates old object selection and triggers consumer analysis. Rename and deletion are unavailable as one-click structured operations until their reference-closure tests pass.

## Interaction model

Canvas, inspector, changes and source views share the same selection and proposal. The canvas supports pan, zoom, fit, search, a keyboard-accessible node list, diagnostics and visible selection. The inspector exposes supported fields and opens raw source for unsupported constructs. Source and inherited values are distinguishable without requiring the user to understand internal parser types.

Dragging a node proposes source-coordinate changes. View zoom and pan never alter game source. A relative layout requires an explicit choice of the relationship being changed, with a before/after preview. Reward-only tasks cannot silently move nodes, compact the tree, alter prerequisites or rewrite naming conventions. A request to improve layout creates a separate layout proposal with its own scope.

Hybrid feedback can identify a branch, selected nodes or a rendered region and attach a natural-language intent. The agent resolves that reference against the projection snapshot, proposes changes and explains resulting behavior. If the source changed, it refreshes the selection instead of guessing that the same screen coordinates still identify the same node. Autonomous mode uses the same projection and evidence APIs without making visual review a mandatory checkpoint for every already authorized action.

## Operations and proposal contract

| Operation | Inputs | Output or refusal |
|---|---|---|
| Inspect tree | Workspace snapshot and tree/path selector | Projection, graph diagnostics, source locations and ambiguity |
| Render tree | Exact snapshot, scenario, bounds and asset policy | C-05 linked artifacts and fidelity statement |
| Propose new focus | Tree, unique ID intent, bounded plan and source base | C-02 with source, localization and asset-reference changes |
| Edit property | Stable object identity, property and desired value | Minimal typed edit or unsupported-source refusal |
| Change relationships | Explicit prerequisite groups or exclusions | Semantic graph diff and reference validation |
| Change layout | Declared absolute/relative policy and selected set | Coordinate-only diff unless broader scope is authorized |
| Compare | Current and proposed snapshots | Source and semantic changes, stale evidence and changed consumers |
| Apply | Final C-02 and C-08 | C-04 receipt and regenerated projection from final bytes |
| Undo or restore | Owned accepted operation and current source | Inverse proposal or conflict, never blind overwrite |

The proposed FocusEditIntent names desired semantics, not an arbitrary executable patch. The native host validates final arguments, current path hashes and allowed operations. All additional files, including localization or generated assets, belong to the same proposal manifest. A review cannot approve one source file while a hidden side effect updates several others.

## Preview and mutation boundary

The upstream public rewrite is a single operation with internal planning and recovery. For a review-first task, the factory executes it in an isolated proposal workspace or uses a specifically tested read-only core preparation adapter. It then reads actual resulting bytes, performs independent checks and renders from that candidate. The host publishes only the reviewed file set. Do not invent an existing upstream public apply token or let an agent authorize itself by returning an ID.

The upstream focus-plan sidecar may retain authored planning information. Treat it as versioned derived data with a source binding, not a second source of gameplay truth. An external edit can make it stale. A mismatched sidecar must be reconciled or regenerated before reuse, with user intent retained as evidence. The source file, not a stale canvas cache, determines the current mod.

Rendering may resolve game or dependency icons from the read-only server game reference of SPEC-006. The browser receives the rendered preview, not the raw game files. Missing icons use a clearly identified placeholder and diagnostics. Do not silently copy game artwork into the mod, a download or a pull request. A source preview never asserts that the game's conditional GUI, animation, font, resolution or availability matches every scenario.

## Errors, concurrency and recovery

Malformed syntax, duplicate definitions, unresolved source spans, unrepresentable encoding, stale projection and unsupported relative/shared constructs have separate errors. Inspection can continue where useful, but unsafe structured mutations are disabled. The source escape hatch remains available through a reviewed text proposal with appropriate validation.

A human and an agent may change the same node in different worktrees. Textual nonoverlap is insufficient if one changes the ID or prerequisite while the other changes layout. Merge semantic intent and dependency effects, regenerate the combined proposal and validate its new base. If the relationship is uncertain, request a concrete conflict decision rather than silently choose the latest screen state.

After a tool timeout, reconcile the C-04 operation and upstream job before replaying a mutation. After host interruption, recover the proposal workspace and publication journal separately. Undo creates an inverse change against current hashes. If a user has edited the same field since application, preserve the edit and present a conflict. The prior receipt and preview remain historical evidence.

## Acceptance matrix

| Test | Observable acceptance |
|---|---|
| One branch in visual, hybrid and autonomous modes | Equivalent final source semantics and equivalent evidence requirements |
| Reward-only request | Reward and approved localization change, with coordinates and edges byte-identical |
| Existing BOM/CRLF/comments/unknown blocks | No-op byte identity and minimal approved edit, or explicit refusal |
| Multiple prerequisite groups | Alternatives and groups survive inspect-edit-render round trip without flattening |
| Shared or relative content outside writable coverage | Inspectable explanation and source escape hatch, no silent absolute conversion |
| Duplicate or shadowed ID | Ambiguity and provenance visible, wrong definition never mutated |
| New ID collision across dependencies | Proposal blocked or regenerated before apply |
| External edit after preview | Old proposal becomes stale and cannot overwrite new source |
| Repeated layout compaction | Second accepted operation is unchanged on identical source and policy |
| Missing icon or localization | Explicit diagnostic and named placeholder, no false completeness |
| Tool crash after candidate write | Reconciliation finds one outcome, no duplicate node or localization entry |
| Malicious preview artifact | Isolated rendering cannot execute code, read the application session or access files outside its project and the read-only game reference |

S01 measures parser fidelity and render cost on small and large fixtures. S04 acceptance includes an actual rendered artifact inspected by a human or visual-capable reviewer, full final source hashes, static diagnostic delta, grant, receipt and Git review. Performance records name server profile, reference browser, node count, asset count, cold/warm cache, render duration, transfer size and memory. Choose initial scale limits from those measurements and expose them honestly.

## Other editor roadmap

After the focus slice, add read-only event-chain, decision, technology/doctrine, country-history, idea and GUI projections where existing tools support them. Each writable family needs a specific source-preserving operation set, reference analysis, scenarios and recovery tests. Content generation through text remains possible under the common proposal contract before every visual editor exists.

S06 adds coordinated content and localization packages. S07 owns maps. Later work can add richer focus relationships, supported relative/shared layouts, graph-wide renaming and deletion, resolution-aware GUI editing and broader scenario previews. None of those capabilities is implied merely by displaying an editable-looking control.
