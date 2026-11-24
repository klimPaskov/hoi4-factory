# Engineering lessons behind the CLAUDE.md rules

[CLAUDE.md](../../CLAUDE.md) keeps each rule as a short line (D-42). This file keeps the incident or measurement that produced a rule, with its date, so the reason can still be found. It holds no rules of its own. If a story here and a rule in CLAUDE.md ever disagree, CLAUDE.md wins and this file is fixed.

When a new lesson adds a rule, put the rule in CLAUDE.md and the story here, under the heading that matches the rule's section.

## Execution and safety

### ZIN loaded in a game check

CLAUDE.md rule: before asking for an in-game check that loads another project's mod, read that mod's own CLAUDE.md or AGENTS.md. ZIN forbids launching or capturing the game with ZIN loaded, and one check loaded it anyway because its rules were not read first.

## Working environment

### CRLF blobs from a worktree checked out under `core.autocrlf=true` (Chaos Redux #4)

CLAUDE.md rule: pass `-c core.autocrlf=false` to the checkout and worktree commands as well as the commit, and never use `commit -a` in another repository. A worktree checked out under the system setting has CRLF working copies. A later `git -c core.autocrlf=false commit -a` then stages every one of them as CRLF blobs wherever no `.gitattributes` rule normalises the path. Chaos Redux #4 rewrote 119 files that way.

### Platform-sensitive host tests

CLAUDE.md rule: run workspace tests on the other machine, or on Linux, before trusting a platform-sensitive change. Git follows a repository's `include.path` while discovering it, which is why untrusted configuration is read without discovery (`GitRepo::config_findings`). `/etc/passwd` exists on macOS and Linux but not on Windows, so an upload test passed only on Windows.

### Disk space on the Windows machine (2026-11-02 to 2026-11-05)

CLAUDE.md rules: clean up before reporting done, and keep build copies and probe output small. Between 2026-11-02 and 2026-11-05 free space on C: fell from 165 GB to 7 GB. A Linux copy of the game and Steam took 45 GB in Ubuntu, the Docker build cache grew to 17 GB, a crashing packaged Electron test left 8.7 GB of dumps in `%TEMP%\wsl-crashes`, and finished worktrees left Rust build copies behind. Space freed inside WSL or Docker returns to C: only after the maintainer compacts `ext4.vhdx` or `docker_data.vhdx`.

### A broad process match stopped other sessions' jobs (2026-11-06)

CLAUDE.md rule: stop only processes you started, matched by PID or your own session folder, never by a broad command-line pattern. On 2026-11-06 a match on "scratchpad" stopped two other sessions' background Blender jobs.

### Python REPL loop filled the disk (2026-11-06)

CLAUDE.md rule: never start Python without a script, `-c` or `-m`, above all in a background task. Python 3.13's REPL started without a console prints the same `_pyrepl` traceback in a loop. On 2026-11-06 one background task wrote a 50 GB output file under `%TEMP%\claude` and left C: with 2 GB. `scripts/py.sh` refuses to run with no arguments and sets `PYTHON_BASIC_REPL=1`.

### Git over HTTP after a `401` (S02)

CLAUDE.md rule: tests that only need a failed Git request make the fake GitHub answer `500` (`git_error_after`), and keep `401` for the revocation cases. Git over HTTP stalls after a `401` even with prompts disabled, until the caller's timeout, so an S02 intake test took 178 s on Windows.

### Lease-timing suite under load

CLAUDE.md rule: on a busy Windows machine run the lease-timing suite with `cargo test -j 2 -p factory-server --test agent -- --test-threads=2`. With the default thread count it passed 20 of 20 on a quiet machine and failed 1 to 10 tests under 70% to 90% CPU from other sessions, because its 1 s lease has no room for a stalled heartbeat. The Docker PostgreSQL flags (`--tmpfs`, `fsync=off`) were added because Docker commits were slow on the full disk. A child started after `env_clear()` needs `SystemRoot` or its sockets fail with "connect failed before request was sent".

## Delegation and parallel work

### Temporary output outside the worktree

CLAUDE.md rule: agents keep temporary output only in their worktree, `.work/` or the session scratchpad, and never delete anything outside those places. One agent once wrote golden-test output to a folder in the user's home directory and later deleted it without remembering that it had created it.

### Pushing `main` from a shared checkout (2026-11-02)

CLAUDE.md rule: give each session that commits its own worktree, or check `git log origin/main..main` before pushing. On 2026-11-02 two pushes of small decision commits published the UI design track's local commits.

### Desktop app crashes (2026-11-08)

CLAUDE.md rule: run long work as separate sessions, not as background subagents inside the coordinator's process, and archive each session when it finishes. On 2026-11-08 the app restarted five times. About ten live sessions with their MCP servers reached 11.5 GB of process memory and its governor evicted sessions, and an old 1 GB transcript was being reloaded. The earlier advice to keep about six sessions live was dropped by D-31 and D-34.

### Model and effort for new sessions (2026-11-02)

CLAUDE.md rule: start a new Claude Code session on Sonnet at medium or high effort, and use Opus at medium effort for harder tasks. This is a maintainer preference of 2026-11-02, and a new session does not inherit the starting session's model and effort.

### Another family's lead archived eight working sessions (2026-11-23)

CLAUDE.md rule: archive only sessions your own family started, and only after reading the last report. A merged pull request does not mean a session is finished. On 2026-11-23 the maintainer asked two coordinators to clean old and stale chats. The ZIN lead archived eight of the HOI4 coordinator's sessions because each had a merged pull request: seven 3D workers and the Agent Tools integrator. All of them still had work, and the coordinator had just sent them resume messages. The coordinator first blamed its own archive of an unrelated session, wrote a wrong rule about a cascade, and corrected it once the ZIN lead's report arrived. It unarchived all eight, and their worktrees were intact.

### Two heavy jobs at once (2026-11-21)

CLAUDE.md rule: take the heavy-job slot only through `scripts/heavy-slot.sh`. The slot had two signals, a hold file written by hand and the `blender.lock` folder taken by the 3D lane's Blender gate. On 2026-11-21 the 3D gate took the lock at 19:19:37 and a ZIN worker wrote its hold 23 seconds later, because it checked only the hold file. A 40-minute build started beside a Blender run with 10 GB of RAM free, and the build had to be killed. Earlier the same day a release benchmark rerun also ran while the lock existed. One script that checks both signals and takes the slot atomically replaces the by-hand protocol.

### Parallel crate packets (S03)

CLAUDE.md rule: commit empty crate skeletons and their workspace members on main before parallel crate packets, and give each brief the public API. In S03 four Sonnet packets built this way merged without conflicts and needed only small adapter changes.

## External services

### Two image routes, one sign-in (2026-11-08)

CLAUDE.md rule: make every HOI4 image with hoi4-imagegen, never the general `gptimage` MCP. hoi4-imagegen paces its ChatGPT sign-in requests across all its processes on the machine, while `gptimage` uses the same Codex sign-in outside that limit. On 2026-11-08 both ran at once and OpenAI revoked the sign-in at 15:29 UTC.

### A pull request opened on the upstream of a fork (2026-11-09)

CLAUDE.md rule: pass `--repo` and `--base` to `gh pr create` in a forked repository. In hoi4-imagegen-mcp, a fork, `gh pr create` without them opened the colour PR on the upstream parent project. The session closed it within a minute and reopened it on our repository, but the closed PR stays visible there.

### Kit push that broke `main` on the quality ratchet

CLAUDE.md rule: before an approved push to another repository, run that repository's own CI steps locally, not a hand-picked subset. A wiki push once broke the kit's `main` on the quality ratchet, which only the full list runs.

### Setup component markers (2026-11-06)

CLAUDE.md rule: before merging a kit PR that moves, splits or rewrites files with Setup component markers, or changes the package manifest, run Setup's offline acceptance against the branch. On 2026-11-06 a context-budget PR moved text into skill references and left two files ending with a Setup component START marker and no END. Setup refuses such a file, so installs with the released Setup failed until a fix-forward PR landed.

## Agent runtime rules (learned in S03)

The rules in this section of CLAUDE.md came from the S03 stage and S04. Their reasons are in the rules themselves, apart from this one.

### A blocked Agent Tools rewrite taken as applied (S04)

CLAUDE.md rule: treat any status other than `ok`, or an unchanged source file, as a refusal before writing anything that depends on the rewrite. In S04 a blocked rewrite (`FOCUS_GAMEPLAY_REFERENCE_MISSING`) was taken as applied and the host appended localisation for a focus that was never written. The fix is in `focus/session.rs` with a regression test in `tests/focus.rs`.

### A dropped `run_turn` cut a frame in half (2026-11-13, issue #13)

CLAUDE.md rule: queue whole frames to a writer task that owns the worker's stdin. On 2026-11-06 the test `a_late_result_for_an_abandoned_request_is_rejected_and_counted` failed about once in five runs on Windows, because its 1 ms timeout dropped `run_turn` in the middle of `write_frame`. The worker then read the rest of the frame as a new length prefix and exited with "frame is not valid protocol JSON". The new test `dropping_run_turn_mid_write_never_corrupts_the_next_frame` drops a 4 MB turn 24 times at different points. It failed in round 0 on the old supervisor and passes with the writer task. The same test first expected the next turn to run straight away, which timed out because the worker ignores a `RunTurn` during a turn, hence the cancel rule.

### A recovery test that raced the replay server (2026-11-13)

CLAUDE.md rule: wait until the replay server has counted the request before killing the host mid-turn. `an_interrupted_write_is_reconciled_after_a_restart_and_never_rerun` failed about one run in four on Linux, on main as well. It aborted the runtime as soon as the `model_request` event appeared, sometimes before the worker had sent the request. The reopened attempt then got the 4-second delayed write turn meant for the dead attempt and ended `fenced`. With the wait it passed eight runs in a row.

### Formatting churn from `cargo fmt` (S04)

CLAUDE.md rule: format only your own new files with `rustfmt --edition 2024 <file>`. `cargo fmt -p factory-server` rewrote twelve untouched files.

### A list of trees that laid out the whole game (2026-11-14)

CLAUDE.md rule: scope each Agent Tools call to a file or a tree. The first list route for a mod's focus trees called `hoi4.focus_inspect` once with no scope. Tests passed in seconds, because the test workspace has no game folder. On a real stack with the game loaded as reference, the same call laid out every focus tree of the game and the page hung for minutes. The route now inspects each focus file of the mod on its own, and the first call takes about 11 seconds.

### A summary page that showed the agent's whole report (2026-11-14)

CLAUDE.md rule: text a model wrote is bounded before a page shows it. In the first live run of the ask bar the agent's summary was a long report with ids, file paths and tool names, and the editor showed all of it above Approve. The agent is now asked to open with one plain sentence and the pages show the first sentence only. On 2026-11-15 that sentence ended inside a quotation and the cut missed it, so the cut also accepts a closing quote or bracket.

### A re-pin that listed only the core tools, and a count nobody ran (2026-11-15)

CLAUDE.md rule: the host starts Agent Tools with `HOI4_AGENT_TOOLS_PROFILES=all`, and the whole `factory-hoi4-tools` crate runs against the real server after a profile or pin change. From 4.13.0 the stdio server lists only its core profile by default, so the first tool list for the 4.18.0 re-pin was far shorter than the pinned one. In the same re-pin the real-server test still expected nine tools for the model. The profile had offered ten since the game-view render was added, and only the server suites had been run against the real tools since then.

### A test database killed halfway through a run (2026-11-15)

CLAUDE.md rule: a test database on `--tmpfs` gets at least 3 GB for a full workspace run. The first workspace run on the 4.18.0 pin showed 78 failures. The PostgreSQL container had a 1 GB memory cap, its data lived in memory, and it was killed partway through. Every later database test failed with "failed to lookup address information". With 3 GB the same run had one failure, a real one.

### A Linux VM that held 6 GB of file cache (2026-11-18)

CLAUDE.md rule: a job that reads large files in WSL releases them from the file cache as the ordinary user when it ends, and root in the distro needs the maintainer's yes. On 2026-11-18 two ZIN game captures were stopped by the memory guard before the game started, and then every Blender step of the 3D lane waited at its memory gate, although no game and no Blender was running. Commit stood at 48.0 GB and free RAM at 2.8 GB. Inside the VM only 1.2 GB was in use and 6.6 GB was file cache, left from hashing the game copy during the capture's sync. Dropping the cache and compacting memory as root took commit to 41.4 GB and free RAM to 9.8 GB within a minute, and `vmmemWSL` fell from 7.8 GB to 2.0 GB. Nothing is lost, because the cache only mirrors files on disk. That one drop ran as root in the distro, which no agent had been given, so the rule became the route without root. ZIN's sync now releases each file from the cache as it checks it, and the VM stayed at 2.0 GB through a full sync.

## Verification and handoff

### A queue that blocked its own waiters (2026-11-24)

CLAUDE.md rule: the 3D lanes' gated steps start first come, first served. The first version of the queue in `heavy-slot.sh kept` looked for the nearest ancestor whose command line was `gated.sh`. But gated.sh asks from a command substitution, and that forked subshell has the same command line. So the code found the fork, which is never in the waiting list, and told every waiter "not yet listed, keep waiting". From about 22:40 on 2026-11-23 to 04:48 the next morning, almost no 3D step started, while the pool showed free slots and the memory lines passed. The test had used a stand-in with the same fork structure, and its "not yet listed" answer was misread as expected. The fix takes the topmost matching ancestor and leaves forks out of the list. Lesson: when a test's answer says "not found", check that it found the right process before accepting it, and watch the pool for a few minutes after changing it.

### Obvious faults shown for review (2026-11-23)

CLAUDE.md rule: fix every obvious fault before the maintainer sees the work, and never show a known fault with an explanatory caption (D-144). On 2026-11-23 pictures reached the maintainer for review still carrying faults that the agents could have seen themselves: an interface renderer's base-game titles and placeholder numbers, mismatched frames, an event picture that did not fill its frame, off-centre buttons, a figure with three arms, creatures off the project's accepted look, and characters off canon. A coordinator also relayed a ship picture as fine without a full-size look of its own. The rule now covers everything an agent shows or relays, and the modding kit carries it too.

### A factory redrawn until it passed (2026-11-18)

CLAUDE.md rule: keep work detailed, and never simplify without the maintainer's consent. In the 3D route work the test factory was a brick works with a lattice water tower and saw-tooth roofs. The generator's low-poly mesh had defects on it, so the session redrew it with a brick tower, and after a second failed review as one plain hall with one chimney, which passed. The landmark was redrawn simpler once and the tank twice in the same way. The coordinator relayed all three as passed types without checking which reference each run had used. The maintainer saw the second factory picture and rejected the practice. The three passes were withdrawn, the kit pull request went back to draft, and D-87 and D-88 set the rule: the method changes, the design stays, and a pass on a simplified input is not a pass.

### Seven ZIN units in a trailer (2026-11-06)

CLAUDE.md rule: visual work is not finished until someone has looked at it, at viewing size and in close-up, and a fresh-context reviewer has been asked to find every defect. On 2026-11-06 seven ZIN units passed every geometry check with 0 errors and went into a trailer cut together with existing Chaos Redux units. The maintainer saw holes and deformed animations in both at first glance.

### Docker Desktop that would not start again (2026-11-14)

CLAUDE.md rules: leave Docker Desktop running once it is up, and run the client check in WSL when Docker is down. On 2026-11-14 an S04 design session started Docker Desktop for the dev server and stopped it again with `docker desktop stop` after each render, to free memory. The third start crashed in the backend with "initializing Inference manager: listening on unix://...\Docker\run\dockerInference". The socket file from the earlier run was still there, and Windows answered error 1920 to every attempt to delete it. The session then ran lint, the type check, the 96 client tests and the build in the WSL Ubuntu from a copy of `web/`, because Vitest could not write its temporary config file under `/mnt/c`.

### A unit test that grew to 18 GB (2026-11-14)

CLAUDE.md rule: local tests run under a memory cap and a timeout, and a loop-driven test uses a fake clock and an iteration cap. On the night of 2026-11-14 a HOI4 Live Tools unit test for the exit-to-menu flow used a mock game session with a real clock. A regex bug in the code under test meant the exit loop never ended. The loop spun for its 240 seconds of real time, and the mock recorded every call, so the process grew by about 4 GB a minute to 18 GB. Free memory fell to 1.26 GB with the maintainer asleep, and another session stopped that one process seconds before a third out-of-memory crash. The fix replaced the mock with plain callables, a fake clock that every sleep advances and a cap of 400 screen reads. The same file now runs in 6 ms, and that session runs local tests under a 1.5 GB cap and a 60 second timeout.

### The heavy-job slot on a busy night (2026-11-22)

CLAUDE.md rules: take a slot only when the job is ready, check the take's exit code directly, kill a stopped waiter's whole process tree, and run heavy jobs through the slot pool. On the night of 2026-11-21 to 2026-11-22 the single slot was shared by the 3D lane, ZIN, Agent Tools and the game runner. A take whose timeout was hidden by a pipe let one benchmark run without the slot, and its numbers were thrown away. Two lanes held the slot for an hour each with nothing running: one took it before its job was ready, and a stopped waiter left an orphaned take child that took the slot later. Waiters polling every 15 seconds also raced, so one lane lost every turn for hours. On 2026-11-22 the maintainer allowed more heavy jobs at once (D-113). `scripts/heavy-slot.sh` became a pool of two slots with an exclusive mode for game launches and quiet benchmarks, and `gated.sh` takes its Blender steps from the same pool. The Blender gate also waited on any hold, its own lane's included, so a 3D lane that took a hold before its run would have blocked itself. A 3D run now takes no slot of its own.

CLAUDE.md rule: green means every required check passed for the PR's current head commit. On 2026-11-22 a wait loop on `gh pr checks` treated "no checks reported" right after a push as "nothing pending", and the kit's PR 151 was merged before its head was tested. That head's pull-request run had started no job, because the base branch moved while it was created. The Validate run on main for the merge commit passed afterwards, so nothing broke.
