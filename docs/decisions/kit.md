# Kit decisions

The Agentic HOI4 Modding kit: its skills, canonical instructions, wiki snapshot, context budgets and generated projections. New decisions for this project are added under Decisions. Older entries live in the history records linked from each row.

## In force

| ID | Since | In short | Full entry |
|---|---|---|---|
| **Questions, investigation and planning** | | | |
| FR-1 | 2026-10-31 | Inspect first, then ask once in an organized initial consultation with suggested answers, record the answers in specs and continue without avoidable interruptions. | [expansion record](../08-expansion-research/00-decision-record.md) |
| P5 | 2026-10-31 | A small kit skill hoi4-investigate holds the static bug protocol: root cause before any edit, tripwire phrases, an evidence ladder and no edits only to silence a validator. | [expansion record](../08-expansion-research/00-decision-record.md) |
| P6 | 2026-10-31 | Log reading order, first-error cascade rule and symptom table live in the P5 skill. They are used only when a log is already available, and the rule against asking for logs stays. | [expansion record](../08-expansion-research/00-decision-record.md) |
| P14 | 2026-10-31 | The kit's hoi4-feature-planning checks feasibility before a spec commits to a mechanic. | [expansion record](../08-expansion-research/00-decision-record.md) |
| **Knowledge and generated copies** | | | |
| FR-4 | 2026-10-31 | Audit and automate the sync of canonical instructions, skills and agents so generated projections can never go silently stale, with CI checks and dependency-aware runs. | [expansion record](../08-expansion-research/00-decision-record.md) |
| P8 | 2026-10-31 | Import Hagane traps that are confirmed on the game and missing from the kit, reworded and evidence-labelled. Contradicted claims are excluded. | [expansion record](../08-expansion-research/00-decision-record.md) |
| P9, M-2 | 2026-10-31 | Every rule carries an evidence tag and a LESSONS.md incident ledger ties each rule to its incident. M-2 folds the ledger into the approved-rules format. | [expansion record](../08-expansion-research/00-decision-record.md) |
| P10 | 2026-10-31 | General rules are written once in the kit. Chaos Redux engine-system skills are generated from kit text plus a Chaos Redux section. Events and event assets stay Chaos Redux's own. Decided in round 4. | [expansion record](../08-expansion-research/00-decision-record.md) |
| W0-1 | 2026-11-02 | Chaos Redux warns when its wiki snapshot is behind the kit's and names the refresh command. See mods. | [expansion record](../08-expansion-research/00-decision-record.md) |
| D-08 | 2026-10-31 | A separate worker reviews and completes the offline Paradox wiki snapshot against the current game, with local improvements surviving syncs. Its push approval step is replaced by D-23, so finished work merges on green checks. | [expansion record](../08-expansion-research/00-decision-record.md) |
| **Context budgets and skill set** | | | |
| P11 | 2026-10-31 | Slim the always-loaded template and the largest skills, but every instruction stays. Text is moved or deduplicated, never dropped, and a diff audit proves it. | [expansion record](../08-expansion-research/00-decision-record.md) |
| R-1 (kit) | 2026-11-06 | CI-enforced ceilings: agent template about 4,000 tokens, each skill's main file about 5,000, and one sentence per skill description. | [expansion record](../08-expansion-research/00-decision-record.md) |
| R-9 | 2026-11-06 | Every kit install keeps installing all skills and the art shelf. Nothing becomes optional. | [expansion record](../08-expansion-research/00-decision-record.md) |
| R-10 | 2026-11-06 | Four new lean skills within the R-1 ceiling: localisation, on_actions, ideologies, and scripted triggers and effects. | [expansion record](../08-expansion-research/00-decision-record.md) |
| **Rejected ideas** | | | |
| R1 | 2026-10-31 | No auto-update of skills with git pull at session start. Pinned sync with checks stays. | [expansion record](../08-expansion-research/00-decision-record.md) |
| R2 | 2026-10-31 | No platform-owned IDs with automatic prefixes. Native files keep the author's IDs. | [expansion record](../08-expansion-research/00-decision-record.md) |
| R3 | 2026-10-31 | No hidden startup-event overlays to modify vanilla characters and leaders. Use direct edits or shadow files, audited by P3. | [expansion record](../08-expansion-research/00-decision-record.md) |
| R4 | 2026-10-31 | Hagane's field reference is not used as the HOI4 reference because it has contradicted claims. | [expansion record](../08-expansion-research/00-decision-record.md) |
| **Agents, art and other projects** | | | |
| D-23 (kit part) | 2026-11-06 | The kit's general modding rules for users stay as they are, even though agents themselves merge and push freely. | [expansion record](../08-expansion-research/00-decision-record.md) |
| D3 | 2026-10-31 | Agents may call a ComfyUI endpoint the maintainer configured, for any asset including portraits of real people. This changes the kit's rule that agents never operate RunPod. See imagegen. | [expansion record](../08-expansion-research/00-decision-record.md) |
| W5-2 (kit part) | 2026-11-01 | The old gptimage paths are removed from the kit once hoi4-imagegen replaces them. See imagegen. | [expansion record](../08-expansion-research/00-decision-record.md) |
| M-1, FR-3 (see agent-tools) | 2026-11-01 | The MCP serves the kit's skills from the pinned kit version, and the kit focus skill follows the FR-3 workflow. See agent-tools. | [expansion record](../08-expansion-research/00-decision-record.md) |
| D-36 | 2026-11-08 | Until about 2026-11-22 tools for other modders come first. The kit is part of the quick install and first run goal that Setup delivers. | [operations](operations.md#decisions) |
| D-40 | 2026-11-08 | Feature releases at most about weekly, fixes any time. A promotion first releases the latest version and cleans the public text. | [operations](operations.md#decisions) |
| D-43 | 2026-11-08 | The next promotion is the whole modder workflow: Setup, the kit and every tool it installs. | [operations](operations.md#decisions) |

## Decisions

No new entries yet.

## Replaced or expired

| ID | Status | By or until |
|---|---|---|
| P10 (round 3) | Replaced | P10 (round 4) |
| D-28 (kit parts) | Expired | Deadline 2026-11-07 passed |
