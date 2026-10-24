# SPEC-012: Engineering guidance, skills and learning

Status: Proposed. Owner: engineering guidance maintainer. Primary stage: S01, maintained at every subsequent handoff. Requirements: REQ-036, REQ-037, REQ-039 and REQ-045. Acceptance: AC-036, AC-037, AC-039 and AC-045.

Read first: [shared instructions](../../AGENTS.md), [master goal](../MASTER_GOAL.md), [stage roadmap](../04-implementation-stages/ROADMAP.md), [knowledge policy](../07-developer-workflow/REUSABLE-KNOWLEDGE-POLICY.md), [instruction map](../07-developer-workflow/SKILLS-AND-INSTRUCTION-MAP.md), and [runtime learning specification](004-context-memory-and-runtime-learning.md).

## Purpose and scope

This subsystem gives a new implementation session enough durable guidance to finish one authorized stage, verify it and hand off without reconstructing the original conversation. It serves maintainers, coding agents and reviewers. It also makes engineering improvements inspectable. A useful correction should survive a context reset without turning every provisional observation into permanent instruction.

The scope is the repository used to build HOI4 Factory. Claude Code is the primary implementation client and Codex is an interoperability target. These clients are development tools. The finished application retains an independent native runtime and its own provider capability negotiation. Product runtime knowledge follows SPEC-004. This subsystem does not automatically import player conversations, design canon, credentials or provider transcripts into engineering memory.

Stable shared instructions live in root AGENTS.md. CLAUDE.md imports them with `@AGENTS.md` and adds only client-specific operations. The approved stage packet is the authority for current implementation bounds. Specs and research remain proposals until accepted. Existing repository instructions must be reconciled before adoption, never replaced wholesale by the archive.

## Evidence and reusable boundaries

Current Claude documentation explicitly supports AGENTS imports and describes version-sensitive native AGENTS discovery. The explicit import avoids requiring a global setting or a Windows symlink. Current Codex documentation discovers repository skills under `.agents/skills`. Therefore this package supplies the requested `.codex/skills` copies and a separate discovery mirror at `.agents/skills`. [SRC-C16](https://code.claude.com/docs/en/memory) [SRC-C12](https://developers.openai.com/codex/skills)

The audited Agentic repository uses canonical Claude instructions and generated client projections. Its synchronization mechanisms and read-only drift checks are useful patterns, but its mod-production policies are not automatically appropriate for building a new application. This package instead keeps one logical shared policy in AGENTS and one canonical body for each workflow. Copies are deterministic generated artifacts, with client-only invocation metadata kept outside shared bodies. [SRC-C05](https://github.com/klimPaskov/Agentic-HOI4-Modding/blob/856940f2a629e0e2f3e03fc484cb0c0309c96734/.tools/sync/README.md)

The four starter skills are start-implementation-stage, validate-stage, capture-engineering-lessons and audit-hoi4-integrations. Their descriptions identify a bounded task, inputs, evidence, stop conditions and error paths. Claude manual invocation and Codex explicit invocation metadata reduce accidental activation. A skill still cannot create authority that the user has not supplied. No broad tool preapproval is shipped. [SRC-C17](https://code.claude.com/docs/en/skills)

## Operations and contracts

`prepare_stage` receives the current user authorization, one stage ID, repository state and dependency handoffs. It reads applicable instructions and produces `evidence/SNN/scope.md`, listing in-scope outputs, excluded later stages, required acceptance rows, tool availability and owned paths. It returns ready when the needed dependencies exist, or blocked with the smallest missing prerequisite. If a proposed implementation choice remains uncertain, it runs only an experiment already in the stage packet. Preparation does not start another stage.

`validate_stage` receives the scope record, changed files and proposed evidence. It creates C-05 EvidenceRecords that identify exact inputs, tool versions, commands, results, coverage and limitations. Each acceptance row is pass, fail, unavailable or not applicable with a reason. Pass requires the appropriate evidence kind. Source inspection cannot substitute for game observation, and a screenshot cannot establish source round-trip integrity. The output is a reviewable checks report and stage disposition.

`capture_lesson` creates a C-07 KnowledgeCandidate with domain `engineering`, source evidence, scope, owner, status, applicability and version bounds, a counterexample, proposed change, review record and supersedes links. A candidate can be accepted only after its evidence and intended behavioral change are reviewed. An unsupported observation remains a candidate or is rejected. The record must distinguish an implementation defect, a tool limitation, an unclear requirement and a useful workflow improvement.

`promote_guidance` takes a tested candidate and review decision. It creates a small Git diff to AGENTS, canonical skill bodies, references, configuration documentation or the changelog. Changes that broaden authority, external effects, costs or product scope require the appropriate owner decision. A normal factual correction within an already authorized stage remains ordinary reviewable work. Promotion updates all mirrors and relevant trigger fixtures in the same change. It never modifies approved game canon or the product's runtime policy as a side effect.

`handoff_stage` creates a self-contained report with exact revision, changed paths, completed scope, actual evidence, limitations, rollback notes, knowledge review and configuration checkpoint. It proposes the next stage only. The recipient rechecks working tree state and evidence hashes before using the report. An interrupted tool with unknown outcome is reconciled before retry, following C-04 rather than assuming transcript completion.

## State and integrity rules

Knowledge progresses through captured, tested, reviewed and accepted states. Rejected, superseded and expired states preserve provenance. A captured note may aid investigation but does not override accepted policy. The owner reviews applicability after a client, provider, dependency or game version changes. A counterexample can return accepted guidance to review without deleting the history.

Canonical workflow bodies live in `docs/07-developer-workflow/skill-workflows/<name>/BODY.md`. Metadata is defined once in skills.json. The local sync script produces three client trees and a hash manifest. Check mode is read-only. Write mode updates only owned package projections after a reviewed source change. It never installs personal skills or changes global configuration. Git review supplies attribution and rollback. Symlinks are unnecessary.

Instruction authority and runtime enforcement remain separate. A Markdown statement cannot enforce a filesystem boundary. A subagent that cannot call Edit may still call a shell or effectful MCP tool unless its actual policy prevents that. Worktree isolation likewise does not provide an OS security boundary. Worker tasks specify paths and expected output, and the parent reviews results before integration. [SRC-C21](https://code.claude.com/docs/en/sub-agents) [SRC-C24](https://code.claude.com/docs/en/permissions)

## Verification and measurable outcomes

S01 first parses frontmatter and settings, checks all relative links, confirms all twelve skill copies exist, and verifies the shared body hashes. A deliberate modification to one copied body must cause the drift check to fail. A malformed header, mismatched directory name or missing required field must fail static validation. These checks establish artifact integrity, not model behavior.

Client trials then record client and model versions, loaded instructions, visible skill names and actual traces. Positive fixtures explicitly invoke each workflow with valid inputs. Negative fixtures ask an ordinary question, omit stage authorization, request a later stage, or supply a misleading historical instruction. The expected outcome is bounded help, a precise missing prerequisite or refusal to broaden scope, while preserving useful authorized work. A missing MCP dependency produces unavailable evidence without fabricating a successful audit.

Learning tests use a reproducible failure, a one-off observation without a reproduction, an accepted lesson contradicted by a new version, and a proposal that would silently change game canon. Only the first can advance after review, the second remains a candidate, the third becomes stale, and the fourth routes to the separate design authority. Evaluation uses outcomes, process, scope and efficiency with deterministic artifact checks and human rubric review. No model success rate is claimed in this plan. [SRC-C14](https://developers.openai.com/blog/eval-skills)

## Delivery, recovery and open experiments

S01 owns adoption, actual client loading, trigger trials and initial learning review. Every later stage updates its handoff and records a knowledge decision even when no reusable lesson emerged. S08 audits stale guidance, version compatibility and archive completeness. Recovery consists of selecting an accepted Git revision, restoring canonical sources, regenerating mirrors and rerunning checks. Resume transcripts are optional context and cannot replace durable evidence.

Open experiments are whether each installed client exposes the expected invocation metadata, how much instruction context is actually loaded, and which task classes benefit from subagents without increasing review cost. Failure leaves the affected client workflow experimental while the bounded stage can continue through an explicitly documented fallback. Changes to shared authority or stage scope remain owner decisions.
