# Claude Code prompt: implement S01 only

Use the following prompt in a new Claude Code session opened from the extracted `hoi4-factory/` project root. The client settings and skills are already in place, and planning documents are under `docs/`. Supplying it as an implementation request authorizes S01 within its stated bounds. Merely reading this planning archive does not. Existing authorization need not be requested again.

## Copyable execution prompt

You are the primary implementation agent for HOI4 Factory. Implement **S01 only**, using this planning package as the reviewed baseline and the actual stage packet as the detailed authority. Do not begin S02 or build production features outside the explicitly bounded S01 probes.

Work directly in this project folder. First read all applicable existing repository instructions and preserve them. Read root CLAUDE.md and confirm its `@AGENTS.md` import. Then read these files in order:

1. [docs/PACKAGE_STATUS.md](../PACKAGE_STATUS.md), [docs/MASTER_GOAL.md](../MASTER_GOAL.md), and [implementation master goal](IMPLEMENTATION_MASTER_GOAL.md).
2. [S01 bootstrap and probes](../04-implementation-stages/stage-01-bootstrap-and-probes.md), including dependencies, EXP-01 through EXP-04, stop conditions and exit criteria.
3. [Architecture options and decisions](../02-architecture/options-and-decisions.md), [recommended architecture](../02-architecture/recommended-architecture.md), [contracts](../02-architecture/contracts-and-interfaces.md), and [deployment/security](../02-architecture/deployment-and-security.md).
4. [SPEC-001 setup reuse](../03-subsystem-specs/001-onboarding-and-mod-setup.md), [SPEC-002 native runtime](../03-subsystem-specs/002-native-runtime-and-models.md), [SPEC-003 durability](../03-subsystem-specs/003-orchestration-and-recovery.md), [SPEC-012 guidance](../03-subsystem-specs/012-engineering-guidance-skills-and-learning.md), and [SPEC-013 configuration](../03-subsystem-specs/013-config-reuse-and-developer-environment.md).
5. [Config migration](../07-developer-workflow/CONFIG-MIGRATION-REPORT.md), [complete field map](../07-developer-workflow/CONFIG-FIELD-MAP.csv), [bootstrap checks](../07-developer-workflow/ENVIRONMENT-BOOTSTRAP-AND-CHECKS.md), [instruction map](../07-developer-workflow/SKILLS-AND-INSTRUCTION-MAP.md), and [knowledge policy](../07-developer-workflow/REUSABLE-KNOWLEDGE-POLICY.md).
6. The relevant [acceptance rows](../05-verification/REQUIREMENTS-ACCEPTANCE-MATRIX.md), [test strategy](../05-verification/TEST-STRATEGY.md), [risks](../05-verification/RISKS-DECISIONS-AND-OPEN-QUESTIONS.md), and existing implementation evidence if any.

Invoke `/start-implementation-stage` with S01. Inspect Git initialization, the actual branch and worktree if present, dirty files, toolchain and available credentials without exposing secrets. Record absent repository metadata or an absent initial commit instead of inventing a revision. Establish `evidence/S01/scope.md` relative to the project root before product-related changes. Record allowed paths and experiments, excluded later-stage work, exact acceptance criteria and known unavailable capabilities. Preserve all unrelated user changes.

Verify and adapt the starter files already at this project root. If the user selected a different existing repository, merge these files there as a reviewable diff without overwriting team instructions or global client configuration. Verify Claude instruction loading and the four project skill names in the installed client. Run `python scripts/validate_package.py --root . --working-tree` and `python scripts/sync_skills.py --check`. Use the validator's default mode only when checking the untouched distribution against its frozen manifest. If Codex is available, perform its separate `.agents/skills` discovery and project-config check. If unavailable, record that limitation according to the S01 criteria.

Keep both inherited MCP connections disabled until a specific approved probe requires one and its installation, root, auth, cost and permissions have been reviewed. Do not inherit never/danger-full-access from the upstream config. The source's 184320 value is an instruction-byte ceiling. No root model context or compaction value was present. Measure or document actual client/model constraints without applying one provider's limit universally.

Run only the architecture probes in the S01 packet. Reproduce or explain the observed Setup recovery mismatch with exact artifact evidence before claiming recovery is sound. Test source preservation and supported focus edits in bounded fixtures. Exercise unsafe replay, revocation and handoff semantics with controlled failure cases. Package the selected native provider loop and demonstrate its independence from an external coding CLI where the required environment is available. Preserve Candidate B and other alternatives when a probe fails. Do not convert the leading architecture recommendation into a final commitment without the stage's review gate.

Use subagents only for independent bounded work, with explicit paths, inputs and expected evidence. Review their outputs. Do not claim that a custom role, tool or test ran because it appeared in a plan. When a tool fails or returns an uncertain outcome, reconcile actual state before retrying. Continue unrelated authorized work where safe and useful.

Invoke `/validate-stage` against the exact S01 criteria. Record every command, version, input revision, outcome, artifact hash and limitation. Separate static source, client loading, actual tool calls, packaging and game evidence. Do not fabricate model comparisons or mark unavailable runtime checks passed.

Invoke `/capture-engineering-lessons`. Record candidate lessons, review decisions and any accepted small guidance changes. Update canonical bodies and all projections together. Complete `evidence/S01/config-review.md` with preserved/adapted settings, actual checks, unavailable dependencies and rollback. A knowledge candidate does not automatically become policy.

Finish with `evidence/S01/environment.json`, scope.md, checks.json, config-review.md, learning-review.md and handoff.md. The final report must state completed scope, files changed, actual verification, failed or unavailable gates, architecture decisions needing review and the exact proposed next action. Use [the handoff template](STAGE-HANDOFF-TEMPLATE.md). Stop after S01. Do not start S02 or publish/merge beyond the user's existing authorization.

## Recovery note

If the session is interrupted, resume the correct repository session and re-read scope.md and handoff.md. Inspect current worktree state and reconcile any pending effect. A transcript can help recover context but cannot establish completion. Do not restart a paid or mutating operation solely because its final message is absent.
