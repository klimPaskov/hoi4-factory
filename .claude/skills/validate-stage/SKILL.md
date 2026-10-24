---
name: validate-stage
description: "Use on explicit invocation to review one HOI4 Factory stage against its acceptance rows, changed artifacts and actual evidence. Distinguish pass, fail, unavailable and not applicable. Do not claim game behavior from source checks or authorize the next stage."
disable-model-invocation: true
---

# Validate one stage

## Trigger and inputs

Use this workflow for an explicitly requested review of one stage's completion. Inputs are the stage packet, scope record, acceptance rows, exact base/current revision, changed paths and collected evidence. Review may proceed without implementation authority, but it must not expand into a new feature or stage.

Read [shared rules](../../../AGENTS.md), [test strategy](../../../docs/05-verification/TEST-STRATEGY.md), [acceptance matrix](../../../docs/05-verification/REQUIREMENTS-ACCEPTANCE-MATRIX.md), [evidence contracts](../../../docs/02-architecture/contracts-and-interfaces.md), and [handoff template](../../../docs/06-agent-handoffs/STAGE-HANDOFF-TEMPLATE.md).

## Procedure

1. Reconfirm the stage scope, selected release surface and actual acceptance criteria. Check that claimed dependency evidence belongs to the current input revision or a documented compatible baseline.
2. Inspect the diff and resulting artifacts. Map each relevant requirement to an output, registration or consumer where applicable, observed behavior and evidence. Counts, filenames or a generated success message alone are insufficient.
3. Distinguish static parsing, semantic validation, preview rendering, tool execution, game observation, user approval and licensing review. Record exactly which kind exists. Never translate one kind into another.
4. Run the required meaningful checks available in the environment. Include normal behavior, critical failures and recovery cases from the stage packet. Record command, version, input snapshot, exit result, artifacts, coverage and limitations as C-05 evidence.
5. Check provenance, permissions and rollback. If a tool timed out after an effect may have occurred, inspect its receipt and reconcile before retry. Verify changed files preserve unrelated user edits and approved semantics.
6. For guidance/config changes, run the skill drift checker, real JSON/TOML parsing, relevant trigger fixtures and documented key checks. Clearly separate static fixture validation from an actual client/model trial.
7. Assign each acceptance row pass, fail, unavailable or not applicable with a reason and evidence reference. A required unavailable check prevents a full completion claim unless the stage explicitly permits a narrower result and the authorized owner accepts that scope.
8. Review whether further testing resolves a concrete remaining risk. Stop optional repetition when the required gate and material risks are adequately covered. Finish checks.json and the stage disposition.

## Outputs and evidence

Produce `evidence/SNN/checks.json` with stage ID, revision, per-check status, exact command, tool versions, evidence kind, artifact references and limitations. Update handoff.md with passed and unresolved criteria, the actual confidence boundary and a precise next action. Use hashes where input identity matters. Do not include secrets or hidden provider reasoning.

## Stop and error paths

If an acceptance criterion is ambiguous, report the ambiguity and the smallest decision needed. Do not silently weaken it. If the game, credentials or optional server is unavailable, perform useful source checks and label the unavailable layer. A successful preview is not in-game verification. A successful schema parse is not an installed-client load test.

If evidence is stale, corrupted or missing, mark the affected result unresolved and recreate only the needed check. Do not regenerate user assets just to produce a green report. Validation does not authorize publishing, merging or beginning the next stage.
