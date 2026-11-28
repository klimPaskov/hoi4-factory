# Engineering knowledge changelog

This is the versioned index of accepted engineering guidance changes. Detailed candidates retain evidence and review records. It is separate from product runtime memory and game design canon. Follow [the policy](REUSABLE-KNOWLEDGE-POLICY.md).

## 0.2.69, 2026-11-28, slot pool priority and dead-slot clearing

- CLAUDE.md, Delegation and parallel work: reserve conflicts go by priority (tutorial, Rust, then 3D and ZIN, oldest waiting take first), a plain take's slot is cleared once its recorded pid is gone for 5 minutes, and `scripts/test_heavy_slot.sh` covers both.
- Reason and evidence: the engineering lessons, "The slot pool deadlocked and kept dead slots (2026-11-28)".
- Authority: a technical default under D-18, D-113 and D-153. It widens no access, cost or scope.

## 0.2.68, 2026-11-28, TaskStop leaves a waiting take alive

- CLAUDE.md, Delegation and parallel work: stopping a background task does not end its waiting take's child, so kill the tree by PID or let the job run or time out.
- Reason and evidence: on 2026-11-28 a Rust packet stopped two queued jobs with TaskStop. The orphaned takes took two slots at 03:25 and 03:29 with no job behind them, and blocked half the pool until 06:10.
- Authority: a technical default under D-18 and D-113. It widens no access, cost or scope.

## 0.2.67, 2026-11-27, one take for a job of several steps

- CLAUDE.md, Delegation and parallel work: a job of several steps that a game run must not split keeps one take across all its steps.
- Reason and evidence: the engineering lessons, "A game run cut into a build between its steps (2026-11-27)".
- Authority: a technical default under D-18 and D-113. It widens no access, cost or scope.

## 0.2.66, 2026-11-27, the Tripo turn waits for 8 GB free

- CLAUDE.md, External services, Tripo browser: `scripts/tripo-turn.sh take` now waits for 8 GB of free RAM (was 3).
- Reason and evidence: after a full PC restart on 2026-11-27, Tripo's Animate and Generate pages still froze Chrome, both times at about 3.6 GB free while other lanes ran. A fresh browser and GPU process did not help, so the free-memory floor was too low.
- Authority: a technical default under D-18 and D-114. It widens no access, cost or scope.

## 0.2.65, 2026-11-27, the tutorial path goes first in the pool

- CLAUDE.md, Delegation and parallel work: the pool keeps 1 slot for take texts starting `tutorial-path:`, checked before the Rust and ZIN reserves.
- Reason and evidence: memory left the pool few free slots on 2026-11-27, and the maintainer chose the order: the tutorial path, then Rust, then 3D and ZIN.
- Authority: maintainer decision D-153.

## 0.2.64, 2026-11-27, the Tripo turn waits for free memory

- CLAUDE.md, External services, Tripo browser: `scripts/tripo-turn.sh take` waits until 3 GB of RAM is free (`TRIPO_MIN_FREE_GB`) before it hands out the turn, and sessions close their own Tripo tabs before they release.
- Reason and evidence: the engineering lessons, "A frozen Tripo tab blocked every later Tripo page (2026-11-27)".
- Authority: a technical default under D-18, D-114 and D-151. It widens no access, cost or scope.

## 0.2.63, 2026-11-23, gated steps start first come, first served

- CLAUDE.md, Delegation and parallel work: the 3D lanes' gated steps start in the order they began waiting. `heavy-slot.sh kept` finds the gated step that asks, ranks it by its process start time among the waiting steps (cached for 15 s), and keeps it waiting while older steps fit the free slots.
- Reason and evidence: on 2026-11-23 about 25 gated steps waited at once with memory near the line. Every waiter took a slot in turn and ran its own PowerShell memory check every 15 s, so newer steps often started before ones that had waited since 18:00, and the checks themselves cost memory and CPU.
- Authority: a technical default under D-18 and D-113. It widens no access, cost or scope.

## 0.2.62, 2026-11-23, no obvious fault reaches the maintainer

- CLAUDE.md, Verification and handoff: before showing or relaying anything, check it against the accepted references and the anatomy of every figure, fix each obvious fault or send it back, and never show a known fault with a caption. A renderer's limit counts as a fault.
- Reason and evidence: the engineering lessons, "Obvious faults shown for review (2026-11-23)".
- Authority: maintainer decision D-144.

## 0.2.61, 2026-11-23, one game depot through Steam's console

- CLAUDE.md, On Windows only: the depot IDs of HOI4 per platform, and that the current Steam client opens its Console tab only when started with `-console`. `steam://open/console` does nothing.
- Reason and evidence: on 2026-11-23 the WSL copy moved to 1.20.0.0. The Linux and macOS binaries came from `download_depot`, which worked only after Steam was restarted with `-console`. Both manifests matched the public build.
- Authority: a technical note under D-18 and D-90. It widens no access, cost or scope.

## 0.2.60, 2026-11-23, a handover word keeps the game windows

- `coordinate-sessions`, Stop and error paths: when the maintainer asks the coordinator to pass a handover word such as "PC is yours" for him, it passes the word only within D-33. The Windows game waits until he has stopped using the PC, and the isolated WSL copy may run at any time on a virtual display.
- Reason and evidence: on 2026-11-23 at 18:45 a coordinator passed "PC is yours" to the Live Tools port worker as if it opened the Windows game at once. The maintainer corrected it five minutes later. The Windows game runs only once he is no longer at the PC. No game had started.
- Authority: D-33 restated. It widens no access, cost or scope.

## 0.2.59, 2026-11-23, reserved slots per group

- CLAUDE.md, Delegation and parallel work: a group keeps slots in the heavy-job pool while one of its takes waits (`.work/tripo-round4/reserves`). Rust keeps 2 for take texts containing `rust` or `cargo`, and ZIN keeps 1 for take texts starting `zin-lane:`.
- Reason and evidence: on 2026-11-22 the Rust packets waited behind six 3D steps with nothing compiled, and on 2026-11-23 four ZIN jobs, among them the maintainer's top priority of the day, waited the same way. A group's unfilled slots open to everyone as soon as none of its takes waits.
- Authority: a technical default under D-18 and D-113. It widens no access, cost or scope.

## 0.2.58, 2026-11-23, archive only your own family's sessions

- CLAUDE.md, Delegation and parallel work: archive only sessions your own family started, after reading the last report. A merged pull request does not mean a session is finished. Ask another family's lead to archive its own.
- Reason and evidence: one coordinator archived eight of another coordinator's working sessions on 2026-11-23. The story is in the engineering lessons. This replaces a first, wrong version of the rule about archives cascading through a shared folder.
- Authority: a technical default under D-18 and D-90. It widens no access, cost or scope.

## 0.2.57, 2026-11-22, root fixes in every lane

- CLAUDE.md, Keeping this guidance current: the D-90 rule holds in every lane and whatever a task first aimed at. A problem met while making production content is still fixed at its root in the shared kit or tool, so it cannot recur for the maintainer's mods or for public users.
- Authority: maintainer decision D-135, given through ZIN's lead.

## 0.2.56, 2026-11-22, green means checks for the head commit

- CLAUDE.md, External services: a pull request is green only when `gh pr checks` lists every required check as passed for its current head commit. "No checks reported" or a run without jobs is not green. Incident in the engineering lessons.

## 0.2.55, 2026-11-22, a pool of heavy-job slots

- CLAUDE.md, Delegation and parallel work: heavy jobs run in a pool of two slots (`scripts/heavy-slot.sh`, D-113), with `take --exclusive` for Windows game launches and quiet benchmarks. A 3D run takes no slot itself, because each `gated.sh` step takes one. Take only when the job is ready, check the take's exit code directly, release in a trap, and kill a stopped waiter's whole process tree.
- Reason and evidence: the single slot stalled the work of 2026-11-22 (a benchmark ran unheld, two holds sat idle, one lane lost every race). The story is in the engineering lessons.
- Authority: the maintainer allowed more heavy jobs at once (D-113). The pool's form is a technical default under D-18.

## 0.2.54, 2026-11-22, downloads need no approval

- CLAUDE.md, External services: agents download what their work needs without asking (D-112), into their own folder, read only, never run.
- Authority: maintainer decision D-112.

## 0.2.53, 2026-11-21, one script for the heavy-job slot

- CLAUDE.md, Delegation and parallel work: every session takes and gives back the heavy-job slot with `scripts/heavy-slot.sh`, which checks the hold file and the Blender lock together, checks the memory and disk lines and takes the slot atomically.
- Reason and evidence: two collisions on 2026-11-21, a benchmark rerun and a ZIN build each started while a Blender run held the lock. The story is in the engineering lessons.
- Authority: a technical default under D-18 and D-90. It widens no access, cost or scope.

## 0.2.52, 2026-11-21, one root command in WSL and the coordinator's model

- CLAUDE.md, Windows only: agents may drop the WSL file cache as root when memory blocks a heavy job and nothing runs in WSL, and they log it. Every other use of root still needs the maintainer's yes.
- CLAUDE.md, Delegation and parallel work: no worker session or subagent runs on Fable, and only the coordinator session may.
- Reason and evidence: D-96 and D-95, answered by the maintainer when a new coordinator session started.
- Authority: maintainer decisions. D-96 widens agent access by exactly one command, on the maintainer's word.

## 0.2.51, 2026-11-18, lessons go into the workflow for weaker models

- CLAUDE.md, Keeping this guidance current: each lesson goes into the shared workflow as a step, rule or check that a small model can follow, and workflow changes are tested with a small model when possible.
- Reason and evidence: D-90, stated by the maintainer on the night the 3D route was first tested end to end with a small model.
- Authority: a maintainer decision about how agents work. It widens no access, cost or scope.

## 0.2.50, 2026-11-18, detailed work and no simplifying without consent

- CLAUDE.md, Verification and handoff: work stays detailed, nothing is simplified without the maintainer's consent, and a pass is checked against the input it used before it is relayed.
- Reason and evidence: D-87 and D-88. Three 3D type passes had come from simpler redraws of their subjects. The story is in the engineering lessons.
- Authority: a maintainer decision that narrows what agents may do. It widens no access, cost or scope.

## 0.2.49, 2026-11-18, the WSL VM gives its file cache back

- CLAUDE.md, Windows: a job that reads large files in WSL releases them from the file cache as the ordinary user when it ends. Dropping the whole cache as root needs the maintainer's yes.
- Reason and evidence: on 2026-11-18 the VM held 6.6 GB of file cache with no job running, which closed the memory gates of the 3D lane and of ZIN's capture. The story is in the engineering lessons.
- Authority: engineering guidance only. It widens no access, cost or scope.

## 0.2.48, 2026-11-15, scoped tool calls, bounded model text and the 4.18.0 re-pin

- CLAUDE.md, Agent runtime rules: scope each Agent Tools call to a file or a tree, start the server with every profile on and run the whole `factory-hoi4-tools` crate against the real server after a profile or pin change, and bound text a model wrote before a page shows it.
- CLAUDE.md, Windows: a test database on `--tmpfs` gets at least 3 GB for a full workspace run.
- Reason and evidence: the S04 browser runs of 2026-11-14 and 2026-11-15 and the re-pin to Agent Tools 4.18.0. The stories are in the engineering lessons, and the results in `evidence/S04/checks.json`.
- Authority: engineering guidance only. It widens no access, cost or scope.

## 0.2.47, 2026-11-14, Docker Desktop stays up and the client check has a WSL fallback

- CLAUDE.md, Working environment: when Docker is down the client check runs in WSL from a copy of `web/`. On Windows, Docker Desktop is left running once it is up.
- Reason: a `docker desktop stop` left a socket file that Windows would not delete, and the next start crashed. The story is in the engineering lessons.
- Evidence: the S04 browser pages passed `npm run check` in WSL on 2026-11-14 (96 tests and the build).
- Authority: engineering guidance only. It widens no access, cost or scope.

## 0.2.46, 2026-11-14, local tests run under a memory cap and a timeout

- CLAUDE.md's verification section gained one rule: local tests run under a memory cap and a timeout, and a loop-driven test uses a fake clock and an iteration cap, never a mock object with a real clock. The maintainer approved the rule.

Evidence: the Live Tools incident of 2026-11-14 in the [engineering lessons](ENGINEERING-LESSONS.md), fixed in HOI4-Live-Tools pull request 68. Rollback: revert this commit and regenerate.

## 0.2.45, 2026-11-13, cancel-safe worker writes and a recovery-test race

- CLAUDE.md's agent runtime rules gained three lines: whole frames go through a writer task that owns the worker's stdin, a dropped `run_turn` is followed by `cancel`, and a test that kills the host mid-turn waits until the replay server has the request.
- The Agent Tools re-pin rule now includes classifying every listed tool in a profile or in `AGENT_TOOLS_WITHHELD` (issue #6).

Evidence: issues #13 and #6, `crates/agent/tests/worker.rs` and `crates/server/tests/agent.rs`, and the [engineering lessons](ENGINEERING-LESSONS.md). Rollback: revert this commit and regenerate.

## 0.2.44, 2026-11-09, detach side sessions before a coordinator archives itself

- `coordinate-sessions` now says the old coordinator detaches its side sessions with `detach_session` before it archives itself, because archiving a session also archives its finished side sessions.

Evidence: the 2026-11-09 handover to a new coordinator. Rollback: revert this commit and regenerate.

## 0.2.43, 2026-11-09, hoi4-imagegen-mcp is no longer a fork (D-54)

- CLAUDE.md's pull request rule now applies to every repository and records that hoi4-imagegen-mcp was recreated as a non-fork with renumbered issues and PRs.

Evidence: the maintainer asked for the repository to be their own with the same name (D-54). Rollback: revert this commit and regenerate.

## 0.2.42, 2026-11-09, explicit repository for pull requests in a fork

- CLAUDE.md now says to pass `--repo` and `--base` to `gh pr create` in a forked repository such as hoi4-imagegen-mcp.

Evidence: on 2026-11-09 a PR meant for hoi4-imagegen-mcp was opened on its upstream parent and closed at once. Rollback: revert this commit and regenerate.

## 0.2.41, 2026-11-09, effort stays at high or below (D-53)

- CLAUDE.md and `coordinate-sessions` now say no session or worker runs above high effort, and a coordinator lowers any it finds above high.

Evidence: the maintainer asked that work not start on Opus at extra high effort and that each session use a fitting model and effort (D-53). Rollback: revert this commit and regenerate.

## 0.2.40, 2026-11-08, gptimage for page mockups (D-52)

- CLAUDE.md and `ui-design-iteration` now allow the general `gptimage` tool for HOI4 Factory page and UI mockups, one request at a time and never while hoi4-imagegen is generating, with game assets still only from hoi4-imagegen. The skill also says to pass a real game render as the reference when a page shows a game surface.

Evidence: hoi4-imagegen has no family for a web page, and the maintainer allowed gptimage for mockups (D-52). Rollback: revert this commit and regenerate.

## 0.2.39, 2026-11-08, CLAUDE.md slimmed (D-42)

- CLAUDE.md keeps every rule as a short line. Dated incident stories, measurements and one-time history moved to the new [engineering lessons](ENGINEERING-LESSONS.md), which says which rule each story belongs to. CLAUDE.md links to it from its opening paragraph and from the section on keeping the guidance current, which now says to put new stories there.
- No rule was added, removed or widened. The "(learned in S03)" heading suffix and the dated asides moved with their stories.

Evidence: a diff audit of the old and new CLAUDE.md (every removed clause is a story, a date or a measurement, and each is in the lessons file). Rollback: revert this commit and regenerate.

## 0.2.38, 2026-11-08, S04 lessons on Agent Tools, Windows Git and formatting

- CLAUDE.md now says a rewrite Agent Tools answered with `status: "blocked"` is a refusal even without an error flag, lists the files to change together when the Agent Tools release is re-pinned, and says to isolate `HOME`, `APPDATA` and the `XDG_*` folders when running Agent Tools by hand.
- Windows notes: Git over HTTP stalls after a `401` until the caller's timeout, the host's npm 12 needs a `package-lock.json` copy of a shipped shrinkwrap, and heredocs with apostrophes can break the Bash tool.
- Verification note: the Rust code is not rustfmt-clean, so format only your own new files.

Evidence: in S04 a blocked rewrite was first recorded as applied (fixed in `focus/session.rs` with a regression test in `tests/focus.rs`), the fake GitHub's 401 made a test wait 178 s per call, and `cargo fmt -p factory-server` rewrote twelve untouched files. Rollback: revert this commit and regenerate.

## 0.2.37, 2026-11-08, review of how the work is run

- Decisions now live per project under `docs/decisions/`, each file with an in-force list, and IDs continue as one global D- sequence (D-35). CLAUDE.md's reading order and recording rule point there. The two older records carry a history banner.
- CLAUDE.md's game rule is one standing window instead of a growing list of dated nights (D-33): Windows any night 00:00 to 06:00 through one runner, the WSL copy any time, never the Mac.
- The coordinator line no longer says to keep about six sessions live, which contradicted D-31 (D-34). It now names the usage reserve (D-39).
- A release rule (D-40) and the new skill `prepare-promotion-release` (D-40, D-43, D-49).
- `coordinate-sessions` checks usage at each check-in (D-39) and slows to every three hours when all sessions are idle (D-48).
- New brief `docs/06-agent-handoffs/GOAL-MACOS-CHECKS.md` for the macOS-only checks (D-47), and the plan `docs/09-planning/NEXT-STEPS-2026-11.md`.

Evidence: a review session with the maintainer on 2026-11-08 found the game rule listing five past windows with a 09:00 and 06:00 mismatch, the session cap contradiction, about 150 decisions in two mixed tables with expired and replaced entries unmarked, and PACKAGE_STATUS still saying S04 was not authorized. The maintainer answered five rounds of questions (D-33 to D-49).

## 0.2.36, 2026-11-08, a coordinator workflow for large multi-session work

- New maintainer-only skill `coordinate-sessions` (D-32): one coordinator session, an hourly check-in, a private status page fed by one JSON snapshot, messaging limits between sessions, and handovers to a fresh coordinator or restarts of running sessions at their break points.

Evidence: on 2026-11-08 the first coordinator's context grew too costly, a new coordinator took over from a written note, built the status page and found that running sessions cannot be moved under a new parent and that the app pauses cross-session messages after about ten. Rollback: revert this commit and regenerate.

## 0.2.35, 2026-11-08, a Live Tools acceptance window

- CLAUDE.md's in-game exceptions now include D-30: from 18:00 on 2026-11-08 to 06:00 on 2026-11-09 one named runner may start the Windows game through HOI4 Live Tools for its release acceptance.

Evidence: the maintainer approved the window because the Live Tools release process requires a passing live acceptance run. Rollback: revert this commit and regenerate.

## 0.2.34, 2026-11-08, HOI4 images come from hoi4-imagegen only

- CLAUDE.md now says every HOI4 image is made with hoi4-imagegen, never the general `gptimage` MCP, because only hoi4-imagegen paces the shared ChatGPT sign-in across processes.

Evidence: OpenAI revoked the sign-in at 15:29 UTC on 2026-11-08 while both tools generated at once. Rollback: revert this commit and regenerate.

## 0.2.32, 2026-11-08, a ZIN static-model check window

- CLAUDE.md's in-game exceptions now include D-29: from 2026-11-08 to 2026-11-09 one named session may start the isolated WSL Linux copy with ZIN loaded to check three ZIN static models, never the Windows game.

Evidence: the maintainer approved fixing the Castle of Yeldenne, the Golden Court of Parim and the Galley of Aenor and checking them in game. Rollback: revert this commit and regenerate.

## 0.2.31, 2026-11-08, keep the desktop app under its memory limit

- CLAUDE.md now says a coordinator runs long work as separate sessions rather than background subagents in its own process, keeps about six sessions live, archives finished ones, and resumes paused sessions after a crash.

Evidence: the desktop app restarted five times on 2026-11-08. Its log showed process memory at 11.5 GB across about ten sessions, governor evictions under critical memory pressure, and a 1 GB transcript being tail-loaded. Rollback: revert this commit and regenerate.

## 0.2.30, 2026-11-06, stop only your own processes

- CLAUDE.md's clean-up rule now says to stop only processes you started, matched by PID or your own session folder, never by a broad command-line pattern.

Evidence: the trailer session stopped its own render by matching any `blender.exe` whose command line contained "scratchpad", and also stopped two other sessions' background Blender jobs. Rollback: revert this commit and regenerate.

## 0.2.29, 2026-11-06, a Live Tools game window and development stays on Windows

- CLAUDE.md's in-game exceptions now include D-24: one runner session may start the game from 00:00 to 09:00 local on 2026-11-07 for the HOI4 Live Tools checklist, under the same limits as the earlier nights.
- The decision record adds D-25: HOI4 Factory development, S04 included, stays on the Windows PC and does not move to the Mac.

Evidence: the Live Tools session merged its DLC prepare step, launch fixes and four check tools, all tested offline only, with ten in-game checks left. The maintainer chose tonight for them and said S04 starts on Windows. Rollback: revert this commit and regenerate.

## 0.2.28, 2026-11-06, merges and pushes need no asking

- CLAUDE.md's routine-steps rule now records D-23. Agents merge and push without asking the maintainer, in every repository the work touches, mods included, and also for configuration changes a recorded decision approved. The D-18 limits stay.

Evidence: a Chaos Redux session left a decision-approved configuration PR for the maintainer to merge, and the maintainer said not to ask again for merges or pushes. Rollback: revert this commit and regenerate.

## 0.2.27, 2026-11-06, kit changes can break every Setup install

- CLAUDE.md now says that HOI4 Mod Setup installs from kit `main`, so a kit PR that moves or rewrites files with Setup component markers, or changes the package manifest, must pass Setup's offline acceptance or the kit's Setup-reading test before it merges.

Evidence: kit #82 (context budget) left `hoi4-project-rules` references `mcp-usage.md` and `skill-routing.md` ending with an unterminated optional block for `workflow.live_tools` and `workflow.live_testing`. Setup's component adapter refused them, so every install of kit `main` failed, the released Setup 0.9.2 included. The kit's own CI passed. Kit #85 removed the dangling markers and added a test that reads every tracked file the way Setup does. Rollback: revert this commit and regenerate.

## 0.2.26, 2026-11-06, provisional design choices and opening the lab

- The `ui-design-iteration` skill now covers provisional picks when the maintainer asks that work go on without approval, and says to list them in chat in the lab's order with the lab's letters.
- It also says to start the dev server and confirm the page before sending the maintainer to the lab. The Docker npm script runs only from Git Bash or zsh, and a lagging shared checkout is served from a worktree at `origin/main`.

Evidence: on 2026-11-06 the maintainer ran the dev server command in PowerShell and nothing opened. The shared checkout was 18 commits behind `origin/main` and lacked the lab work, and the chat summary listed the four provisional pages in the reverse of the lab's order. Rollback: revert this commit and regenerate.

## 0.2.25, 2026-11-06, look at visual work before calling it done

- CLAUDE.md now says visual work is finished only after it has been rendered at the viewer's size and in close-up, looked at, and reviewed by a fresh-context reviewer that sees only the images and the quality bar. Passing numeric checks is not evidence that it looks right.

Evidence: on 2026-11-06 the 3D units trailer's first cut showed seven ZIN units that had passed every geometry check with 0 errors, next to existing Chaos Redux units. The maintainer saw holes in the meshes and deformed animations in both at first glance, and said the models are nowhere near acceptable. Rollback: revert this commit and regenerate.

## 0.2.24, 2026-11-06, no bare Python REPL

- CLAUDE.md now says never to start Python without a script, `-c` or `-m`, and to look for large task output files under the Claude temp folder first when C: suddenly fills.
- `scripts/py.sh` refuses to run with no arguments and sets `PYTHON_BASIC_REPL=1`.

Evidence: at 07:39 on 2026-11-06 a background task's Python 3.13 REPL, started without a console, looped one `_pyrepl` WinError traceback into a 50 GB output file and left C: with 2.2 GB free. The 3D session found it when its writes were about to fail. Deleting the file brought C: back to 52 GB. Rollback: revert this commit and regenerate.

## 0.2.23, 2026-11-06, one decision at a time in the design lab

- The skill's image step now paces generation: one image at a time, 10 to 20 seconds apart, a longer wait after an error, and a stop when the account is signed out (W5-5).
- The `ui-design-iteration` skill now has a review step: the lab's front page shows only the one open decision as large lettered options, a full-screen in-page viewer switches options with the arrow keys, and decided questions move to an archive page.

Evidence: the maintainer found the mixed page of every generated image confusing and asked for a simple review with arrow-key switching, 2026-11-05.

## 0.2.22, 2026-11-05, the maintainer's ratings stay private

- CLAUDE.md now says the maintainer's own ratings and review verdicts stay in private notes. Public repositories, docs, test fixtures, release notes and videos state the resulting rule instead of quoting, citing or attributing them.

Evidence: on 2026-11-05 the maintainer said their gallery ratings must not appear in public-facing docs. The same day a public hoi4-imagegen pull request carried a test fixture labelled with their verdicts and a private notes path, and the trailer's sources doc described their ratings. Rollback: revert this commit and regenerate.

## 0.2.20, 2026-11-05, clean up after yourself

- CLAUDE.md now says every session and agent cleans up before it reports a task done: merged worktrees and their Rust build copies, renders, probe output, logs, archives, test containers and one-off Docker images, and its own clones and builds inside WSL. It notes that space freed inside WSL or Docker returns to C: only after a compaction, and that shared Docker images, build cache and volumes need the maintainer.
- The `delegate-parallel-work` skill adds the same cleanup rule to every brief.

Evidence: on 2026-11-05 C: had 7.2 GB free, three days after a cleanup left 165 GB. Measured causes were 45 GB of Steam and the game in Ubuntu, 17 GB of Docker build cache, 8.7 GB of WSL crash dumps from a packaged Electron test, and Rust build copies left by finished worktrees. Deleting the dumps and build entries older than two hours freed 41.5 GB. Rollback: revert this commit and regenerate.

## 0.2.18, 2026-11-02, image-first UI design

- The `ui-design-iteration` skill now starts every page and asset from the image model: page mockups first, then icons, logo and mascot poses, with one master image as the reference for consistency and fresh prompts when the maintainer asks for new ideas. Code then matches the chosen image. It also points to fixture mode and `web/scripts/lab-screenshots.mjs` for before and after captures, notes the maintainer's preference for minimal screens, and explains Vitest worker start timeouts on a saturated host.
- CLAUDE.md notes that the Docker dev server polls for edits on Windows and should be stopped for client tests on a loaded machine.

Evidence: maintainer instructions in the UI design track session on 2026-11-02. A mockup set, an icon set, logo rounds and 15 consistent mascot poses were produced this way. Vitest workers timed out at 60 seconds while the host CPU was at 100% with 2.3 GB free.

## 0.2.19, 2026-11-05, the second in-game night moves

- CLAUDE.md's game-launch rule now places the second window from 00:00 local on 2026-11-06 (D-22), because the D-20 night never ran after sessions stopped on 2026-11-02.

Evidence: maintainer decision D-22 of 2026-11-05. Rollback: revert this commit and regenerate. Remove both exceptions once the window has passed.

## 0.2.18, 2026-11-02, shared-checkout pushes and the DeepSeek key

- CLAUDE.md's delegation section now warns that pushing `main` from a shared checkout publishes every local commit on it, and says to give committing sessions their own worktrees or to check `git log origin/main..main` first.
- CLAUDE.md's model-provider line now names the DeepSeek key and model used for live checks of the OpenAI-compatible profile (D-21).

Evidence: on 2026-11-02 the coordinating session's decision pushes, and the Discord session's push, published the UI design track's local commits `535205b` and `5515988`. The same day the compatible adapter was verified live on DeepSeek (`evidence/S03/live-deepseek-compat/`). Rollback: revert this commit and regenerate.

## 0.2.17, 2026-11-02, model and effort for new sessions

- CLAUDE.md's delegation section now says that a new Claude Code session starts on Sonnet at medium or high effort, and on Opus at medium effort for harder tasks, instead of inheriting the starting session's model and effort.

Evidence: maintainer preference stated to the coordinating session on 2026-11-02. Rollback: revert this commit and regenerate.

## 0.2.16, 2026-11-02, a second in-game night

- CLAUDE.md's game-launch rule now names two exceptions: D-19 for the night of 2026-11-01 to 2026-11-02, and D-20 from 00:00 local on 2026-11-03 for the checks the first night missed. Both windows end with no new launch after 09:00 local, and the rule applies again outside them.

Evidence: maintainer decision D-20 of 2026-11-02, relayed by the coordinating session. Rollback: revert this commit and regenerate. Remove both exceptions once the second window has passed.

## 0.2.15, 2026-11-02, read a mod's own rules before an in-game check

- CLAUDE.md now says to read another project's CLAUDE.md or AGENTS.md before asking for an in-game check that loads its mod, and to follow its rules.

Evidence: on 2026-11-02 the Meshy 3D session asked the in-game runner to place five ZIN units in game. ZIN's own rules forbid launching or capturing the game with ZIN loaded, and that was only noticed afterwards. Rollback: revert this commit and regenerate.

## 0.2.14, 2026-11-01, one-night in-game window

- CLAUDE.md's rule that agents never start the game now names one exception (D-19): on the Windows PC during the night of 2026-11-01 to 2026-11-02, one runner session may start it through HOI4 Live Tools, one instance at a time, with no new launch after 09:00 local on 2026-11-02, restoring the user-folder files a launch rewrites. The rule applies again after the window.

Evidence: maintainer decision D-19 of 2026-11-01, relayed by the coordinating session. Rollback: revert this commit and regenerate. Remove the exception text once the window has passed.

## 0.2.13, 2026-11-01, routine steps need no approval

- CLAUDE.md now lets agents merge pull requests for authorized work once every required check is green and push the commits that finish authorized work, without waiting for the maintainer. Only true blockers wait: a sign-in or credential, money, access only the maintainer holds, the Mac and in-game checks. The rules against skipping failing checks, force pushes, lost work, new spending, starting the game and touching Docker Desktop are unchanged.

Evidence: maintainer decision D-18 of 2026-11-01, relayed by the coordinating session. Rollback: revert this commit and regenerate.

## 0.2.12, 2026-11-01, autocrlf covers the checkout too

- CLAUDE.md now says `-c core.autocrlf=false` must cover worktree creation and sparse-checkout changes, not only the commit. It also says never to use `commit -a` in another repository, and to check `git diff --ignore-cr-at-eol --stat` before committing. It notes two Git Bash quirks: `grep` hides CR bytes, and `rev:path` arguments get rewritten.

Evidence: Chaos Redux #4 (ce260deb1c) stored 119 files as CRLF blobs. They were checked out CRLF by `git sparse-checkout add` under the system `core.autocrlf=true`, then staged by `git -c core.autocrlf=false commit -a`. Chaos Redux #5 (082247083b) restored the LF blobs and changed line endings only. Rollback: revert this commit and regenerate.

## 0.2.11, 2026-11-01, agents never start the game

- CLAUDE.md now says agents never start Hearts of Iron IV on the maintainer's machines. In-game checks go to the HOI4 Live Tools MCP or the maintainer, with an exact list of what to check.

Evidence: a session launched the game on the maintainer's machine on 2026-11-01, and the maintainer set the rule (D-17). Rollback: revert this commit and regenerate.

## 0.2.10, 2026-11-01, issues in every repository

- The `track-issues` skill now files each problem in the repository it belongs to (hoi4-factory, hoi4-agent-tools, the kit, HOI4 Mod Setup or hoi4-imagegen-mcp), with an owner and a "Done when" check, and closes it only with proof. It no longer forbids issues on the upstream repositories. The rule against secrets and private paths is unchanged.

Evidence: maintainer decision of 2026-11-01, relayed by the planning session. The 15 live-test problems are klimPaskov/hoi4-agent-tools#57 to #71. Rollback: revert this commit and regenerate.

## 0.2.9, 2026-11-01, decisions as statements of intent

- CLAUDE.md now says to write each maintainer decision as a clear, complete statement of intent in our own words, not a verbatim quote. The maintainer asked for it after A1-1, FR-5 and Q-1 were first recorded as quotes, and those entries were rewritten in the same change.

Evidence: maintainer request relayed by the parent session on 2026-11-01. Rollback: revert this commit and regenerate.

## 0.2.8, 2026-10-31, run the target repository's CI list before pushing

- CLAUDE.md now says to run another repository's own CI steps locally before an approved push to it. The kit's wiki follow-ups were pushed after seven hand-picked checks passed, and the kit's Validate workflow then failed on the quality complexity ratchet, which only `check.py full --only ... quality` runs. The fix (`aa09a0c` in the kit) moved three checks into helpers without touching the baseline.

Evidence: kit Validate run 41229595759 failed on `4261df9` because `verify` had a complexity of 55 against a recorded baseline of 50, and the same check passed locally after the fix. Rollback: revert this commit and regenerate.

## 0.2.7, 2026-10-31, deep clones on Windows

- CLAUDE.md notes that cloning a deep repository into the long session scratchpad path fails on Windows with "Filename too long", and that cloning inside a Linux container with the checkout mounted read-only works and doubles as a fresh-checkout Linux run.

Evidence: the W1 kit synchronization work of 2026-10-31. A `git clone` of the Agentic kit into the scratchpad failed on the skill asset paths, and the same clone inside `python:3.13-slim` ran the 31 synchronization tests. Rollback: revert this commit and regenerate.

## 0.2.6, 2026-10-25, issue tracking

- New skill `track-issues`. Problems that cannot be fixed in the current session become GitHub issues on `klimPaskov/hoi4-factory`, each with evidence, the reason and an acceptance check, and are later closed with `Fixes #N` and test results. Everything else is fixed in the session. CLAUDE.md has an "Issues" section, the generator and validator list the skill, and the repository has the labels `blocked`, `verification`, `backlog` and `stage:SNN`.
- New `scripts/audit-deps.sh` for the dependency scan, with ignores and their reasons in `.cargo/audit.toml`.

Evidence: issues #2 to #11 and the S03 handoff's follow-up section. Rollback: revert this commit and regenerate.

## 0.2.5, 2026-10-25, S03 agent runtime

- CLAUDE.md has a new section, "Agent runtime rules (learned in S03)". It covers:
  - Anthropic tool-schema conditioning: top-level combinators, and strict-mode keywords and combinators.
  - The lease heartbeat, tied to its owner with an abort-on-drop guard.
  - Biased selects, so a request goes out before a cancel is acted on.
  - REPEATABLE READ snapshots for answers that combine several tables, and one transaction for an attempt's end and its task's next state.
  - Tool calls deferred on pause.
  - Real SHA-256 values in ledger hash fields.
  - Worker test faults passed through the launcher environment.
  - An init process for containers whose PID 1 is our own binary.
  - The two Node runtimes in the server image.
- CLAUDE.md's delegation section and the `delegate-parallel-work` skill say how to run crate packets in parallel. The parent commits empty crate skeletons first, writes the integration API into each brief (saved as a file the agent reads), and keeps the composition that spans crates.
- `scripts/linux-tests.sh` runs its container with `--init`, and the server service in `compose.yaml` sets `init: true`.

Evidence: [S03 learning review](../../evidence/S03/learning-review.md) (S03-L01 to S03-L11) with the tests named there. Rollback: revert this commit and regenerate.

## 0.2.4, 2026-10-25, Windows and macOS development

- CLAUDE.md's working environment now covers both development machines, Windows 11 and macOS on Apple Silicon, with shared rules first and per-platform quirks after them. The Python, Compose and disk-check commands are the same on both.
- New `scripts/py.sh` runs the Python scripts with Python 3.11 or newer and `PYTHONUTF8=1` on either machine. Every guide, skill body and goal prompt uses it in place of `py -3.13` or `python`.
- New `scripts/compose.sh` resolves the game folder from the machine's Steam libraries, so one `.env` works on both machines. The setup guide and `compose.yaml` use it.
- Shell scripts are committed with the executable bit. macOS refused to run `scripts/linux-tests.sh` and `web/scripts/docker-npm.sh` without it.
- The first macOS run found that an uploaded repository with `include.path = /etc/passwd` was reported as not a repository instead of unsafe config, because Git follows includes while it discovers a repository and that file exists on macOS and Linux. `GitRepo::config_findings` now reads `.git/config` without discovery, and a platform-independent regression test covers it.

- New `scripts/setup-local.sh`: one command per machine that asks for the Anthropic key, the GitHub client secret and the App key file, saves them in `.env`, the shell profile (or a Windows user variable) and `.work/github-app/`, then starts and checks the stack. CLAUDE.md and the setup guide point to it.
- CLAUDE.md notes that Docker Desktop on the Mac can hang with its VM unreachable until it is restarted, and the setup guide notes a one-time `scripts/compose.sh pull` when a fresh machine reports `No such image`.

Evidence: the macOS session of 2026-10-25 on Apple Silicon. `cargo test --no-fail-fast --workspace --exclude factory-runtime-probe` passed 384, failed 0, ignored 8 (live) on macOS against PostgreSQL 17.6, and the same on Linux through `scripts/linux-tests.sh`. The web checks passed (63 tests and the build). `scripts/compose.sh up -d --build --wait` found the game in the Mac's Steam library, `/readyz` reported `status: ok` with the game reference available and read-only, and `scripts/live_flow.py` completed every step through apply with one real `claude-haiku-5-5` analysis call (the server does not report token counts). Rollback: revert this commit and regenerate.

## 0.2.3, 2026-10-25, UI design track

- New skill `ui-design-iteration` for designing client pages from working variants that the maintainer chooses in a development-only design lab, by maintainer decision ADR-025. Generator, validator, CLAUDE.md and the instruction map list it.
- New goal prompt [GOAL-UI](../06-agent-handoffs/GOAL-UI.md) for the interactive design session that runs after S03.

Evidence: the maintainer's answers of 2026-10-25 recorded as ADR-025. Rollback: revert this commit and regenerate.

## 0.2.2, 2026-10-25, S02 handoff

- New [developer setup guide](DEVELOPER-SETUP.md) for a fresh machine, with `.env.example` and a readiness checklist. CLAUDE.md links it.
- `scripts/linux-tests.sh` is self-contained: it builds `docker/test-runner.Dockerfile` (Rust 1.97.1, git, Debian Node) and starts its own database when `FACTORY_TEST_ADMIN_URL` is unset.
- CLAUDE.md: run workspace tests with `--no-fail-fast`, because `cargo test` stops at the first failing test binary, and treat failures on a nearly full disk as suspect.
- CLAUDE.md: assert pinned upstream behavior against `SETUP_PIN` instead of fixed values, and rerun the whole workspace after a pin change. The Linux test note now names the self-built test image.
- The server image now builds the browser client in a pinned Node stage and serves it from `FACTORY_WEB_DIST`.

Evidence: [S02 learning review](../../evidence/S02/learning-review.md) and [checks](../../evidence/S02/checks.json). Rollback: revert this commit and regenerate.

## 0.2.1, 2026-10-24, S02 finish

- CLAUDE.md gained three tool quirks found while finishing S02: apply gates run on Linux through `scripts/linux-tests.sh` (Setup's readiness and Node trust differ by platform), Setup's source client must be built and dropped off the async runtime, and Python heredocs with backslashes can fail silently in this shell.

Evidence: the S02 finish session. Rollback: revert this commit and regenerate.

## 0.2.0, 2026-10-24, self-maintained guidance (ADR-024)

- CLAUDE.md is now the canonical instruction file and AGENTS.md its generated byte-identical copy. The validator and `sync_skills.py --check` fail on any difference.
- CLAUDE.md gained the rules learned so far: the low-friction product stance, Windows and Git line-ending handling, the shared Cargo build cache and disk limits, never restarting the maintainer's Docker Desktop, worktree creation from the repository root, subagents returning Markdown as text, model choice for subagents, GitHub test-repository and allowlist rules, pre-approved upstream pull requests and Anthropic probe use, and asking questions up front.
- New skill `delegate-parallel-work` for worktrees, subagent briefs, merging and cleanup.
- `capture-engineering-lessons` now applies lessons directly and creates skills, keeping only the authority limit.

Evidence: the S01 and S02 sessions of 2026-10-24 and the maintainer's instructions recorded in ADR-021 to ADR-024. Rollback: revert this commit and regenerate.

## 0.1.3, 2026-10-24, S01 handoff

- S01-L06, promoted automatically under ADR-023: classify provider quota on error type and code. OpenAI exhausted credit (429, `insufficient_quota`, `credit_balance_exhausted`) is a non-retryable quota stop. Destination: SPEC-002. Test: `live_credit_exhausted_is_quota_not_retryable`, passing on 2026-10-24.
- `validate-stage` and `capture-engineering-lessons` became agent-invocable and `start-implementation-stage` and `audit-hoi4-integrations` stayed maintainer-only, by maintainer decision ADR-023. Generator and validator changed together.
- The package validator now expects REQ-001 through REQ-050, after ADR-022 added the two memory requirements.
- 22 other S01 candidates are recorded under `engineering/knowledge/candidates/` and are not accepted. Nine wait for a regression test, nine are captured only, one is rejected and three were applied by maintainer decision.

Evidence: [S01 learning review](../../evidence/S01/learning-review.md) and [checks](../../evidence/S01/checks.json). Rollback: revert the S01 commit.

## 0.1.2, 2026-10-24, maintainer revision and repository setup

- The package validator now expects REQ-001 through REQ-048, after the maintainer decisions added three requirements.
- The README says to set `PYTHONUTF8=1` on Windows. Without it the validator reads files in the locale encoding and fails on UTF-8 text. This was observed with Python 3.13 on Windows 11. The default `python` on that machine was 3.9, which also lacks `tomllib`.
- Added `.gitattributes` to keep text files LF. Python text-mode writes and some editors on Windows converted whole files to CRLF during the revision.
- Distribution-mode validation no longer matches the revised files. The frozen manifest describes the original v02 package, kept as the first commit.

Evidence: validator runs recorded in the revision session, the [maintainer decision record](../02-architecture/maintainer-decisions-2026-10-24.md) and the repository history. Scope: planning tooling and repository hygiene. No runtime, provider or game result is implied.

## 0.1.1, 2026-10-23, working project layout

- Organized the project under one stable `hoi4-factory/` root with all planning documentation in `docs/` and maintenance utilities in `scripts/`.
- Updated project entry points and stage handoffs to use the extracted folder directly.
- Rebased canonical workflow links when generating client copies, because canonical sources and generated skills now have different directory depths.
- Separated frozen distribution validation from explicit working-tree checks so ordinary implementation files do not invalidate the planning check.

Evidence: [project layout review](../05-verification/PROJECT-LAYOUT-REVIEW.md), the updated scripts, the regenerated skill manifest and the current package verification receipt. Scope: file organization and navigation. No runtime, provider or game result is implied.

## 0.1.0, 2026-10-23, planning starter

- Added shared AGENTS authority with explicit Claude import.
- Added four canonical workflow bodies with Claude, required Codex and current `.agents` discovery projections.
- Added complete 196-field source configuration mapping and dormant MCP definitions.
- Recorded client discovery and schema compatibility findings from current official documentation.
- Added candidate review, counterexample, applicability, expiry and rollback rules.

Evidence: [migration report](CONFIG-MIGRATION-REPORT.md), [instruction map](SKILLS-AND-INSTRUCTION-MAP.md), and [package quality review](../05-verification/PLANNING-PACKAGE-QUALITY-REVIEW.md). Status: planning artifact authored and structurally checked as recorded by package QA. No client load, model trigger, MCP runtime or game result is implied.

## Future entry format

Record date and revision, candidate ID, changed guidance, reason, affected clients and versions, actual evidence, reviewer and authority, compatibility impact, superseded guidance and rollback revision. If a stage yields no reusable lesson, record that in its learning review instead of adding a ceremonial changelog entry.
