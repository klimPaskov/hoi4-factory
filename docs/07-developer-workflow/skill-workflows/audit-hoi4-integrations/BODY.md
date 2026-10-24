# Audit a bounded HOI4 integration

## Trigger and inputs

Use this workflow for an explicitly requested source, config or available-tool audit relevant to the approved stage. Inputs are the named repository/tool, exact requested capability, source access, allowed workspace roots and existing evidence. Source inspection does not authorize installation, server activation, paid generation or external writes.

Read [HOI4 tools specification](../../../03-subsystem-specs/006-hoi4-tools-and-validation.md), [onboarding specification](../../../03-subsystem-specs/001-onboarding-and-mod-setup.md), [config migration](../../CONFIG-MIGRATION-REPORT.md), [source inventory](../../CONFIG-SOURCE-INVENTORY.json), and [research log](../../../00-research/04-evidence-and-research-log.md).

## Procedure

1. Define the capability and evidence needed. Pin the repository revision, manifest, package version and relevant file paths. Distinguish the actual Agentic repository config from the supplied ChaosRedux historical config.
2. Read the actual implementation and its relevant interfaces, tests and license notices. Follow referenced config layers where available. Do not infer native behavior solely from README claims, generated types or a command name.
3. Record inputs, outputs, workspace assumptions, effects, dependencies, version limits, auth/cost needs, failure behavior and recovery. Separate verified reuse from a proposed adapter. Compare source and generated artifacts where drift matters.
4. For Mod Setup, preserve its inspect, plan, approval, staged apply and actual recovery contracts. Record any known failure, including exact-restore limitations. Do not substitute a new file copier or claim arbitrary rollback from a resume command.
5. For tool access, inspect available capabilities without enabling a missing dependency. A harmless read-only probe may run when already authorized and installed. Record exact arguments, versions, input hashes and returned limitations. Use an effectful tool only within its independently authorized scope.
6. Classify results as source inspected, statically checked, client loaded, tool exercised or game observed. Record unavailable checks explicitly. A declared CI workflow is not evidence of a successful CI run.
7. Map the finding to requirements, contracts and stage decisions. Propose the smallest adapter or experiment that resolves a material uncertainty. Update config-review.md for connection or capability changes.
8. Return provenance, compatibility, license, safety, test gaps and next decisions. Capture a reusable engineering lesson when warranted, with review before promotion.

## Outputs and evidence

Create a bounded audit under `evidence/SNN/` with exact sources, revisions, paths, capabilities, test commands/results, artifact hashes, confidence and limitations. Link it from checks.json and handoff.md. Record source-to-target config changes in the migration map rather than activating an undocumented key.

## Stop and error paths

If a source or layer is inaccessible, mark that claim unknown and identify the exact missing evidence. Continue independent accessible inspection. If required dependencies are absent, report an unavailable runtime check and a concrete activation plan. Never fabricate an executable path, token, schema field or game pass.

If a source contract conflicts with the stage assumption, preserve the finding and route the architecture decision. If a tool's outcome is uncertain, reconcile its receipt and actual state before retrying. Do not upload licensed game data or private mod content merely to complete an audit.
