# S04 session handoff, 2026-11-14

This is a session handoff inside S04, not the stage's final handoff. S04 stays open.

## Identity and authorized scope

- Stage ID and title: S04, the focus authoring vertical slice.
- Authorization: S04 authorized on 2026-11-08. This session was started by the HOI4 coordinator session for the two browser pages after D-61, and widened by the maintainer during the session.
- Agent: Claude Code desktop, started by the HOI4 coordinator session.
- Reviewer and review state: the maintainer chose the designs from images (D-63). No human review of the code yet.
- Repository: `hoi4-factory`, worktree `hoi4-factory-wt-s04-ui3`, branch `s04-ui-lab3`, merged into main at the end of the session.
- Dirty paths before work: ten uncommitted files of the earlier lab session under `web/src/design-lab/focus/`, kept as a WIP commit before anything else changed.

## Outcome and changed artifacts

| Item | Result | Evidence | Status |
|---|---|---|---|
| Minimal design round for the focus editor | Three working variants in the lab, reviewed by a fresh-context reviewer, sent to the maintainer | Mockups 100 to 102 | done |
| Minimal design round for the change card | Four working variants, reviewed the same way | Mockups 105 to 108 | done |
| The maintainer's choice | The ask bar with direct editing kept, and a change card that shows only the final result (D-63) | Mockups 110 to 113, [design decisions](../../docs/design/DESIGN-DECISIONS.md) | recorded |
| Focus editor page | `/projects/:id/tasks/:taskId/focus/:treeId` on the existing routes | [checks](browser-pages-2026-11-14.md) | built |
| Change card page | `/projects/:id/tasks/:taskId/changes/:proposalId`, with links from the task page | [checks](browser-pages-2026-11-14.md) | built |
| Follow-up request | `POST .../attempts` takes an optional `instruction`. The attempt's bundle scope says what it was asked. A task in review is reopened in the same transaction | `a_follow_up_request_is_what_the_next_attempt_is_asked` | built |
| Text operation | `set_text` rewrites only the line of its key in the localisation file that defines it. The projection carries each description | Two tests in `tests/focus.rs`, three unit tests | built |
| Lab cleanup | The lab's focus variants are removed. Sample scenarios cover both pages | `web/src/design-lab/scenarios.ts` | done |

The approval for the two server additions came from the maintainer in this session, after the coordinator passed on the question.

## Verification record

See [the checks of this session](browser-pages-2026-11-14.md). In short: the client check passes (98 tests and the build), every server test passes on Linux with PostgreSQL and the pinned Agent Tools 4.12.0, and both pages worked in a browser against a throwaway stack built from the branch.

Verified:

- Loading the tree, selecting a focus, the card with real title, description and cost.
- A title edit, a save, a move with the arrow keys, the task page links, the change card and a reject, each against the real server.
- The refusals: a stale tree, a failed check, a check that cannot finish, a text edit outside the task's allowed paths.

Not verified:

- **The ask bar with a real model.** The throwaway stack had no model key. Only the recorded-real replay test covers it.
- **Gate 1.** A new project and a GitHub import through the whole slice with onboarding in the browser. It needs a real model key and the GitHub App.
- **Gates 2 and 4 as a whole.** The pieces they need work, but nobody walked all three interaction modes through the pages in one sitting.
- **Dragging with the mouse against the real server.** The lab and the arrow keys were tried. The drag sends the same operation.
- **validate-stage and the stage handoff.** Not started.

## Decisions and unresolved issues

Technical defaults taken under D-18:

- A follow-up request goes into the bundle's scope objective, with a reminder of the task's first objective cut to 600 characters. No migration was needed, and a recovered attempt renders the same text from the stored bundle.
- A text edit never calls the tool's rewrite, so the tree's source keeps every byte. The editor route and the agent's grant check both authorise the files it writes.
- A person's own edits are saved or undone. The agent's changes are approved or rejected. Both are the same commit and discard routes.
- The pages load the picture as bytes through the API, so the lab's sample scenarios can serve a bundled picture.

Open:

- **A route that lists a mod's focus trees.** The task page can link to a tree only after the task has a change for it. The question is with the maintainer.
- **Speed.** Each edit took 30 to 50 seconds to come back in the browser. That time is spent in the pinned Agent Tools 4.12.0. The re-pin to 4.17.x for gate 11, after 4.17.1 ships, should bring it to a few seconds. Measure it then.
- **Docker Desktop on the Windows PC.** A `docker desktop stop` left a socket file that Windows would not delete, and the next start crashed until the maintainer fixed it. The rule and the WSL fallback for the client check are in CLAUDE.md (knowledge changelog 0.2.47).

## Effects, recovery and rollback

No model calls and no GitHub writes. The throwaway stack, its volumes and its image were removed. Reverting the merge commit of `s04-ui-lab3` restores the earlier client and server. No migration was added, so nothing in a database needs undoing.

## Next step

- Next: try the ask bar with a real model on the real stack, then gate 1, then the Agent Tools re-pin for gate 11, `validate-stage`, the stage handoff and GOAL-S05.
- Read first: this file, [the checks](browser-pages-2026-11-14.md), [the checkpoint](checkpoint-2026-11-08.md) and [the earlier session handoff](handoff-2026-11-13-session.md).
- Pending operations needing reconciliation: none.
