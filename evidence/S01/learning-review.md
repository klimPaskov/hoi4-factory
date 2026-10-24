# S01 engineering learning review

Stage S01. Date 2026-10-24. Author: Claude Code subagent draft, parent review pending. Owner: maintainer. Base revision `290af7c48ce160d4f7461b90aedc92528295330b` plus uncommitted S01 changes. Policy: [REUSABLE-KNOWLEDGE-POLICY.md](../../docs/07-developer-workflow/REUSABLE-KNOWLEDGE-POLICY.md) and ADR-023.

## Summary

23 candidates were considered. One was promoted automatically under ADR-023 (L06). Nine need a regression test first. Nine are captured only. One is rejected. Three were resolved by the maintainer or by ADR-023 and are already applied. EXP-01 lessons are L19 to L22. No test was re-run for this review, so "passing" below means "recorded as passing in the cited report".

## Candidates

### S01-L01 Package validator needs Python 3.11+ and PYTHONUTF8=1 on Windows
- Evidence: `docs/07-developer-workflow/KNOWLEDGE-CHANGELOG.md` (0.1.2), `evidence/S01/config-review.md` (Python row), `README.md` line 52, `scripts/validate_package.py` line 18 (`import tomllib`).
- Status: observed on Python 3.13.5 (works) and 3.9.12 (no `tomllib`). Only the README states it. No test enforces it.
- Bounds: Windows 11, locale cp1252, validator at this revision.
- Counterexample: Linux or macOS with a UTF-8 locale needs only the version rule.
- Destination: probe test. Add a version check and an explicit encoding check to `scripts/validate_package.py`.
- Disposition: tested, needs regression test. The README fact was applied as a routine factual correction.

### S01-L02 Python text-mode writes turned LF files into CRLF
- Evidence: KNOWLEDGE-CHANGELOG 0.1.2, `.gitattributes`.
- Status: observed during the revision session. No automated check exists.
- Bounds: Windows Python text mode and some editors. `*.csv` is intentionally `-text`.
- Counterexample: a file that must stay CRLF, such as the CSV field map.
- Destination: probe test. Make the validator fail when a tracked text file other than `*.csv` contains CR.
- Disposition: tested, needs regression test. `.gitattributes` is already applied.

### S01-L03 Git for Windows system `core.autocrlf=true` rewrites CRLF on commit
- Evidence: `evidence/S01/exp04-github/report.md` (incident item 2), `probes/github/app_probe.py` lines 119 to 124. A pushed and read-back CRLF file was identical (sha256 `766cb9e7...`).
- Status: reproduced and fixed in the live probe. It is a probe, not a CI test.
- Bounds: Git for Windows 2.x with the default system config. A Linux image avoids the default but must still set the value.
- Counterexample: a workflow that wants eol conversion, which no HOI4 mod workflow does.
- Destination: SPEC-005.
- Disposition: tested, needs regression test. The test belongs in the server Git wrapper, which does not exist before S02.

### S01-L04 An all-repositories App installation defeats a repository-scoped token
- Evidence: `exp04-github/report.md` (incident item 1, design consequences). Issue #1 was created in the public `klimPaskov/hoi4-factory`. The same token was refused (404) on the private probe-b.
- Status: observed once. The final run's guard `installation_is_selected_repositories_only` passed, but no allowlist test exists.
- Bounds: GitHub as of 2026-10-24, public repositories under an all-repositories installation.
- Counterexample: a private repository outside the list, which was correctly refused.
- Destination: SPEC-005. The host enforces its own per-project repository allowlist and requires `repository_selection == "selected"`.
- Disposition: tested, needs regression test. It changes access roots and external effects, so owner approval is also required. See the maintainer section.

### S01-L05 GitHub returns 200 on a repeated merge and 422 on a duplicate pull request
- Evidence: `exp04-github/report.md` (final run table, incident item 3).
- Status: reproduced in the final run (14 of 14 checks).
- Bounds: GitHub REST behavior on 2026-10-24. This could change without notice.
- Counterexample: a branch-protection failure on merge returns 405, so codes still carry some information.
- Destination: SPEC-005 reconciliation rules. Read state, never infer from status.
- Disposition: tested, needs regression test (an idempotence test in the reconciler).

### S01-L06 OpenAI exhausted credit is 429 with type `insufficient_quota` and code `credit_balance_exhausted`
- Evidence: `evidence/S01/exp03/report.md` (defect section), `crates/agent/src/openai.rs` line 405, unit test `live_credit_exhausted_is_quota_not_retryable` using the provider's real values. The original misclassification is kept in `runs/live-record-quota-refusal.json`.
- Status: the test is recorded as passing. A live re-check was not run because there is no OpenAI credit.
- Bounds: OpenAI Chat Completions error shape as observed on 2026-10-24.
- Counterexample: a genuine rate-limit 429 must stay retryable (covered by `status_classes`).
- Destination: SPEC-002 (provider error classes). The code fix already exists.
- Disposition: accepted, promoted automatically under ADR-023. The test passed again during validation on 2026-10-24. The change only narrows paid retries and grants no authority, so it is inside the ADR-023 bound. Destination applied: SPEC-002 "Authentication, fallback and spend" and KNOWLEDGE-CHANGELOG 0.1.3.

### S01-L07 An SSE stream that ends without its completion marker is an error
- Evidence: `exp03/report.md` (RT-03 rows, provider crate comparison), `crates/agent/src/sse.rs` line 152 (`reports_partial_tail`).
- Status: the project adapter was tested by recorded Anthropic replay. The async-openai 0.42.2 and eventsource-stream 0.2.3 claims come from reading source only. No OpenAI stream was recorded.
- Bounds: those crate versions. The OpenAI `[DONE]` case is unverified.
- Counterexample: a provider that closes the stream cleanly after a complete message with no marker.
- Destination: SPEC-002 plus a Cargo test that needs no Docker or database.
- Disposition: tested, needs regression test. The existing unit test covers only the partial tail.

### S01-L08 Anthropic stream data lines carry whitespace padding that replay must keep
- Evidence: `exp03/report.md` (candidates section), `probes/runtime/recordings/`.
- Status: stated by the report. No test asserts it.
- Bounds: `claude-haiku-5-5`, `anthropic-version: 2026-10-21`.
- Counterexample: other models may not pad.
- Destination: probe test that replays a recording byte for byte.
- Disposition: captured.

### S01-L09 hoi4-agent-tools rewrites bytes 0x81, 0x8D, 0x8F, 0x90 and 0x9D to 0x9D, and applies over a stale base
- Evidence: `evidence/S01/exp02/report.md` (findings 3 and 4, go table). The 5 invalid-UTF-8 edits and the stale semantic edit were NO-GO. Two full runs gave identical hashes.
- Status: reproduced twice with the oracle in `probes/focus/oracle/oracle.py`.
- Bounds: commit `25f5641bbc07c65a822ef576ac6e92bd12666880`, package 4.3.0, iconv-lite 0.7.3.
- Counterexample: valid Windows-1252 and every valid UTF-8 variant were GO.
- Destination: SPEC-009. Add host guards that reject invalid encodings and compare `provenance.sourceHash` immediately before each call.
- Disposition: tested, needs regression test (host guard, S04). Upstream or guard-only is a maintainer decision.

### S01-L10 Claude Code subagents here cannot create new `.md` files
- Evidence: none in the repository. It was observed in this session and is reported by the parent.
- Status: not reproduced and no client version recorded.
- Bounds: this desktop environment only.
- Counterexample: the parent session may have a different tool policy.
- Destination: `start-implementation-stage` skill body, as a delegation note.
- Disposition: captured. Needs a version-stamped reproduction.

### S01-L11 `disable-model-invocation` on all four skills blocked automatic validation and capture
- Evidence: `config-review.md` (instruction and skill loading), `scripts/sync_skills.py` line 28, `scripts/validate_package.py` line 254.
- Status: resolved by ADR-023. `sync_skills.py --check` passed, and the client then listed both skills as model-callable.
- Bounds: Claude Code 2.1.304.
- Counterexample: starting a stage must stay maintainer-only.
- Destination: SPEC-012 and the instruction map (already updated).
- Disposition: captured, applied by maintainer decision. It is not an automatic promotion because it changes invocation authority.

### S01-L12 A user-scope MCP server stayed connected despite the project disable list
- Evidence: `config-review.md` (MCP connections). `hoi4_agent_tools` and `gptimage` connected from user scope.
- Status: observed, not isolated. S01 probes did not use them.
- Bounds: Claude Code 2.1.304.
- Counterexample: a clean user profile has no such server.
- Destination: `ENVIRONMENT-BOOTSTRAP-AND-CHECKS.md`, as a check of all MCP scopes before a stage.
- Disposition: captured. It touches tool authority, so it needs owner review.

### S01-L13 Backslash escapes in heredoc text were altered between bash and Python
- Evidence: none in the repository (session observation).
- Status: no minimal reproduction recorded.
- Bounds: this Windows Git Bash and Python combination.
- Counterexample: a quoted heredoc delimiter or a PowerShell here-string may not alter text.
- Destination: none until reproduced. Then a short CLAUDE.md note to use the Edit tool.
- Disposition: captured.

### S01-L14 A successful focus rewrite always writes `<file>.focus-plan.json` beside the source
- Evidence: `exp02/report.md` (finding 2).
- Status: reproduced in all applied rewrites. Blocked calls wrote nothing.
- Bounds: the pinned commit.
- Counterexample: the no-op leaves the source byte-identical but still writes the sidecar.
- Destination: SPEC-009 declared outputs.
- Disposition: captured. Whether the sidecar may live in the mod is a maintainer decision.

### S01-L15 Focus inspection can succeed while the plan is incomplete
- Evidence: `exp02/report.md` (findings 7 and 8, parser limits). A stray brace silently dropped later focuses, and one missing anchor blocked writes to the whole tree.
- Status: reproduced on the synthetic fixtures.
- Bounds: the pinned commit.
- Counterexample: well-formed trees imported fully.
- Destination: SPEC-009. Treat inspection success as not proof of completeness.
- Disposition: captured.

### S01-L16 Negative-authorization probes may only target disposable private repositories
- Evidence: `exp04-github/report.md` (incident item 1).
- Status: the final run followed the rule. It is a process rule, not a test.
- Bounds: any live external-service probe.
- Counterexample: none found.
- Destination: AGENTS.md or `start-implementation-stage`.
- Disposition: captured. It changes external-effect policy, so it needs owner review.

### S01-L17 Single clean-build timings cannot compare the provider crates
- Evidence: `exp03/report.md` (comparison table). Identical runs ranged from 106.7 to 342.5 s.
- Status: measured, three runs each.
- Bounds: one Windows host under load.
- Counterexample: none.
- Destination: none.
- Disposition: rejected. It is already in the EXP-03 report and is too narrow to reuse.

### S01-L18 Filename scans of an image listing give false positives
- Evidence: `evidence/S01/exp04/report.md` (filesystem proof). `usr/lib/ssl/cert.pem` and `usr/share/gcc/python` matched wrongly.
- Status: reviewed by hand in `runs/review.out`.
- Bounds: `probes/runtime/scan_image_listing.py`, which matches names only.
- Counterexample: none found.
- Destination: probe test with an allowlist for those two paths.
- Disposition: captured.

### S01-L19 A random-name placement temp file survives a hard kill and rollback in Setup
- Evidence: `evidence/S01/exp01/report.md` section 1, `trials/summary.json`, `patch/fix-placement-temp.diff`.
- Status: reproduced in 17 of 20 Windows and 20 of 20 Linux apply-kill trials. A local fix that binds the temp name to the transaction and operation restored exactly in 20 of 20 per side. Not upstream.
- Bounds: HOI4-Mod-Setup `4488a418`.
- Counterexample: kills that land between files restore exactly without the fix.
- Destination: HOI4-Mod-Setup upstream fix plus regression test, and SPEC-001 (apply stays blocked until then).
- Disposition: tested, needs regression test upstream. Maintainer decision on landing the fix.

### S01-L20 A digest-only inventory failure means extra files, not changed ones
- Evidence: `exp01/report.md` section 1. `assertOriginals` checks every original file first, so reaching the digest assertion proves all originals matched.
- Status: derived from the upstream script and confirmed by the reproduction.
- Disposition: captured. Destination: test-writing guidance for preservation oracles.

### S01-L21 Cross-platform build and container pitfalls
- Evidence: `exp01/report.md` section 7.
- A `#[path]` that passes through a missing directory with `..` compiles on Windows and fails on Linux.
- A Docker restart can corrupt rlibs in a cargo target volume (error E0786).
- A wrong bind-mount source is silently created as an empty directory.
- Serialized Setup plans holding raw bytes reached 188 MB each.
- Status: each observed once. No reproduction scripts.
- Disposition: captured. Destination: a probe-environment checklist in the S02 packet.

### S01-L22 Setup on a server needs readable ancestor directories and an explicit state root
- Evidence: `exp01/report.md` section 3. `RootedDir` opens every ancestor for reading, so apply fails with EACCES under a 0711 account directory. `run_transaction` falls back to desktop data paths without `app_data_root`.
- Status: reproduced in the Linux probe.
- Destination: SPEC-001 and the S02 Setup adapter.
- Disposition: tested, needs regression test in the S02 adapter.

### S01-L23 Adding a workspace member silently broke the server image build
- Evidence: `evidence/S01/checks.json` (validation, server rebuild). The probe crate joined the root workspace during EXP-03, and `docker/server.Dockerfile` copied only `crates/`, so Cargo failed with "failed to load manifest for workspace member". Found only because validation rebuilt the image.
- Status: reproduced and fixed by copying the probe manifest and source into the build stage. No automated check builds the images.
- Bounds: any Cargo workspace whose images copy a subset of members.
- Counterexample: an image built from the whole repository context.
- Destination: regression test. Add an image build of every Dockerfile to the stage validation commands from S02.
- Disposition: tested, needs regression test.

## Already applied in S01

- KNOWLEDGE-CHANGELOG 0.1.2: validator expects REQ-001 to REQ-048 (three requirements added), README documents `PYTHONUTF8=1`, `.gitattributes` added, and distribution-mode validation is known stale against the revised files.
- `.gitattributes`: `* text=auto eol=lf` with `*.csv -text`.
- Skill invocation split: `MANUAL_ONLY = {"start-implementation-stage", "audit-hoi4-integrations"}` in `sync_skills.py`, with a matching check in `validate_package.py`.
- A new changelog entry (proposed 0.1.3) should record any promotion made from this review. The only one eligible is L06.

## Maintainer decisions needed

1. L04: require selected-repository installations and a host-side allowlist (access roots and external effects).
2. L06: confirm that narrowing paid retries is not a "paid operation" change.
3. L09: fix the encoding and stale-base behavior upstream, or rely on host guards only.
4. L12 and L16: approve the MCP scope check and the disposable-repository rule as standing guidance.
5. L14: allow the focus-plan sidecar inside the mod, or relocate it.
6. L19: land the Setup placement-temp fix upstream with a regression test, which unblocks production apply.

Next review trigger: the S01 EXP-01 report, then S02 start.
