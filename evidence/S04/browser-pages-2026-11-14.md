# S04 browser pages, checks of 2026-11-14

This note records what was run for the focus editor and change card pages and the two server additions they needed (D-61, D-63). It is a session record, not the stage handoff. Branch `s04-ui-lab3`.

## What was run

| Check | Kind | Result |
|---|---|---|
| Client check: lint, type check, 98 tests, production build | Test run in the pinned Node image (`web/scripts/docker-npm.sh run check`) | Pass. 15 of the tests are new and cover both pages |
| Server unit tests for the text operation and the follow-up objective | Test run on the Windows host (`cargo test -p factory-server --lib`) | Pass, 12 in the touched modules |
| Every server test on Linux | Test run in the test-runner image with PostgreSQL and the pinned Agent Tools 4.12.0 from the server image | Pass after one fix. Agent suite 22 of 22, focus suite 9 of 9, HTTP 12, intake 20, publish 3, setup gates 13 |
| Both pages against a real server | Manual run in a browser on a throwaway Compose stack built from this branch | Pass for the steps listed below |

The first Linux run failed one new test. A task that waits in review refused a new attempt, so the editor's follow-up request was rejected. A follow-up request now reopens the task in the same transaction. The rerun passed.

## Browser run against the real server

The stack was built from the branch with `scripts/compose.sh -p hf-s04ui up -d --build --wait` on ports 18080 and 18025, with a placeholder model key, so no model call was possible. A script signed up an account, verified it through Mailpit, created a project, uploaded the `focus-small` fixture and created an edit task.

| Step in the browser | Observed |
|---|---|
| Open the focus editor for the task's tree | The game render from the server loads and all four focuses are selectable |
| Click a focus | The card shows its real title, description and cost |
| Change the title and leave the field | One `set_text` operation, HTTP 201. The focus reads "Left road, changed" and the bar offers Save and Undo |
| Save | Commit, HTTP 201. The bar clears |
| Move a focus with the arrow key | One `move` operation, HTTP 201. The bar says "Changed: Right." |
| Task page | Shows "Open the focus tree" and "Review the change" |
| Change card for the move | The final picture with the moved focus outlined, one sentence, Approve and Reject, Details closed |
| Reject | Discard, HTTP 200. The card says "Rejected. Nothing was changed." |

## Limits

- Sending a request to the agent from the bar was not tried in the browser, because the throwaway stack had no model key. It is covered by the recorded-real replay test `a_follow_up_request_is_what_the_next_attempt_is_asked`.
- Each edit took 30 to 50 seconds to come back. That is the known round trip of the focus tool, tracked in hoi4-agent-tools#137.
- Gate 1 (a new project and a GitHub import through the whole slice with onboarding) was not run. It needs a real model key and the GitHub App.
- The picture is a source render. Nothing here is a check in the game.
- The task page can link to a tree only after the task has a change for it. A route that lists a mod's focus trees is not built.

## Found and fixed during the run

- A follow-up request was refused for a task in review (server).
- The change card kept the old picture after a reject (client).
- A problem sentence from the tools named a focus by its internal name. The pages now put the focus title in its place.
