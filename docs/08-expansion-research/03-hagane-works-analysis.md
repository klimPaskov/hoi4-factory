# Hagane Works analysis

Status: **Research record and Phase B outcome, 2026-10-31**. This document records what Hagane Works is, what its skills repository teaches, how each useful idea compares with our existing practice, and what the maintainer decided. The verbatim decisions are in the [decision record](00-decision-record.md). The full source reading, with file and line references for all 183 catalogued ideas and the claim ledgers, is in the research appendices: [platform](research/01-hagane-platform.txt), [skills half A](research/02-hagane-skills-a.txt), [skills half B](research/03-hagane-skills-b.txt) and [Hagane Studio reuse](research/11-hagane-studio-reuse.txt).

## Summary

- **What it was.** A hosted visual HOI4 workbench at `scharnhorst.hagane.works`, a Next.js web application with editors for focus trees, events, decisions, ideas, characters, technologies, states, military content and more, an AI assistant, a command wiki and an MCP endpoint driven by personal access tokens. It was a web application, not a desktop application.
- **What happened.** Its updates page announced on 2026-10-28 that the service closes on 2026-11-11 because revenue never covered server and maintenance costs, after about 10,000 creators used it. Registrations, purchases and game preview were already closed. The team published a local edition, [Hagane Studio](https://github.com/hagane-works/hagane-studio) v0.1.2 (Windows, Go backend and React frontend, GPL-3.0-only) on 2026-10-30.
- **Permission.** The maintainer agreed with the Hagane Works author to take over development and may use the project in any way. The permission is in writing and covers both repositories (D-05).
- **Our verdict.** The platform owns its projects as JSON tables and regenerates mod files on export, which is the opposite of our native-files-and-Git design. Its value for us is its hard-won lessons, a handful of reusable modules and a strong investigation workflow, not its architecture.

## Sources and licensing

| Source | Revision | License | Use |
|---|---|---|---|
| [hoi4-modmaking-skills](https://github.com/haganeworks/hoi4-modmaking-skills) | `ef7028c12cd021216bfb7660f85ad93e5efc93e1` | CC BY-NC-SA 4.0, plus written permission held by the maintainer | Ideas and verified facts, reworded in our style. The original request asks not to copy skills wholesale |
| [hagane-studio](https://github.com/hagane-works/hagane-studio) | `1e831623ed162e1a64763c242c55b249376ba20c` | GPL-3.0-only, plus written permission held by the maintainer | Harvest of selected modules (A1) |
| Public site pages (landing, docs, updates) | Fetched 2026-10-31 | Not licensed for reuse | Feature facts only. Text copies stay in the git-ignored `.work/` folder |

Licensing notes that still apply after D-05: the command wiki text on the hosted site is Paradox's own documentation text and is not part of the permission. Screenshots in the studio repository that show game map data, the runtime path that converts game sprites for the browser, and avatar images of unknown provenance must not be moved. Hagane's name and icon need their own confirmation before any public reuse. Before code moves, each adapted file carries a provenance note with the source path and commit, because the originals have no per-file headers.

## How the skills repository works

The repository holds 13 skills, a 40-file field reference, one human guide and a changelog. Every skill drove the hosted platform through its REST or MCP API, so many mechanics are platform-specific. The generic patterns are the valuable part.

| Element | How it works | Generic lesson |
|---|---|---|
| Activation | Each skill's description says when to use it. Process skills (investigate, design, wiring) fire automatically, build cards fire on entity nouns | Descriptions as activation contracts, with when-not-to-use stated |
| Orchestration | One design orchestrator, specialist skills per project shape, thin per-domain build cards that defer field tables to the shared reference and end with cross-link footers | Small loaded bodies, shared references, trap lists at the end of each card |
| Verification | A ladder: request accepted, platform validator green, entity re-read, fresh export, game `error.log`. Only the last rungs close a bug | Separate intermediate signals from real evidence |
| Knowledge hygiene | The guide states what was run in game on which build. The changelog records the incident behind each rule with numbers | Evidence labels and an incident ledger |
| Updates | Every skill tells the agent to `git pull` at session start | Rejected for us (R1) |

The repository also documents its own platform bugs honestly, which makes it a useful test-case source. Examples: custom technologies outside vanilla grid boxes were not rendered (0 of 718 sampled), and the older validation route silently returned fewer findings (283 against 284 issues, 165 against 173 event findings).

## Verification against HOI4 1.19.3.0

The two reading passes checked 141 Hagane claims against the installed game's `documentation/` folder, vanilla script and, for engine messages, the game executable's strings. About 15 were contradicted or misleading. None of these may enter our knowledge:

| Claim in Hagane | What 1.19.3.0 shows |
|---|---|
| Continuous focuses are declared with `continuous = yes` on a node | Vanilla uses palette files under `common/continuous_focus/` and no node uses that key |
| Option keys ending `.d` collide with descriptions | Only against the platform's own derived keys. Vanilla uses `.d` for fourth options |
| Script `.txt` files must not carry a BOM | 1,264 of 4,404 vanilla script files start with one |
| `set_state_owner = TAG` | The state-scope form is `set_state_owner_to` |
| `on_send_attache` and `on_lend_lease` hooks | Neither exists in the installation |
| Cosmetic tag `_DEF` is the adjective | `_DEF` is the definite form, `_ADJ` is the adjective |
| `production_speed_factor` modifier | Not a real modifier |
| `war_with_on_add` decision key | Not a decision key |
| Wargoal folder `common/wargoal_types` | The folder is `common/wargoals` |
| Buttons cannot have `size` | 116 vanilla buttons have it |
| Two nodes at one coordinate crash the game | Vanilla has coincident pairs, some gated by `allow_branch` |
| `set_character_name` takes a character token | It takes a name |
| Military industrial organisations belong to Trial of Allegiance | That DLC's data ties to South American AI strategies |

Confirmed claims with high value include: `ROOT` in `on_state_control_changed` is the new controller, a non-1936 start needs the bookmark date, the start-date define and history to agree, 21 of 21 impassable states carry `force_link_ownership_to`, `mutually_exclusive` must be declared on both sides (vanilla has zero one-sided pairs among 10,668 focus IDs), and a malformed effect in a focus reward makes the parser skip the rest of the file while static checks stay green. Several runtime claims could not be tested offline and stay `[unverified]` until an in-game test.

The game also ships helpers Hagane never used: first-party `_documentation.md` files in about 40 `common/` subfolders, loc formatters such as `idea_desc`, and console commands such as `loc_check`, `loc_check_events`, `loc_check_focuses` and `debug_checkportraitssanity`. These feed P2 and the P7 test plans.

## Universal HOI4 requirements versus Hagane conventions

| Kind | Examples | Treatment |
|---|---|---|
| Universal engine behaviour, verified | Reciprocal `mutually_exclusive`, AND-of-OR prerequisite groups, impassable state ownership links, start-date agreement, parser cascade after a malformed block, scope of `ROOT` in on_actions | Enters the kit with an evidence tag (P8, P9) |
| Universal engine behaviour, unverified offline | Localisation key charset limits, no fallback between languages, folder override rules, the `:0` suffix | Stays `[unverified]` until tested in game |
| Hagane platform conventions | Personal access tokens, the `/api/go` route family, auto-prefixed IDs and `_1` suffixes, hidden startup events for overlays, standalone and overlay export modes, export logs at the archive root | Not adopted (R2, R3). Useful only to recognise a Hagane export on import |
| Hagane design conventions | History spine, two axes, four endings, historicity tags, tone lock | Not adopted. Chaos Redux has its own design rules |

## Proposals and decisions

All proposals were presented with the idea, Hagane's approach with file references, our current approach from repository evidence, the change, the home, the trade-offs and a recommendation. Hagane file references below point at commit `ef7028c`.

| ID | Idea | Hagane source | Our current approach | Decision |
|---|---|---|---|---|
| P1 | Whole-mod reference closure with resolved, unresolved and unknown results and per-kind coverage including zeros | [`_validation-tools.md` lines 6 to 41](https://github.com/haganeworks/hoi4-modmaking-skills/blob/ef7028c12cd021216bfb7660f85ad93e5efc93e1/field-reference/_validation-tools.md#L6-L41) | Forward checks for localisation and sprites only. Script references checked in reverse (`REF-DEAD`). `script_validate` warns on changed files. The agent-tools symbol index covers a few kinds without a whole-mod report. Chaos Redux live QA B001 found 547 startup errors in game that a closure check would catch | **Adopt** in the shared Rust engine (D-07) |
| P2 | Vocabulary generated from the game's `documentation/` files, plus reciprocal `mutually_exclusive`, vanilla namespace collision, inert MTTH on triggered-only events, duplicate keys inside one event and unknown modifier keys | [`hoi4-build-focus` lines 38 to 59](https://github.com/haganeworks/hoi4-modmaking-skills/blob/ef7028c12cd021216bfb7660f85ad93e5efc93e1/skills/hoi4-build-focus/SKILL.md#L38-L59), [`hoi4-build-events` lines 38 to 61](https://github.com/haganeworks/hoi4-modmaking-skills/blob/ef7028c12cd021216bfb7660f85ad93e5efc93e1/skills/hoi4-build-events/SKILL.md#L38-L61) | `script_validate` already checks commands against the installed documentation. `focus_inspect` covers missing targets, duplicates and cycles. The three named lints are absent | **Adopt** in the Rust engine |
| P3 | Override and load-order audit: definitions removed by same-path files, emptied `replace_path` directories, exact shadow filenames | [`hoi4-submod`](https://github.com/haganeworks/hoi4-modmaking-skills/blob/ef7028c12cd021216bfb7660f85ad93e5efc93e1/skills/hoi4-submod/SKILL.md), [`_timeline-and-replace-path.md`](https://github.com/haganeworks/hoi4-modmaking-skills/blob/ef7028c12cd021216bfb7660f85ad93e5efc93e1/field-reference/_timeline-and-replace-path.md) | No load-order check. The MCP server applies `replace_path` silently | **Adopt** in the Rust engine |
| P4 | Import preservation report per path and a visible shrinking findings baseline | Studio `web/src/generated/exportCoverage.ts` (41 rows) | Setup keeps byte inventories but no coverage explanation. The kit ratchet covers Python tooling only | **Adopt** |
| P5 | Static investigation protocol: root cause first, premise tripwires, evidence ladder, surprising fixes as red flags | [`hoi4-investigate` lines 26 to 56](https://github.com/haganeworks/hoi4-modmaking-skills/blob/ef7028c12cd021216bfb7660f85ad93e5efc93e1/skills/hoi4-investigate/SKILL.md#L26-L56) | Strong root-cause rules exist only in the opt-in live playtest skill | **Adopt** as a small kit skill `hoi4-investigate` with a Chaos Redux mirror |
| P6 | Log reading order, first-error parser cascade, launcher `.mod` drift, symptom table, used only when a log is already available | [`hoi4-investigate` lines 58 to 77](https://github.com/haganeworks/hoi4-modmaking-skills/blob/ef7028c12cd021216bfb7660f85ad93e5efc93e1/skills/hoi4-investigate/SKILL.md#L58-L77) | Deliberate rule against asking for logs. Crash knowledge only in the opt-in skill | **Adopt**. The no-asking rule stays |
| P7 | Short in-game test checklist | [`hoi4-mod-design` lines 27 to 34 and 138 to 141](https://github.com/haganeworks/hoi4-modmaking-skills/blob/ef7028c12cd021216bfb7660f85ad93e5efc93e1/skills/hoi4-mod-design/SKILL.md#L27-L34) | Rule against telling the user to run in-game validation | **Adapt**: a test plan section in Chaos Redux event docs and an optional HOI4 Factory card, never in chat |
| P8 | Import confirmed traps missing from the kit, reworded and evidence-labelled | Trap lists in `hoi4-investigate`, `hoi4-wiring`, `hoi4-build-scripting`, `hoi4-submod` | Many already covered in "Common mistakes and silent failures" sections. No start-date rule exists | **Adopt** |
| P9 | Evidence tag on every rule and a `LESSONS.md` incident ledger | [guide lines 3 and 394 to 400](https://github.com/haganeworks/hoi4-modmaking-skills/blob/ef7028c12cd021216bfb7660f85ad93e5efc93e1/guides/dynamic-spirit-from-variables.en.md#L394-L400), [`CHANGELOG.md`](https://github.com/haganeworks/hoi4-modmaking-skills/blob/ef7028c12cd021216bfb7660f85ad93e5efc93e1/CHANGELOG.md) | No evidence labels. Repeated-mistake capture relies on agent judgement | **Adopt** |
| P10 | One authoring home for generic rules. Chaos Redux engine skills generated from kit text plus an overlay. Design skills audited | Changelog lines 176 to 219 (three incidents of edits landing only downstream) | 32 tool files synced. Skills drift (0.11 to 0.95 similarity) | **Adopt** generation (option A) |
| P11 | Slim the always-loaded template and largest skills with thin cards, shared references and sharper descriptions | Skills of 7 to 19 KB with cross-link footers | Template 55 bytes under its cap, five skills near theirs, rules repeated in up to 27 files | **Adopt with a constraint**: every instruction stays, text is only deduplicated or moved, with a diff audit |
| P12 | Design brief whose blanks become tiered questions | [`CONCEPT_TEMPLATE.md`](https://github.com/haganeworks/hoi4-modmaking-skills/blob/ef7028c12cd021216bfb7660f85ad93e5efc93e1/skills/hoi4-mod-design/CONCEPT_TEMPLATE.md), [`DESIGN_DOC_TEMPLATE.md` lines 42 to 54](https://github.com/haganeworks/hoi4-modmaking-skills/blob/ef7028c12cd021216bfb7660f85ad93e5efc93e1/skills/hoi4-mod-design/DESIGN_DOC_TEMPLATE.md#L42-L54) | Chaos Redux plans in an external chat. The kit asks only when an essential input is missing | **Adopt for HOI4 Factory only** (semi-autonomous design checkpoint) |
| P13 | Stable IDs and locked, derived or open tags on spec items, cited by handoffs, with a two-way gap audit | `hoi4-mod-design` lines 81 to 86, 105 to 107 and 169 | Per-claim acceptance tables, no stable item IDs, no global decision log | **Adopt** for new Chaos Redux packages and HOI4 Factory specs. Drift Bot itself was later rejected (B3) |
| P14 | Feasibility check before a spec commits to a mechanic | `hoi4-mod-design` lines 74 to 79 and 178 to 216, `hoi4-submod` lines 193 to 196 | No feasibility step in either planning skill | **Adopt** |
| R1 | `git pull` at every session start | `README.md` "Staying up to date" | Pinned sync with checks | **Reject** |
| R2 | Platform-owned IDs with auto prefixes and suffixes | `README.md` lines 15 to 20, `hoi4-build-api` lines 121 to 123 | `next_id` allocation and collision checks | **Reject** |
| R3 | Hidden startup-event overlays for vanilla characters | Platform documentation | Direct edits or shadow files | **Reject** |
| R4 | Their field reference as our reference | `field-reference/` | Generated vocabulary and the kit wiki snapshot | **Reject** |

## Product patterns taken into HOI4 Factory

These come from the hosted platform and Hagane Studio. They are carried into the [architecture](07-architecture.md) and the [feature specifications](05-feature-specifications.md) rather than voted one by one, because they refine designs the product already has.

| Pattern | Where Hagane has it | How HOI4 Factory uses it |
|---|---|---|
| Preview then apply, with apply or discard per node and all-or-nothing rollback | AI master agent in the hosted platform | Visual review checkpoints show the proposal on the canvas and accept or discard per node |
| Edit revision tokens that refuse stale writes, and receipts that separate "saved" from "verified" | Studio `edit_revision`, `mutation_receipt` | Expected file hashes on every proposal (already in C-04) and receipts that state the evidence rung reached |
| Hash-guarded span repair plans | Studio `clausewitz/crossref.go` | Source-bound proposals revalidated on the server |
| Findings introduced, improved and resolved by an edit | Studio save diagnostics | Per-job finding delta shown on the review card |
| Fidelity ledger with honest statuses | Studio `exportCoverage.ts` | Import preservation report (P4) |
| One operation layer for the editor and the agent, with parity tests | The hosted editor and MCP shared one Go service, and a second route family drifted and under-reported | Editors and agents call the same server operations, with tests that compare them. Required for every visual editor by FR-5 |
| Click on a preview region to jump to its source field | Event preview card | Render regions mapped to source spans |
| Per-project DLC selection that changes checks and previews | Hosted platform | Project setting used by validation and previews |
| Field guidance inside tool schemas | Studio `hw describe` and `hw fields` | Tool descriptions carry per-field guidance |

## Hagane Studio reuse

Measured size: about 195,000 lines of non-test TypeScript and 234,000 lines of non-test Go, with almost no comments or architecture documents. Every editor works on JSON table rows and regenerates files on export. The Go parser keeps comments but not byte offsets and re-emits canonical text, so it cannot provide byte-preserving edits. Its focus grid differs from the game grid our tools use.

The maintainer chose **harvest, then archive** (A1) and narrowed it on 2026-11-01 (A1-1): nothing of Hagane's look is reused, because HOI4 Factory's UI comes from its own design track (ADR-025), and only what would take a lot of work to build from scratch is harvested. About 2.5 percent of the Go code is worth taking, and at most one piece of TypeScript:

| Harvest item | Size | Destination |
|---|---|---|
| Focus graph shell on xyflow: the wiring only, no layout code or styling | About 3,400 lines | HOI4 Factory client, only if it actually saves work when S04 is designed |
| Hash-guarded span repair plan, per-edit finding delta, dead-flag lint, DLC lint rule | Small | Shared Rust engine |
| draw.io sketch importer | 398 Go lines | Shared Rust engine |
| 158 finding codes | Catalog | Test fixtures for the Rust engine |
| Game discovery, safe mod install, log parsing | Small | Local companion, rewritten in Rust |

Hagane Studio gets a final tagged release, a banner pointing to HOI4 Factory and maintenance-only fixes. It is archived once HOI4 Factory imports a Hagane export ZIP with a preservation report, the companion syncs local mod folders, and the focus, event, decision and character editors exist. Porting effort was estimated at 45 to 60 engineer-days for the phased path.

## Ideas not proposed

These ideas were catalogued but not put to a vote, because existing practice already covers them or Chaos Redux has its own approach. The maintainer can raise any of them.

| Idea | Reason |
|---|---|
| Story design template: history spine, two axes, four endings, historicity tags, tone lock | Chaos Redux has its own design rules and planning workflow |
| Multi-nation ownership rules (only the leader nation creates a faction, agree global flag names up front) | Useful but narrow. Recorded as a candidate trap for P8 if verified |
| Versioned demo exports with an accepted snapshot | Git tags and HOI4 Factory restore points cover it |
| Intent-to-recipe cookbook in the form "the author wants X, fill these fields" | Tied to the platform's forms. The kit's skills already explain intent |
| Response envelope normalisation and self-describing API shapes | Platform API hygiene. Our tool schemas already describe their shapes |
| Idempotent create and creation order rules for the platform API | Platform-specific. Agent-tools jobs already use stable request keys |
| Never write game data from memory, pull reference data | Already a strong kit rule |
| Two-channel operating model where the agent never asks the human to click | Covered by the existing rule set and by P7 |
| Domain-expert self-critique passes (historian, numbers reviewer) | Close to the existing improvement loop and auditor agents |
