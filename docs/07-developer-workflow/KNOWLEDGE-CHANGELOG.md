# Engineering knowledge changelog

This is the versioned index of accepted engineering guidance changes. Detailed candidates retain evidence and review records. It is separate from product runtime memory and game design canon. Follow [the policy](REUSABLE-KNOWLEDGE-POLICY.md).

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
