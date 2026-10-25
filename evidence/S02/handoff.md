# S02 handoff

## Identity and authorized scope

- Stage: S02, accounts, server workspaces, Mod Setup and GitHub import.
- Authorization: the maintainer invoked `/start-implementation-stage S02` on 2026-10-24, with the decisions recorded in [scope.md](scope.md). Later instructions in chat authorized merging the four HOI4-Mod-Setup pull requests ("you can merge yourself them") and said to finish S02 and not start S03.
- Implementing client: Claude Code 2.1.304 (desktop app). Parent model `claude-sonnet-5-5`, later `claude-opus-5-5`. Subagents ran on Sonnet and Haiku.
- Reviewer: the maintainer. Review requested by this handoff.
- Repository: `klimPaskov/hoi4-factory`, branch `main`. Base revision `4634d93f9fc39dca6ae02ba42cd19d6b98eb0069`. S02 is the 43 commits up to merge `78ffc1a` and the handoff commit that follows it.
- Dirty paths before work: none.
- Interruptions: the session ran out of context several times and resumed from files on disk. Docker Desktop hung once when the disk filled. The maintainer restarted it.

S03 is authorized and its inputs are recorded, but by the maintainer's instruction it was not started. Excluded from S02 and not built: model-written source, the focus canvas, collaborator invitations, push or pull request creation from the factory, public deployment, payments and memory.

## Outcome

A user can sign up with email or GitHub, verify the email through Mailpit, create a project, and then start a new mod, upload a ZIP or folder, or import a repository from GitHub. The server keeps each project in its own workspace with a Git baseline, runs HOI4 Mod Setup on it (scan, model analysis, evidence approval, plan, conflict choices, apply, recovery) and returns a C-01 descriptor. The browser client has a Setup wizard for the whole flow. Everything runs in Docker Compose with PostgreSQL, Mailpit and the read-only game mount.

| Gate | Intended result | Actual result | Evidence | Status |
|---|---|---|---|---|
| S02-G1 | New, upload and GitHub import produce a C-01 descriptor from real Setup logic, per-account, no cross-account access | `g1_new`, `g1_upload` and `g1_import` pass against merged Setup `c28198b` with recorded-real model replies. The live Compose flow produced C-01 with a real `claude-haiku-5-5` analysis (1464 operations, applied). Cross-account reads return 404 and row-level security is forced | [checks.json](checks.json), [finish-live-compose.json](finish-live-compose.json) | pass |
| S02-G2 | Protected and unrelated files stay byte-identical unless a reviewed change covers them | Both G2 tests pass: every protected, custom and unrelated file keeps its hash, and a reviewed replacement changes exactly that file | [checks.json](checks.json) | pass |
| S02-G3 | A rerun is clean, edited generated files become conflicts | `g3_rerun_is_clean_and_an_edited_generated_file_is_a_conflict` passes | [checks.json](checks.json) | pass |
| S02-G4 | Every injected interruption is classified and recoverable, restoration matches baseline | The three G4 tests pass, including a killed apply process, which leaves `rollback_required` and restores the baseline inventory | [checks.json](checks.json) | pass |
| S02-G5 | Manifest, tool closure, game build and DLC set visible, game files read-only and absent from images and exports | `/readyz` reports the game reference (HOI4 1.19.3.0, 12 DLC) as read-only. C-01 records the Setup revision. The server image filesystem has no game file, coding CLI or credential. There is no single dedicated G5 test | [checks.json](checks.json), [live-compose.json](live-compose.json) | pass, with that limitation |

Stage disposition: complete.

## Upstream work

HOI4-Mod-Setup, all merged into its main as `c28198bb9b4d3fd90cea294cf136cad357ed5bbc` with the maintainer's authorization:

- #105: placement temp files no longer survive rollback.
- #106: a library service facade with per-session state, an injected credential store and an optional `keyring` feature.
- #107: a recovery dead end at finalization and an Anthropic schema rejection.
- #108: a plan builder, and tests that no longer need a Windows-only Node runtime.

The merges used `gh pr merge --squash --admin` only for the code-owner review rule, after every required check passed. hoi4-agent-tools #42 (source-preservation guards) was authorized for merging on 2026-10-25. Its branch was brought up to date with main (one changelog conflict, both entries kept) and was waiting for CI when this handoff was written. The merge result is recorded in the next commit.

## Changed artifacts

Production code:

- Crates: `crates/accounts`, `crates/workspace`, `crates/game-ref`, `crates/intake`, `crates/setup-adapter` and `crates/server`.
- Migrations under `migrations/` (0100-0101, 0300-0303, 0400-0402).
- The browser client under `web/`.
- `compose.yaml`, `docker/server.Dockerfile`, `docker/db-init/` and `docker/test-runner.Dockerfile`.

Test fixtures with recorded real model replies are under `crates/server/tests/fixtures/recorded/` and `crates/setup-adapter/tests/fixtures/`.

Tooling: `scripts/linux-tests.sh` and `scripts/live_flow.py`.

Documentation and guidance:

- [Developer setup](../../docs/07-developer-workflow/DEVELOPER-SETUP.md), `.env.example` and the [S03 goal prompt](../../docs/06-agent-handoffs/GOAL-S03.md).
- CLAUDE.md and the regenerated AGENTS.md, plus the new skills `production-readiness-review` and `delegate-parallel-work`.
- The proportionate [production readiness checklist](../../docs/05-verification/PRODUCTION-READINESS-CHECKLIST.md).
- Status updates in the README, the package status, the roadmap and the S01 and S02 packets.

## Verification record

[checks.json](checks.json) holds the commands, versions, exit codes and test counts:

- The Windows workspace test run.
- The Linux Setup gate run in `scripts/linux-tests.sh`.
- The browser client checks.
- The Compose build with the browser client.
- The validator and the skill check.

The final Windows run first had three failures in `crates/setup-adapter`. They were stale assertions written for the pre-merge Setup pin. Two expected the #105 fix to be absent. One expected the finalization dead end that #107 removed. The tests now assert against `SETUP_PIN`, and the finalization case asserts the fixed behavior: a failure after the rollback record is a partial apply that rolls back exactly. The adapter still guards against a refused resume, turning it into manual review, but merged Setup no longer produces that state, so the guard has no test now.

Evidence kinds are kept apart: automated tests against real PostgreSQL and real Setup code, recorded-real model replay, one live model run through the whole stack, and an image filesystem scan. No game was run, because S02 reads only game metadata.

Model spend: live `claude-haiku-5-5` calls were used for the recorded fixtures, the ignored live tests and the live flow. The recorded calls total about 54 thousand input and 16 thousand output tokens. The total was not tallied separately. It is a few cents at most and is visible in the Anthropic Console.

## Decisions and unresolved issues

Accepted decisions are in [scope.md](scope.md) and ADR-011 to ADR-024 in the [decision record](../../docs/02-architecture/maintainer-decisions-2026-10-24.md). Open items, none blocking:

- The GitHub App email permission and installation scope ([maintainer questions](../maintainer-questions.md) 1 and 2).
- The hoi4-agent-tools #42 merge, after CI. S04 pins the merged revision.
- The maintainer's GitHub App changes (email permission, installation narrowed to the test repository) and deleting the old probe App and repositories.
- RustSec advisories against `rustls-webpki` and the webpki root crates, which a version bump can address.
- Per-route rate limits for upload and Setup routes (PR-098).

## Effects, recovery and rollback

External effects:

- Four merged HOI4-Mod-Setup pull requests.
- Feature branches pushed to HOI4-Mod-Setup and hoi4-agent-tools.
- Test writes to `klimPaskov/hoi4-factory-test-mod` only.
- Live Anthropic calls.

No effect has an unknown outcome. Reverting the S02 merges on `main` restores S01. `docker compose -p hoi4-factory down -v` drops the development database. The upstream merges are reverted in HOI4-Mod-Setup itself if ever needed.

## Configuration checkpoint

See [config-review.md](config-review.md). New: the Compose stack with Mailpit and the read-only game mount, the GitHub App key mount, the Linux test image and `.env.example`. Project MCP servers stay disabled.

## Engineering learning review

See [learning-review.md](learning-review.md). Fifteen lessons were applied to CLAUDE.md, the skills or the code during the stage. No change widened authority.

## Production readiness

See [readiness.json](readiness.json). 29 checklist rows were updated: 27 are done and 2 are in progress. Recommendations: dependency scanning (PR-032) and per-route API rate limits (PR-098), both small.

## Next stage proposal and resume instructions

- Next stage: S03, native agent and durability. The maintainer has authorized it and its inputs are recorded. It has not started.
- Ready for it: merged Setup pin, accounts and workspaces, the agent crate from S01, the [draft S03 scope](../S03/scope.md).
- On a new machine: follow [developer setup](../../docs/07-developer-workflow/DEVELOPER-SETUP.md), then give Claude Code the [S03 goal prompt](../../docs/06-agent-handoffs/GOAL-S03.md) after `/start-implementation-stage S03`.
- Read first: CLAUDE.md, this handoff, the S03 packet and the decision record's S03 inputs.
- Pending operations needing reconciliation: none.

Next action: review this handoff, then start S03 with the goal prompt.
