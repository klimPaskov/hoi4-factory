# Agent Tools decisions

hoi4-agent-tools (the MCP server) and the shared Rust HOI4 engine inside it: engine moves, tool coverage, accuracy, benchmarks, packaging and release handling. New decisions for this project are added under Decisions. Older entries live in the history records linked from each row.

## In force

| ID | Since | In short | Full entry |
|---|---|---|---|
| **Engine and Rust move** | | | |
| D-07 | 2026-10-31 | One shared Rust HOI4 engine (parser, symbol index, reference closure, lints) is built first. Tools move onto it one at a time behind the same MCP names. New backend code is Rust where practical. | [expansion record](../08-expansion-research/00-decision-record.md) |
| C1 | 2026-10-31 | The engine is a Cargo workspace inside hoi4-agent-tools under Apache-2.0. MCP tool names stay stable. | [expansion record](../08-expansion-research/00-decision-record.md) |
| M-8 | 2026-11-01 | Move order: engine binaries, then mod_index and source_lookup, then the whole-mod report in script_validate, then map hot paths and analyzers, and finally retire the TypeScript index. | [expansion record](../08-expansion-research/00-decision-record.md) |
| C2 | 2026-10-31 | CWTools MD Edition is adopted as a validation layer only if a bounded spike shows it measurably improves validation. It is never the write path. | [expansion record](../08-expansion-research/00-decision-record.md) |
| P1 | 2026-10-31 | Whole-mod reference closure marks every reference resolved, unresolved or unknown. Unknown is never a pass. Built in the Rust engine. | [expansion record](../08-expansion-research/00-decision-record.md) |
| P2 | 2026-10-31 | Lint vocabulary is generated from the installed game's documentation, plus a set of specific event and modifier lints. Built in the Rust engine. | [expansion record](../08-expansion-research/00-decision-record.md) |
| P3 | 2026-10-31 | Override and load-order audit covering removed definitions, emptied replace_path directories and exact shadow filenames. Built in the Rust engine. | [expansion record](../08-expansion-research/00-decision-record.md) |
| **Coverage and features** | | | |
| M-9 | 2026-11-01 | Cover as much of HOI4 modding as possible, starting with localisation in every language, then ideas and characters, on_actions and the other listed systems. | [expansion record](../08-expansion-research/00-decision-record.md) |
| M-11 | 2026-11-01 | Event, decision and technology writers arrive together with their visual editors, in stage-plan order. The same operations serve agents and two-way editing. | [expansion record](../08-expansion-research/00-decision-record.md) |
| M-3 | 2026-11-01 | hoi4.error_log groups errors repeated across launches into signatures, counts them and links documentation and approved rules. | [expansion record](../08-expansion-research/00-decision-record.md) |
| R-5 | 2026-11-06 | All four feature groups come first: mod health check and explain-error, localisation writer and trigger guard, compatibility and patch-impact checks, and Live Tools smoke tests (see live-tools). | [expansion record](../08-expansion-research/00-decision-record.md) |
| P-1 | 2026-11-02 | Next improvements run in parallel: game preview speed and rendering, rules lookup quality, missing tools for localisation, ideas, characters, units and assets, and faster cold lookups. | [expansion record](../08-expansion-research/00-decision-record.md) |
| FR-3 | 2026-10-31 | Improve focus-tree quality: separate structural, visual and design quality, add diagnostics, scoring, comparison and partial layout, and test on real trees. No separate focus MCP. | [expansion record](../08-expansion-research/00-decision-record.md) |
| D-10 | 2026-10-31 | The supplied Soviet focus-tree screenshot has no known source. The FR-3 investigation works from the screenshot alone. | [expansion record](../08-expansion-research/00-decision-record.md) |
| W7-2 | 2026-11-02 | Orthogonal connector metrics are confirmed for the default UI scale and zoom after PR #84. Other scales, T-junctions and several connectors into one child stay unverified. Replaces W7-1. | [expansion record](../08-expansion-research/00-decision-record.md) |
| **Skills, rules and documentation** | | | |
| M-1 | 2026-11-01 | The MCP serves the kit's skills through a hoi4.skill tool and as resources, at the kit version pinned for the mod, with a hash. A check fails on drift. | [expansion record](../08-expansion-research/00-decision-record.md) |
| M-2 | 2026-11-01 | Approved rules use one format at kit level and mod level. Agents propose and a rule takes effect only after maintainer approval, with reason, evidence and regression test. LESSONS.md is folded in. | [expansion record](../08-expansion-research/00-decision-record.md) |
| M-5 | 2026-11-01 | Tool documentation is generated from tool definitions. CI fails on drift, duplicate names or an undocumented tool. | [expansion record](../08-expansion-research/00-decision-record.md) |
| M-10 | 2026-11-01 | Lookup vocabulary comes mostly from the game's own localisation. An optional local embedding model is kept only if it clearly wins on an untouched blind test set. | [expansion record](../08-expansion-research/00-decision-record.md) |
| M-4 | 2026-11-01 | No redundant safety checks such as an edit-size limit. The existing drift guards stay. | [expansion record](../08-expansion-research/00-decision-record.md) |
| **Packaging and context budget** | | | |
| R-1 (tool list) | 2026-11-06 | The hoi4-agent-tools tool list stays within about 8,000 tokens, enforced by CI. | [expansion record](../08-expansion-research/00-decision-record.md) |
| R-4 | 2026-11-06 | Packaged as profiles: a core profile of about 12 tools plus optional map and advanced profiles. A downloadable docs pack lets doc search work with no mod open. A single configured mod is auto-selected. | [expansion record](../08-expansion-research/00-decision-record.md) |
| **Quality bar and benchmarks** | | | |
| Q-1, Q-1a | 2026-11-01 | Decisive lead over comparable tools with absolute targets, proven by CI benchmarks. See factory. | [expansion record](../08-expansion-research/00-decision-record.md) |
| R-11 | 2026-11-06 | Beating Hagane and HOI4-MCP on the Q-1 benchmark stays a release gate. | [expansion record](../08-expansion-research/00-decision-record.md) |
| M-6 | 2026-11-01 | Benchmark mods are vanilla, Chaos Redux, ZIN, Ireland Total Overhaul and two or three large public Workshop mods at pinned versions, run locally. | [expansion record](../08-expansion-research/00-decision-record.md) |
| M-7 | 2026-11-01 | cpntodd/HOI4-MCP is not a base. One head-to-head run in an isolated container is the Q-1 proof against it. | [expansion record](../08-expansion-research/00-decision-record.md) |
| M-12, S04-4 | 2026-11-06 | A nightly real-game benchmark runs on a maintainer machine with the game, as a Windows scheduled task, with results kept local with hashes. The accuracy session registers it after a fresh baseline. | [expansion record](../08-expansion-research/00-decision-record.md) |
| M-13 | 2026-11-01 | A macOS arm64 job joins the hoi4-agent-tools Node CI. | [expansion record](../08-expansion-research/00-decision-record.md) |
| **Private plugin, releases and sessions** | | | |
| M-14, S04-1 | 2026-11-06 | The chaosx tools move to a private plugin in the private chaos-redux-vault repository. The public package has no chaosx tools and loads the plugin only from a configured path. | [expansion record](../08-expansion-research/00-decision-record.md) |
| S04-2 (tool parts) | 2026-11-06 | The hoi4.skill tool (M-1), tool-doc drift check (M-5) and private plugin (M-14) finish during S04 and do not gate its start. | [expansion record](../08-expansion-research/00-decision-record.md) |
| M-15 | 2026-11-01 | No licensing action is needed for HOI4-MCP because nothing of it is used. | [expansion record](../08-expansion-research/00-decision-record.md) |
| M-16 | 2026-11-01 | One release owner owns versions, CHANGELOG release sections and tags. Others write only under Unreleased. Rust builds share the main checkout's engine/target folder. | [expansion record](../08-expansion-research/00-decision-record.md) |
| D-68 | 2026-11-14 | Releases bundle the engine for four platforms: Windows x64, Linux x64, Linux ARM64 and Apple silicon. Intel Macs get no bundled engine for now, and a clear message with build steps. | [below](#decisions) |
| D-71 | 2026-11-15 | Focus edits aim for 1 s with two tool changes: a game-build manifest so a call skips the file walk, and an answer sent once the safe write is done, with the after-edit check result following. | [below](#decisions) |
| D-79 | 2026-11-17 | The Agent Tools benchmark runs on demand, not nightly: the release owner runs it by hand in a quiet slot before each release and on pull requests that touch speed. The nightly scheduled task is turned off. | [below](#decisions) |
| D-80 | 2026-11-17 | The Agent Tools audit gate stays strict for what users install. The full audit may pass only advisories on a short reviewed list in the repository, which starts with one dev-only advisory that has no fix yet. | [below](#decisions) |
| FR-5 | 2026-11-01 | The read and render tools double as a visual editor through one operation layer. See factory. | [expansion record](../08-expansion-research/00-decision-record.md) |
| D-36 | 2026-11-08 | Until about 2026-11-22 tools for other modders come first. Agent Tools gains more coverage: the R-5 modder features and the R-4 profiles. | [operations](operations.md#decisions) |
| D-40 | 2026-11-08 | Feature releases at most about weekly, fixes any time. A promotion first releases the latest version and cleans the public text. | [operations](operations.md#decisions) |
| D-41 | 2026-11-08 | A fresh read-only scan of HOI4 AI and modding tools feeds Agent Tools priorities and the Q-1 comparison set. | [operations](operations.md#decisions) |
| D-98 | 2026-11-21 | The under-one-second edit plus inspect target (issue 137) is reached by letting the inspect reuse the index the rewrite just built. Writes stay durable. | [below](#decisions) |

## Decisions

New entries go here in full, oldest first.

| ID | Date | Question | Decision |
|---|---|---|---|
| D-68 | 2026-11-14 | Should releases also build the engine for Intel Macs, now that the tools need it? | Not now. Releases keep bundling the engine for Windows x64, Linux x64, Linux ARM64 and Apple silicon. On an Intel Mac a tool call fails with a clear message that names the supported platforms and how to build the engine from source. The question can be reopened if modders ask for it. Maintainer decision |
| D-71 | 2026-11-15 | Which tool-side changes go after the 1 s focus edit target (issue 137)? | Both. A manifest of the game build lets a call skip walking the game files. A focus edit answers as soon as its safe write is done, and the result of the check after the edit follows separately, so a failing check can still undo or flag the edit. Maintainer decision |
| D-79 | 2026-11-17 | Why does the benchmark run nightly, and should it? | It ran nightly because the PC used to be quiet at night. Agents now work through the night, and two nightly runs were 15 to 30 times slower than the same suite run by hand, which was clean. So the benchmark runs on demand: the release owner runs it by hand in a quiet slot, with the heavy-job slot from the coordinator, before every Agent Tools release and on pull requests that touch speed. The nightly scheduled task is turned off. The benchmark half of S04 gate 10 closes on the clean hand run of 2026-11-16 08:53 on main `4c4e77cb`, which replaces the nightly condition in D-72. Maintainer decision |
| D-80 | 2026-11-17 | Every Agent Tools check fails on an advisory with no fixed version (GHSA-fc5q-39xc-vjpf in picomatch, reached only through build and test tools). How does the audit gate handle that? | The audit of the installed package (`npm audit --omit=dev --audit-level=high`) stays strict, with no exceptions. The full audit still fails on any high or critical advisory, except those on a short reviewed list kept in the repository, where each entry names the advisory, the package, the reason and a review date. The list starts with this one advisory, which is dev-only, and the entry stops counting once a fixed picomatch is published. Waiting for an upstream fix with no merges and no release was the alternative and is not chosen. Maintainer decision |
| D-98 | 2026-11-21 | Issue 137 offers two ways to get an edit plus inspect under one second: weaken the durable write, or let the inspect reuse the index the rewrite built. Which one? | The inspect reuses the index. The durable write is not weakened, so a crash or power loss still cannot lose a finished write. Maintainer decision |

## Replaced or expired

| ID | Status | By or until |
|---|---|---|
| W7-1 | Replaced | W7-2 |
