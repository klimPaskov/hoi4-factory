# Package status and continuation

Version: v02, revised 2026-10-25. Phase: **implementation, S01 and S02 complete, S03 next**.

## Current state

| Stage | State | Evidence |
|---|---|---|
| S01 bootstrap and architecture probes | Complete and accepted. Rust host and native Rust agent loop selected, Setup binding proven, GitHub App and memory decisions recorded | [S01 handoff](../evidence/S01/handoff.md) |
| S02 accounts, server workspaces, Mod Setup and GitHub import | Complete. Email and GitHub sign-in, per-account workspaces with row-level security, ZIP and folder upload, GitHub import, the full Setup flow over HTTP against merged HOI4-Mod-Setup `c28198b`, and the browser Setup wizard. Gates G1 to G4 pass. G5 passes with the limitation recorded in the handoff | [S02 handoff](../evidence/S02/handoff.md), [checks](../evidence/S02/checks.json) |
| S03 native agent and durability | Authorized inputs recorded, not started | [goal prompt](06-agent-handoffs/GOAL-S03.md), [draft scope](../evidence/S03/scope.md) |
| UI design track (ADR-025) | Decided, runs after S03 and before S04. The maintainer chooses each page from live variants | [goal prompt](06-agent-handoffs/GOAL-UI.md) |
| S04 to S08 | Not authorized | [roadmap](04-implementation-stages/ROADMAP.md) |

A new machine is set up with the [developer setup guide](07-developer-workflow/DEVELOPER-SETUP.md). Open questions for the maintainer are in [maintainer questions](../evidence/maintainer-questions.md). Under ADR-024, CLAUDE.md and the skills are maintained by the implementing agent, and AGENTS.md is generated from CLAUDE.md.

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

On a new machine, follow the [developer setup guide](07-developer-workflow/DEVELOPER-SETUP.md) to its readiness checklist. Then launch Claude Code from the repository root, run `/start-implementation-stage S03`, and give it the [S03 goal prompt](06-agent-handoffs/GOAL-S03.md). The goal prompt is self-contained and lists what to read, the S03 maintainer decisions, what to build and how to finish.

Do not begin S04 automatically. S03 ends with its own handoff and a goal prompt for S04. The package stands alone without any earlier conversation.
