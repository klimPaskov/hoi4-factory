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

## Verification and handoff

### Seven ZIN units in a trailer (2026-11-06)

CLAUDE.md rule: visual work is not finished until someone has looked at it, at viewing size and in close-up, and a fresh-context reviewer has been asked to find every defect. On 2026-11-06 seven ZIN units passed every geometry check with 0 errors and went into a trailer cut together with existing Chaos Redux units. The maintainer saw holes and deformed animations in both at first glance.

### A unit test that grew to 18 GB (2026-11-14)

CLAUDE.md rule: local tests run under a memory cap and a timeout, and a loop-driven test uses a fake clock and an iteration cap. On the night of 2026-11-14 a HOI4 Live Tools unit test for the exit-to-menu flow used a mock game session with a real clock. A regex bug in the code under test meant the exit loop never ended. The loop spun for its 240 seconds of real time, and the mock recorded every call, so the process grew by about 4 GB a minute to 18 GB. Free memory fell to 1.26 GB with the maintainer asleep, and another session stopped that one process seconds before a third out-of-memory crash. The fix replaced the mock with plain callables, a fake clock that every sleep advances and a cap of 400 screen reads. The same file now runs in 6 ms, and that session runs local tests under a 1.5 GB cap and a 60 second timeout.
