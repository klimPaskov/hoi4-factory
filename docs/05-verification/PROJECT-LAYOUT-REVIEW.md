# Project layout review

Project: `hoi4-factory`. Distribution version: **v02**. Date: 2026-10-23.

## Requested change

Prepare the completed planning package as the main working project folder, with one stable parent folder and all planning documentation under `docs/`.

## Delivered organization

| Location | Role |
|---|---|
| Project root | Short README, AGENTS.md, CLAUDE.md and native client configuration |
| `docs/` | Documentation index, master goal, status, manifests and all eight numbered documentation areas |
| `scripts/` | The relocated planning validator and skill synchronization utility |
| `.claude/`, `.codex/`, `.agents/` | Existing client-specific settings and reusable skill copies |

The ZIP contains a single `hoi4-factory/` parent. Every file from the v01 planning package has a corresponding file in this project layout. The root README and this review are additions. No source repository, application scaffold, dependency installation, Git history or game asset is bundled.

The research baseline, requirements, architecture alternatives, 13 subsystem specifications, eight stages, 48 work packets, 47 gates, 43 test definitions and 196-field configuration migration are retained. This is an organizational revision. The completed research and future implementation gates keep their original evidence status.

## Changes needed beyond moving files

Relative documentation links now resolve from their new locations. Root instructions and plain project-relative command paths point into `docs/` and `scripts/` as appropriate. Stage handoffs use this working folder directly and require merging only when integrating into a different existing repository.

Canonical workflow bodies now live one directory level deeper than their generated skill files. The synchronization utility renders each body's local Markdown links relative to its destination, preserving a single canonical source and correct navigation in all three client directories. The workflow metadata version is 0.1.1.

The validator retains its strict default mode for the frozen distribution. Its explicit `--working-tree` mode checks maintained documentation and owned starter assets while allowing unrelated application source, Git metadata and stage evidence. It skips frozen manifest hashes and reports that exclusion. It cannot be combined with archive verification.

## Verification and evidence interpretation

The current [verification receipt](verification-run.json) records the v02 distribution checks. The final ZIP is checked separately against the finalized directory after writing has stopped. The [quality review](PLANNING-PACKAGE-QUALITY-REVIEW.md) documents commands and limitations.

The source-specific [starter receipt](starter-config-and-skills-checks.json) and [validator negative fixtures](validator-negative-fixtures.json) retain their original v01 checker identities. They are historical evidence from the completed planning baseline. They are not relabeled as executions of the revised scripts. Current static parsing, field mapping, skill rendering, link, manifest and archive checks are recorded by the v02 verification run.

Focused checks of the relocated utilities cover generated local links, fragment and external-link preservation, refusal of links outside the project, working-tree acceptance of unrelated repository files, and rejection of working-tree mode combined with archive verification. These artifact checks do not invoke a model, connect an MCP server, load a live client or run HOI4.

## Working-folder entry point

Open the extracted `hoi4-factory/` folder itself. Use the [root README](../../README.md) for setup navigation, the [documentation index](../README.md) for the full plan, and the [Claude Code S01 handoff](../06-agent-handoffs/CLAUDE-CODE-START-STAGE-01.md) for the first authorized implementation stage.
