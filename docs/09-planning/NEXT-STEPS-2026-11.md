# Next steps, November 2026

Written on 2026-11-08 after a research and review session with the maintainer across every HOI4 project: HOI4 Factory, HOI4 Agent Tools, the Agentic HOI4 Modding kit, HOI4 Mod Setup, HOI4 Live Tools, hoi4-imagegen, the 3D route, Chaos Redux, ZIN, and how the work itself is run. The maintainer answered five rounds of questions. The answers are D-33 to D-49 in [decisions in force](../decisions/README.md). This plan orders the work. It authorizes nothing beyond those decisions and the stages already authorized.

## The next weeks in short

| Period | Main effort | Also running | Paused |
|---|---|---|---|
| Now to about 2026-11-22 | Tools for other modders, three goals at once (D-36): a quick install and first run, more Agent Tools coverage, better hoi4-imagegen images | S04 with one low-priority session. HOI4 Live Tools in-game testing after the three goals. A read-only scan of rival tools (D-41) | 3D production (D-37). New mod content (D-45) |
| From about 2026-11-22 | Restarting the 3D route (D-46) | S04 with one session until the maintainer changes it. Tool fixes and the weekly release rhythm (D-40) | Nothing new |
| When the three tool goals land | Promotion of the whole modder workflow (D-43), prepared with `prepare-promotion-release` | The macOS checks prompt, when the maintainer is at the Mac (D-47) | |

## Priorities per project

### HOI4 Mod Setup and the kit: a quick install and first run

The goal: a modder installs through Setup and has a working agent with every tool within minutes.

1. Merge the green 0.9.4 release pull request and publish it.
2. Turn the goal into one repeatable fresh-install test that runs on Windows and in Linux CI from a clean machine. It installs a new test mod with the kit and every MCP server Setup offers, then checks that each server starts and lists its tools, and records the time taken and every manual step.
3. Remove manual steps the test finds, and write one short first-run page that matches what the test does.
4. The kit has commits since its last package release. From now on it pins the week's tool releases once and Setup releases once after the fresh-install test (D-40). Pin Agent Tools 4.12.0 once it is out.
5. Make the Agent Tools bootstrap find Node on Linux and macOS, which the kit's own docs list as a gap.

Done when the fresh-install test passes on Windows and Linux, and the macOS results exist or the gap is named in the docs.

### HOI4 Agent Tools: more coverage

1. Release 4.12.0 through the release owner (M-16).
2. Rebase pull request 128 (the R-5 health check and explain-error tools), which conflicts with main and has no CI run, then merge it on green. Merge the green docs pack pull request.
3. Finish the R-5 modder features in order: health check and explain-error, the localisation writer and fixer with the trigger and effect guard, then the compatibility check and the patch report. Finish the R-4 profiles: a core profile of about 12 tools, map and advanced profiles, and the docs pack with automatic mod selection.
4. Continue M-8: `source_lookup` and the closure option of `script_validate` on the engine. Record fresh M-6 baselines for every benchmark mod and re-enable the nightly benchmark (S04-4).
5. Tidy the issue list: issues 66 and 71 look mostly done by merged pull requests, and issue 69 overlaps issue 107. Close, narrow or link them with evidence.
6. Use the rival tool scan (D-41) to adjust the order of the remaining coverage work.

### hoi4-imagegen: better images

1. Family fixes (agency and doctrine colours), model routing under W5-10 with a probe of what the sign-in route returns, palette variety across every family, and the TFR style packs.
2. Keep the `model_reference` family general and lean, and put specific review points into how agents review their results.
3. Land the open docs pull request. Close the macOS release-blocker issue if macOS CI passes on main, since a release already shipped after it.
4. Respect the sign-in concurrency rule (W5-8) and any temporary one-at-a-time period after a revocation.

### HOI4 Live Tools: after the three goals

1. Tonight's release acceptance under D-30, then the release.
2. From the night of 2026-11-09 on, in-game checks use the standing window (D-33).
3. Then the R-5 smoke tests for events and decisions, the missing-key and GUI sweep, and the trigger and focus availability check, shipped through Setup.

### HOI4 Factory: S04 at low priority

1. One S04 session continues: the engine binary in the image, the S03 leftover worktree, live checks on DeepSeek only (D-21). It never holds up a tool release.
2. Re-pin the Factory image once, after the last tool release of the stage (S04 gate 11).
3. Deferred to before S08: the game-rights question (D-38), queue priority and spend alerts (D-44).
4. Issue hygiene: issues 2 and 12 overlap on the OpenAI-compatible adapter, and 5 and 6 overlap on Agent Tools on the server. Merge or link them when S04 touches that code.

### 3D route: restart from about 2026-11-22

Nothing moves before the restart, apart from the Tripo plan approval the maintainer is waiting for.

1. Finish the bake-off: the Tripo test once its plan is approved, and the free SkinTokens and UniMate routes.
2. Assess every open lesson in the shared lessons log (53 of 114 on 2026-11-08): build it into the shared Blender stage or decline it with a reason (D-37, 3D-12).
3. Update the kit route, then make new units through tools and presets (3D-11), reviewed under 3D-14.
4. Redo the restored Chaos Redux units (3D-15), then draft a new trailer for the maintainer (D-49).
5. Open items that wait for the restart: the mechanized assassin's fused wheels and the gorilla's in-game texture.

### Chaos Redux and ZIN: test beds

1. Until about 2026-11-22 the mods serve the tool push as benchmarks, dogfood and fixtures. Agents start no new mod content (D-45).
2. ZIN: the static-model check under D-29 finishes by 2026-11-09. The ZIN lead and the maintainer's own workers continue as the maintainer directs.
3. Chaos Redux: the 3D restore is done. The Discord bot migration continues. Issue 7 (mip levels in `convert_to_dds.py`) waits for a session that touches the asset pipeline.

## How the work is run

| Topic | Rule now | Where |
|---|---|---|
| Game launches | One standing rule: Windows any night 00:00 to 06:00, the WSL copy any time, one runner, mod rules first | D-33, CLAUDE.md |
| Sessions | No cap. Archive finished sessions at once, one heavy job at a time | D-31, D-34, CLAUDE.md |
| Usage | No new work session above about 80% of the weekly limit | D-39, CLAUDE.md, `coordinate-sessions` |
| Check-ins | Hourly while sessions run, every three hours when all are idle | D-48, `coordinate-sessions` |
| Releases | Feature releases at most weekly per tool, fixes any time, one kit pin and one Setup release per week | D-40, CLAUDE.md |
| Promotion | Latest releases, clean public text, a video draft, then the maintainer promotes | D-40, D-43, D-49, `prepare-promotion-release` |
| macOS | One prompt the maintainer runs on the Mac | D-47, [GOAL-MACOS-CHECKS](../06-agent-handoffs/GOAL-MACOS-CHECKS.md) |
| Decisions | Per project files with in-force lists | D-35, [decisions in force](../decisions/README.md) |

Work still to do on how the work is run:

1. **Slim CLAUDE.md (D-42).** One guidance session moves dated incident stories and one-time history into a linked lessons file under `docs/07-developer-workflow/`, keeps every rule as a short line, and proves with a diff audit that no rule was lost. It runs when no other session is editing CLAUDE.md.
2. **Rival tool scan (D-41).** One read-only research session, its report saved under `docs/08-expansion-research/research/`.
3. **Running log.** `.work/overnight-summary.md` has grown past 80 KB. As a technical default the coordinator moves entries older than a week into a dated file under `.work/logs/` at the weekly cleanup.
4. **Local checkout.** The main `hoi4-factory` checkout fell four commits behind `origin/main` during the day. Sessions that read guidance from it should pull first, or work in their own worktree from `origin/main`.

## Waiting on the maintainer

None of these block the plan. They are listed so nothing is forgotten.

- The Tripo plan approval, then the Tripo downloads for the bake-off.
- Running the macOS checks prompt when at the Mac.
- Optional system housekeeping on the Windows PC: a reboot to shrink the page file, and turning hibernation off.
- A funded OpenAI key, only when the OpenAI profile itself must be checked live (D-21).

## Open questions left

These were not decided in the review and need no answer now.

1. The pace of S04 after about 2026-11-22. D-46 keeps one session until the maintainer changes it.
2. Whether HOI4 Live Tools joins the whole-workflow promotion or follows later.
3. Whether HOI4 Factory should run on the Mac again. The macOS prompt checks its scripts and tests only.
4. The Hagane Studio wind-down (W8) stays parked.

The review also found older entries that read as conflicts. They were settled as clarifications in the decision files, not as new decisions: the image model that W5-10 names and the outer model that D-12b fixes are different layers, the kit's wiki push approval of D-08 gave way to D-23, D-21's Meshy line predates the Meshy Pro plan, 3D-12 widened 3D-10 from Meshy alone to Meshy and Tripo, and S04-3's Windows checks stand without the Mac move that D-25 cancelled.

## How we decide and record

- **Who decides.** Agents make technical decisions themselves, prefer reversible options and record them (D-18). The maintainer decides scope, priorities, money, credentials, access and in-game checks.
- **How to ask.** Ask every foreseeable question up front, in batches of at most four, each with two to four concrete options and the recommendation first. Use earlier answers to shape the next round. Never ask before a merge or a push.
- **Where to record.** In the project's file under [decisions in force](../decisions/README.md), with the next global D- number, the date, the question and the decision as a complete statement of intent in the recorder's own words. Add the one-line in-force row, and update or retire the rows it replaces.
- **What stays out.** The maintainer's own ratings and review verdicts, private repository material (D-01), personal preferences that are not project rules, and private links such as the status page.
- **Who hears about it.** A decision that changes how running sessions work goes to the coordinator at once, and the coordinator passes it on. A decision that changes how agents work also updates CLAUDE.md or a skill in the same change.
