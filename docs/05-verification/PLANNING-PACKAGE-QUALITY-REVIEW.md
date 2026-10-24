# Planning package quality review

Project: hoi4-factory. Distribution version: v02. Review date: 2026-10-23. Scope: **Research and planning artifacts only**.

## Acceptance basis

This package was checked against the complete supplied brief, with separate reviews of source reuse, runtime/security contracts and developer guidance. The final automated result is recorded in [verification-run.json](verification-run.json). The distributed archive is additionally checked read-only against the finalized directory after ZIP creation.

The review does not claim implemented product behavior. Native runtime, actual client loading, skill invocation, MCP connections, paid models, operating-system containment, actual game runs and user trials remain future stage tests. The validator's used-key checks are not an exhaustive JSON Schema implementation.

## Required content coverage

| Required area | Review outcome and evidence |
|---|---|
| Full source protocol and supplied examples | Complete collaborative full reading, duplicate byte verification and 250-chunk coverage in the [source log](../00-research/04-evidence-and-research-log.md) |
| Primary external evidence | 77 normalized records with exact pins or inspected document URLs, source/test/guidance coverage, licenses, metadata and limits in [the registry](../00-research/source-registry.json) |
| Product vision and requirements | Stable master goal, users/scenarios and 45 requirements with MVP, later or research ownership |
| Multiple architecture candidates | Tauri/Pi and Electron/OpenCode with the same host authority, plus OpenHands/custom/fork alternatives and reconsideration triggers |
| Specific Setup reuse | Existing portable planning core plus desktop scanner/transaction services, actual Tauri surface, provider-analysis gates and C-01 handoff |
| Deep subsystem coverage | 13 substantive specs covering normal, failure, recovery, source integrity, verification and open experiments |
| Optional visual/autonomous convergence | Shared proposal/grant/source pipeline, focus editor, stale preview handling, undo/redo and no-canvas execution |
| Map ambition with honest limits | Structured import and bounded S07 operation, source-linked reference workflow and deferred drawing/world-design contracts |
| Dependency-led implementation | Eight stages, 48 owned packets, explicit dependencies/non-goals, 47 exit gates, rollback and continuation stop |
| Verification and evaluation | 43 stable test definitions, original fixture design, all required scenarios and two API profiles plus one local profile, without invented results |
| Requirement and gate traceability | All 45 requirements and all 47 exit gates map to defined tests and observable outcomes |
| Claude-first portable development | AGENTS, importing CLAUDE, four real skills under each required client path plus current Codex discovery mirrors |
| Actual configuration migration | 196 unique source-field mappings from pinned root and 23 role files, separate native client settings and disabled MCP connections |
| Governed improvement | Evidence-based learning review, minimal change or justified no change, cross-client synchronization, approval and rollback |
| Packaging and provenance | Relative links, complete inventory, SHA-256 manifest, hidden starter files and archive/member checks |

No intentionally deferred capability is silently removed. The backlog preserves its user value, prerequisite contracts and promotion tests. Full map generation and large swarms remain future work. A narrowed focus release must explicitly exclude later features and still pass relevant release gates.

## Executed static and investigative checks

The Python package validator checks required paths, all JSON/TOML parsing and Python syntax, relative Markdown file/heading links, prose/fence structure, specification/stage structure, requirement/test/gate mappings, skill frontmatter and mirrors, starter settings and exact config field identities, normalized source IDs, source reading coverage, artifact hygiene, manifest hashes and optional ZIP inventory/CRC/member bytes.

The retained v01 [starter-specific receipt](starter-config-and-skills-checks.json) additionally records documented schema pins, actual source-layer hashes, deliberate skill-drift rejection, a missing-workflow-section negative fixture and the distinction between static triggers and live client invocation. Its limited upstream projection check is also recorded in [agentic-projection-check.json](../00-research/agentic-projection-check.json).

These checks do not establish that an external service is reachable or a provider is permitted. Both inherited MCP relationships are intentionally disabled. No file in this package installs a dependency or enables a connection merely because it is opened.

## Independent review corrections

Three reviewers checked different failure classes. The final author integrated their findings and reread the affected contracts.

| Finding | Resolution |
|---|---|
| Short architecture summaries could imply Setup's portable crate includes native installation | Explicitly include extraction/adaptation of desktop scanner and transaction services |
| Conceptual UI labels could be mistaken for a second task enum | Map labels to authoritative task/attempt states, with pause as a durable scheduler condition |
| A receipt hash alone could not reconstruct a recovery query | Persist immutable normalized intent, credential references, retry class, idempotency and reconciliation inputs |
| Uncertain provider requests could be retried as harmless reads | Preserve possible billing effects and require the explicit bounded retry policy |
| S03 needed context before S05's full builder | Add minimal immutable C-06 and its restart fixture in S03 |
| Three-mode focus creation needed a precise supported subset | Probe node/link creation in S01, require verified subset in S04 and narrow claims if unavailable |
| Redo and deferred map drawing were under-specified | Add current-source redo acceptance and typed future drawing/design contracts |
| Mixed-provider E04 trials changed budget arithmetic | Define the switch schedule, 57 episodes with possible API spend and 30 with possible local compute |
| Repository guidance/release identities needed fuller coverage | Inspect present guidance and record development, private beta, stable release and generated-context limits separately |
| Validator negative probes exposed false passes | Correct ZIP directory checking, report-write ordering, exact field identities, nested active-key checks and duplicate manifest rejection |

No product defect is claimed fixed by these document corrections. They improve the plan and its own validation.

## Negative testing of the package validator

The independent reviewer used isolated copies rather than mutating the deliverable. [Negative fixture evidence](validator-negative-fixtures.json) retains the v01 checker identity and observed rejections. Those original probes are historical evidence. The v02 layout changes and focused utility checks are described in the [project layout review](PROJECT-LAYOUT-REVIEW.md), with current static validation in verification-run.json.

The suite covers an invented Claude setting, an altered source field with the same CSV row count, duplicate manifest entries, an unsafe directory-only ZIP member, an in-package report write during archive comparison, and an unsafe manifest path. Path safety is checked before inspecting or reading a manifest target outside the package.

Negative fixture failures are expected successes of the checker. They are distinct from a failing delivered package. No temporary malicious fixture is distributed.

## Diagram and link review

Relative links and heading fragments are checked after the full directory exists. Source URLs are pinned where available. Existing primary retrieval established the cited sources, while a live HTTP check of every external URL is not claimed.

Mermaid diagrams were reviewed for component responsibility, dependency order, state-label clarity and bounded width. Code fences and surrounding Markdown were checked programmatically. A Mermaid rendering engine was not executed. The diagrams are compact text diagrams that can be rendered by a supporting reader.

Future implementation paths shown in code or handoff templates are deliberately not distributed as empty evidence stubs. They describe files the authorized future stage must create. Every actual Markdown link targets an included file or an external source.

## Archive and manifest rules

[docs/MANIFEST.json](../MANIFEST.json) records path, length and SHA-256 for every package file except itself and verification-run.json. These two self-audit exclusions avoid recursive hashes. docs/MANIFEST.md contains the complete readable inventory including those files.

The ZIP must have one `hoi4-factory/` top-level directory, preserve all hidden starter paths, contain no symlink or unsafe path, and match every finalized file byte-for-byte. The final read-only archive check runs after file writing has stopped. It refuses to write a report inside the package during comparison, preventing the check from invalidating its own input.

The filesystem verification report intentionally has archive_checked false because it is produced before packaging. Final archive verification is a separate read-only run, summarized in this review and the delivery response. Re-running the archive command below independently checks the delivered bytes.

## Reproducible commands

From the unpacked root with Python 3.11 or newer:

```bash
python scripts/sync_skills.py --check
python scripts/validate_package.py --root .
python scripts/validate_package.py --root . --archive ../hoi4-factory-v02.zip
```

Adjust only the archive path if the ZIP is stored elsewhere. The checker reads the package and archive. Its ordinary invocation does not install, connect, invoke a model, launch a game or rewrite the package. An optional filesystem report can be written with --write-report before packaging. An archive report, if requested, must be outside the package.

## Development checks after extraction

The manifest records the frozen distribution. During implementation, new source files, Git metadata and stage evidence are expected. Use `python scripts/validate_package.py --root . --working-tree` to check the maintained planning and owned guidance without enforcing those original hashes or inspecting unrelated application files. The report explicitly marks manifest comparison skipped. This mode does not replace stage tests and cannot be combined with `--archive`.

## Residual verification limits

The credential scan is pattern-based. It found no matching credential material or personal machine paths in distributed artifacts, but no heuristic detects every possible encoded secret. The source files and starter values were also reviewed for provenance and intentionally dormant connections.

The hash manifest detects accidental change and supports reproducibility. It is not a publisher signature or a guarantee of source authenticity. Upstream licenses and current client behavior must be rechecked when implementation pins change.

The remaining product unknowns are cataloged in [risks and experiments](RISKS-DECISIONS-AND-OPEN-QUESTIONS.md). They are represented as future tests, not waived by artifact QA.

