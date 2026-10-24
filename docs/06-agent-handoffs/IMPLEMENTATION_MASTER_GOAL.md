# Implementation master goal

Status: Proposed long-term implementation charter. This file is usable without the original conversation. It does not authorize implementation by itself. Read [package status](../PACKAGE_STATUS.md) to distinguish completed planning from future work.

## Purpose

Build HOI4 Factory, an independent application for creating and maintaining Hearts of Iron IV mods. A user may work through conversation, visual editors and previews, or bounded autonomous agent tasks. All three modes operate on the same authoritative project and can be combined without a separate export-and-reimport workflow. The application must remain usable without an external coding-agent CLI.

Reuse HOI4 Mod Setup for proven create/import and workspace preparation behavior. Reuse existing HOI4 tools through explicit adapters with provenance, validation and appropriate permissions. Do not replace working setup logic with an unverified copier. Preserve user files, source formatting and unsupported syntax when editing. Keep preview fidelity labels and actual game evidence distinct.

The future native runtime negotiates provider/model capabilities and owns durable tasks, tool receipts, recovery and scoped approval. Claude Code is the primary tool for building the application. It is not an architectural requirement imposed on the application's users. A chat subscription does not imply API credentials, and no model performance claim is valid without a recorded evaluation.

## Authority and navigation

Use the extracted `hoi4-factory/` folder as the working project root. Read the current user instruction and applicable repository guidance first. Then read [AGENTS.md](../../AGENTS.md), [CLAUDE.md](../../CLAUDE.md) when using Claude, [docs/MASTER_GOAL.md](../MASTER_GOAL.md), the one approved stage packet, its referenced specs, prior evidence and accepted decisions. Guidance and client files are already in place. Preserve existing team instructions if integrating into another repository. Research and historical notes inform decisions but do not override accepted requirements or authorize effects.

Use [the roadmap](../04-implementation-stages/ROADMAP.md) for sequence, [architecture options](../02-architecture/options-and-decisions.md) for reversible decisions, [contracts](../02-architecture/contracts-and-interfaces.md) for shared data and [the acceptance matrix](../05-verification/REQUIREMENTS-ACCEPTANCE-MATRIX.md) for observable outcomes. The stage packet is the detailed task boundary. Do not independently redefine it from this charter.

## First milestone and first stage

The first proposed useful demonstration is S04, a bounded focus-authoring task exercised through conversational, visual and autonomous modes with a shared source workspace and evidence. S01 through S03 establish the necessary environment, setup reuse, durable state and permission boundaries. Later collaboration, content and map capabilities follow their own staged scope.

The next implementation request should authorize S01 only. That stage verifies and adapts the developer files already in this project folder, checks actual instruction and skill loading, and runs the bounded architectural probes described in its packet. It must confront the observed Setup exact-recovery uncertainty, source preservation, unsafe replay and revocation, and packaging a native provider loop without an external coding CLI. Experiments produce evidence and decision receipts. They do not authorize building the full product.

## Quality and completion bar

Every stage connects requirement IDs to changed artifacts, actual checks and evidence. Treat syntax, semantic checks, preview, tool execution and game observation as different evidence kinds. Preserve partial results and limitations. An unavailable client, game, credential or server remains unavailable, not passed.

Use ordinary Git review and isolated ownership. Keep secrets and personal configuration out of the repository. Maintain explicit effect authority and reconcile unknown outcomes before retry. Preserve source lineage, artifact hashes and rollback information. Do not silently expand into hosted collaboration, world generation or broad asset automation to make an early milestone look complete.

At handoff, deliver `evidence/SNN/scope.md`, environment.json, checks.json, config-review.md, learning-review.md and handoff.md under the project root. Review reusable engineering knowledge, update accepted guidance through its review process, and keep client skill projections consistent. Use the validator's `--working-tree` mode for evolving documentation and guidance, with stage-specific tests for implementation behavior. The [handoff template](STAGE-HANDOFF-TEMPLATE.md) carries exact next dependencies. Completion of one stage proposes the next stage and does not authorize it.
