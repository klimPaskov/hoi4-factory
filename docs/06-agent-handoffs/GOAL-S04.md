# Goal: implement S04 of HOI4 Factory

For the maintainer: run this session after the UI design track ([GOAL-UI](GOAL-UI.md)) is finished and pushed. Open Claude Code at the root of an up-to-date clone of https://github.com/klimPaskov/hoi4-factory, run `/start-implementation-stage S04`, then give it everything below this line as its goal. Running that command is the S04 authorization.

---

You are the implementing agent for HOI4 Factory, a free, open-source hosted web app where people and AI agents build Hearts of Iron IV mods. Implement stage S04, the focus authoring vertical slice in all three modes, completely and autonomously. S03, the native agent loop with durable attempts and scoped tools, is finished and pushed. Its result is in `evidence/S03/handoff.md`. The UI design track ran after S03 and produced the client's design system.

## Before any code

1. If this is a new computer, follow `docs/07-developer-workflow/DEVELOPER-SETUP.md` until every box in its section 7 is checked. On a machine that is already set up, pull, start the stack with `scripts/compose.sh up -d --build --wait`, confirm `/readyz` is `ok` with `checks.agent` available, and run the validator and skill check. Record anything missing in `evidence/maintainer-questions.md` and continue with what does not need it.
2. Read CLAUDE.md fully, including "Agent runtime rules (learned in S03)".
3. Read `docs/PACKAGE_STATUS.md`, `docs/MASTER_GOAL.md`, the S04 packet `docs/04-implementation-stages/stage-04-focus-vertical-slice.md`, the decision record `docs/02-architecture/maintainer-decisions-2026-10-24.md` (ADR-014, ADR-018, ADR-025 and the S03 decisions), the contracts `docs/02-architecture/contracts-and-interfaces.md` (C-02, C-04, C-05, C-08, C-10), SPEC-005, SPEC-006, SPEC-007 and SPEC-009.
4. Read `evidence/S03/handoff.md`, `evidence/S01/exp02/report.md` (the Agent Tools writable focus subset and its two defects) and `docs/design/DESIGN-DECISIONS.md` from the UI design track. Write `evidence/S04/scope.md` and `evidence/S04/environment.json`.

## What S03 gives you

- The agent host in `crates/server/src/agent/`.
  - Admission issues a C-08 grant, builds and stores an immutable C-06 bundle, takes the workspace lease and admits the attempt.
  - The runner drives a supervised worker (`factory-server agent-worker`) turn by turn.
  - Each tool call passes the broker's closed schema, then the fence and grant checks and the C-04 intent are committed together, then the receipt is settled.
  - Pause, interrupt (with kill evidence), revocation and restart recovery are covered by `crates/server/tests/agent.rs`.
- The agent writes only in the task's own Git worktree on branch `hoi4factory/<task>` and does not commit. The attempt outcome lists the changed files. Turning that worktree into a reviewed commit, and publishing it, is S04's job (C-02, C-10).
- Built-in tools are `list_files`, `read_file`, `write_file` (expected-before hash, idempotent) and `finish` (structured report, one repair). HOI4 Agent Tools `731e64f` runs over MCP stdio with a read-only profile of nine tools. Rewrite tools are withheld. S04 decides which focus rewrite operations to expose, using EXP-02's supported subset and host hash guards.
- Profiles:
  - `anthropic:claude-haiku-5-5`: default, live-verified, funded by the $1 monthly allowance or the user's own key.
  - `openai:gpt-5.4-nano`: adapter tested against recorded and refused responses only.
  - OpenAI-compatible self-hosted: connection and model listing only, capabilities unknown, so agent tasks on it are refused with `capability_unsatisfied`.
- Minimal task and API key screens. Rebuild them with the design system when S04 touches them.

## Maintainer decisions to respect

- Free use with the $1 operator allowance per verified user per month, or the user's own key. No enterprise features, MFA or extra sign-up steps.
- OpenAI live inference stays unavailable until an OpenAI organization with balance exists. The self-hosted profile is verified by connection, listing and the destination policy only, unless the maintainer has answered otherwise in `evidence/maintainer-questions.md` (check items 7 to 10 there first).
- Automated GitHub writes go only to `klimPaskov/hoi4-factory-test-mod`. Pushes and pull requests need their own publication grant (`push_owned_branch`, `open_pull_request`). Never integrate into a default branch.
- The UI design track's design system is the base for every new or changed page. Send each new page through the `ui-design-iteration` skill: build three or four working variants in the design lab, ask the maintainer to choose, then implement the choice. Batch these questions so the maintainer answers several at once, and keep building non-UI parts while waiting.

## What to build

Follow the S04 packet (W1 to W6). In short:

- **W1, focus read projection.** Parse focus trees through Agent Tools' read tools and the server game reference, render the tree in the browser canvas with diagnostics for unsupported constructs, and record the parser, renderer and game reference versions.
- **W2, bounded visual mutation.** Node creation, prerequisite linking, moves and scalar edits, only inside EXP-02's source-preserving subset. Every edit becomes a C-02 proposal through the S03 grant and receipt path. Use host hash guards before every rewrite, and refuse invalid encodings.
- **W3, autonomous focus task.** The S03 agent creates three to five nodes with supported effects and localization, within its grant and budget, with the browser tab closed.
- **W4, unified change card.** Semantic summary, raw diff, a source-linked preview, the checks actually performed, limits and a restore point. Final approval is bound to the proposal hash or a valid wider grant.
- **W5, commit, GitHub publication and restore.**
  - Commit only owned files under the expected HEAD.
  - Push the owned branch and open a pull request through C-10, under a separate publication grant, on the test repository.
  - Reconcile lost replies by reading state.
  - Restore an earlier snapshot.
- **W6, three-mode equivalence.** Run the same intent in visual, hybrid and autonomous modes and compare native semantics and preservation, not model wording.

Delegate independent packets to Sonnet subagents (Haiku for mechanical work) with the `delegate-parallel-work` skill. Commit empty crate skeletons first, write each brief's API into a file the agent reads, keep cross-crate composition yourself, and keep `CARGO_TARGET_DIR` on the main checkout's `target/`.

## How to work

- Work autonomously. Append every question or request for the maintainer to `evidence/maintainer-questions.md`, with what you did meanwhile. Stop only for a direct blocker that no reasonable default gets around. Page design choices are the maintainer's: ask them in batches through the design lab.
- Never fabricate model output, test results or evidence. Use recorded real responses for faults a live API cannot produce, and label them. Live Anthropic calls with `claude-haiku-5-5` are pre-approved. Record spend.
- Run platform-sensitive tests on Linux with `scripts/linux-tests.sh` as well as on the host.
- Whenever you learn something useful, update CLAUDE.md or a skill in the same session and regenerate with `scripts/py.sh scripts/sync_skills.py --write`.
- Commit locally as you go, with the attribution trailer CLAUDE.md requires.

## Finish

1. Evaluate S04 exit gates 1 to 7 on the final tree with `validate-stage`, and write `evidence/S04/checks.json`.
2. Run `production-readiness-review` lightly, then `capture-engineering-lessons`. Write `evidence/S04/config-review.md`, `learning-review.md` and `readiness.json`.
3. Write `evidence/S04/handoff.md` from the template, update `docs/PACKAGE_STATUS.md`, the README status and the developer setup guide.
4. Write `docs/06-agent-handoffs/GOAL-S05.md` in this same style.
5. Run the validator and the skill check, then push everything to `origin main`.
6. Do not start S05.
