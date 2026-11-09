# Package status and continuation

Version: v02, revised 2026-11-08. Phase: **implementation, S01 to S03 and the UI design track complete, S04 authorized on 2026-11-08 and in progress**.

## Current state

| Stage | State | Evidence |
|---|---|---|
| S01 bootstrap and architecture probes | Complete and accepted. Rust host and native Rust agent loop selected, Setup binding proven, GitHub App and memory decisions recorded | [S01 handoff](../evidence/S01/handoff.md) |
| S02 accounts, server workspaces, Mod Setup and GitHub import | Complete. Email and GitHub sign-in, per-account workspaces with row-level security, ZIP and folder upload, GitHub import, the full Setup flow over HTTP against merged HOI4-Mod-Setup `c28198b`, and the browser Setup wizard. Gates G1 to G4 pass. G5 passes with the limitation recorded in the handoff | [S02 handoff](../evidence/S02/handoff.md), [checks](../evidence/S02/checks.json) |
| S03 native agent and durability | Complete. The native Rust agent runs in a supervised worker of the server binary. Every step is behind a C-08 grant and a workspace lease. Each attempt consumes an immutable C-06 bundle, and every tool call is a C-04 intent followed by a receipt. Tools are the built-in file tools in the task's own worktree plus a read-only HOI4 Agent Tools profile. Users can bring their own API key, and usage is metered apart from the $1 allowance, with budget stops. The stage also adds the event stream, pause, interrupt, revocation and restart recovery, and minimal task and key screens. G1 to G5 pass. G6 is partial: OpenAI is tested against recorded and refused responses only, and the self-hosted profile is checked by connection and listing only | [S03 handoff](../evidence/S03/handoff.md), [checks](../evidence/S03/checks.json) |
| UI design track (ADR-025) | Complete on 2026-11-06. The maintainer chose every page from live variants: sign-in and account pages, project list and page, Setup wizard, task list and task page. The design system is in `web/src/styles.css` and `web/src/components/ui.tsx`. Later pages follow the `ui-design-iteration` skill | Handoff commit `257a180`, [design decisions](design/DESIGN-DECISIONS.md#handoff-2026-11-06), [goal prompt](06-agent-handoffs/GOAL-UI.md) |
| Expansion research (ADR-028) | Research, consultation and plan complete and approved on 2026-10-31. Phase D started with W1, the kit synchronization work. The first phase covers workflow updates in the kit, hoi4-agent-tools, a new image MCP and Chaos Redux, not HOI4 Factory stages | [package](08-expansion-research/README.md) |
| S04 focus vertical slice | Authorized by the maintainer on 2026-11-08 and in progress. Entry items still open on that date finish during the stage, see the next section | [goal prompt](06-agent-handoffs/GOAL-S04.md), [readiness](#s04-readiness-checked-on-2026-11-06) |
| S05 to S08 | Not authorized | [roadmap](04-implementation-stages/ROADMAP.md) |

**Priorities until about 2026-11-22 (D-36).** Tools for other modders come first, and S04 runs as one low-priority session that never holds up a tool release. From about 2026-11-22 the main effort moves to restarting the 3D route (D-46). The order of work is in [next steps, November 2026](09-planning/NEXT-STEPS-2026-11.md), and every decision in force is listed per project in [decisions in force](decisions/README.md).

A new machine is set up with the [developer setup guide](07-developer-workflow/DEVELOPER-SETUP.md). Open questions for the maintainer are in [maintainer questions](../evidence/maintainer-questions.md). Under ADR-024, CLAUDE.md and the skills are maintained by the implementing agent, and AGENTS.md is generated from CLAUDE.md.

## S04 readiness (checked on 2026-11-06)

Update of 2026-11-08: the maintainer authorized S04, and the open rows finish during the stage. Rows changed on that date say so. Development then stayed on Windows (D-25), so the move to the Mac described below did not happen, and the macOS-only checks run from the [macOS checks prompt](06-agent-handoffs/GOAL-MACOS-CHECKS.md) (D-47). The rows below are the S04 entry conditions from the [roadmap](04-implementation-stages/ROADMAP.md#engine-editors-and-tools-across-stages), the [S04 packet](04-implementation-stages/stage-04-focus-vertical-slice.md#entry-conditions) and the [S04 goal prompt](06-agent-handoffs/GOAL-S04.md), each checked against code, CI runs and releases on 2026-11-06. The checks looked at HOI4 Factory, hoi4-agent-tools, the Agentic HOI4 Modding Kit and HOI4 Mod Setup. Decision D-13a (see the [expansion decision record](08-expansion-research/00-decision-record.md)) keeps all tool work on Windows and moves development to the Mac only when everything is done and S04 is about to start. The maintainer answered four questions on 2026-11-06, recorded as S04-1 to S04-4 in the same record: the home of the private Chaos Redux plugin, that M-1, M-5 and M-14 finish during S04, that the Windows check of S03 comes first, and that the nightly benchmark may run as a scheduled task.

| Entry item | State | Evidence | Owner or next step |
|---|---|---|---|
| S03 accepted | Done | [S03 handoff](../evidence/S03/handoff.md) | None |
| UI design track | Done | Handoff commit `257a180`, [design decisions](design/DESIGN-DECISIONS.md#handoff-2026-11-06) | None |
| W3 layout verdict and W7 Rust layout solver | Done | hoi4-agent-tools [#53](https://github.com/klimPaskov/hoi4-agent-tools/pull/53) and [#80](https://github.com/klimPaskov/hoi4-agent-tools/pull/80), shipped in 4.7.0 | None |
| Engine binaries for Windows x64, macOS arm64 and Linux x64 and arm64 | Done, in the Factory image since 2026-11-08 | [#86](https://github.com/klimPaskov/hoi4-agent-tools/pull/86) bundles all four with a checksum manifest. `docker/server.Dockerfile` installs the release tarball by SHA-256 (4.12.0 since 2026-11-08) and keeps the checked binary for its architecture. T-ENG-01 passed in the image: [evidence](../evidence/S04/t-eng-01/run.log) | Re-pin once more when the rest of M-8 is released (S04 gate 11) |
| Engine client that reports the engine as unavailable | Done | `src/hoi4_agent_tools/core/engine-client.ts` in hoi4-agent-tools. An image built with `ENGINE_BINARY=omit` answers `FOCUS_ENGINE_UNAVAILABLE` and writes nothing, and `factory-server engine-status` reports `binary_not_found` | None |
| `hoi4.mod_index`, `hoi4.source_lookup` and the closure option of `hoi4.script_validate` on the engine (M-8) | Partial, finishes during S04 | Step 1, `mod_index` on the engine ([#123](https://github.com/klimPaskov/hoi4-agent-tools/pull/123)), is released in 4.12.0 and pinned by the Factory. `source_lookup` and the closure option are still open in [#107](https://github.com/klimPaskov/hoi4-agent-tools/issues/107) | Agent Tools session. The Factory re-pins to the release that carries them |
| M-1 `hoi4.skill` | Done | [#121](https://github.com/klimPaskov/hoi4-agent-tools/pull/121), released in 4.11.0 | None |
| M-5 generated tool documentation with a CI drift check | Done | [#112](https://github.com/klimPaskov/hoi4-agent-tools/pull/112), released in 4.11.0 | None |
| M-6 benchmark mods | Partial, finishes during S04 | `bench/corpus.json` pins the seven mods. The real-game baseline of 2026-11-06 is recorded in [#122](https://github.com/klimPaskov/hoi4-agent-tools/pull/122) | Agent Tools session records the baselines after the accuracy fixes |
| M-7 HOI4-MCP measured once | Done | The head-to-head of 2026-11-01 is committed as evidence in [#113](https://github.com/klimPaskov/hoi4-agent-tools/pull/113) | None |
| M-13 macOS arm64 CI | Done | The `test (macos-latest, 22)` and `(24)` jobs run on every pull request. [Run 42198261412](https://github.com/klimPaskov/hoi4-agent-tools/actions/runs/42198261412) used image `macos-26-arm64` and passed | None |
| M-14 Chaos Redux tools in a private plugin | Done | [#132](https://github.com/klimPaskov/hoi4-agent-tools/pull/132) loads plugins from configured paths and moves the ChaosX tools into the private `chaos-redux-vault` plugin (S04-1). Released in 4.12.0, which the Factory image pins | None |
| Vanilla job on a runner with a licensed game | Partial, finishes during S04 | `scripts/bench/nightly.sh` and the vanilla cases exist. The nightly run was described in [#122](https://github.com/klimPaskov/hoi4-agent-tools/pull/122), and the Windows scheduled task (S04-4) is not yet registered | Agent Tools session registers it |
| Q-1 baseline (S04 gate 12) | Done | The real-game baseline of 2026-11-06 (99 cases, 93 passing, [#122](https://github.com/klimPaskov/hoi4-agent-tools/pull/122)) is stored with its corpus pins, hardware and comparator in [evidence/S04/q1-baseline](../evidence/S04/q1-baseline/README.md). The game pin moved to 1.19.4.2 in [#131](https://github.com/klimPaskov/hoi4-agent-tools/pull/131) | The nightly comparison is the Agent Tools session's |

Windows work still open before the move to the Mac (D-13a):

- hoi4-agent-tools release 4.9.0 is publishing, and pull requests [#98](https://github.com/klimPaskov/hoi4-agent-tools/pull/98), [#102](https://github.com/klimPaskov/hoi4-agent-tools/pull/102), [#103](https://github.com/klimPaskov/hoi4-agent-tools/pull/103) and [#106](https://github.com/klimPaskov/hoi4-agent-tools/pull/106) wait for CI.
- The lean `hoi4_wiki` pass is running on the kit branch `claude/lean-wiki` and the agent-tools branch `claude/lean-wiki-index`.
- The 3D route overhaul runs in the Chaos Redux and ZIN 3D sessions. Kit pull request [#76](https://github.com/klimPaskov/Agentic-HOI4-Modding/pull/76) and Setup pull request [#165](https://github.com/klimPaskov/HOI4-Mod-Setup/pull/165) are open, and the Meshy model comparison is due on 2026-11-08.
- hoi4-imagegen is deriving style packs from a mod and preparing the third gallery style, The Fire Rises.
- HOI4 Live Tools pull request [#11](https://github.com/klimPaskov/HOI4-Live-Tools/pull/11) is open.
- A kit package release must follow the last agent-tools release, so the coding-client profile that Setup installs names the same tool version as the Factory image.
- [Issue #3](https://github.com/klimPaskov/hoi4-factory/issues/3) asks for the S03 tests to run on Windows and for process-tree kills there. It blocks the move to the Mac (S04-3) and is being fixed first, in its own session on this PC.

Checks recorded for the move, because they only run on macOS:

- The real macOS Keychain sign-in of `hoi4-imagegen-mcp` (its layer L6, unexercised).
- The Mac setup of Factory itself: the `.env` file, the Anthropic key in `~/.zshrc`, the GitHub App key and Docker Desktop, as listed in [maintainer questions](../evidence/maintainer-questions.md) item 5.

The sections below describe the planning baseline. Statements in them that implementation has not started are historical.

## Revision of 2026-10-24

The maintainer selected a public hosted web application with GitHub or email accounts and invited collaborators, a Rust server with a native Rust agent loop and a React browser client, Docker deployment, native GitHub integration, a server-held read-only game reference with all DLC, free use with bring-your-own keys and a monthly allowance, GitHub issues and pull requests fully inside the app, Claude and OpenAI adapters, Stripe for later credits, a gVisor tool sandbox, account and project policies, automatic personal, project and global memory with verified lessons, test-gated automatic engineering learning, and the AGPL-3.0 license. The full records are ADR-011 through ADR-023 in the [maintainer decision record](02-architecture/maintainer-decisions-2026-10-24.md). ADR-002, the Tauri and Pi worker preference, is superseded.

The master goal, requirements, architecture, contracts, specs, stages, risks, tests and handoffs were revised to match. Twelve requirements were reworded and REQ-046 through REQ-050 were added for accounts, GitHub, the game reference, memory and memory isolation. Contract C-10 covers repository links. Twelve planned tests and four fixtures were added. The research files under `docs/00-research/` are kept as the original evidence record and still describe the desktop candidates.

The folder became a Git repository on 2026-10-24. Its first commit is the unmodified v02 package. The revision is the following commit. The validator's requirement count changed from 45 to 48 in the same change.

## Project-folder edition

The v02 ZIP extracts into one `hoi4-factory/` folder for direct use as the project root. All planning documentation is under `docs/`, the two maintenance utilities are under `scripts/`, and the client instructions, settings and skills remain at the project root. This revision reorganizes the completed v01 research baseline and repairs navigation and working-folder instructions. Product scope and architecture recommendations are unchanged.

## Completed planning scope

The package contains a source-grounded research recommendation, the 2026-10-24 maintainer decisions, product workflows, 50 stable requirements, 13 subsystem specs, ten proposed integration contracts, twenty-three ADRs, eight stages with 48 packets and 47 exit gates, a full test/evaluation design, resumable handoffs and real Claude/Codex starter assets.

All supplied Project Source text was read before external research. The exact distinct-file coverage and duplicate method are in [the research log](00-research/04-evidence-and-research-log.md) and [source coverage](00-research/supplied-source-coverage.json). Requested public repositories, protocols, official client docs, papers and articles are accounted for in [the registry](00-research/01-research-sources.md).

The actual Agentic configuration and all referenced role layers were inspected. The migration covers 196 scalar leaves. The starter preserves useful relationships, disables optional connections and adapts unsafe defaults through documented client-specific formats.

Independent reviews corrected the Setup service boundary, provider retry accounting, recoverable intent, minimal early context, redo behavior, deferred map drawing/generation contracts, diagram responsibilities and three-mode task scope.

## What was actually verified

| Activity | Actual scope |
|---|---|
| Supplied-source gate | Complete collaborative reads of 152 distinct text contents through 250 chunks, with duplicate byte equality and repaired truncated reads |
| Repository research | Selected implementation, tests, guidance, licenses, metadata and integration boundaries at exact pinned revisions |
| External documentation | Official client/protocol documents and selected paper methods/limitations, with documentation-only status where appropriate |
| Upstream CI observation | Selected existing CI results and Setup's failing recovery log were inspected, not executed locally |
| Agentic projection comparison | Pure isolated projection reproduced 23 role files and their registry with no differences |
| Starter QA | JSON/TOML parsing, documented key/shape checks, frontmatter, complete config mapping, canonical skill mirrors and deliberate negative drift checks |
| Package QA | Relative links, traceability, file coverage, manifests and archive checks as recorded in [the quality review](05-verification/PLANNING-PACKAGE-QUALITY-REVIEW.md) |

The exact executed artifact checks and counts are in [verification-run.json](05-verification/verification-run.json) and [starter checks](05-verification/starter-config-and-skills-checks.json). These files are authoritative for the artifact validation scope.

## What remains unverified

No product binary was built. No candidate runtime suite, native Setup installation, model API call, MCP connection, actual Claude/Codex loading trial, operating-system sandbox test, game run or contributor usability session was executed. The full standards-compliant JSON-schema engine was unavailable, so starter validation uses parsing and explicit documented-key checks rather than claiming exhaustive schema validation.

The source audit is selective for external repositories. It is not a claim that every external source file or transitive dependency was read. The supplied Project Source gate is the separate full-read assertion.

## Material decisions before implementation or release

1. S01 must resolve the selected Setup recovery envelope and prove the service binding on server workspaces. The observed exact-HEAD recovery CI failure remains an open defect signal.
2. S01 must measure the Rust agent loop, its provider adapters, containerized packaging and source preservation before dependencies are committed.
3. The first writable focus subset must be demonstrated, including bounded creation/linking if advertised in the three-mode slice.
4. Exact provider profiles, credentials, local model hardware and supported platforms remain to be chosen and tested.
5. Tenant containment for model-directed tools on shared servers and third-party executable containment require evidence. A separate process or container alone is insufficient.
6. Map synthesis remains unavailable in the audited path. S07 cannot enable it through a feature flag.
7. Game verification needs a separately recorded user-operated scenario. The server game reference supports source and static evidence only.
8. Public exposure needs a dependency and asset inventory, the game-rights review for a hosted service with a server game copy, a privacy policy and a chosen hosting provider.

These are future stage gates, not missing planning sections. Their owners and experiments are in [the risk register](05-verification/RISKS-DECISIONS-AND-OPEN-QUESTIONS.md).

## Exact continuation

On a new machine, follow the [developer setup guide](07-developer-workflow/DEVELOPER-SETUP.md) to its readiness checklist.

S04 is authorized and in progress on Windows with the [S04 goal prompt](06-agent-handoffs/GOAL-S04.md), as one low-priority session until about 2026-11-22 (D-36). Continue it from its latest handoff. Do not begin S05 or any later stage without a new authorization. Work across the other projects follows [next steps, November 2026](09-planning/NEXT-STEPS-2026-11.md).

The open questions in [maintainer questions](../evidence/maintainer-questions.md) items 11 and 12 wait until before S08 (D-44). The package stands alone without any earlier conversation.
