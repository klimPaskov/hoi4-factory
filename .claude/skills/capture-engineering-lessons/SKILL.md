---
name: capture-engineering-lessons
description: "Use at every substantive handoff, after a repeated failure or tool quirk, or when the maintainer states a preference about how the project is built. Apply useful lessons directly to CLAUDE.md and the skills, create new skills when a workflow repeats, and ask the maintainer only for changes that would widen authority, cost or scope."
---

# Capture and apply engineering lessons

## Trigger and inputs

Use this workflow at every substantive handoff, after a repeated failure, when a tool quirk cost time, when a working pattern is worth reusing, or when the maintainer states a preference about how the project is built. Inputs are the stage or task evidence, the current [instructions](../../../CLAUDE.md), the existing skills and the [changelog](../../../docs/07-developer-workflow/KNOWLEDGE-CHANGELOG.md).

Read [knowledge policy](../../../docs/07-developer-workflow/REUSABLE-KNOWLEDGE-POLICY.md), [instruction map](../../../docs/07-developer-workflow/SKILLS-AND-INSTRUCTION-MAP.md) and [runtime learning boundaries](../../../docs/03-subsystem-specs/004-context-memory-and-runtime-learning.md). Engineering lessons about building the factory belong here. Lessons about product users or mod canon belong to the product's runtime memory and design process.

## Procedure

1. Review what actually happened: failures, retries, recovery work, verification gaps, maintainer corrections and things that went smoothly because of a pattern. Compare with the current CLAUDE.md and skills before adding anything.
2. For each useful lesson, choose the smallest destination that will reach the next session: a line in CLAUDE.md for a stable project-wide rule, an edit to the skill whose workflow it improves, a new skill when a workflow repeats or a task type needs a written procedure, or a regression test when the lesson is testable.
3. Apply it now. Edit CLAUDE.md (never AGENTS.md, which is generated), edit or add canonical skill bodies under `docs/07-developer-workflow/skill-workflows/`, add new skills to `skills.json` and the skill sets in `scripts/sync_skills.py` and `scripts/validate_package.py`, add a trigger fixture, and run `PYTHONUTF8=1 py -3.13 scripts/sync_skills.py --write`. Add the regression test where feasible.
4. Keep each change specific and bounded. State the observation, the rule and when it applies. Avoid broad claims from one anecdote, and keep stage-specific workarounds in stage evidence.
5. Check the limit: a change must not widen authority, access roots, external data transfer, paid operations, approved product scope or game canon. If it would, write it up as a proposal and ask the maintainer instead of applying it.
6. Remove secrets, private data and hidden reasoning from anything you write.
7. Run `PYTHONUTF8=1 py -3.13 scripts/sync_skills.py --check` and the package validator, then add a changelog entry naming what changed and why.

## Outputs and evidence

Produce `evidence/SNN/learning-review.md` listing the lessons considered, what was applied and where, what was proposed to the maintainer, and what was set aside with the reason. Keep longer candidate records in `engineering/knowledge/candidates/` when a lesson needs reproduction detail. Update config-review.md when a lesson changes a tool, provider, permission or setup step.

## Stop and error paths

If a lesson cannot be confirmed, record it as captured with its limitation instead of turning it into a rule. If it conflicts with a maintainer decision, the decision wins until the maintainer changes it. If later evidence contradicts a rule, fix or remove the rule and note the reason in the changelog. Reverting guidance does not undo external side effects.
