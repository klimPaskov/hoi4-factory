# S04 live run: the ask bar with a real model in the browser

Evidence kind: live model calls with real spend, driven by hand in a browser against a throwaway Compose stack built from branch `s04-finish`. The stack ran the real server, PostgreSQL, the pinned HOI4 Agent Tools 4.12.0 and the game folder as reference. No game was run.

| Item | Value |
|---|---|
| Date | 2026-11-14, on the Windows PC |
| Stack | `scripts/compose.sh -p hf-s04ui up -d --build --wait` on ports 18080 and 18025 |
| Model | `deepseek-flash` at `https://api.deepseek.com`, as the account's own probed key (D-21). The probe reported text, tool calls, streaming, usage accounting and model listing as supported |
| Fixture | F-FOCUS-SMALL, the four-focus tree, uploaded as a ZIP |
| Task | One edit task with the allowed paths `common/national_focus` and `localisation` and a budget of US$0.50 |
| Spend | 9,607 micro-dollars for both attempts together (about US$0.0096), on the account's own key. The operator allowance was not touched |

## What was done in the browser

| Step | Observed |
|---|---|
| Task page, "Open the focus tree" | The list of trees answered and the page went straight to the mod's one tree |
| Request 1 in the bar: add one focus called Reform below Join, cost 5, with a description and a small reward | `POST .../attempts` with the instruction, HTTP 201. The editor showed "The agent is working on it." |
| While the agent worked | The new focus appeared on the tree in the open editor without a reload, marked as new |
| Attempt 1 | Succeeded in 80 seconds. One proposal by the agent added `exp_reform` to the focus file and its two lines to the tree's English localisation file |
| Approve | Commit, HTTP 201. The bar cleared |
| Request 2: make Reform cost 3 instead of 5 | Accepted on the task that already had a finished attempt, HTTP 201. Attempt 2 succeeded in 62 seconds with one `set_cost` operation |

Files: `detail.json` (the task and both attempts), `events.json` (the event stream), `proposals.json` (both proposals with hashes and diffs) and `usage.json` (the account's usage after the run). The account's email is replaced by a placeholder.

## Found and fixed during the run

- **The list of trees hung for minutes.** With the game folder loaded, one inspect of the whole workspace laid out every tree of the game. The route now inspects each focus file of the mod on its own. A probe in the running container measured 13 seconds for the first call.
- **The agent's summary was a long technical report.** It named ids, file paths and tools. The agent is now asked to begin its summary with one plain sentence, and the pages show only the first sentence.

## Limits

- The two fixes above were made after the run. The list fix was tried again in the browser. The summary fix is covered by the client check only.
- Each attempt took 60 to 80 seconds, most of it in the pinned Agent Tools 4.12.0.
- The picture is a source render. Nothing here is a check in the game.
