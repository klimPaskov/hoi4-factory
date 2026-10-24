---
name: capture-engineering-lessons
description: "Use automatically at every substantive engineering handoff or after a repeatable tooling failure to capture and test reusable guidance, promoting test-backed lessons that change no authority under ADR-023. Keep candidates separate from accepted policy, product runtime memory and game design canon."
---

# Capture and review engineering lessons

## Trigger and inputs

Use this workflow at an explicitly requested substantive handoff, after a repeated failure or when a tool limitation suggests a reusable improvement. Inputs are the stage/task evidence, relevant accepted guidance, exact versions and the proposed behavioral change. A lesson about product users or mod canon belongs to its separate runtime/design process.

Read [knowledge policy](../../../docs/07-developer-workflow/REUSABLE-KNOWLEDGE-POLICY.md), [instruction map](../../../docs/07-developer-workflow/SKILLS-AND-INSTRUCTION-MAP.md), [changelog](../../../docs/07-developer-workflow/KNOWLEDGE-CHANGELOG.md), and [runtime learning boundaries](../../../docs/03-subsystem-specs/004-context-memory-and-runtime-learning.md).

## Procedure

1. Review the task's actual outcomes, failed attempts, recovery work and verification gaps. Compare relevant existing lessons before proposing another rule. If no reusable change emerged, record that conclusion with a short reason.
2. Select a specific observation. Distinguish a reproducible defect, a tool/version limitation, an unclear requirement and a useful workflow improvement. Do not infer broad model performance from one anecdote.
3. Create a C-07 candidate in `engineering/knowledge/candidates/<id>.md` with domain engineering, evidence, source hashes, reproduction, owner, scope, version bounds, counterexample, proposed diff, review status and rollback.
4. Remove secrets, unrelated personal data and hidden thought artifacts. Keep enough nonsecret detail to reproduce the claim. Mark unsupported observations captured rather than accepted.
5. Test the narrow claim where feasible. Use deterministic artifact checks and positive/negative workflow cases. A model trial must record its actual client/model, prompt, trace and outcome. Do not invent a success rate.
6. Choose one destination: stable rule, focused skill, linked reference, config guide, regression test or task evidence. Keep the smallest useful change. A stage-specific workaround should not become universal policy without justification.
7. Route the candidate for the applicable review. A change that broadens authority, costs, external effects, product scope or game design needs the appropriate owner decision. Routine corrections within an authorized stage follow normal repository review. Capture and review do not themselves approve a broader change.
8. For an accepted change, update canonical guidance, regenerate all client skill projections, adjust relevant fixtures, run drift checks, and record the changelog entry. Add supersedes and expiry links. Keep rejected or deferred candidates traceable.

## Outputs and evidence

Produce `evidence/SNN/learning-review.md` listing observations considered, candidate IDs, accepted changes, rejected or deferred items, affected versions and the next review trigger. Link the actual diff and tests. Update config-review.md if the lesson changes a tool, provider, context assumption, permission or setup procedure.

## Stop and error paths

Do not promote a claim without evidence or review. If a tool cannot reproduce the result, keep the candidate bounded and mark its limitation. If a proposed instruction conflicts with accepted authority, record the conflict and route the decision instead of overriding it. Do not modify runtime memory or approved game canon as a side effect.

If new evidence contradicts accepted guidance, mark it for review and preserve history. Recovery uses a reviewed Git revision of canonical sources followed by regeneration and the affected checks. Reverting guidance does not undo remote side effects.
