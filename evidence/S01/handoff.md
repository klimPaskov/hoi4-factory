# S01 handoff

## Identity and authorized scope

- Stage: S01, Claude Code bootstrap and architectural probes.
- Authorization: the maintainer invoked `/start-implementation-stage S01` on 2026-10-24 and amended the packet in chat, as recorded in [scope.md](scope.md). S02 is not authorized.
- Implementing client: Claude Code 2.1.304 (desktop 3.1623.0). Parent model `claude-opus-5-5`, later `claude-sonnet-5-5` by maintainer switch. Subagents ran on Opus for the experiments and on Sonnet and Haiku for the closing work.
- Reviewer: the maintainer. Review requested by this handoff.
- Repository: `klimPaskov/hoi4-factory`, branch `main`. Base revision `290af7c48ce160d4f7461b90aedc92528295330b`. The S01 work is the commit that follows this handoff.
- Dirty paths before work: none.
- Interruptions: the Claude Code process restarted once mid-stage and the EXP-01 subagent hit the Claude usage limit once. Both were resumed from saved state after checking outputs on disk.

S01 delivered environment evidence, guidance checks, a Cargo and Compose skeleton, and four experiments. No production feature, account system, browser client or public deployment was built.

## Outcome

| Requirement or experiment | Intended result | Actual result | Evidence | Status |
|---|---|---|---|---|
| S01-G1 guidance and config | Configs parse, skill projections match, client loads guidance | Validator 11 of 11, skill drift none, CLAUDE.md imported AGENTS.md once in this client | [checks.json](checks.json), [config-review.md](config-review.md) | pass |
| S01-G2, EXP-01 Setup on server paths | Preservation fixtures and the recovery case with exact evidence | Import preservation exact. The CI recovery mismatch is reproduced and explained: a random-name placement temp file survives a hard kill and rollback (17 of 20 Windows, 20 of 20 Linux). A local fix restored exactly 20 of 20 per side | [exp01/report.md](exp01/report.md) | fail, apply reliance blocked at `4488a418` |
| S01-G3, EXP-03 Rust agent loop | Every listed fault handled, mutated arguments and stale authority rejected, comparisons measured | All faults pass on Anthropic live and recorded-real replay. Broker, journal, fencing and worker probes pass on real PostgreSQL and processes. OpenAI live faults unavailable | [exp03/report.md](exp03/report.md) | partial |
| S01-G4, EXP-04 container and GitHub | Native request from an image with no coding CLI, GitHub App scope recorded | Live `claude-haiku-5-5` tool-use turn from the container. The image has no coding CLI, credential or game file. GitHub App `HOI4 Factory Dev` passes 14 of 14 on `hoi4-factory-test-mod` | [exp04/report.md](exp04/report.md), [exp04-github/report.md](exp04-github/report.md) | pass |
| EXP-02 focus fidelity | Supported writable subset of Agent Tools in the Linux image | 105 checks: 91 exact, 8 refused safely, 6 damaged (invalid UTF-8 rewrite and stale-base revert) | [exp02/report.md](exp02/report.md) | subset defined, two defects |
| S01-G5 decision receipt | Pins, operations, unknowns, next scope | This document | this file | pass |

Stage disposition: complete with narrowed scope. G2 fails as the gate intends, because the reproduced recovery case is an unresolved source-preservation defect upstream. G3 is partial because OpenAI could not be exercised.

## Decision receipt

**Runtime (ADR-011).** Confirmed. Use thin project adapters for Anthropic and OpenAI over vetted HTTP, TLS, JSON and JSON Schema crates. async-openai added 36 crates and 22.8 percent binary size, offered no raw-byte capture and hid a missing completion marker. Run the loop in a supervised worker process of the same binary. It costs about 0.19 ms per turn and gives a killable boundary with identical fencing.

**Setup binding (ADR-001).** Hybrid. Embed the `hoi4-mod-setup` scanner and transaction services directly, which build on Linux without Tauri but link libdbus through `keyring`. Extract a service facade for analysis, evidence approval, plan building, conflicts and approval, preferably upstream, because `commands.rs` keeps per-process global state and reads the OS keychain. Production apply stays blocked until the placement-temp fix lands upstream with a regression test.

**Focus writes (EXP-02).** The supported subset is no-op, cost, move, adding a prerequisite group and appending a focus, on well-formed valid UTF-8 or Windows-1252 trees with unique IDs, present anchors and no prerequisites on shared focuses. Host guards must reject invalid encodings and compare the source hash immediately before every rewrite.

**GitHub (ADR-014, ADR-018).** Require selected-repository installations, and enforce a host allowlist before every call. Force `core.autocrlf=false`. Reconcile by reading state.

**Exact pins.** Rust 1.97.1. Setup `4488a418cad56a158273cb66c4ce7794696b7ed1`. Agent Tools `25f5641bbc07c65a822ef576ac6e92bd12666880`. Crates: axum 0.8.9, tokio 1.54.1, reqwest 0.13.5, rustls 0.23.46, jsonschema 0.60.1, tokio-postgres 0.7.18, serde_json 1.0.154 (full lock in `Cargo.lock`). Images: `rust@sha256:2775a09d...bcdd3`, `debian@sha256:7ff6ba31...e3f8`, `postgres@sha256:f3bd19c6...e6e3`. Server image `sha256:bd05985c...3f50`. Agent probe image `sha256:d31ea492...4e27`. Licenses: no crate incompatible with AGPL-3.0.

**Provider profiles.** M-API-B Anthropic `claude-haiku-5-5` live. M-API-A OpenAI unavailable, because both keys return 429 `credit_balance_exhausted`. M-LOCAL-C skipped by the maintainer. Total live spend about 0.0008 USD.

## Changed artifacts

Product skeleton: `Cargo.toml`, `Cargo.lock`, `rust-toolchain.toml`, `crates/server`, `crates/agent` (Anthropic and OpenAI adapters, SSE parser, transport, worker protocol), `crates/setup-adapter` (stub), `compose.yaml`, `docker/server.Dockerfile`, `docker/agent-probe.Dockerfile`, `.dockerignore`, `.gitignore`, `.gitattributes`.

Disposable probes: `probes/setup`, `probes/focus`, `probes/runtime` (workspace member, with real recordings), `probes/github`.

Evidence: everything under `evidence/S01/`. Candidates: `engineering/knowledge/candidates/S01-L01.md` to `S01-L23.md`.

Planning and guidance changed during the stage by maintainer decision: ADR-022 and ADR-023 in the decision record, SPEC-002, SPEC-004, S05, risks, requirements REQ-049 and REQ-050, tests T-MEM-01 to T-MEM-05, AGENTS.md, CLAUDE.md, skill invocation modes, the knowledge policy and changelog 0.1.3.

## Verification record

See [checks.json](checks.json) for every command and result. On the final tree: package validator pass, skill drift none, `cargo test --workspace` 9 passed, all 48 EXP-03 input hashes and all 17 EXP-02 fixture hashes match their evidence, both probe image IDs match, and the server image rebuilt and answered `/healthz`. Validation caught one regression, where the server Dockerfile no longer built after the probe crate joined the workspace, and fixed it (S01-L23). Live provider probes and EXP-01 trials were not re-run because their inputs are unchanged.

Not run: OpenAI inference, Codex client loading, gVisor containment, an OS sandbox outside-root test, the Setup offline lifecycle test (needs Node), the F-SETUP-NEW generated set and webhooks.

## Effects, recovery and rollback

External effects, all within the authorized scope:
- Created private repositories `hoi4-factory-s01-probe`, `hoi4-factory-s01-probe-b` and `hoi4-factory-test-mod`.
- Created two GitHub Apps through the maintainer's clicks. The first was uninstalled everywhere.
- Opened, merged and commented on branches, pull requests and issues in the probe and test repositories.
- One unintended write: issue #1 "must be refused" in the public `hoi4-factory` repository, caused by an all-repositories installation. Closed with an explanation at the maintainer's choice.
- About 0.0008 USD of Anthropic usage.

Pending cleanup that only the maintainer can do: delete the first App registration and the two `s01-probe` repositories, and optionally issue #1. Local cleanup when convenient: `docker compose down -v`, the probe images, the `hoi4f-exp01-*` volumes and `.work/` (about 2.4 GB of trial data).

Rollback: revert the S01 commit. GitHub resources and Docker state need the operation-specific cleanup above.

## Configuration checkpoint

See [config-review.md](config-review.md). Changed: Rust toolchain pin, Compose and Dockerfiles, Git attributes, skill invocation modes (ADR-023), and the user-environment API keys used by probes. The project MCP servers stay disabled. A user-scope `hoi4_agent_tools` server was present and unused.

## Engineering learning review

See [learning-review.md](learning-review.md). 23 candidates. S01-L06 (provider quota classification) was promoted automatically under ADR-023, with its test passing. Nine wait for regression tests, nine are captured, one is rejected and three were applied by maintainer decision. No skill body changed.

## Next stage proposal

- Proposed next stage: S02, onboarding, accounts and server workspaces.
- Ready: the Setup binding recommendation, the GitHub App and canonical test repository, the server and PostgreSQL skeleton, the agent crate.
- Unresolved before S02 apply work:
  - the upstream Setup placement-temp fix, a maintainer decision on HOI4-Mod-Setup
  - the facade location
  - libdbus in the server image or a `keyring` feature gate
  - readable ancestor directories for server workspaces
  - which provider satisfies Setup's analysis gate
  - Node in the image for the plan builder
- Also open for S03 and S04: an OpenAI account with API balance, the two Agent Tools defects (fix upstream or guard only), the focus-plan sidecar location, and writable prerequisites on shared focuses.
- Read first: this file, `scope.md`, `exp01/report.md`, the S02 packet and the maintainer decision record.
- Pending operations requiring reconciliation: none. All probe containers stopped or are healthy development services.
- Needed before implementation: the maintainer's review of this S01 result and a separate authorization for S02.

## Maintainer answers for S02 (2026-10-24, after S01 review)

| Topic | Decision |
|---|---|
| Setup placement-temp fix | Pull request to HOI4-Mod-Setup with the S01 fix and a regression test that kills apply mid-write. The factory pins the merged revision |
| Setup service facade | Upstream in HOI4-Mod-Setup as a library API shared with the desktop app |
| Agent Tools defects | Factory host guards now, plus a pull request to hoi4-agent-tools fixing both, using the S01 fixtures as tests |
| Upstream pull requests | Standing permission to open pull requests on HOI4-Mod-Setup and hoi4-agent-tools without asking each time. Never push to their default branches or merge. The maintainer reviews and merges |

Defaults chosen by the implementing agent, revisable in S02 scope: make `keyring` an optional feature in the same Setup facade pull request instead of shipping libdbus in the server image, include a pinned Node runtime in the tool image for Setup's plan builder, satisfy Setup's analysis gate through the factory's Anthropic adapter, and relocate the Agent Tools `.focus-plan.json` sidecar into factory state outside the mod.
