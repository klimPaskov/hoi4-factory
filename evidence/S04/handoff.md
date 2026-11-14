# S04 stage handoff

Status: **draft, not final**. The rows marked "to run" wait for the heavy-job slot on the Windows PC. `validate-stage` has not been run on this draft. Nothing here authorizes S05.

## Identity and authorized scope

- Stage ID and title: S04, the focus authoring vertical slice.
- User authorization reference and bounds: S04 authorized on 2026-11-08, run at low priority (D-36, D-46). Design decisions D-50, D-51, D-61 and D-63. Server additions D-64 and D-66.
- Implementing agent and client: Claude Code desktop sessions started by the HOI4 coordinator session, on the Windows PC.
- Reviewer and review state: the maintainer chose the page designs from images. No human review of the code.
- Repository and revision: `hoi4-factory`. The stage's work up to 2026-11-14 is on `main` at `8fd02b8`. The work after that is on branch `s04-finish` and is merged when its checks have run.
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
| 7 | Preview and interaction times recorded against the budgets | Warm preview p95 131 ms. An edit took 20 to 93 s on the tools of 2026-11-08 and 30 to 50 s in the browser on 4.12.0. On 4.18.0: to measure | [measure](measure/measure-2026-11-08.json), [browser pages](browser-pages-2026-11-14.md) | to run on 4.18.0 |
| 8 | The server links the engine, and every agent commit passed the commit check | Covered. Imports have a preservation report and a baseline | `tests/focus.rs`, `tests/publish.rs`, [t-eng-01](t-eng-01/run.log) | pass |
| 9 | Gesture and agent edits share one operation. An agent edit appears in an open editor without reload | Parity in the server test. In the browser the agent's new focus appeared in the open editor while the attempt ran | `tests/focus.rs`, [live ask bar](live-ask-bar/README.md) | pass |
| 10 | No regression from the baseline, and the round trip measured against Hagane Studio | Hagane round trip measured. T-BENCH-01 for the Factory pin: to run after the re-pin | [hagane-round-trip](hagane-round-trip/README.md) | to run |
| 11 | The native loop and the coding-client profile name the same tools release and engine | Prepared for Agent Tools 4.18.0 on branch `s04-finish`. Not built or tested yet | commit `db01c97` | to run |
| 12 | A Q-1 baseline stored with pins and hardware | Stored on 2026-11-06 | [q1-baseline](q1-baseline/README.md) | pass |

## Verification record

Run so far for the pages and the server additions, on the pinned Agent Tools 4.12.0:

- Client check in the pinned Node image: 105 tests and the production build pass.
- Linux server suites with PostgreSQL and the real Agent Tools: agent 22 of 22, focus 10 of 10, HTTP 12, intake 20, publish 3, setup gates 13. The focus suite was rerun after the list route changed. The full run on the final tree: to run.
- Browser runs on throwaway stacks built from the branch: the pages with person edits, and the ask bar with `deepseek-flash`.

Could not run, with the reason:

- Gate 1: no real Setup analysis key in the throwaway stack, and the GitHub App is installed only for the maintainer's use.
- Mouse dragging against the real server: the arrow keys and the lab were used. The drag sends the same operation.
- Anything in the game: every picture is a source render.

## Decisions and unresolved issues

Technical defaults taken under D-18 are listed in the two session handoffs. Added on 2026-11-14 and 2026-11-15:

- The list of trees inspects each focus file of the mod on its own, because one inspect of the whole workspace also laid out every tree of the game.
- An edit is drawn at once and blocks further edits until the server answers. A refused edit puts the tree and the card back and says why.
- The pages show only the first sentence of the agent's summary. The agent is asked to begin with one plain sentence.

Open:

- Edit speed on 4.18.0, to measure in the browser against the 30 to 50 seconds on 4.12.0.
- Gate 1, which waits for the maintainer.

## Effects, recovery and rollback

Live model spend in this stage: about US$0.006 on 2026-11-08 and US$0.0096 on 2026-11-14, both on the maintainer's DeepSeek key. No GitHub writes outside the stand-in and the test repository. No migration was added after 2026-11-08, so reverting the merge commits restores the earlier client and server.

## Configuration checkpoint

The Agent Tools pin moves from 4.12.0 to 4.18.0 (to run). The tool process is started with `HOI4_AGENT_TOOLS_PROFILES=all`. Five new tools are withheld from the model with a reason. No provider, permission or MCP connection changed.

## Engineering learning review

Recorded in the knowledge changelog: 0.2.45 (cancel-safe worker writes), 0.2.46 (tests under a memory cap) and 0.2.47 (Docker Desktop stays up, the WSL fallback for the client check). Candidates from this session, to record with the final handoff: a tool call that takes no scope can cover the whole game when the game folder is loaded, and a summary written by a model needs a bound before a page shows it.

## Next stage proposal and resume instructions

- Proposed next stage: S05, as a proposal only. It needs the maintainer's authorization.
- Still to do in S04: the full Linux run on the final tree, the re-pin's build and tests with `scripts/t-eng-01.sh`, the edit timing on 4.18.0, T-BENCH-01, `validate-stage`, and gate 1 when the maintainer can provide what it needs.
- Read first: this file, the three session records above and [the checks of the pages](browser-pages-2026-11-14.md).
- Pending operations needing reconciliation: none.
