# hoi4-agent-tools improvement plan

Written 2026-11-01. Covers version 4.6.4 (origin/main `109b55d`). Planning only: each work package starts when the maintainer approves it.

## Goal

hoi4-agent-tools must be so far ahead of every comparable tool that the question never comes up (decision Q-1). In practice it must be:

- **faster:** interactive answers in well under a second once warm
- **more accurate:** right answers, no false alarms, honest unknowns
- **more reliable:** the same on every platform and every run, with measured evidence
- **more deeply integrated:** coding agents and HOI4 Factory's visual editors work on one shared state (FR-5)

This plan rests on three sources, all gathered on 2026-11-01:

- a static analysis of cpntodd/HOI4-MCP
- an inventory of our own server
- 50 live calls against Chaos Redux with the read-only tools

## Where we stand against HOI4-MCP

HOI4-MCP is a 19-tool Python server built on a July 2026 copy of our kit. 654 of its 747 files are byte-identical to kit files. It is not a base for anything (M-7). The comparison:

| Surface | HOI4-MCP | hoi4-agent-tools |
| --- | --- | --- |
| Parser | Hand-written and lossy. Drops comments, folds `>` `<` `!=` into `=`, garbles bare lists such as `{ a b c }`, and can crash on `nan`/`inf` values | TypeScript parser, plus a Rust engine that round-trips all 7,717 vanilla files byte for byte in 0.6 s |
| Validation | Brace matching and whole-file substring heuristics. Never reads the game's `documentation/` | Commands, kinds and scopes checked against the installed docs. Typo suggestions. 0 false alarms on four real files |
| HOI4 facts | Wrong rules shipped as hard rules:<br>- `hide_window` (doesn't exist)<br>- BOM advice reversed<br>- `add_ideas_effect` (doesn't exist)<br>- a 387-entry modifier table missing 585 of 742 modifiers | Read from the installed game, so it follows the game build |
| Reference lookup | Exact-ID SQLite over 8 content kinds. No effect or trigger lookup | Ranked search over the game docs and the 1.19.3-reviewed wiki, with cited, revision-bound reads |
| Rendering | None | Focus, event, technology, GUI and map renders |
| Writes | One fuzzy text-replace tool with no path limit, no backup and a lossy encoding round trip | Atomic byte-range rewrites with drift guards, a journal and rollback |
| Tests and CI | 104 small tests. No CI, although its docs say otherwise | About 1,970 tests. CI on Linux and Windows. Engine CI on four platforms |
| Wiki | Our July snapshot, from before the 1.19.3 reviews | 64 pages, reviewed for 1.19.3 |

**Ideas worth taking, made better:**
- loading skills on demand (M-1)
- rules as reviewed data (M-2)
- signatures for recurring errors (M-3)
- generated tool docs with drift tests (M-5)
- timing large public mods (M-6)

## What the live test found in our own tools

**What works well:**
- focus inspect and render
- technology inspect, trace and render
- `next_id`
- `source_lookup` and `impact_inspect` for real symbols
- script typo and scope checks
- honest unknowns in probability and decision results

**What is wrong:**

| # | Problem | Evidence |
| --- | --- | --- |
| 1 | `event_inspect` reports scripted effects that exist as missing. Helper expansion is deferred on large workspaces, so traces lose their helpers | `chaosx.nr9.1`: 6 helpers in `009_white_peace_effects.txt` marked `EVENT_HELPER_UNRESOLVED`. 10,224 nodes affected. `expandHelpers: true` changes nothing |
| 2 | Upstream callers come back empty, so "callerless" counts are inflated. A modder could delete live events | `chaosx.nr10.20` shows 0 callers, while `source_lookup` finds the caller. 5,895 "callerless" events |
| 3 | Selectors, ids and subjects are ignored, and unknown ids return `ok` with no results | Event `scan` file selector, `state_flow` subject, decision category id. Missing event, state, decision, window or symbol. A missing GUI window returns a 116 MB artifact |
| 4 | False "probable defect" on event targets | `has_event_target` presence checks and `meta_effect` names marked `EVENT_TARGET_READ_BEFORE_SAVE`, confirmed |
| 5 | Map reference false errors | `prioritize = { … }` provinces treated as states. The `NOT = { state = 0 }` sentinel flagged as an error |
| 6 | GUI false errors and warnings | Sprite names returned through `defined_text` flagged as missing loc keys, about 5,500 times, burying real errors. Intended overlaps and empty-meter clipping flagged |
| 7 | `probability_analyze` ignores the requested adapter and step count | Asked for `decision_ai_will_do`, got `mission_ai_will_do`, and 1 sweep point instead of 5 |
| 8 | Plain-English lookup misses often | About 5 of 12 live questions hit. The held-out baseline is 7 of 24 first. Misses include `add_timed_idea`, `has_war_with`, `owner =` and `mean_time_to_happen` |
| 9 | Misleading results | `error_log` says "no errors in this mod" for a 5-day-old log from a launch that never loaded the mod. `script_validate` reports `PARTIAL` with 0 unresolved. The `source_lookup` example in the tool description fails |
| 10 | Answers hidden in artifacts | Most tools return counts inline and the answer in a 5 KB to 116 MB artifact. Every reply carries 12 unrelated `filesScanned` entries. Large-workspace deferrals make almost every result `PARTIAL` |
| 11 | False error on `owner = GER` in state scope | `SCRIPT_DYNAMIC_VARIABLE_AS_TRIGGER`. Vanilla uses it, for example `common/decisions/AFG.txt:824` |
| 12 | Parameters are never checked | `parametersChecked: false`. A wrong idea id inside `add_timed_idea` passes |
| 13 | Slow cold paths | First `source_lookup` takes 58.9 s. `mod_index` on Chaos Redux takes 13 to 15 s, a decision inventory 33 to 42 s, an event-consumer lookup up to 145 s |
| 14 | Stale game data | The native-effects list and the probability adapters are pinned to 1.19.2. The installed game is 1.19.3 |
| 15 | Coverage gaps | No localisation tool, and only English is indexed. No tools for ideas, characters, MIOs, balance of power, special projects, raids, units, templates or OOB, GFX assets, or sound. Event, decision and technology tools are read-only |

## Targets

The benchmark harness enforces every target, and the numbers become CI gates once the harness exists (WP-1). Starred items are proposed defaults that the first measurement may adjust.

| Surface | Target |
| --- | --- |
| Rules lookup | Right answer first on at least 22 of 24 blind questions\*, and in the top three on all 24. Exact names always first. Negative queries return nothing |
| Lookup speed | Warm p95 at most 50 ms, and at most 1 s after a source change\* |
| Index and source lookup | Cold `mod_index` at most 2 s and first `source_lookup` at most 3 s on Chaos Redux\*. Warm at most 50 ms |
| Event, decision, technology and GUI inspectors | No deferral that changes results on workspaces up to five times Chaos Redux's size. Helpers and callers complete whenever references are complete |
| Validation | No false errors across vanilla and the known-good files of the benchmark mods. Arguments checked wherever the game docs define them |
| Unknown inputs | Every tool returns an explicit not-found with suggestions, never `ok` with nothing |
| Inline answers | The direct answer appears inline in at most 8 KB\*. Artifacts are only for detail and pictures |
| Status | Three distinct states: complete, partial with the exact reason and scope, and failed. "Partial" never hides a real problem |
| Platforms | Identical results on Windows, macOS and Linux |
| Game build | Vocabulary and adapters follow the installed build, and a mismatch is reported |

## Work packages

Each package lists who owns it, what it does, and when it's done.

### WP-1 Benchmark harness and CI gates (Q-1, M-6, M-7, M-12, M-13)

**Owner:** the new accuracy session (proposed).

**What it does:**
- One harness for every surface: lookup, source lookup, validation, reference closure, each inspector and renderer, rewrites and latency.
- **Corpus:**
  - vanilla, Chaos Redux, ZIN and Ireland Total Overhaul
  - two or three large public mods (Kaiserreich, Old World Blues, The New Order) pinned at fixed versions
- **Ground truth:** a hand-checked answer set for each surface, including a zero-false-positive set of known-good files.
- **Where it runs:**
  - CI runs the synthetic fixtures.
  - A nightly run on the maintainer's machine covers real vanilla and the mods. Results stay local with hashes, and regressions are flagged.
- A macOS arm64 Node CI job.
- One run of HOI4-MCP at a pinned commit in an isolated container (read-only mods, no network), as the Q-1 proof.

**Done when:** baselines for every surface are recorded, and CI fails on any regression.

### WP-2 Accuracy fixes from the live test

**Owner:** the accuracy session. Problem 5 goes to the ZIN MCP worker, because it is map code.

**What it does:**
- Fix problems 1 to 4, 6, 7, 9, 11 and 14, each with a regression test drawn from the real case.
- **Problems 1 and 2:** helpers and callers must never be deferred in a way that changes results. Either compute them, or say exactly what was skipped and keep "callerless" counts out of the report until they are complete.

**Done when:**
- Each case from the live test passes in the harness.
- The zero-false-positive set stays clean.

### WP-3 Answers agents can act on (problem 10)

**Owner:** the accuracy session.

**What it does:**
- Every tool returns the direct answer inline: the not-found items, the absent package items, the owner of a state, the evaluated probability.
- Unrelated `filesScanned` noise and workspace-wide counts are removed from focused calls.
- The three status states from the targets.
- Tool examples that work, and errors that name the bad field and the expected shape.
- `tools/list` kept lean, and tool docs generated from the definitions with drift tests (M-5).

**Done when:**
- An agent without artifact access can complete each benchmark task.
- The docs drift test is in CI.

### WP-4 Rules lookup (problem 8, M-10)

**Owner:** the session "Measure and improve the MCP rules lookup", running now.

**What it does:**
- Command cards merged from the game docs, wiki rows and localisation tooltips.
- A vocabulary derived from localisation, plus a short checked list.
- A measured experiment with an optional local embedding model, pinned by hash, with word ranking as the fallback. It is kept only if it clearly wins on the untouched blind set.

**Done when:** the lookup targets are met on the blind set, or the PR records the remaining gap and why.

### WP-5 Move onto the Rust engine (problem 13, D-07, M-8)

**Owner:** the ZIN MCP worker.

**Order:**
1. Ship the engine binaries.
2. Move `mod_index`, then `source_lookup` and the symbol index.
3. Add the whole-mod reference report to `script_validate`.
4. Move the map hot paths, then the remaining analyzers.
5. Retire the TypeScript parser and index.

**Done when:** each route has a merged PR with parity tests against the old output, before and after numbers, and green CI on every platform.

### WP-6 Skills and rules served by the MCP (M-1, M-2, M-3)

**Owner:** the accuracy session, after WP-3.

**What it does:**
- **Skills (M-1):** a `hoi4.skill` tool (list skills, load one) and matching MCP resources. They serve the kit version pinned for the mod, with hashes, and a check fails on drift from the source.
- **Approved rules (M-2):** one format at two levels, kit-wide and per mod. Agents propose a rule. It takes effect after the maintainer approves it, and it carries its reason, evidence and regression test. LESSONS.md moves into this format.
- **Recurring errors (M-3):** `error_log` groups repeated errors across launches into signatures, links them to documentation and rules, and says when the last launch didn't load the mod.

**Done when:** Claude Code, Codex and HOI4 Factory's agent all load the same skill text and rules through the MCP.

### WP-7 Parameter and argument checks (problem 12)

**Owner:** the accuracy session, after WP-5 step 3.

**What it does:**
- Argument checks for documented commands, starting with identifier arguments (ideas, events, decisions, technologies, sprites, localisation keys), built on the engine's symbol index.
- Value-type checks wherever the docs define the type.

**Done when:** a wrong id in any identifier argument is caught, and the zero-false-positive set stays clean.

### WP-8 Coverage expansion (problem 15, M-9, M-11)

**Owner:** decided when the earlier packages land, after the engine routes move, so new tools are built on Rust from the start.

**Order:**
1. Localisation in every language.
2. Ideas and characters.
3. on_actions, MIOs, balance of power, special projects, operations and raids.
4. Division templates, OOB and equipment, plus GFX and sound validation, with the cross-family sprite check shared with hoi4-imagegen-mcp.

Event, decision and technology writers arrive together with their visual editors, in stage-plan order. Each new area gets inspect, plus render and safe rewrite where they help, all with two-way-editing parity tests (FR-5).

**Done when:** each area has benchmark ground truth and meets the validation and not-found targets.

### WP-9 Cleanup

**Owner:** the accuracy session.

- Move the `chaosx.*` tools into a private plugin that the server loads only when it's present (M-14).
- Split the largest files when they are next touched: `map/operations.ts` (6,440 lines) and `gui/layout.ts` (3,851 lines).
- Fix the ADR index numbering.
- Release the engine's CHANGELOG entry.

### Related work tracked elsewhere

- **W3 (PR #53) and W7:** focus layout quality and the Rust focus solver. Owned by the research session.
- **W5:** hoi4-imagegen-mcp, being re-planned around asset families, style that adapts to each mod, required processing and the ChatGPT login route (D-12a).

## Order and parallel work

Three writers can work in hoi4-agent-tools at once, because their areas don't overlap:

- **ZIN worker:** WP-5 and the map fixes
- **Lookup session:** WP-4
- **Accuracy session:** WP-1, WP-2, WP-3, WP-6, WP-9, then WP-7

WP-8 starts once WP-5 has moved the index. Each package lands as small PRs with before and after numbers. No package is done until the harness shows it meeting its targets.

## Open questions for the maintainer

1. Start the accuracy session for WP-1, WP-2, WP-3, WP-6, WP-9 and WP-7?
2. Send problem 5 (the map false errors) to the ZIN MCP worker?
3. Are the starred target defaults right, or should any be stricter?
