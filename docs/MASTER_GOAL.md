# Master goal: a native HOI4 modding workspace

Version: v02. Planning date: 2026-10-23. Status: **Product vision and proposed delivery plan for review**. This package does not approve an architecture or authorize implementation.

## Purpose and users

Build an independent desktop environment in which people and AI agents can create, maintain, inspect and verify real Hearts of Iron IV mods. A lore writer should express approved history and design constraints without first learning script. An artist should contribute attributable assets. A map designer should see geography and its source relationships. An experienced modder should retain exact source control. A maintainer should be able to delegate a bounded task, leave the editor closed and return to a reviewable result.

The product must work directly with permitted model APIs or local inference. End users must not need Claude Code, Codex, Cursor or another coding harness. Claude Code is the preferred tool for future development of this application, with portable guidance for other contributors. That development preference does not restrict the finished product's model choices.

## Required product behavior

Use existing HOI4 Mod Setup as the substantive foundation for creating and importing projects. Preserve its verified planning and installation rules, adapt the actual native services where required, and hand a validated workspace to agents and editors. Existing source, assets, Git state and human instructions must survive onboarding unless an exact reviewed change authorizes modification.

Keep editable native mod files as creative source truth. The factory may add operational state and rebuildable indexes, but users must be able to continue outside the application. Approved canon, task state, source bytes, derived previews and learned suggestions have separate roles and lifetimes.

Provide optional visual authoring and preview alongside natural-language tasks. Visual-first, hybrid and autonomous modes must share the same source, ownership, validation and history. Human gestures become source-bound proposals. Agent changes refresh the relevant projections. A stale proposal cannot silently overwrite an intervening edit.

Support scoped autonomy with meaningful review boundaries. A grant can cover a whole bounded task. Reuse that authority while it remains valid. Pause, revocation, cost limits, source conflicts and uncertain effects must remain visible and durable. Completion depends on the accepted objective and actual evidence, not the model's declaration.

Make correctness claims precise. A source render, a static check, a simulation and an actual game observation are different evidence kinds. Show which files and versions were checked, what remains unknown and how to restore the prior state.

Preserve contribution ownership through intent, source changes, evidence and Git review. Start with one reliable writer and expand to coordinated contributors after conflict and recovery tests pass.

## First useful slice and first release

The first useful demonstration, S04, creates or imports a mod through reused Setup, runs a native model, makes a small three-to-five-focus change, offers a useful source-linked preview and a bounded direct edit, and supports an equivalent no-canvas task. It validates source, presents semantic and raw diffs, commits owned changes and restores or continues externally.

S05 adds stronger context, freshness, model switching and long-sequence recovery. Applicable S08 packaging, backup, protection and pilot gates are required before a narrow release. A focus-only pilot may explicitly defer S06 collaboration expansion and S07 maps. It may not claim those deferred capabilities.

Maps remain a first-class later product area. The first map stage inspects existing data, imports a complete structured closure and supports one validated bounded edit. Rich drawing, terrain painting and generation from geography, cultures, nations and lore have intended contracts and a future sequence in SPEC-008. The audited tool's unavailable geography synthesis is a real constraint.

## Guiding principles

1. Reuse verified working components before writing substitutes, while preserving their licenses and engineering boundaries.
2. Keep the model replaceable and enforce integrity in the host and operating system.
3. Prefer deterministic tools for parsing, file application, validation, dependency checks and state transitions.
4. Make every mutation attributable, source-bound, reviewable and recoverable.
5. Preserve approved human intent separately from inference and retrieved content.
6. Add orchestration, editors and retrieval infrastructure only when measured workflow value justifies them.
7. Use progressive UI with understandable status and on-demand technical detail.
8. Treat source inspection as evidence about implementation, without claiming it is an executed product test.
9. Maintain engineering knowledge through small evidenced improvements, with explicit review for changes to authority or architecture.
10. Keep the native source escape hatch and avoid dependence on proprietary project formats.

## Constraints and assumptions

Planning is complete before product implementation begins. The leading architecture is conditional on S01 experiments. Windows is a proposed first platform, not a claim about the user's current machine. Every advertised platform needs its own packaging and recovery results.

Game and DLC files remain local and outside distributable source fixtures. Code licenses, assets, provider terms and game agreements are separate decisions. No consumer subscription is assumed to grant API access. Remote provider use requires the user's destination and budget authority.

The initial runtime uses trusted bundled adapters. Third-party executable extensions require reviewed code and demonstrated containment. Skills and repository instructions do not create new operating-system permissions. Worktrees isolate Git work, not malicious code.

## Non-goals and later aspirations

The first slice excludes a complete map suite, every HOI4 editor, arbitrary extension execution, large agent swarms, automatic shared-main publication and cloud-first hosting. Full real-time collaboration, advanced generated assets, semantic retrieval infrastructure and generalized world generation remain prioritized backlog items.

This assignment does not create product code, initialize a new application repository, install model tooling, edit a mod or run the game. The archive supplies plans, starter guidance and package validation utilities only.

## Measurable outcomes

| Outcome | Proposed acceptance |
|---|---|
| Independent operation | Packaged task works with no external coding CLI installed or invoked |
| Onboarding preservation | All unapproved bytes and unrelated Git state match the before inventory |
| Shared visual/autonomous state | All three interaction modes produce accepted native semantics through the same contracts |
| Recoverable effects | Injected crashes and lost replies cause no duplicate unsafe write or commit |
| Source fidelity | No-op is byte-identical and supported edits preserve untouched spans |
| Human control | Revocation stops new dispatch, pause persists and unresolved in-flight effects remain explicit |
| Model continuity | Different provider resumes the same task and evidence identity within its measured capabilities |
| Honest evidence | Preview/static/simulation/game results are correctly labeled with exact scope |
| Useful collaboration | Required semantic and binary conflicts are caught without losing either contribution |
| Engineering continuity | A clean future session finds the authorized stage, known workflow and evidence without the old chat |

All numeric performance and evaluation budgets are proposed and attached to fixtures in [the test strategy](05-verification/TEST-STRATEGY.md). Product success remains unmeasured until implementation and evaluation.

## Navigation and change control

The 45 stable requirements are in [product requirements](01-product/product-requirements.md). [Traceability](01-product/requirements-traceability.md) links them to decisions, 13 subsystem specs, stages and acceptance. The [roadmap](04-implementation-stages/ROADMAP.md) contains eight dependency-led stages. The [implementation master goal](06-agent-handoffs/IMPLEMENTATION_MASTER_GOAL.md) tells a future agent how to begin one authorized stage.

Changes to approved requirements, architecture, security or distribution need a recorded maintainer decision. Routine evidence-backed corrections within an authorized stage remain normal reviewable work. Every substantive handoff includes an engineering learning review and a configuration review, including a justified no-change result.

