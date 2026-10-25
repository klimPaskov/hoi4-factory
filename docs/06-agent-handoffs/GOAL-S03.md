# Goal: implement S03 of HOI4 Factory

For the maintainer: open Claude Code at the root of a fresh clone of https://github.com/klimPaskov/hoi4-factory, run `/start-implementation-stage S03`, then give it everything below this line as its goal. Running that command is the S03 authorization.

---

You are the implementing agent for HOI4 Factory, a free, open-source hosted web app where people and AI agents build Hearts of Iron IV mods. Implement stage S03, the native agent loop with durable attempts and scoped tools, completely and autonomously. S02 is finished, accepted and pushed. Its result is in `evidence/S02/handoff.md`.

## Before any code

1. This may be a new computer. Follow `docs/07-developer-workflow/DEVELOPER-SETUP.md` until every box in its section 7 is checked: the stack runs, `/readyz` is ok, the validator, skill check, Rust tests and web checks pass, and Claude Code lists the seven project skills. Local items that are not in Git (the game folder in `.env`, the Anthropic key, the GitHub App private key at `.work/github-app/private-key.pem`) are described there. If one is missing, record it in `evidence/maintainer-questions.md` and continue with everything that does not need it.
2. Read CLAUDE.md fully. It is canonical and holds the working rules learned so far: Windows, macOS and Git handling, disk limits and the shared build cache, Docker etiquette, how to delegate with worktrees, GitHub and upstream rules, model use and verification.
3. Read `docs/PACKAGE_STATUS.md`, `docs/MASTER_GOAL.md`, the S03 packet `docs/04-implementation-stages/stage-03-native-agent-and-durability.md`, the decision record `docs/02-architecture/maintainer-decisions-2026-10-24.md` (especially ADR-011, ADR-016, ADR-019, ADR-020 and "S03 inputs"), the contracts `docs/02-architecture/contracts-and-interfaces.md` (C-03, C-04, C-06, C-08), SPEC-002, SPEC-003, SPEC-004 (the C-06 parts), SPEC-006 and SPEC-011.
4. Read `evidence/S02/handoff.md` and the draft `evidence/S03/scope.md`. Confirm the scope against the packet, finalize it, and write `evidence/S03/environment.json`.

## Maintainer decisions already made for S03

- Operator allowance: $1 of model usage per verified user per month. Users may also bring their own API key, stored encrypted per owner and never used for anyone else.
- Default model `claude-haiku-5-5`. Live Anthropic calls for development and tests are pre-approved. Record spend in evidence.
- OpenAI live inference is unavailable, because the keys have no balance. Test the OpenAI adapter against recorded and refused responses only.
- The self-hosted OpenAI-compatible profile is verified by connection, model listing and the outbound destination policy only. Do not run inference on a local model.
- The product stays low friction. No MFA, lockouts, extra sign-up steps or enterprise features.

## What to build

Follow the packet and the draft scope's crate plan:

- `crates/agent`: production provider adapters from S01 (thin, own SSE), capability probing per profile, structured output with local validation and at most one repair attempt, normalized errors and retry classes, usage accounting, and the outbound destination policy that blocks private, loopback, link-local and metadata addresses unless the operator allows them.
- `crates/tasks` (migrations 0600-0699): C-03 ordered events with gap detection, expected-version transitions, host incarnation and lease fencing, durable pause, attempts versus task completion, immutable C-06 bundles with a manifest hash and a typed narrowing error, C-04 intent before effect and observed result after, effect classes, replay identity bound to workspace, root, source and tool version, and reconciliation of unknown effects.
- `crates/broker` (migrations 0700-0799): C-08 grants with operation class, paths, destination, budget, expiry and revocation epoch, closed final schema validation after any hook or repair, user API keys encrypted per owner, and metering that keeps the operator allowance and user-key usage separate, with budget stops per task and per account.
- `crates/hoi4-tools`: configured tool profiles only, Agent Tools over MCP stdio in its pinned container (pin hoi4-agent-tools `731e64f5b5c964bd1b33adad9c085bd3168b1940` or later, which contains the #42 source-preservation guards that the S01 pin `25f5641` lacks), schema and version discovery, health separate from feature presence, read-only mode, timeouts and bounded retries.
- Integration in `crates/server`, `web/`, `compose.yaml` and `docker/`: task, attempt, pause, revoke, event stream and API key routes, minimal task and API key screens built from the existing components (the client is redesigned in a separate UI design track after S03, ADR-025, so spend no effort on visual polish), and the agent loop in the server image, placed as S01 recommended (a supervised worker process of the same binary).
- W6 tests for process death, disconnect, a worker that ignores cancellation, stale results, and reopening a task without repeated writes.

Delegate independent packets to Sonnet subagents (Haiku for mechanical work), following the `delegate-parallel-work` skill: one worktree per packet created from the repository root, self-contained briefs with owned paths, review, merge into main, then remove the worktree. Keep `CARGO_TARGET_DIR` on the main checkout's `target/` and check free disk space often.

## How to work

- Work autonomously. Do not stop to ask. Append every question or request for the maintainer to `evidence/maintainer-questions.md`, with what you did meanwhile. Stop only for a direct blocker that no reasonable default can get around.
- Never fabricate model output, test results or evidence. Use recorded real responses for faults a live API cannot produce, and label them.
- Whenever you learn something useful, update CLAUDE.md or a skill in the same session and regenerate the copies with `scripts/py.sh scripts/sync_skills.py --write`. Create a new skill when a workflow repeats.
- Commit locally as you go, with the attribution trailer CLAUDE.md requires.

## Finish

1. Evaluate S03 exit gates 1 to 6 on the final tree with the `validate-stage` workflow. Write `evidence/S03/checks.json`. Expect gate 6 to be partial, because OpenAI live is unavailable and the self-hosted profile is connection-only. Say so plainly.
2. Run `production-readiness-review` lightly, then `capture-engineering-lessons`. Write `evidence/S03/config-review.md`, `learning-review.md` and `readiness.json`.
3. Write `evidence/S03/handoff.md` from `docs/06-agent-handoffs/STAGE-HANDOFF-TEMPLATE.md`, update `docs/PACKAGE_STATUS.md` and the README status, and keep `docs/07-developer-workflow/DEVELOPER-SETUP.md` accurate for anything S03 changed.
4. Write `docs/06-agent-handoffs/GOAL-S04.md`, a self-contained goal prompt for S04 in this same style. S04 builds its new pages with the design system from the UI design track and sends them through the `ui-design-iteration` skill. Also update `docs/06-agent-handoffs/GOAL-UI.md` so its screen inventory includes the screens S03 added. The next session after S03 is the UI design track, then S04.
5. Run the validator and the skill check, then push everything to `origin main`. The maintainer asked for every change to be pushed so work can continue on another computer.
6. Do not start S04 or the UI design track.
