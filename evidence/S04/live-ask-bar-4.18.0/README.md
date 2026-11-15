# S04 browser run on HOI4 Agent Tools 4.18.0: edit times and the ask bar

Evidence kind: actions by hand in a browser against a throwaway Compose stack, with request times read from the browser's own resource timing, and one live model call with real spend. The stack was built from branch `s04-finish` at `db01c97` (image `sha256:3a460977ad8d`) and ran the real server, PostgreSQL, HOI4 Agent Tools 4.18.0 and the game folder as reference. No game was run.

| Item | Value |
|---|---|
| Date | 2026-11-15, on the Windows PC, with no other heavy job running |
| Stack | `scripts/compose.sh -p hf-s04ui up -d --wait` on ports 18080 and 18025 |
| Fixture | F-FOCUS-SMALL, the four-focus tree, uploaded as a ZIP |
| Model | `deepseek-flash` at `https://api.deepseek.com`, as the account's own probed key (D-21) |
| Spend | 6,020 micro-dollars for the one attempt (about US$0.0060), on the account's own key. The operator allowance was not touched |

## Times in the browser

| Step | Request | Time |
|---|---|---|
| Open the list of trees, first call after the server started | `GET .../focus/trees` | 11.1 s |
| Open the tree | projection, then the picture and its hover map | 2.3 s, then 3.9 s |
| Change a cost from the card | `POST .../operations`, then the new picture | 8.9 s, then 2.8 s |
| Change a title from the card | `POST .../operations`, then the new picture | 9.8 s, then 2.6 s |
| Move a focus one column with the arrow key | `POST .../operations`, then the new picture | 8.9 s, then 2.9 s |
| Save one edit | `POST .../commit` | 7.4 s |
| Save two edits together | `POST .../commit` | 13.8 s |

On the pinned Agent Tools 4.12.0 the same kind of edit took 30 to 50 seconds in the browser on 2026-11-14. With instant edits the page draws the change at once and shows "Checking the change" for these 9 to 10 seconds.

## The ask bar

One request in the bar: add one focus called Reform below Join, cost 5, 50 political power, with a short description. The attempt was admitted with HTTP 201 and succeeded in 43 seconds (80 seconds for the same request on 4.12.0). The new focus appeared in the open editor without a reload after about 25 seconds, marked as new. The agent's summary began with one plain sentence in the game's titles, as the system prompt now asks.

Files: `detail.json` (the task and the attempt), `events.json` (the event stream), `proposals.json` (the three edits by hand and the agent's proposal, with hashes and diffs) and `usage.json` (the account's usage after the run). The account's email is replaced by a placeholder.

## Found and fixed during the run

- **The Title field saved only when the field lost focus.** Enter did nothing, unlike the Cost field. Enter now saves it, with a client test.
- **The first sentence was cut too late.** The agent's opening sentence ended inside a quotation, and the page went on into the details until its length limit. The cut now ends at a sentence that closes with a quote or a bracket, with a client test.

Both fixes were made after the run and are covered by the client check only (106 tests).

## Limits

- One sample of each step, on one small tree. These are not percentiles.
- The agent's change was not approved in this run. Approve was walked on 2026-11-14.
- The mouse drag was not used. The arrow key sends the same move operation.
- The picture is a source render. Nothing here is a check in the game.
