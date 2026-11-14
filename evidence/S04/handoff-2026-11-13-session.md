# S04 session handoff, 2026-11-13

This is a session handoff inside S04, not the stage's final handoff. S04 stays open. Its browser pages wait for the maintainer's design pick.

## Identity and authorized scope

- Stage ID and title: S04, the focus authoring vertical slice.
- Authorization: S04 authorized on 2026-11-08, run as one low-priority session (D-36, D-46). This session took only the work that does not wait for the design pick.
- Agent: Claude Code desktop, Claude Opus 5.5, started by the HOI4 coordinator session.
- Reviewer and review state: no human review yet.
- Repository: `hoi4-factory`, worktree `hoi4-factory-wt-s04-issues`, branch `s04-issues`, base `cfd2061`, code commit `627b576`.
- Dirty paths before work: none in the new worktree. The UI lab worktree `hoi4-factory-wt-s04-ui3` was not touched. The main checkout has an untracked `.claude/launch.json` from another session, left alone.
- Start and end: 2026-11-13, one session interrupted by a desktop app crash at about 20:30 and resumed.

## Outcome and changed artifacts

| Item | Intended result | Actual result | Evidence | Status |
|---|---|---|---|---|
| Engine binary in the image | Agent Tools engine in the server image | Already done on 2026-11-08 (T-ENG-01, gate 11 part one) | [checkpoint](checkpoint-2026-11-08.md) | pass, earlier |
| S03 leftover worktree | Removed | No S03 worktree exists any more. The only other worktree is the UI lab | `git worktree list` | not applicable |
| #13 | A dropped `run_turn` never corrupts the next frame | Writer task owns stdin and writes whole frames. New regression test fails on the old code and passes now | `I13-REGRESSION`, `I13-FLAKE` | pass |
| #15 | Test states which tool classes are exposed | Asserts the exposed names and classes (pure read and derived job, never write) | `AGENT-CRATES-WINDOWS` | pass |
| #16 | Script LF, hash re-recorded with a note | Done, the S01 manifest kept as recorded | `I16-LF` | pass |
| #12 | Bound honoured, reasoning control, 402, price | Live on DeepSeek. The price record already existed | `I12-LIVE`, [live evidence](live-deepseek-adapter/README.md) | pass |
| #6 | Each tool exposed or withheld with a reason | 11 exposed, 23 withheld. Rewrite calls in the worktree and stale-source refusal are covered by `tests/focus.rs` for focus | `I6-WITHHELD` | pass for 4.12.0 |
| Recovery test race | Not in the brief, found while testing | Fixed in the test | `RECOVERY-RACE` | pass |

Changed paths: `crates/agent/src/{supervisor,protocol,openai,request}.rs` (production), `crates/hoi4-tools/src/profile.rs` (production table and test), tests under `crates/agent/tests`, `crates/hoi4-tools/tests` and `crates/server/tests`, `probes/runtime/run-exp03.sh`, `evidence/S01/exp03/results.json`, `evidence/S04/live-deepseek-adapter/`, CLAUDE.md, AGENTS.md (generated), the engineering lessons and the knowledge changelog.

## Verification record

See [checks-2026-11-13.json](checks-2026-11-13.json). Every check is a real test run, a live call or static validation, labelled as such. Not run: the gated real Agent Tools tests on the host, because their environment variables were not set. The withheld list was checked against a real `tools/list` from the server image instead.

Remaining risk: the Windows `fake_server` process-group test hung once under heavy concurrent load and passed alone. It was not reproduced, so no issue was filed.

## Decisions and unresolved issues

Technical defaults taken under D-18:

- The compatible profile sends both output-bound fields with the same value. The OpenAI profile keeps only `max_completion_tokens`.
- Reasoning effort is per request and `None` by default, so agent runs keep the provider default until a task type chooses otherwise.
- The withheld read tools can be exposed later. Each needs a closed host schema, an output bound and a pinned schema hash first.

Issue hygiene: #12 closes with this work. #2 stays open for the funded OpenAI key only. #5 was already closed, and #6 now has its withheld list. See the issue comments.

Still open in S04: the two browser pages (maintainer's pick, questions 14 and 15), the Agent Tools re-pin after the last tool release of the stage (gate 11) with the T-BENCH-01 half of gate 10, then the stage's `validate-stage`, final handoff and GOAL-S05. Gate 10's Hagane round trip and T-IMPORT-01 were done later the same day (below).

## Effects, recovery and rollback

Effectful operations: three live DeepSeek calls, about US$0.00003. No GitHub writes apart from issue comments and closures. Reverting `627b576` restores the previous behavior. The S01 evidence keeps its original manifest, so no evidence is lost by a revert.

## Configuration checkpoint

No tool, provider, MCP endpoint, permission or client configuration changed. CLAUDE.md gained three agent runtime rules and a re-pin step, regenerated into AGENTS.md and checked with `sync_skills.py --check`.

## Engineering learning review

Accepted and applied (knowledge changelog 0.2.45): cancel-safe frame writes, cancel after a dropped turn, and waiting for the replay server before killing the host in a test. Each has a regression test. The incident stories are in the engineering lessons.

## Later the same day: T-IMPORT-01 and gate 10

The coordinator asked for gate 10 and T-IMPORT-01 next, after first checking whether either needs something only the maintainer can give. Neither does. Hagane Studio is public GPL source that A1-2 allows running locally. The W8 wind-down is not involved, because nothing in the Hagane repository changes. The GitHub import runs against the stand-in GitHub used by every S02 to S04 GitHub test.

| Item | Result | Evidence | Status |
|---|---|---|---|
| T-IMPORT-01, ZIP upload | Already covered on 2026-11-08 | `an_upload_gets_a_preservation_report_and_a_baseline` in `tests/publish.rs` | pass, earlier |
| T-IMPORT-01, GitHub import | New test `a_github_import_gets_a_preservation_report_and_a_baseline`. The imported commit gets a report with exactly one row per imported file and a known status for each. The baseline is recorded for that commit, and no byte of the workspace changes | `IMPORT-GITHUB` in [checks](checks-2026-11-13.json) | pass, against the stand-in GitHub |
| Gate 10, Hagane round trip | Measured on the same tree. Hagane takes about 0.1 s to save in its editor and about 2 s warm to write native files. HOI4 Factory takes 20 to 93 s. Hagane's export also rewrote the source and changed a modifier value | [hagane-round-trip](hagane-round-trip/README.md) | measured, and HOI4 Factory misses the 1 s target (hoi4-agent-tools#137) |
| Gate 10, T-BENCH-01 | Not run for the Factory pin. The latest nightly ran on Agent Tools 4.16.0, not the 4.12.0 pin, and only latency cases regressed. That run happened while the PC was overloaded before the crash | Agent Tools bench folder, run `2026-11-13T17-10-55Z` | waits for gate 11 |

The full `intake_api` suite then showed that `the_setup_migrations_run_in_order_after_intake` had gone stale when S04 added the `focus` migration component (`c7de79d`). It now expects that component, and the suite passes 20 of 20 on Linux.

A Hagane Studio finding is not filed anywhere. Its exported tree had `add = 10` turned into `add = 100` in the country modifier. Agents do not post to outside projects (R-12), and the Hagane repository is the maintainer's to change under D-05. It is recorded in the evidence for the maintainer.

## Next step

- Next: the re-pin to the next Agent Tools release once the stage's last tool release is out (set `HOI4_AGENT_TOOLS_PROFILES` as the checkpoint says, and classify new tools in `AGENT_TOOLS_WITHHELD`). Then the browser pages after the maintainer's design pick.
- Read first: [checkpoint](checkpoint-2026-11-08.md), this file, [maintainer questions](../maintainer-questions.md) 14 and 15.
- Pending operations needing reconciliation: none.
