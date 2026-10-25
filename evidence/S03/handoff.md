# S03 handoff

## Identity and authorized scope

- Stage: S03, native agent, durable attempts and scoped tools.
- Authorization: the maintainer invoked `/start-implementation-stage S03` on 2026-10-25 with "I authorize S03 now". They asked for a quick readiness check on this Mac, then autonomous work to the end of the goal ([GOAL-S03](../../docs/06-agent-handoffs/GOAL-S03.md)): this handoff, GOAL-S04, an updated GOAL-UI, everything pushed. Questions go to [maintainer questions](../maintainer-questions.md). S04 and the UI design track were not started.
- Implementing client: Claude Code 2.1.308 (desktop 3.6556.2, agent SDK 0.3.308). Parent model `claude-opus-5-5`. Five Sonnet subagents built the packets: agent, tasks, broker, HOI4 tools and web.
- Reviewer: the maintainer. Review is requested by this handoff.
- Repository: `klimPaskov/hoi4-factory`, branch `main`, on the macOS machine (Apple Silicon). The base revision is `7c70b0c`. S03 is the commits from `f6872db` through the handoff commit that follows this file.
- Dirty paths before work: none. Worktrees: the main checkout only, before and after. The five packet worktrees were created, merged and removed.
- Interruptions: none.

Excluded and not built:
- a fleet or multi-node scheduler, or an unrestricted shell
- visual editors, credit purchasing and payments
- the S05 memory system and local model inference
- Agent Tools rewrite tools, and committing or publishing a task branch
- the gVisor sandbox (EXP-07), the UI design track and S04.

## Outcome

A signed-in, verified user gives the agent a bounded task on a project, read-only or edit with allowed paths, a budget and a model. Starting an attempt runs as follows:

1. **Admission.** The server issues a C-08 grant, builds and stores an immutable C-06 context bundle, takes the workspace lease and admits the attempt at the task's expected version.
2. **The loop.** A supervised worker process of the server binary calls the model turn by turn through the thin Rust adapters. The host checks each tool call against its closed schema, then commits the fence check, grant check and C-04 intent together, marks the receipt started, runs the effect and settles it.
3. **Writes.** The agent writes only in the task's own Git worktree on branch `hoi4factory/<task>`, under the granted paths.
4. **Metering.** Each model call reserves its cost bound first and settles to the reported usage, or stays `usage_unknown`. The operator allowance and the user's own key are counted apart.

The user can watch the C-03 event stream and can pause, resume, interrupt or revoke. After a restart, nothing is repeated: uncertain effects are reconciled by observation and the task is handed to the next attempt with its receipts and uncertainty.

| Gate | Intended result | Actual result | Evidence | Status |
|---|---|---|---|---|
| S03-G1 | A containerized native run consumes a persisted immutable C-06, uses the selected provider directly through the Rust loop, and spawns or contains no coding CLI | Live Compose run with real `claude-haiku-5-5`: 3 turns, 1 granted write, `succeeded`, task `in_review`, 24 s, $0.00215. The C-06 bundle was stored before admission. Replay tests show the first request carries its ID and manifest hash, and `load_bundle` verifies it. The image contains only `factory-server`, Debian Node (Setup) and Node 22 (Agent Tools), with no coding CLI, npm or npx. Worker processes are `factory-server agent-worker` | [live-agent-flow.json](live-agent-flow.json), [c06-fixture.json](c06-fixture.json), `a_native_run_consumes_its_bundle_and_writes_only_inside_the_grant`, [checks.json](checks.json) | pass |
| S03-G2 | Final tool payloads pass a closed host schema and current grant checks, with zero writes to unauthorized fixture paths | A write outside the grant gives `path_not_granted` and an off-schema call (the recorded S01 tool turn) gives a schema refusal, both with zero intents. A read-only task has no write tool. Hook-mutated paths and types are refused by the broker. The worktree hashes equal the source after every refused case | `writes_outside_the_grant_and_off_schema_calls_change_nothing`, `a_read_only_task_has_no_write_tool`, broker `schema`/`paths` tests | pass |
| S03-G3 | An interrupted uncertain effect is reconciled or stays blocked, never silently rerun | After a simulated process death, recovery in a new incarnation reconciled three started writes as applied, not applied and conflict by observing hashes. It wrote nothing, kept the model call `usage_unknown` and ended the attempt as `host_restarted`. The next attempt got the reconciled receipts in its bundle, and no model turn was repeated | `an_interrupted_write_is_reconciled_after_a_restart_and_never_rerun`, tasks `receipts` tests | pass |
| S03-G4 | Pause and revocation persist across restart, and no later effect uses an obsolete epoch or fence | Pause during a turn suspended the attempt with its tool call deferred. The pause survived a restart. Resume took a new lease and ran the deferred call once, and the turn was not resent. The old incarnation's fence is refused. A revocation (epoch 2) stopped effects with zero intents and stayed revoked after restart, and the old epoch is refused | `a_pause_survives_a_restart_and_resume_does_not_repeat_the_turn`, `revocation_stops_effects_and_persists_across_restart`, tasks `ledger`, broker `grants` tests | pass |
| S03-G5 | Events replay in order with gap detection, cost unknowns stay unknown, budget exhaustion stops dispatch per task and account, and payers are attributed apart | Events are gapless 1..n with cursor replay, and the SSE stream resumes from `Last-Event-ID`. Clients get a gap check. An exhausted allowance or task budget stops dispatch with zero provider requests. User-key and allowance spend are counted apart. Lost streams, killed workers and dead workers leave `usage_unknown` and are never retried | `events_replay_in_order...`, `an_exhausted_allowance...`, `a_task_budget_stops_dispatch_and_user_keys_are_metered_apart`, `a_stream_lost_after_acceptance_is_not_repeated`, broker `meter` tests | pass |
| S03-G6 | Three profiles have truthful capability outcomes, unsupported features are explicit, and custom endpoints to blocked destinations are refused | M-API-B Anthropic: live, with listing and inference probes. M-API-A OpenAI: recorded and refused responses only (401, 429 quota, 429 rate limit, synthetic streams labelled synthetic), with no live call, by maintainer decision. M-LOCAL-C self-hosted: an Ollama container with no model is refused by default (private address) and allowed only by an operator range, and listing works while every inference capability stays unknown. Private, loopback, link-local, metadata and IPv6-embedded addresses are refused | [selfhosted-profile.json](selfhosted-profile.json), agent `probe`/`destination`/`live` tests, `custom_endpoints_to_blocked_destinations_are_refused` | partial, as the goal expected: OpenAI live is unavailable and the self-hosted profile is connection-only |

Stage disposition: complete, with G6 partial as planned.

## What was built

- `crates/agent` (W1):
  - Thin Anthropic and OpenAI-compatible adapters with the project's own SSE parser, and a provider-neutral request.
  - Profiles with an outcome and basis per capability, plus listing and inference probes.
  - The outbound destination policy, with a connect-time resolver, no redirects and no proxy.
  - Normalized errors with C-envelope retry classes and cost certainty, and micro-dollar cost estimates.
  - A serializable conversation state with one structured repair.
  - Worker protocol v2 with Cancel, the worker loop, and the host supervisor (handshake, generation fencing, kill of the process group).
  - A replay HTTP server for tests, and Anthropic schema conditioning.
- `crates/tasks` (W2, W4, migration 0600):
  - Tasks with expected-version transitions and a durable pause.
  - Attempts kept separate from task completion.
  - Gapless C-03 events written in the same transaction as each state change.
  - Host incarnations, leases and fencing.
  - The pure C-06 builder (estimated budget, omissions, the typed `required_context_too_large`) and an immutable store with an integrity check on load.
  - C-04 intents, receipts, replay decisions and reconciliation.
  - A narrow `SECURITY DEFINER` recovery scan.
- `crates/broker` (W3, migrations 0700-0702):
  - C-08 grants with role limits, revocation epochs and `FOR SHARE` checks before effects.
  - Path and destination authorization.
  - Closed-schema compilation and a tool registry that validates after hooks.
  - Sealed user API keys.
  - A per-account serialized meter for the allowance, user keys and task budgets.
- `crates/hoi4-tools` (W5):
  - A thin MCP stdio client with bounded frames, an environment allowlist and process-group kill.
  - The S03 read-only Agent Tools profile: nine tools with closed host schemas and pinned schema hashes.
  - Discovery with health reported apart from feature presence, catalog-change withholding, timeouts and pure-read-only retries.
- `crates/server`:
  - The agent host (`src/agent/`: runtime, tools, runner, recovery, profiles) and the routes (`src/agent_api.rs`).
  - The `agent-worker` subcommand and agent status in `/readyz`.
  - Tests in `tests/agent.rs` (17), with recorded-real streams recorded by `tests/agent_record.rs`.
- `web/`: the minimal task list and task page with the live event log, plus API keys and usage on the account page.
- Image and Compose:
  - The Agent Tools build stage in `docker/server.Dockerfile`.
  - The key mount and agent variables in `compose.yaml`, and `init: true`.
  - `compose.selfhosted.yaml`.
  - Key creation in `scripts/compose.sh`, `--init` in `scripts/linux-tests.sh`, and `scripts/live_agent_flow.py`.

## Verification record

[checks.json](checks.json) has commands, versions, exit codes and counts.
- **Final tree on macOS:** the whole workspace with `--no-fail-fast`, with the real Agent Tools tests enabled.
- **Linux:** the new crates and the agent suite through `scripts/linux-tests.sh`.
- **Web:** checks in the pinned Node image.
- **Live runs:** the image build and Compose stack, `/readyz`, the image scan, the live agent run, the self-hosted check, the validator and the skill check.

Evidence kinds are kept apart:
- automated tests against real PostgreSQL with row-level security as the app role, the real worker process and the real Agent Tools 4.4.0
- recorded-real replay of Anthropic streams, with faults injected into real bytes
- synthetic OpenAI streams, labelled synthetic
- live Anthropic calls
- an image filesystem scan.

No game was run.

**Model spend in S03** totalled about $0.0063 (6.3 thousand micro-dollars), all on `claude-haiku-5-5` at $0.10 per million input tokens and $0.50 per million output tokens (price retrieved 2026-10-24):

| Use | Tokens | Cost |
|---|---|---|
| Agent crate live tests | about 950 in, 140 out | about $0.00016 |
| Recordings | about 14.7 thousand in, 0.85 thousand out | about $0.0019 |
| Two live Compose runs | about 36 thousand in, 1.2 thousand out | $0.0042 |

## Decisions and unresolved issues

The S03 maintainer inputs (allowance, default model, OpenAI unavailable, self-hosted connection-only) were applied as recorded. The implementing agent's defaults are in [scope.md](scope.md) and in the decision record under "S03 implementation defaults". Open questions are in [maintainer questions](../maintainer-questions.md) items 7 to 10:
- where Agent Tools runs
- the self-hosted check endpoint
- not resuming interrupted attempts automatically
- agent edits staying uncommitted until S04.

None is blocking.

Known limitations:
- **OpenAI.** No live OpenAI call was made, and the stream tests use synthetic data.
- **Self-hosted profile.** Its capabilities stay unknown, so agent tasks on it are refused until an inference probe is allowed.
- **Grants.** A grant lasts two hours, so an attempt longer than that fails with `grant_expired`.
- **Frame size.** The worker frame limit is 1 MiB, so very long conversations need compaction, which is S05's job.
- **Agent Tools output.** `resource_link` artifacts are not readable by the model, because the profile has no `resources/read`.
- **Repeated live runs.** The live C-06 manifest hash was identical across two runs with identical inputs, as intended, but bundle IDs differ.
- **Readiness recommendations.** Operator spend alerts, dependency scanning and per-account rate limits on task routes.

## Follow-up after the maintainer's answers (2026-10-25)

The maintainer answered questions 7 to 10 and added ADR-026 (a desktop companion app, synced with the web app) and ADR-027 (queue priority, plan tiers, full Agent Tools on the server, an optional local MCP). They also asked for an issue system. In the same session:

- **Self-hosted check with the smallest local model.** The probe gained an opt-in inference mode. The run used `smollm2:135m` (270 MB) in Ollama. The local model connects: text, streaming and usage are measured as supported. Tool calls are measured as unsupported, because the model refuses tools. The first run exposed two probe defects, which were fixed with a regression test. See [selfhosted-profile.json](selfhosted-profile.json).
- **Fixed limitations:**
  - Per-account rate limits on the agent, upload, GitHub import and Setup routes (PR-098 done).
  - `allowance_exhausted` and `usage_unknown` warnings for the operator.
  - Worker frames raised to 16 MiB.
  - The grant lifetime scales with the task's turn limit, and resuming after an expired or revoked grant is refused with no side effects.
  - Translated error texts for the new codes.
- **Dependency audit:** `scripts/audit-deps.sh` (cargo audit and npm audit, PR-032 done). The only Rust advisory is a dev-only `rsa`, recorded with its reason in `.cargo/audit.toml`. The Agent Tools npm advisories are fixed upstream in https://github.com/klimPaskov/hoi4-agent-tools/pull/49, which waits for the maintainer's merge.
- **Issue system:** the `track-issues` skill, a CLAUDE.md section, and repository labels.

Issues that could not be fixed in this session, filed on GitHub:

| Issue | Why it waits |
|---|---|
| [#2](https://github.com/klimPaskov/hoi4-factory/issues/2) Verify the OpenAI adapter live | Needs an OpenAI account with balance |
| [#3](https://github.com/klimPaskov/hoi4-factory/issues/3) Run the S03 tests on Windows and kill process trees there | Needs the Windows machine |
| [#4](https://github.com/klimPaskov/hoi4-factory/issues/4) Bump the Agent Tools pin after hoi4-agent-tools#49 | Done: #49 merged as `fbc5144` with the maintainer's go-ahead, the pin moved to 4.6.1 and the issue was closed |
| [#5](https://github.com/klimPaskov/hoi4-factory/issues/5) Let the agent read Agent Tools artifacts | S04 preview and review design |
| [#6](https://github.com/klimPaskov/hoi4-factory/issues/6) Make the full Agent Tools surface usable on the server | S04 guarded proposal path |
| [#7](https://github.com/klimPaskov/hoi4-factory/issues/7) Use probed self-hosted endpoints as agent profiles | Feature beyond S03's gate |
| [#8](https://github.com/klimPaskov/hoi4-factory/issues/8) Optionally connect a locally installed MCP server | Needs a local bridge or the desktop app |
| [#9](https://github.com/klimPaskov/hoi4-factory/issues/9) Desktop companion app, synced with the web app | Backlog scope, ADR-026 |
| [#10](https://github.com/klimPaskov/hoi4-factory/issues/10) Queue priority for new users and plan tiers | Needs plans and the maintainer's fairness rules |
| [#11](https://github.com/klimPaskov/hoi4-factory/issues/11) Alert the operator about spend | Needs the maintainer's channel and thresholds |

## Effects, recovery and rollback

External effects:
- live Anthropic calls
- an Ollama image pulled (about 2.8 GB) and run without a model
- no GitHub writes, and no upstream pull requests.

No effect has an unknown outcome.
- **Code:** reverting the S03 commits on `main` restores S02.
- **Database:** migrations 0600 and 0700 to 0702 have down scripts, or drop the development volume with `scripts/compose.sh -p hoi4-factory down -v`.
- **Encryption key:** keep `.work/secrets/factory-secret.key` with the database, because saved user keys cannot be opened without it.
- **Disk:** the Ollama image can be removed with `docker image rm` when no longer needed.

## Configuration checkpoint

See [config-review.md](config-review.md).
- **Image:** the Agent Tools stage with Node 22.
- **Compose:** the key mount, the agent variables, `init: true` and the self-hosted check file.
- **Migrations:** 0600 and 0700 to 0702.

Project MCP servers stay disabled, and the product's MCP client starts only the configured profile.

## Engineering learning review

See [learning-review.md](learning-review.md): eleven reproduced lessons, S03-L01 to S03-L11, applied to code, tests, CLAUDE.md and the delegation skill. They cover Anthropic schema limits, the lease heartbeat, biased selects, snapshot reads, pause semantics, hash fields, zombie reaping, test fault isolation, Node runtimes, parallel packets and withheld schemas. None widens authority.

## Production readiness

See [readiness.json](readiness.json).
- 15 checklist rows reviewed: 6 done, 8 in progress, 1 unchanged.
- Recommendations: an operator alert for runaway spend, dependency scanning and per-account rate limits on the task routes, all small.

## Next stage proposal and resume instructions

- **Next session:** the UI design track ([GOAL-UI](../../docs/06-agent-handoffs/GOAL-UI.md), interactive, no stage command). It now lists the S03 screens and task states.
- **Then:** S04, the focus vertical slice ([GOAL-S04](../../docs/06-agent-handoffs/GOAL-S04.md)), after `/start-implementation-stage S04`.
- **Ready for S04:**
  - a reliable single-writer proposal path (grant, lease, intent, receipt, task branch worktree)
  - the working initial C-06 builder
  - the read-only Agent Tools profile
  - truthful profiles and metering.
- **Read first:** CLAUDE.md (S03 agent runtime rules), this handoff, the S04 packet and maintainer questions 7 to 10.
- **Pending operations needing reconciliation:** none.

Next action: review this handoff and answer questions 7 to 10. Then start the UI design track with GOAL-UI.
