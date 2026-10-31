# Current-state audit

Status: **Research record, 2026-10-31**. This audit describes what exists today in HOI4 Factory and the related projects, so the expansion plan builds on evidence and not on assumptions. Detailed per-project reports are in the [research appendices](research/README.md). Every statement below was checked against source at the revisions listed in the table.

## Revisions inspected

| Project | Revision | License | Where |
|---|---|---|---|
| HOI4 Factory | `7c70b0c` (main, clean) | AGPL-3.0 | this repository |
| Agentic HOI4 Modding Kit | `4d9b309` working tree, one manifest commit behind `1dd36a5` | Apache-2.0 | `klimPaskov/Agentic-HOI4-Modding` |
| Chaos Redux | `c8782e065e` plus the maintainer's uncommitted edits, read only | not declared | `klimPaskov/Chaos-Redux` |
| hoi4-agent-tools | `fbc5144` | Apache-2.0 | `klimPaskov/hoi4-agent-tools` |
| HOI4 Live Tools | `6b91a2a` | Apache-2.0 | `klimPaskov/HOI4-Live-Tools` |
| HOI4 Mod Setup | `9b7cd08` (release 0.8.0) | Apache-2.0 | `klimPaskov/HOI4-Mod-Setup` |
| Hagane Works skills | `ef7028c` (v1.5.0 line) | CC BY-NC-SA 4.0, maintainer holds written permission (D-05) | `haganeworks/hoi4-modmaking-skills` |
| Hagane Studio | `1e83162` (v0.1.2) | GPL-3.0-only, maintainer holds written permission (D-05) | `hagane-works/hagane-studio` |
| HOI4 game reference | 1.19.3.0 with 12 DLC | Paradox, read only | local Steam install |

## HOI4 Factory

**Implemented: S01 and S02 only.** A hosted multi-tenant foundation exists. It has email and GitHub accounts, projects with four roles (owner, maintainer, contributor, viewer), PostgreSQL row-level security on every project table, server workspaces with a controlled Git layer, ZIP and folder upload, read-only GitHub App import, the full HOI4 Mod Setup flow over HTTP with recovery, a read-only game reference check and a React Setup wizard. The S02 gates pass, as recorded in the [S02 handoff](../../evidence/S02/handoff.md).

| Area | State in code |
|---|---|
| Crates | `accounts`, `workspace`, `game-ref`, `intake`, `setup-adapter`, `server`, and a probe-grade `agent` used in production only for Setup analysis |
| HTTP routes | Health, game reference, auth including GitHub OAuth, project CRUD and restore, uploads, GitHub repository listing, import and link, workspace descriptor (C-01), twelve Setup routes |
| Database | 16 tables in 9 migrations. Forced row-level security on projects, members, intake, repository links, the allowlist and every Setup table |
| Browser client | React 19, react-router, i18next, English only, strict CSP with no inline styles, a lint test that fails on literal UI text, about 54 test cases. No design system yet (ADR-025 track not started) |
| Compose | Server, PostgreSQL and Mailpit. No worker, gVisor runtime, tool container, object store or webhook ingress |
| Helpers without routes | Task worktrees, restore-point refs and `commit_paths` exist in the workspace crate but nothing calls them. `add_member` exists without an invitation route |

**Not implemented anywhere in this checkout:** tasks, runs, attempts, grants, event streams, API key routes, memory, canon, notes, issue and pull request mirroring, webhooks, push, invitations, diffs, exports, comments, notifications and help. The S03 crates (`tasks`, `broker`, `hoi4-tools`) are planned in [the S03 draft scope](../../evidence/S03/scope.md) and may be in progress on the Mac.

**Already designed, ready to extend.** The planning package holds a rich authority model that the new areas plug into: C-03 run events, C-04 tool receipts, C-06 context bundles, C-07 memory items, C-08 approval grants with revocation epochs, the SPEC-003 task and attempt state machines, SPEC-005 review records and the ADR-018 in-app GitHub hub. Autonomy levels, jobs, the Git workflow and drift detection are therefore extensions of existing contracts rather than new systems.

**Genuinely uncovered before this research:** a Kanban or project view, labels, milestones and acceptance criteria on tasks, user-editable notes, first-run tours or in-app help, spec-versus-implementation drift detection, image generation pipelines beyond a spec paragraph, a browsable decision history, configurable approval checkpoints, mid-task mode switching and a review queue.

**Tensions with recorded decisions** that the plan resolves explicitly:

| Tension | Recorded text | Resolution in this plan |
|---|---|---|
| Notes versus ADR-022 memory | ADR-022 has no page to browse or edit memories | Notes are project documents, not memory items. Memory stays page-less ([feature specifications](05-feature-specifications.md#shared-notes)) |
| Parallel agents versus ADR-007 | One writer before parallel agents | Decided in the product rounds ([decision record](00-decision-record.md)) |
| Desktop use versus ADR-012 | Public web application only | Optional local companion, ADR-012 stands (D-02) |
| Jobs board versus ADR-018 | GitHub authoritative for issues | Native board fields overlay linked issues. Issue text stays on GitHub |
| Autonomy wording | The existing "autonomous" interaction mode already includes checkpoints | Existing modes renamed editing styles, new autonomy levels are checkpoint presets (A2) |
| Contributor approvals | Contributors cannot approve proposals | Checkpoints route to owners and maintainers |
| Background model cost | Who pays for reflection jobs is open (RISKS) | Explicit budgets per feature, see [risks](11-risks-and-open-questions.md) |

Stale documentation found during the audit, useful as fixtures for the P13 gap audit: the ADR summary table omits ADR-025, `PACKAGE_STATUS.md` still counts twenty-three ADRs, `web/README.md` mentions a removed pending-endpoint list, REQ-042 has no stage packet and the SPEC-001 maintain route has no implementation.

## Agentic HOI4 Modding Kit

The kit packages agent instructions, 32 skills, 23 agents, MCP configuration and Python checks for any HOI4 mod. It is distributed through HOI4 Mod Setup.

| Area | Finding |
|---|---|
| Always-loaded text | `CLAUDE_template.md` is 59,945 bytes, 55 bytes under the kit's own 60,000 byte cap. About 48 percent is routing for skills, agents, MCP and runtimes. Only about 9.6 KB is HOI4 coding rules |
| Skills | 782 KB of skill bodies plus 496 KB of references. Five skills sit between 55.7 and 57.8 KB against a 58 KB cap |
| Projections | `AGENTS_template.md` differs from the template on one line. `.agents/skills` and `.qoder/skills` are copied by `.tools/sync/sync_skills.py` and are git-ignored. Agents are generated by `sync_agents.py` |
| Deterministic checks | `check.py quick` and `full` run a localisation audit with 14 rule IDs, an asset wiring audit, three lexical script rules, a GFX conformance audit, size and projection drift checks and unit tests |
| Missing checks | Events that call missing events, focus prerequisites pointing at missing focuses, bracket balance, duplicate IDs other than localisation keys, `replace_path` and load order, BOM checks outside localisation, a findings baseline for HOI4 script |
| MCP use | 28 pinned hoi4-agent-tools routes. Six of them (`impact_inspect`, `decision_inspect`, `mechanic_test`, `package_check`, `scenario_test`, `job_*`) appear in no skill or document |
| Strongest themes | Honest completion reports, scope and authority rules, handoffs, separating static evidence from game evidence |
| Weakest themes | General debugging and crash triage outside the opt-in playtest skill, repeated-mistake capture (no lessons file), feasibility checks before promising a mechanic |
| Contradictions | The template says an unavailable MCP tool does not block completion while several skills call the same evidence mandatory. Skills mix `CLAUDE.md` and `AGENTS.md` names, three plan-folder conventions and two MCP tool spellings |

## Chaos Redux

Chaos Redux is the maintainer's large event mod and the upstream of 32 byte-identical tool files the kit carries.

| Area | Finding |
|---|---|
| Instructions | `CLAUDE.md` is canonical (459 lines, at its 60 KB ceiling) and `AGENTS.md` is generated by `.tools/sync/sync_instructions.py` |
| Skills | 26 `chaos-redux-*` skills and 23 agents. Engine-system skills are 0.91 to 0.95 line-similar to the kit's, but `events` is 0.20, `event-assets` 0.17 and `debug-playtest` 0.11. No script keeps them in step |
| Event lifecycle | Catalog workbook row, external planning chat producing a spec package ZIP, import with hash evidence and per-claim acceptance, implementation with strict naming (`chaosx.nr<ID>.<n>`), temporary asset workspace, localisation, verification, event documentation, catalog export, commit |
| Checks | Strong forward checks for localisation and sprites, reverse-only checks for script references (`REF-DEAD`), flag set or checked audits, clone detection, generator freshness. No check for events, effects or triggers that are used but never defined |
| Evidence of the gap | The live QA run of 2026-10-03 (`docs/testing/live_qa/20261003_full_playtest/bugs.md`, B001) found 547 startup errors, including unknown scripted triggers and an undefined effect, only by launching the game |
| Done rules | Strict: no completion claim while anything is simplified or stale, a passing check proves only what it checks, live testing is user-owned unless explicitly invoked, `Playable` is never inferred. None of the 155 catalog rows is `Playable` |
| Policy on logs | Agents never ask for or search for logs and never tell the user to run in-game validation (`CLAUDE.md:319`). This is a deliberate maintainer rule and the plan keeps it |

## HOI4 tool servers

| Tool | Capability relevant to the plan | Limits |
|---|---|---|
| hoi4-agent-tools 4.6.1 | 28 tools. Focus inspect writes full plans with resolved grid coordinates, AND-of-OR prerequisite groups, mutual exclusions, diagnostics and a source map. Focus rewrite patches only changed byte ranges with hash guards. Event graph snapshots, decision inspection, impact graphs, probability analysis, offline scenario and mechanic tests. About 1,994 tests | No whole-mod unresolved-identifier report, no reciprocal mutual exclusion lint, no namespace or modifier-key lint, no focus rewrite dry run, no per-property edit, no undo. Cold start 12 to 33 seconds on a large mod |
| HOI4 Live Tools | 47 Python tools that launch HOI4, fire events, run console commands, read logs and saves, take screenshots | Needs an interactive Windows desktop. The Rust Linux preview needs an X server and one exact game build. Cannot run on the hosted server |
| HOI4 Mod Setup 0.8.0 | Tauri-free server facade already used by `crates/setup-adapter`. Scan, ownership classes, plan and apply with conflicts, recovery, before and after inventories, `.hoi4-mod-setup/install.lock.json` per mod | Apply needs a confirmed analysis. One symlink or case collision blocks an import. Update and repair builders are desktop-only |

Specs 006, 007 and 009 of this repository describe an older Agent Tools pin and are partly outdated: there is no minimal typed edit operation, and preview exists for map rewrites and proposed-source comparisons but not for focus or GUI rewrites.

## Hagane Works

Hagane Works ran a hosted visual HOI4 workbench from May 2026. Its own updates page announced on 2026-10-28 that the service closes on 2026-11-11 because revenue never covered server and maintenance costs, with registrations, purchases and game preview already closed. The team published a GPL-3.0 local edition, Hagane Studio, on 2026-10-30. The maintainer has since agreed with the author to take over development (D-05). Full findings are in the [Hagane Works analysis](03-hagane-works-analysis.md).

## What the audit means for the plan

1. Most new product areas extend contracts that already exist. The plan adds records and screens, not a second orchestration system.
2. The biggest quality gap across the ecosystem is forward reference closure. Several real failures would have been caught statically.
3. HOI4 knowledge is spread over three copies (kit, Chaos Redux, Hagane) that drift. One authoring home and generation fix that.
4. Local game work (Live Tools, logs, installing a mod to play) cannot run on the hosted server, which is why the local companion exists.
5. Hosting cost killed the closest comparable product. Cost per user is a first-class design constraint.
