# Stage handoff template

Use this template for `evidence/SNN/handoff.md` relative to the `hoi4-factory/` project root when an authorized implementation stage produces evidence. Replace fields with observed facts. Do not prefill pass results. Read the approved stage packet, [knowledge policy](../07-developer-workflow/REUSABLE-KNOWLEDGE-POLICY.md) and [configuration report](../07-developer-workflow/CONFIG-MIGRATION-REPORT.md).

## Identity and authorized scope

- Stage ID and title:
- User authorization reference and bounds:
- Implementing agent/client/model and actual versions:
- Reviewer and review state:
- Repository, branch/worktree and exact base/current revision:
- Dirty paths present before work and their disposition:
- Stage/spec/decision versions used:
- Start/end time and interrupted or resumed attempts:

State what was authorized, what was completed and which later-stage features remain excluded. A handoff is evidence, not authority for a following stage.

## Outcome and changed artifacts

Describe the behavior or experiment outcome in plain language. List changed paths with purpose, owner and meaningful hashes. Identify generated artifacts and their canonical sources. Distinguish production code, disposable probes, fixtures, documentation and configuration. Record preserved user edits and any unresolved merge conflict.

| Requirement or experiment | Intended result | Actual result | Evidence reference and kind | Status |
|---|---|---|---|---|
| Fill with actual ID | Observable criterion | Observed outcome | C-05 record or artifact | pass, fail, unavailable or not applicable |

## Verification record

Link `checks.json` and include the checks that materially support completion. For each, retain command, working directory, exact tool/version, input revision/hash, exit status, output artifact/hash, coverage and limitations. Identify whether the check was source inspection, static validation, preview, client loading, tool execution, package installation, game observation or user review.

Record required checks that could not run and why. Do not substitute a different evidence kind silently. Summarize focused regression coverage and remaining risk. A declared upstream CI workflow is not an executed test result.

## Decisions and unresolved issues

List accepted decisions with reviewer and rationale. Keep proposals separate. For every blocker, identify impact, exact missing evidence, work already completed, safe continuation and the smallest next decision. Include compatibility, licensing and provider constraints that affect the next stage. Preserve failed architectural probes and alternatives.

## Effects, recovery and rollback

Record effectful operations with receipts and scoped authority. Note any unknown outcome, reconciliation performed and replay decision. List affected external resources without secrets. Explain what a Git revert restores and what needs operation-specific recovery. Give a safe restore point and unresolved state to inspect on resume.

## Configuration checkpoint

Link `config-review.md`. State whether tools, providers, MCP endpoints, auth mechanisms, permissions, instruction discovery or context assumptions changed. For each change, record source and target keys, version evidence, required dependency, actual validation and rollback. Confirm project scope and identify unavailable client checks. If no config change was needed, explain what was reviewed.

## Engineering learning review

Link `learning-review.md`. List candidate IDs, reproducible evidence, applicability/version bounds, counterexamples, owners and review outcomes. Identify accepted instruction/skill/test changes, rejected or deferred candidates and expiry triggers. Confirm canonical workflow and all client projections were checked. State explicitly if no reusable lesson emerged.

Do not move runtime memory or game canon into engineering policy by implication. Capture does not equal acceptance, and a skill change does not expand task authority.

## Next stage proposal and resume instructions

- Proposed next stage and its dependencies:
- Evidence ready for the next owner:
- Prerequisites still unresolved:
- Exact files to read first:
- Current working directory and nonsecret environment facts:
- Pending operations requiring reconciliation:
- Scope decision or authorization needed before implementation:

Finish with a precise next action. Do not start the next stage unless the user separately authorizes it. A resumed session must verify current Git state and evidence identity before continuing.
