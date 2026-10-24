---
name: production-readiness-review
description: "Use at a stage handoff, before real users can reach a deployment, or when asked what is worth hardening next. Treat the production readiness checklist as a menu, not requirements, recommend only proportionate low-friction items, and leave enterprise and compliance items alone unless the maintainer asks."
---

# Review production readiness

## Trigger and inputs

Use this workflow at a stage handoff, before real users can reach a deployment, or when the maintainer asks what is worth hardening next. Inputs are the [production readiness checklist](../../../docs/05-verification/PRODUCTION-READINESS-CHECKLIST.md), the current stage scope and evidence, and the [maintainer decisions](../../../docs/02-architecture/maintainer-decisions-2026-10-24.md).

The checklist is a menu of considerations, not a requirements list and not a source of truth. HOI4 Factory is a free tool for hobbyist modders. Enterprise and compliance practices do not apply by default, and friction in sign-up, sign-in or everyday use costs users. The maintainer decides what is built.

## Procedure

1. Read the current stage scope and its changes. Pick only the checklist items the stage touches, plus any item marked "now" that is still missing.
2. Judge each picked item by proportion. Recommend it only when it is cheap or invisible to users and it prevents something real at the current scale: lost mod source or data, one account seeing another account's data, leaked secrets, runaway model or hosting cost, or a broken core flow. Prefer the simplest version that does that job.
3. Never recommend adding a step a user must take (extra verification, MFA, lockouts, mandatory settings, consent walls beyond what the law requires) unless the maintainer asked for it. If an item would add friction, say what the user would feel and leave it as "when needed".
4. Leave enterprise, certification and scale items (SSO, SAML, SCIM, SOC 2, HIPAA, BAAs, data residency, pen tests, security questionnaires, access reviews, on-call rotations, second regions and similar) as "when needed" with their trigger. Do not raise them again unless their trigger has happened.
5. For each picked item, report done with an evidence path, a short recommendation, or "skip for now" with the reason. A plan or a spec paragraph is not done.
6. Update only the checklist rows whose status changed, and add a row only for a genuinely new risk the stage introduced. Run `PYTHONUTF8=1 python scripts/validate_package.py --root . --working-tree`.

## Outputs and evidence

Add a short "Readiness notes" section to the stage handoff: what is already handled, at most a handful of recommendations worth doing soon with their cost, and anything that changed fit. Write `evidence/SNN/readiness.json` with the item IDs reviewed and their outcome. Keep it brief. The point is a useful nudge, not an audit.

## Stop and error paths

Nothing here blocks a stage or a release on its own. Only the stage's own exit gates and the maintainer's decisions do that. If an item raises a real business or legal question, such as a paid launch, a contract term or a new kind of personal data, ask the maintainer instead of building it. Do not run penetration tests, sign attestations or contact auditors.
