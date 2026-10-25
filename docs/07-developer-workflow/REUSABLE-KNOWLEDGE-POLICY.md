# Reusable engineering knowledge policy

Status: Proposed standing workflow. Owner: engineering guidance maintainer. Applies from S01 onward. Read [SPEC-012](../03-subsystem-specs/012-engineering-guidance-skills-and-learning.md), [shared instructions](../../AGENTS.md) and [stage handoff](../06-agent-handoffs/STAGE-HANDOFF-TEMPLATE.md).

## Purpose and authority

Engineering knowledge helps the team repeat reliable work and avoid known failures while building HOI4 Factory. It is distinct from the future application's runtime knowledge, player conversations and approved mod design canon. An engineering lesson cannot silently change product requirements, game content, permissions or provider policy.

At task start, read relevant accepted guidance and prior stage evidence. During work, collect reproducible failures, tool limitations, successful repair patterns, unclear instructions and missing tests. At every substantive handoff, record a learning review even when the correct result is “no reusable change.” This prevents silent omission without rewarding large amounts of low-value documentation.

## Candidate record

Use `engineering/knowledge/candidates/<id>.md` relative to the `hoi4-factory/` project root when an authorized task produces a candidate. Keep these engineering records separate from the planning documents under `docs/`. A C-07 record contains these fields:

| Field | Required content |
|---|---|
| Identity | Stable ID, short title, domain `engineering`, author, owner and date |
| Observation | What happened, actual input and output, exact tool/model/client/dependency versions |
| Evidence | Reproduction steps, command or trace, relevant artifact and hash, stage and requirement links |
| Applicability | Where the lesson applies, assumptions, version bounds and excluded cases |
| Counterexample | A case where the recommendation would be wrong or unsafe |
| Proposed change | Small diff to a skill, instruction, test, configuration guide or dependency check |
| Review | Reviewer, decision, rationale, validation and required authority |
| Lifecycle | Status, supersedes/superseded-by links, expiry or recheck trigger and rollback |

Record nonsecret evidence. Do not copy raw credentials, private personal paths, unrelated user data or hidden provider reasoning into a lesson. Link a restricted artifact by approved identifier if the evidence cannot be shared. A summary must retain enough detail to reproduce the relevant claim.

## Lifecycle and review gates

| State | Entry condition | Permitted use |
|---|---|---|
| Captured | Specific observation with provenance | Investigation aid, explicitly unaccepted |
| Tested | Relevant reproduction or bounded experiment recorded | Candidate recommendation with limitations |
| Reviewed | Owner assessed scope, counterexample and proposed diff | Ready for an acceptance or rejection decision |
| Accepted | Review and applicable authority recorded, or automatic promotion under ADR-023 | Versioned project guidance within stated bounds |
| Rejected | Claim unsupported, harmful or not reusable | Retain reason, do not use as policy |
| Superseded | A newer accepted record replaces it | Keep history and replacement link |
| Expired | Version bounds or review trigger no longer hold | Revalidate before applying as accepted guidance |

One successful task does not automatically justify a global rule. Prefer the narrowest useful instruction. Promote a reusable workflow when it has a clear trigger, expected result, verification and a failure path. Keep transient command output in stage evidence instead of loading it into every session.

Under ADR-024, the implementing agent applies useful lessons directly to CLAUDE.md and the skills, and creates new skills when a workflow repeats, without a test gate or review. It adds a regression test where the lesson is testable and records each change in the changelog. A later counterexample means fixing or removing the rule. Changes that touch tool authority, access roots, external data transfer, paid operations, approved product scope or game design authority still need the owner's approval. The implementation agent may capture candidates, write tests and propose diffs within its already authorized stage. Routine factual corrections can follow ordinary repository review without repeated permission requests. Changes that broaden tool authority, access roots, external data transfer, paid operations, approved product scope or game design authority require the corresponding owner approval before activation. A skill edit must not be used to give itself broader permission.

## Choosing the destination

Put stable cross-client rules in AGENTS.md. Put Claude-specific usage in CLAUDE.md. Put a focused repeatable procedure in a canonical skill body. Put detailed reference facts in a linked document. Put environment compatibility in the config migration report and bootstrap checks. Put task-specific state in `evidence/SNN/`. Do not duplicate the same normative rule across these places.

The product runtime may eventually generate its own KnowledgeCandidates under SPEC-004. Importing one into engineering review requires an explicit provenance-preserving copy with domain `engineering`. Accepted engineering guidance likewise does not enter runtime memory automatically. Separate reviewers and authority apply.

## Skill and configuration evaluation

Before promotion, check the desired outcome, the process followed, scope adherence, evidence quality and unnecessary work. Use deterministic checks for files, schemas and hashes. Use a human rubric for judgment and clarity. Add positive and negative trigger cases whenever the trigger or boundary changes. Include an unavailable-tool case and a request that exceeds stage scope. [Evaluation guidance](https://developers.openai.com/blog/eval-skills)

Static parsing cannot measure whether a model invokes a skill correctly. A model trial must record client/model version, exact prompt, selected skill, tool trace, artifacts and outcome. Do not invent a pass rate. Limit trial counts to the material change under review, and state the sample size and environment limitations.

Update canonical workflow bodies under `docs/07-developer-workflow/skill-workflows/` and regenerate every client projection with `scripts/py.sh scripts/sync_skills.py --write` from the project root. The generator rebases local links for each generated location. The drift check must pass before accepting a skill change. Use `scripts/py.sh scripts/validate_package.py --root . --working-tree` for the evolving documentation and guidance. If a client cannot support a behavior, document the precise gap and affected version rather than copying an unsupported field. Configuration changes also update `config-review.md`, the field mapping or additions register, environment instructions and the changelog.

## Retirement, recovery and cadence

Recheck accepted guidance when its tool, client, provider, dependency, game version or source contract changes. A contradiction should open a new candidate and mark the old lesson under review. Preserve attribution and the original evidence. Retire redundant instructions to keep context concise.

Rollback uses a reviewed Git revision of canonical sources and their generated outputs. Rerun the relevant checks after reverting. Reverting a lesson cannot undo a remote side effect, so keep operation receipts separate. Every stage report includes candidates considered, accepted changes, rejected or deferred candidates, config impact and the next review trigger. S08 performs a package-wide stale-guidance audit before release.
