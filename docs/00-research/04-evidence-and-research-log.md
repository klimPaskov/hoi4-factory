# Evidence and research log

Date: 2026-10-23. This log distinguishes supplied-source reading, external source inspection, actual investigative checks and proposed product verification. It is not a transcript of every retrieval call.

## Mandatory supplied-source gate

The supplied source protocol was read before substantive research. All available Project Source archives and nested archives were inventoried and safely extracted into a temporary audit workspace. The resulting coverage was 199 file occurrences, including 194 text occurrences and five archive occurrences. Four distinct archives contained duplicate distributions of some material.

Deduplication used both SHA-256 and full byte equality. The 194 text occurrences resolved to **152 distinct text contents, totaling 3,286,648 bytes**. Those contents were divided into **250 deterministic reading chunks**. Every chunk was fully displayed and read by one of four readers before external research began. Truncated retrieval attempts were repaired by complete smaller reads before completion was recorded.

| Reader assignment | Distinct text contents | Bytes | Full chunks |
|---|---:|---:|---:|
| Root | 17 | 746,213 | 46 |
| Reader A | 45 | 846,606 | 69 |
| Reader B | 45 | 847,247 | 68 |
| Reader C | 45 | 846,582 | 67 |
| Total | 152 | 3,286,648 | 250 |

This is a collaborative full-read assertion. It does not claim that every reader personally read all 152 contents. The union of assigned chunks is exactly 1 through 250, without gaps or overlapping ownership. No unsafe archive member or extraction error was recorded. [Machine-readable coverage](supplied-source-coverage.json)

The archive contains coverage metadata and hashes, not copies of the user's supplied mod content. A future reviewer can match an authorized source against its recorded hash without distributing that material here.

## How supplied guidance affected this plan

| Supplied evidence or conflict | Resolution in this package |
|---|---|
| Chaos Redux-style master goals, specs, stage packets and continuation conventions | Adopt explicit source truth, bounded ownership, exit gates, evidence and self-contained handoffs |
| Old and newer instruction layouts | Prefer the newer dated canonical .claude sources where they supersede older examples, while preserving actual current target-repository precedence |
| Old mandatory render/retry language versus newer bounded-tool guidance | Keep bounded attempts and honest limits when an optional tool is unavailable |
| Preview language that could imply exact game fidelity | Separate source renders, static checks, simulations and actual game observations |
| Game-event quotas, specific story content and Chaos balancing conventions | Treat them as project-specific evidence, not product-wide requirements |
| Historical large context and worker settings | Do not attribute them to the actual current Agentic TOML or all model providers |
| Existing Git ownership and staged-work rules | Use owned paths, expected HEAD and a temporary index where required, with preservation of unrelated work |
| Learning guidance | Promote minimal evidenced improvements, permit justified no change and prevent self-granted authority |

The current user's brief defines this planning assignment. Historical examples cannot reduce its required technical scope or authorize implementation.

## External research method

The team inspected the 21 named repositories through approved GitHub retrieval, plus official protocols/client documents, two research papers and the named workflow articles. Repository observations use exact commits. Relevant implementation paths, interfaces, test fixtures, licenses, source guidance and selected CI evidence were read. External code inspection is selective, with exact paths or ranges in the ledgers.

A retrieved tree or file is not automatically counted as inspected. B's ledger distinguishes fetched files from read sections. Root guidance audit records resolved symlink contents separately from original Git blobs. Entire and MFS use AGENTS symlinks to CLAUDE, while agentglass's complete pinned tree has none of the requested guidance filenames. Those facts inform reuse and do not authorize copying upstream installation or publication procedures. [Guidance audit](context-guidance-audit.json)

Maintenance observations include branch, commit date, archived status, recent activity and observed release metadata. Pushed time is not substituted for audited commit time. A manifest version is not assumed to equal the latest stable release. Activity does not establish production reliability or guaranteed future support.

## High-impact source findings

The [asset audit](02-existing-assets-audit.md) describes actual Mod Setup UI and service boundaries, source planning, conflict handling, generated outputs and native recovery. Its exact audited HEAD has an observed failed Windows recovery candidate with unequal restoration inventories. This is an upstream result, not a locally reproduced root cause. S01 carries the explicit experiment.

The runtime comparison corrects stale assumptions: OpenCode has a current in-process SDK, Pi has both small core libraries and a durable package, Fusion embeds Pi, and Gas City has runtime abstractions beyond terminal sessions. Their actual persistence and authority boundaries still need adaptation. [Candidate comparison](03-candidate-comparison.md)

The actual Agentic config is .codex/config.toml at the pinned revision, with all 23 referenced role files. Its 58 root scalar leaves and 138 role leaves produce the exhaustive 196-row mapping. Model-context and compaction overrides are absent. The original full-access/never-approval defaults are intentionally adapted in safe starter files. [Configuration migration](../07-developer-workflow/CONFIG-MIGRATION-REPORT.md)

Map inspection and rendering are distinct from native world generation. The inspected geography-model synthesis write path is unavailable. Live game integration is also platform/build qualified. These limitations directly shape S07 and the evidence UI.

## Actual investigative checks

| Check | Observed result | Limit |
|---|---|---|
| Source inventory, byte equality and chunk accounting | Complete coverage and no extraction errors | Verifies reading coverage, not product behavior |
| Pinned GitHub source, license and tree inspection | Source assertions tied to exact paths and revisions | Selective external audit, not every transitive dependency |
| Existing upstream CI retrieval | Named success/failure observations including Setup recovery failure | No upstream job was run by this assignment |
| Agentic pure projection comparison | 23 role files plus generated registry reproduced without differences | No upstream suite, live client, bootstrap installer or MCP call |
| Root guidance blob identity | Seven regular entries and two symlink targets reconciled with fetched bodies | Guidance behavior remains a declaration unless implementation evidence supports it |
| Starter and archive checks | Recorded in [package QA](../05-verification/PLANNING-PACKAGE-QUALITY-REVIEW.md) | Structural validation cannot prove actual client invocation or product correctness |

[Agentic projection result](agentic-projection-check.json) and [starter checks](../05-verification/starter-config-and-skills-checks.json) preserve the limited executed evidence.

## Access and verification limits

Direct shell networking through urllib and Git was blocked by the environment. Approved GitHub and web retrieval worked. Failed path guesses were corrected from actual trees, with the relevant failure retained in the ledger. This access limitation is not treated as a repository defect.

No installed HOI4, Clausewitz, Meshy or Blender tool was available through the enabled tool inventory. No native Setup application, candidate runtime suite, external coding client, model API, MCP server, game process or usability trial was executed. The effectful Agentic bootstrap script was inspected and deliberately not invoked. Its quiet flag is not a read-only mode.

The source-code harness paper was read for its methodology, subsystem taxonomy and limitations. The runtime paper was read for its evidence model and controlled example. Neither is a HOI4 benchmark. The source-code paper's weaker Claude-source basis was noted without obtaining leaked code. Its metadata discrepancy is preserved in SRC-R09 instead of silently corrected.

Official docs are time-sensitive. Their date, URLs and available schema pins are recorded. Installed-client behavior remains a future S01 check. The supplied publisher agreement is recorded with its applicability limit, not used to infer a universal commercial permission.

## Synthesis and independent review

Research findings were translated into requirements, contracts and provisional ADRs before stage planning. Three reviewers then checked the brief, subsystem boundaries, stage dependencies, handoff continuity and starter behavior. Corrections included the desktop Setup extraction boundary, durable pause semantics, reconstructable intent, model-request cost uncertainty, initial C-06 context, deferred map drawing contracts and redo against current source.

The final pass validates all required content areas, requirements-to-tests coverage, every stage gate, actual starter content, relative links and ZIP inventory. The [quality review](../05-verification/PLANNING-PACKAGE-QUALITY-REVIEW.md) reports actual results and remaining verification scope.

## Rechecking evidence later

Re-fetch the exact pinned path before modifying a reused component. Read its current local instructions and the upstream change history if selecting a newer version. A source upgrade invalidates affected capability, preservation, recovery, config and license evidence. Preserve the old record, add the new pin and explain the changed decision. Do not overwrite provenance with “latest.”

