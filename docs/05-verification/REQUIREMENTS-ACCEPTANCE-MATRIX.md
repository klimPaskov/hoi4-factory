# Requirements and stage acceptance matrix

Status: **Planned product acceptance, with separately verified planning artifacts**. There are 45 requirement acceptance records and 47 stage exit-gate mappings. None is a fabricated product test result.

The test IDs link logically to [the test catalog](TEST-STRATEGY.md#test-catalog), whose fixture, success oracle and evidence rules apply. The machine-readable counterpart is [acceptance-matrix.json](acceptance-matrix.json). Requirement-to-architecture links are in [traceability](../01-product/requirements-traceability.md).

## Requirement acceptance

| Acceptance / requirement | Primary owner | Required observable outcome | Tests |
|---|---|---|---|
| **AC-001 / REQ-001** | [SPEC-002](../03-subsystem-specs/002-native-runtime-and-models.md), [S03](../04-implementation-stages/stage-03-native-agent-and-durability.md) | Clean packaged task succeeds with no coding CLI installed or spawned. | T-RUN-01, T-RELEASE-01 |
| **AC-002 / REQ-002** | [SPEC-002](../03-subsystem-specs/002-native-runtime-and-models.md), [S03](../04-implementation-stages/stage-03-native-agent-and-durability.md) | Each of three profiles has an actual capability receipt and explicit unsupported-feature behavior. | T-RUN-05 |
| **AC-003 / REQ-003** | [SPEC-002](../03-subsystem-specs/002-native-runtime-and-models.md), [S03](../04-implementation-stages/stage-03-native-agent-and-durability.md) | Credentials remain outside artifacts, budget stops dispatch and fallback obeys destination authority. | T-RUN-06, T-SEC-01 |
| **AC-004 / REQ-004** | [SPEC-001](../03-subsystem-specs/001-onboarding-and-mod-setup.md), [S02](../04-implementation-stages/stage-02-onboarding-and-workspaces.md) | New mod uses actual Setup analysis, planning and apply, producing native files and C-01. | T-SETUP-01 |
| **AC-005 / REQ-005** | [SPEC-001](../03-subsystem-specs/001-onboarding-and-mod-setup.md), [S02](../04-implementation-stages/stage-02-onboarding-and-workspaces.md) | Existing and partial mod imports preserve unapproved files and use supported recovery variants. | T-SETUP-02, T-SETUP-03 |
| **AC-006 / REQ-006** | [SPEC-001](../03-subsystem-specs/001-onboarding-and-mod-setup.md), [S02](../04-implementation-stages/stage-02-onboarding-and-workspaces.md) | Exact protected-byte inventory and unrelated Git index state match the baseline. | T-SETUP-02, T-GIT-01 |
| **AC-007 / REQ-007** | [SPEC-001](../03-subsystem-specs/001-onboarding-and-mod-setup.md), [S02](../04-implementation-stages/stage-02-onboarding-and-workspaces.md) | C-01 carries root identity, resolved manifest/tool closure, receipt and unresolved checks. | T-SETUP-04 |
| **AC-008 / REQ-008** | [SPEC-004](../03-subsystem-specs/004-context-memory-and-runtime-learning.md), [S05](../04-implementation-stages/stage-05-context-and-recovery-hardening.md) | Approved lore and unresolved questions retain ownership and trace to the accepted source change. | T-CTX-01 |
| **AC-009 / REQ-009** | [SPEC-007](../03-subsystem-specs/007-visual-state-synchronization.md), [S04](../04-implementation-stages/stage-04-focus-vertical-slice.md) | Visual, hybrid and autonomous paths share proposal, grant, source and evidence contracts. | T-VIS-02 |
| **AC-010 / REQ-010** | [SPEC-007](../03-subsystem-specs/007-visual-state-synchronization.md), [S04](../04-implementation-stages/stage-04-focus-vertical-slice.md) | Pilot contributors find source, status and evidence limits without traversing every advanced control. | T-RELEASE-04 |
| **AC-011 / REQ-011** | [SPEC-007](../03-subsystem-specs/007-visual-state-synchronization.md), [S04](../04-implementation-stages/stage-04-focus-vertical-slice.md) | No-op is byte-identical, supported writes preserve untouched spans and stale updates cannot overwrite. | T-VIS-01, T-VIS-03 |
| **AC-012 / REQ-012** | [SPEC-009](../03-subsystem-specs/009-focus-and-other-editors.md), [S04](../04-implementation-stages/stage-04-focus-vertical-slice.md) | Focus preview exposes supported relationships, source revision, icons and explicit fidelity diagnostics. | T-VIS-02, T-HOI-02 |
| **AC-013 / REQ-013** | [SPEC-009](../03-subsystem-specs/009-focus-and-other-editors.md), [S04](../04-implementation-stages/stage-04-focus-vertical-slice.md) | Supported move/property edits, undo and redo preserve unrelated bytes, unsupported edits refuse safely. | T-VIS-01, T-VIS-03 |
| **AC-014 / REQ-014** | [SPEC-003](../03-subsystem-specs/003-orchestration-and-recovery.md), [S04](../04-implementation-stages/stage-04-focus-vertical-slice.md) | Bounded focus task completes with canvas closed and stops at the approved limits. | T-VIS-02, T-RUN-06 |
| **AC-015 / REQ-015** | [SPEC-011](../03-subsystem-specs/011-extensibility-sandbox-and-permissions.md), [S03](../04-implementation-stages/stage-03-native-agent-and-durability.md) | Durable pause/revocation stops new effects, in-flight work settles or remains explicitly uncertain. | T-RUN-03, T-SEC-01 |
| **AC-016 / REQ-016** | [SPEC-003](../03-subsystem-specs/003-orchestration-and-recovery.md), [S03](../04-implementation-stages/stage-03-native-agent-and-durability.md) | Restart restores task, grant and receipt identity without duplicate unsafe effects. | T-RUN-04, T-RELEASE-02 |
| **AC-017 / REQ-017** | [SPEC-003](../03-subsystem-specs/003-orchestration-and-recovery.md), [S06](../04-implementation-stages/stage-06-collaboration-and-content.md) | Cycles, stale claims and exhausted retries block work with an attributable reason. | T-DAG-01 |
| **AC-018 / REQ-018** | [SPEC-004](../03-subsystem-specs/004-context-memory-and-runtime-learning.md), [S05](../04-implementation-stages/stage-05-context-and-recovery-hardening.md) | Different provider resumes from canonical context with no unsafe replay or hidden capability substitution. | T-CTX-03 |
| **AC-019 / REQ-019** | [SPEC-004](../03-subsystem-specs/004-context-memory-and-runtime-learning.md), [S05](../04-implementation-stages/stage-05-context-and-recovery-hardening.md) | Changed source/tool profile invalidates affected facts, evidence and projections. | T-CTX-02 |
| **AC-020 / REQ-020** | [SPEC-004](../03-subsystem-specs/004-context-memory-and-runtime-learning.md), [S05](../04-implementation-stages/stage-05-context-and-recovery-hardening.md) | Lesson review supports approval, rejection, supersession and rollback without changing canon. | T-CTX-04 |
| **AC-021 / REQ-021** | [SPEC-005](../03-subsystem-specs/005-git-collaboration-and-review.md), [S02](../04-implementation-stages/stage-02-onboarding-and-workspaces.md) | Owned worktrees and expected-HEAD commits preserve the user's real index and unrelated files. | T-GIT-01 |
| **AC-022 / REQ-022** | [SPEC-005](../03-subsystem-specs/005-git-collaboration-and-review.md), [S06](../04-implementation-stages/stage-06-collaboration-and-content.md) | Required semantic and binary collisions are detected before integration and both edits survive. | T-GIT-02 |
| **AC-023 / REQ-023** | [SPEC-005](../03-subsystem-specs/005-git-collaboration-and-review.md), [S04](../04-implementation-stages/stage-04-focus-vertical-slice.md) | Review binds intent, raw and semantic diff, evidence and actor to the exact source result. | T-GIT-02, T-HOI-02 |
| **AC-024 / REQ-024** | [SPEC-005](../03-subsystem-specs/005-git-collaboration-and-review.md), [S04](../04-implementation-stages/stage-04-focus-vertical-slice.md) | Restore or reviewed revert recovers the accepted state while preserving later unrelated work. | T-GIT-03 |
| **AC-025 / REQ-025** | [SPEC-006](../03-subsystem-specs/006-hoi4-tools-and-validation.md), [S04](../04-implementation-stages/stage-04-focus-vertical-slice.md) | Known bad fixtures fail appropriate checks and no render/simulation is called actual game evidence. | T-HOI-01, T-HOI-02 |
| **AC-026 / REQ-026** | [SPEC-006](../03-subsystem-specs/006-hoi4-tools-and-validation.md), [S03](../04-implementation-stages/stage-03-native-agent-and-durability.md) | Adapter schema/version/health, timeout, cancellation and retry limits are observable and tested. | T-SETUP-04, T-RUN-02 |
| **AC-027 / REQ-027** | [SPEC-006](../03-subsystem-specs/006-hoi4-tools-and-validation.md), [S02](../04-implementation-stages/stage-02-onboarding-and-workspaces.md) | Game, DLC and load order are recorded or explicitly unknown, with invalidation on change. | T-SETUP-04, T-HOI-02 |
| **AC-028 / REQ-028** | [SPEC-010](../03-subsystem-specs/010-assets-localization-and-content.md), [S06](../04-implementation-stages/stage-06-collaboration-and-content.md) | Assets retain rights/hash/recipe/consumer provenance and localization preserves encoding and keys. | T-CONTENT-01, T-CONTENT-02 |
| **AC-029 / REQ-029** | [SPEC-008](../03-subsystem-specs/008-maps.md), [S07](../04-implementation-stages/stage-07-maps-and-structured-import.md) | Existing structured layers are source-linked and inspectable without destructive conversion. | T-MAP-01 |
| **AC-030 / REQ-030** | [SPEC-008](../03-subsystem-specs/008-maps.md), [S07](../04-implementation-stages/stage-07-maps-and-structured-import.md) | Import plan identifies exact closure, conflicts and remapping, and invalid inputs cause no apply. | T-MAP-01, T-MAP-02 |
| **AC-031 / REQ-031** | [SPEC-008](../03-subsystem-specs/008-maps.md), [S07](../04-implementation-stages/stage-07-maps-and-structured-import.md) | Reference remains attributed intent until separately modeled and validated topology exists. | T-MAP-03 |
| **AC-032 / REQ-032** | [SPEC-008](../03-subsystem-specs/008-maps.md), [S07](../04-implementation-stages/stage-07-maps-and-structured-import.md) | S07 demonstrates one bounded edit and a reviewed future drawing/generation contract. Full-world synthesis remains deferred until its independent gates pass. | T-MAP-02, T-MAP-03, T-PERF-02 |
| **AC-033 / REQ-033** | [SPEC-011](../03-subsystem-specs/011-extensibility-sandbox-and-permissions.md), [S03](../04-implementation-stages/stage-03-native-agent-and-durability.md) | Host/OS boundaries reject malicious input and unauthorized effects, unsafe executables remain disabled. | T-SEC-01, T-SEC-02, T-RUN-02 |
| **AC-034 / REQ-034** | [SPEC-011](../03-subsystem-specs/011-extensibility-sandbox-and-permissions.md), [S08](../04-implementation-stages/stage-08-packaging-and-pilot.md) | Package has reproducible dependencies and a reviewed distribution basis excluding restricted code and game files. | T-RELEASE-01, T-RELEASE-03 |
| **AC-035 / REQ-035** | [SPEC-005](../03-subsystem-specs/005-git-collaboration-and-review.md), [S06](../04-implementation-stages/stage-06-collaboration-and-content.md) | Each contributor role has attributable intent and source-linked review, conflict and restore paths. | T-GIT-02, T-RELEASE-04 |
| **AC-036 / REQ-036** | [SPEC-012](../03-subsystem-specs/012-engineering-guidance-skills-and-learning.md), [S01](../04-implementation-stages/stage-01-bootstrap-and-probes.md) | Every substantive handoff records an evidenced minimal learning update or justified no-change result. | T-DEV-04 |
| **AC-037 / REQ-037** | [SPEC-012](../03-subsystem-specs/012-engineering-guidance-skills-and-learning.md), [S01](../04-implementation-stages/stage-01-bootstrap-and-probes.md) | Real starter files parse and match now. Actual target-client instruction loading and trigger behavior pass later in S01. | T-DEV-01, T-DEV-02, T-PKG-01 |
| **AC-038 / REQ-038** | [SPEC-013](../03-subsystem-specs/013-config-reuse-and-developer-environment.md), [S01](../04-implementation-stages/stage-01-bootstrap-and-probes.md) | All 196 actual source leaves have explicit dispositions. Starter parse/shape passes now, live-client/MCP checks remain S01 work. | T-DEV-03, T-PKG-01 |
| **AC-039 / REQ-039** | [SPEC-012](../03-subsystem-specs/012-engineering-guidance-skills-and-learning.md), [S01](../04-implementation-stages/stage-01-bootstrap-and-probes.md) | Every stage has read-first scope, packet dependencies, gates and a self-contained stop-before-next-stage handoff. | T-DEV-01, T-PKG-01 |
| **AC-040 / REQ-040** | [SPEC-006](../03-subsystem-specs/006-hoi4-tools-and-validation.md), [S01](../04-implementation-stages/stage-01-bootstrap-and-probes.md) | Requested sources and supplied-file coverage are accounted for with pins, inspection scope, licenses and limitations. | T-RESEARCH-01 |
| **AC-041 / REQ-041** | [SPEC-002](../03-subsystem-specs/002-native-runtime-and-models.md), [S08](../04-implementation-stages/stage-08-packaging-and-pilot.md) | All required scenarios have three-profile planned cells and truthful results when run, including failures, cost and rework. | T-RUN-05, T-RELEASE-04 |
| **AC-042 / REQ-042** | [SPEC-011](../03-subsystem-specs/011-extensibility-sandbox-and-permissions.md), [S05](../04-implementation-stages/stage-05-context-and-recovery-hardening.md) | MCP/skills are bounded adapters, optional ACP unsupported states are explicit and runtime replacement preserves host authority. | T-SEC-02 |
| **AC-043 / REQ-043** | [SPEC-013](../03-subsystem-specs/013-config-reuse-and-developer-environment.md), [S08](../04-implementation-stages/stage-08-packaging-and-pilot.md) | Upgrade checks validate exact schema/tool closure, preserve prior guidance and record migrations or intentional omission. | T-DEV-03, T-RELEASE-01 |
| **AC-044 / REQ-044** | [SPEC-007](../03-subsystem-specs/007-visual-state-synchronization.md), [S04](../04-implementation-stages/stage-04-focus-vertical-slice.md) | Native source opens and remains editable outside the app, with external changes detected on reopen. | T-VIS-03, T-RELEASE-01 |
| **AC-045 / REQ-045** | [SPEC-012](../03-subsystem-specs/012-engineering-guidance-skills-and-learning.md), [S01](../04-implementation-stages/stage-01-bootstrap-and-probes.md) | Actual ZIP includes substantive linked requirements, specs, stages, tests, starter assets, source evidence and passed artifact QA. | T-PKG-01 |

## Stage gate coverage

Gate numbering is the ordered “Measurable exit gates” list in each stage packet. This table adds stable identifiers and test ownership without rewriting the full gate text. Every gate is evaluated against the final stage revision. A test may cover multiple gates, but evidence must name the actual assertion checked.

| Gate | Exact gate definition | Planned checks |
|---|---|---|
| S01-G1 | [S01, exit gate 1](../04-implementation-stages/stage-01-bootstrap-and-probes.md#measurable-exit-gates) | T-DEV-01, T-DEV-02, T-DEV-03 |
| S01-G2 | [S01, exit gate 2](../04-implementation-stages/stage-01-bootstrap-and-probes.md#measurable-exit-gates) | T-SETUP-01, T-SETUP-02, T-SETUP-03 |
| S01-G3 | [S01, exit gate 3](../04-implementation-stages/stage-01-bootstrap-and-probes.md#measurable-exit-gates) | T-RUN-02, T-RUN-04 |
| S01-G4 | [S01, exit gate 4](../04-implementation-stages/stage-01-bootstrap-and-probes.md#measurable-exit-gates) | T-RUN-01 |
| S01-G5 | [S01, exit gate 5](../04-implementation-stages/stage-01-bootstrap-and-probes.md#measurable-exit-gates) | T-RESEARCH-01, T-DEV-04 |
| S02-G1 | [S02, exit gate 1](../04-implementation-stages/stage-02-onboarding-and-workspaces.md#measurable-exit-gates) | T-SETUP-01, T-SETUP-02 |
| S02-G2 | [S02, exit gate 2](../04-implementation-stages/stage-02-onboarding-and-workspaces.md#measurable-exit-gates) | T-SETUP-02, T-GIT-01 |
| S02-G3 | [S02, exit gate 3](../04-implementation-stages/stage-02-onboarding-and-workspaces.md#measurable-exit-gates) | T-SETUP-02 |
| S02-G4 | [S02, exit gate 4](../04-implementation-stages/stage-02-onboarding-and-workspaces.md#measurable-exit-gates) | T-SETUP-03 |
| S02-G5 | [S02, exit gate 5](../04-implementation-stages/stage-02-onboarding-and-workspaces.md#measurable-exit-gates) | T-SETUP-04 |
| S03-G1 | [S03, exit gate 1](../04-implementation-stages/stage-03-native-agent-and-durability.md#measurable-exit-gates) | T-RUN-01, T-CTX-01 |
| S03-G2 | [S03, exit gate 2](../04-implementation-stages/stage-03-native-agent-and-durability.md#measurable-exit-gates) | T-RUN-02, T-SEC-01 |
| S03-G3 | [S03, exit gate 3](../04-implementation-stages/stage-03-native-agent-and-durability.md#measurable-exit-gates) | T-RUN-04 |
| S03-G4 | [S03, exit gate 4](../04-implementation-stages/stage-03-native-agent-and-durability.md#measurable-exit-gates) | T-RUN-03 |
| S03-G5 | [S03, exit gate 5](../04-implementation-stages/stage-03-native-agent-and-durability.md#measurable-exit-gates) | T-RUN-06, T-RUN-04 |
| S03-G6 | [S03, exit gate 6](../04-implementation-stages/stage-03-native-agent-and-durability.md#measurable-exit-gates) | T-RUN-05 |
| S04-G1 | [S04, exit gate 1](../04-implementation-stages/stage-04-focus-vertical-slice.md#measurable-exit-gates) | T-SETUP-01, T-SETUP-02, T-VIS-02 |
| S04-G2 | [S04, exit gate 2](../04-implementation-stages/stage-04-focus-vertical-slice.md#measurable-exit-gates) | T-VIS-02 |
| S04-G3 | [S04, exit gate 3](../04-implementation-stages/stage-04-focus-vertical-slice.md#measurable-exit-gates) | T-VIS-01, T-VIS-03 |
| S04-G4 | [S04, exit gate 4](../04-implementation-stages/stage-04-focus-vertical-slice.md#measurable-exit-gates) | T-HOI-01, T-HOI-02 |
| S04-G5 | [S04, exit gate 5](../04-implementation-stages/stage-04-focus-vertical-slice.md#measurable-exit-gates) | T-GIT-01, T-GIT-03 |
| S04-G6 | [S04, exit gate 6](../04-implementation-stages/stage-04-focus-vertical-slice.md#measurable-exit-gates) | T-VIS-03, T-RELEASE-01 |
| S04-G7 | [S04, exit gate 7](../04-implementation-stages/stage-04-focus-vertical-slice.md#measurable-exit-gates) | T-PERF-01 |
| S05-G1 | [S05, exit gate 1](../04-implementation-stages/stage-05-context-and-recovery-hardening.md#measurable-exit-gates) | T-CTX-01, T-DEV-04 |
| S05-G2 | [S05, exit gate 2](../04-implementation-stages/stage-05-context-and-recovery-hardening.md#measurable-exit-gates) | T-CTX-02 |
| S05-G3 | [S05, exit gate 3](../04-implementation-stages/stage-05-context-and-recovery-hardening.md#measurable-exit-gates) | T-CTX-03 |
| S05-G4 | [S05, exit gate 4](../04-implementation-stages/stage-05-context-and-recovery-hardening.md#measurable-exit-gates) | T-CTX-04 |
| S05-G5 | [S05, exit gate 5](../04-implementation-stages/stage-05-context-and-recovery-hardening.md#measurable-exit-gates) | T-CTX-02 |
| S05-G6 | [S05, exit gate 6](../04-implementation-stages/stage-05-context-and-recovery-hardening.md#measurable-exit-gates) | T-RUN-04, T-RELEASE-02, T-PERF-03 |
| S06-G1 | [S06, exit gate 1](../04-implementation-stages/stage-06-collaboration-and-content.md#measurable-exit-gates) | T-DAG-01 |
| S06-G2 | [S06, exit gate 2](../04-implementation-stages/stage-06-collaboration-and-content.md#measurable-exit-gates) | T-GIT-02 |
| S06-G3 | [S06, exit gate 3](../04-implementation-stages/stage-06-collaboration-and-content.md#measurable-exit-gates) | T-CONTENT-01, T-CONTENT-02 |
| S06-G4 | [S06, exit gate 4](../04-implementation-stages/stage-06-collaboration-and-content.md#measurable-exit-gates) | T-RELEASE-04 |
| S06-G5 | [S06, exit gate 5](../04-implementation-stages/stage-06-collaboration-and-content.md#measurable-exit-gates) | T-DAG-01, T-GIT-02 |
| S06-G6 | [S06, exit gate 6](../04-implementation-stages/stage-06-collaboration-and-content.md#measurable-exit-gates) | T-GIT-02, T-RUN-04 |
| S07-G1 | [S07, exit gate 1](../04-implementation-stages/stage-07-maps-and-structured-import.md#measurable-exit-gates) | T-MAP-01 |
| S07-G2 | [S07, exit gate 2](../04-implementation-stages/stage-07-maps-and-structured-import.md#measurable-exit-gates) | T-MAP-01 |
| S07-G3 | [S07, exit gate 3](../04-implementation-stages/stage-07-maps-and-structured-import.md#measurable-exit-gates) | T-MAP-02 |
| S07-G4 | [S07, exit gate 4](../04-implementation-stages/stage-07-maps-and-structured-import.md#measurable-exit-gates) | T-MAP-03 |
| S07-G5 | [S07, exit gate 5](../04-implementation-stages/stage-07-maps-and-structured-import.md#measurable-exit-gates) | T-MAP-03 |
| S07-G6 | [S07, exit gate 6](../04-implementation-stages/stage-07-maps-and-structured-import.md#measurable-exit-gates) | T-MAP-02, T-PERF-02, T-HOI-02 |
| S08-G1 | [S08, exit gate 1](../04-implementation-stages/stage-08-packaging-and-pilot.md#measurable-exit-gates) | T-SEC-01, T-VIS-01, T-SETUP-03 |
| S08-G2 | [S08, exit gate 2](../04-implementation-stages/stage-08-packaging-and-pilot.md#measurable-exit-gates) | T-RUN-01, T-RELEASE-01 |
| S08-G3 | [S08, exit gate 3](../04-implementation-stages/stage-08-packaging-and-pilot.md#measurable-exit-gates) | T-RUN-05, T-CTX-03 |
| S08-G4 | [S08, exit gate 4](../04-implementation-stages/stage-08-packaging-and-pilot.md#measurable-exit-gates) | T-RELEASE-02 |
| S08-G5 | [S08, exit gate 5](../04-implementation-stages/stage-08-packaging-and-pilot.md#measurable-exit-gates) | T-RELEASE-03 |
| S08-G6 | [S08, exit gate 6](../04-implementation-stages/stage-08-packaging-and-pilot.md#measurable-exit-gates) | T-RELEASE-04, T-RUN-06 |

## Result recording and exclusions

At runtime, each gate result includes the final source revision, exact fixture and profile, test invocation, observed output, artifact hashes and reviewer. Source inspection, upstream CI observation, static config parsing, fake-provider execution and actual model/game execution are different scopes. Report them independently.

No critical source-loss or unauthorized-effect failure can be waived through a generic “known limitations” sentence. Narrow the advertised operation, fix the defect or hold the release. Unavailable optional game checks remain visible, and the product cannot claim verified gameplay for those scenarios.

S03's minimal C-06 context fixture covers source/task/receipt/capability/budget continuity. S05 extends that foundation with canon, retrieval and reviewed learning. T-CTX-01 can therefore run a narrowed S03 assertion without pretending the entire S05 system exists.

The quality-review report records the completed planning checks for T-PKG-01 and T-RESEARCH-01 and the structural subset of T-DEV-02/T-DEV-03. Live client loading, model capability trials, MCP connectivity, product recovery and actual game acceptance remain unexecuted.

