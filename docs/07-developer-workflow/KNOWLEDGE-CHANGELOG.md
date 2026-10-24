# Engineering knowledge changelog

This is the versioned index of accepted engineering guidance changes. Detailed candidates retain evidence and review records. It is separate from product runtime memory and game design canon. Follow [the policy](REUSABLE-KNOWLEDGE-POLICY.md).

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
