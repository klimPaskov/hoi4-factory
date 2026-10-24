# Package status and continuation

Version: v02. Date: 2026-10-23. Phase: **Research and planning complete, pending human review of architecture and roadmap**. Product implementation has not started.

## Project-folder edition

The v02 ZIP extracts into one `hoi4-factory/` folder for direct use as the project root. All planning documentation is under `docs/`, the two maintenance utilities are under `scripts/`, and the client instructions, settings and skills remain at the project root. This revision reorganizes the completed v01 research baseline and repairs navigation and working-folder instructions. Product scope and architecture recommendations are unchanged.

## Completed planning scope

The archive contains a source-grounded recommendation and alternative, product workflows, 45 stable requirements, 13 subsystem specs, nine proposed integration contracts, ten provisional ADRs, eight stages with 48 packets and 47 exit gates, a full test/evaluation design, resumable handoffs and real Claude/Codex starter assets.

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

1. S01 must resolve the selected Setup recovery envelope and prove the native service binding. The observed exact-HEAD recovery CI failure remains an open defect signal.
2. S01 must compare real packaging, source preservation and runtime recovery before selecting Candidate A or B.
3. The first writable focus subset must be demonstrated, including bounded creation/linking if advertised in the three-mode slice.
4. Exact provider profiles, credentials, local model hardware and supported platforms remain to be chosen and tested.
5. Third-party executable containment requires evidence. A separate process alone is insufficient.
6. Map synthesis remains unavailable in the audited path. S07 cannot enable it through a feature flag.
7. Game verification needs a qualified platform/build or a separately recorded user-operated scenario.
8. Distribution needs a dependency/asset inventory and resolution of the applicable game, provider and business-model terms.

These are future stage gates, not missing planning sections. Their owners and experiments are in [the risk register](05-verification/RISKS-DECISIONS-AND-OPEN-QUESTIONS.md).

## Exact continuation

Review the executive recommendation, architecture alternatives, risk register and roadmap. If S01 is authorized, open the extracted `hoi4-factory/` working folder, read its local instructions, and use [CLAUDE-CODE-START-STAGE-01](06-agent-handoffs/CLAUDE-CODE-START-STAGE-01.md). Use the guidance already in place. If integrating into another repository, merge its existing guidance carefully. Record environment and source pins, run bounded probes and return the decision receipt.

Do not begin S02 automatically. The next reviewer should receive S01's actual scope, revision, checks, failures, selected bindings, supported operations, remaining limits, learning review and configuration review. The package stands alone without this conversation.

