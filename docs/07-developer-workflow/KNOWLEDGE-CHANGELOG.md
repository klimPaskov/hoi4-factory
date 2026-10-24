# Engineering knowledge changelog

This is the versioned index of accepted engineering guidance changes. Detailed candidates retain evidence and review records. It is separate from product runtime memory and game design canon. Follow [the policy](REUSABLE-KNOWLEDGE-POLICY.md).

## 0.1.2, 2026-10-24, maintainer revision and repository setup

- The package validator now expects REQ-001 through REQ-048, after the maintainer decisions added three requirements.
- The README says to set `PYTHONUTF8=1` on Windows. Without it the validator reads files in the locale encoding and fails on UTF-8 text. This was observed with Python 3.13 on Windows 11. The default `python` on that machine was 3.9, which also lacks `tomllib`.
- Added `.gitattributes` to keep text files LF. Python text-mode writes and some editors on Windows converted whole files to CRLF during the revision.
- Distribution-mode validation no longer matches the revised files. The frozen manifest describes the original v02 package, kept as the first commit.

Evidence: validator runs recorded in the revision session, the [maintainer decision record](../02-architecture/maintainer-decisions-2026-10-24.md) and the repository history. Scope: planning tooling and repository hygiene. No runtime, provider or game result is implied.

## 0.1.1, 2026-10-23, working project layout

- Organized the project under one stable `hoi4-factory/` root with all planning documentation in `docs/` and maintenance utilities in `scripts/`.
- Updated project entry points and stage handoffs to use the extracted folder directly.
- Rebased canonical workflow links when generating client copies, because canonical sources and generated skills now have different directory depths.
- Separated frozen distribution validation from explicit working-tree checks so ordinary implementation files do not invalidate the planning check.

Evidence: [project layout review](../05-verification/PROJECT-LAYOUT-REVIEW.md), the updated scripts, the regenerated skill manifest and the current package verification receipt. Scope: file organization and navigation. No runtime, provider or game result is implied.

## 0.1.0, 2026-10-23, planning starter

- Added shared AGENTS authority with explicit Claude import.
- Added four canonical workflow bodies with Claude, required Codex and current `.agents` discovery projections.
- Added complete 196-field source configuration mapping and dormant MCP definitions.
- Recorded client discovery and schema compatibility findings from current official documentation.
- Added candidate review, counterexample, applicability, expiry and rollback rules.

Evidence: [migration report](CONFIG-MIGRATION-REPORT.md), [instruction map](SKILLS-AND-INSTRUCTION-MAP.md), and [package quality review](../05-verification/PLANNING-PACKAGE-QUALITY-REVIEW.md). Status: planning artifact authored and structurally checked as recorded by package QA. No client load, model trigger, MCP runtime or game result is implied.

## Future entry format

Record date and revision, candidate ID, changed guidance, reason, affected clients and versions, actual evidence, reviewer and authority, compatibility impact, superseded guidance and rollback revision. If a stage yields no reusable lesson, record that in its learning review instead of adding a ceremonial changelog entry.
