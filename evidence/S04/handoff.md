# S04 stage handoff

Status: **complete on 2026-11-17 under maintainer decisions D-72 and D-79** (see the closing note at the end). The text below is the 2026-11-15 record. `validate-stage` was run on 2026-11-15 and its record is [checks.json](checks.json). Ten of twelve gates pass on the final revision. Gate 1 is unavailable and gate 10 is unresolved. Nothing here authorizes S05.

## Identity and authorized scope

- Stage ID and title: S04, the focus authoring vertical slice.
- User authorization reference and bounds: S04 authorized on 2026-11-08, run at low priority (D-36, D-46). Design decisions D-50, D-51, D-61 and D-63. Server additions D-64 and D-66.
- Implementing agent and client: Claude Code desktop sessions started by the HOI4 coordinator session, on the Windows PC.
- Reviewer and review state: the maintainer chose the page designs from images. No human review of the code.
- Repository and revision: `hoi4-factory`, branch `s04-finish` merged into `main`. The code revision checked is `e5bcb5b`.
- Dirty paths present before work: none in the worktrees this stage used.
- Earlier session records: [checkpoint of 2026-11-08](checkpoint-2026-11-08.md), [session handoff of 2026-11-13](handoff-2026-11-13-session.md), [session handoff of 2026-11-14](handoff-2026-11-14-session.md).

Authorized and built: the focus projection and operations, the agent's focus tools on the same path, the review and commit flow with restore and publication, the engine in the image, and the two browser pages. Later-stage features stay out: other editors, the desktop app and the pilot.

## Outcome and changed artifacts

A modder opens a task's focus tree in the browser and sees it as the game draws it. They ask the agent for a change in plain words, drag a focus, or change a title, description or cost. Every change is a proposal on the task's own branch. The agent's changes are approved or rejected, and the modder's own edits are saved or undone. The change card shows the final result of one proposal with one sentence and two actions.

| Gate | Intended result | Actual result | Evidence | Status |
|---|---|---|---|---|
| 1 | A new project and a GitHub import complete the slice through onboarding in the browser | Not run. It needs a real Setup analysis model key and the GitHub App on the test repository | none | unavailable |
| 2 | All three interaction modes produce native files through the same contracts | Gesture, native agent and coding-client paths give identical bytes in the server test. In the browser the gesture and the agent paths were walked on a real stack | `gesture_agent_and_coding_client_paths_give_identical_bytes`, [browser pages](browser-pages-2026-11-14.md), [live ask bar](live-ask-bar/README.md) | pass for the server paths, browser walk without the coding client |
| 3 | Supported edits keep untouched bytes, and unsupported or stale writes are refused | Covered for move, cost, link, create and text edits | `crates/server/tests/focus.rs` | pass |
| 4 | Review links source, preview and validation hashes, and no render is called a game check | The change card binds Approve to the proposal hash. The diff, checks, limits and restore point are behind Details. Its limits say the picture is a source render | `tests/focus.rs`, `web/src/test/focus.test.tsx`, [browser pages](browser-pages-2026-11-14.md) | pass |
| 5 | Commit, push, pull request and restore, with no duplicate on a lost reply | Covered against the stand-in GitHub | `tests/focus.rs`, `tests/publish.rs` | pass |
| 6 | Download, edit outside, push back and reopen with outside changes detected | Covered against the stand-in GitHub | `tests/publish.rs` | pass |
| 7 | Preview and interaction times recorded against the budgets | Warm preview p95 131 ms. On 4.18.0 an edit takes 9 to 10 s to check and about 3 s to redraw in the browser, against 30 to 50 s on 4.12.0. The page draws the change at once | [measure](measure/measure-2026-11-08.json), [times on 4.18.0](live-ask-bar-4.18.0/README.md) | pass, recorded |
| 8 | The server links the engine, and every agent commit passed the commit check | Covered. Imports have a preservation report and a baseline | `tests/focus.rs`, `tests/publish.rs`, [t-eng-01 on 4.18.0](t-eng-01-4.18.0/README.md) | pass |
| 9 | Gesture and agent edits share one operation. An agent edit appears in an open editor without reload | Parity in the server test. In the browser the agent's new focus appeared in the open editor while the attempt ran, on both pins | `tests/focus.rs`, [live ask bar](live-ask-bar/README.md), [on 4.18.0](live-ask-bar-4.18.0/README.md) | pass |
| 10 | No regression from the baseline, and the round trip measured against Hagane Studio | Hagane round trip measured. T-BENCH-01 has no run on the pin. The nightly of 2026-11-15 measured Agent Tools `a6292e75`, 29 commits after 4.18.0: 47 improvements, 15 latency regressions and one correctness case that newly fails on ZIN | [hagane-round-trip](hagane-round-trip/README.md), [checks.json](checks.json) | unresolved |
| 11 | The native loop and the coding-client profile name the same tools release and engine | Re-pinned to Agent Tools 4.18.0. The pin agreement test, the real Agent Tools tests and T-ENG-01 pass | [checks.json](checks.json), [t-eng-01 on 4.18.0](t-eng-01-4.18.0/README.md) | pass |
| 12 | A Q-1 baseline stored with pins and hardware | Stored on 2026-11-06, hash rechecked | [q1-baseline](q1-baseline/README.md) | pass |

## Verification record

On the final tree, on HOI4 Agent Tools 4.18.0:

- Linux workspace run with PostgreSQL and the real Agent Tools: 667 passed, 1 failed and 26 ignored in 77 test binaries. The one failure was a stale tool count in the real Agent Tools test. It was fixed and that crate rerun: 36 of 36.
- Server suites inside that run: agent 22, focus 10, HTTP 12, publish 3, setup gates 13.
- `scripts/t-eng-01.sh` against the server image: eight checks ok.
- Client check in the pinned Node image: 106 tests and the production build pass.
- Browser run on a throwaway stack: three edits by hand, two saves and one ask-bar request with `deepseek-flash`.

The first workspace run was not valid, because the test database was killed for memory. It is described in [checks.json](checks.json).

Could not run, with the reason:

- Gate 1: no real Setup analysis key in the throwaway stack, and the GitHub App is installed only for the maintainer's use.
- T-BENCH-01 on the exact pin: the nightly benchmark is owned by the Agent Tools session and runs on its `main`.
- Mouse dragging against the real server: the arrow keys and the lab were used. The drag sends the same operation.
- Anything in the game: every picture is a source render.

## Decisions and unresolved issues

Technical defaults taken under D-18 are listed in the two session handoffs. Added on 2026-11-14 and 2026-11-15:

- The list of trees inspects each focus file of the mod on its own, because one inspect of the whole workspace also laid out every tree of the game.
- An edit is drawn at once and blocks further edits until the server answers. A refused edit puts the tree and the card back and says why.
- The pages show only the first sentence of the agent's summary. The agent is asked to begin with one plain sentence.
- The tool process is started with every Agent Tools profile on, and the host's own profile decides what the model sees.
- The Title field saves on Enter as well as on leaving the field.

Open:

- Gate 1, which waits for the maintainer.
- Gate 10: someone has to decide whether the nightly's latency regressions and the new ZIN false positive are real, with a benchmark run on the pin or on a quiet machine. The Agent Tools session owns the benchmark.
- The focus editor profile offers the model the game-view render, whose description says host only. It cannot write. Either the description or the offer should change.
- An edit still takes 9 to 10 seconds to check. Nearly all of it is one rewrite call in Agent Tools.

## Effects, recovery and rollback

Live model spend in this stage: about US$0.006 on 2026-11-08, US$0.0096 on 2026-11-14 and US$0.0060 on 2026-11-15, all on the maintainer's DeepSeek key. No GitHub writes outside the stand-in and the test repository. No migration was added after 2026-11-08, so reverting the merge commits restores the earlier client and server. Reverting commit `db01c97` alone restores the 4.12.0 pin.

## Configuration checkpoint

The Agent Tools pin moved from 4.12.0 to 4.18.0, with the engine at revision `fb78624c7c76c993268c33fe1c50a62632547b98`. The tool process is started with `HOI4_AGENT_TOOLS_PROFILES=all`. Five new tools are withheld from the model with a reason. No provider, permission or MCP connection changed. The dev stack's server image has to be rebuilt to get the new pin (`scripts/compose.sh build server`).

## Engineering learning review

Recorded in the knowledge changelog: 0.2.45 (cancel-safe worker writes), 0.2.46 (tests under a memory cap), 0.2.47 (Docker Desktop stays up, the WSL fallback for the client check) and 0.2.48 (scoped tool calls, bounded model text, every tool profile on, memory for the test database). The stories are in the engineering lessons.

## Next stage proposal and resume instructions

- Proposed next stage: S05, as a proposal only. It needs the maintainer's authorization, and S04 still has two open gates.
- Still to do in S04: gate 1 when the maintainer can provide what it needs, and the T-BENCH-01 half of gate 10.
- Read first: this file, [checks.json](checks.json), the three session records above and [the checks of the pages](browser-pages-2026-11-14.md).
- Pending operations needing reconciliation: none.

## Closing note, 2026-11-17

- Gate 1 is accepted as unavailable by maintainer decision D-72. It needs a real model key for Setup's analysis step and the GitHub App on the test repository, and it runs in a later stage once those exist.
- The benchmark half of gate 10 (T-BENCH-01) closes on the clean hand run of the full nightly suite on 2026-11-16 at 08:53, by maintainer decision D-79. The run used the same command and engine as the scheduled task on Agent Tools main `4c4e77cb`, not the 4.18.0 pin. Every case passed, and it was accepted as the benchmark baseline. The two nightly runs of 2026-11-16 and 2026-11-17 were 15 to 30 times slower on the same cases with every result correct, and the load log of 2026-11-17 showed no other work, so the slowness belongs to the scheduled task's context. Since D-79 the benchmark runs on demand and the nightly task is off.
- With both, S04 is complete. This does not authorize S05, which needs the maintainer's authorization.
