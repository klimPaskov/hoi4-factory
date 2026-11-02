# Engineering knowledge changelog

This is the versioned index of accepted engineering guidance changes. Detailed candidates retain evidence and review records. It is separate from product runtime memory and game design canon. Follow [the policy](REUSABLE-KNOWLEDGE-POLICY.md).

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
