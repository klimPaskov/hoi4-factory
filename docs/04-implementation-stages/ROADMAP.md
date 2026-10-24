# Implementation roadmap

Status: **Proposed, awaiting review and bounded stage authorization**. This package completes planning. It does not authorize product implementation. The maintainer has deferred implementation. [MASTER_GOAL](../MASTER_GOAL.md) defines the product, the [maintainer decisions](../02-architecture/maintainer-decisions-2026-10-24.md) of 2026-10-24 set the hosted web, Rust runtime, Docker, GitHub and licensing direction, and each stage below defines a separately reviewable unit of work.

## Dependency order and release checkpoints

```mermaid
flowchart TD
    S01["S01 Bootstrap and probes"] --> S02["S02 Accounts, workspaces and import"]
    S02 --> S03["S03 Rust agent runtime"]
    S03 --> S04["S04 Focus vertical slice"]
    S04 --> S05["S05 Recovery and context"]
    S05 --> S06["S06 Collaborators and content"]
    S05 --> S07["S07 Maps"]
    S06 --> S08["S08 Deployment and public pilot"]
    S07 --> S08
```

| Stage | Reviewable capability | Entry dependency | Release significance |
|---|---|---|---|
| [S01](stage-01-bootstrap-and-probes.md) | Claude-first workspace, verified config migration, Cargo and Compose skeleton, evidence from Setup on server paths, preservation, Rust runtime, container and GitHub probes | Explicit authorization for S01 | Architecture confirmation, not a product release |
| [S02](stage-02-onboarding-and-workspaces.md) | Accounts with GitHub and email sign-in, per-account server workspaces, reused new/import setup, upload and GitHub import, read-only game reference | S01 decision receipt and cleared preservation risk for the selected pin | Foundation usable by later stages |
| [S03](stage-03-native-agent-and-durability.md) | Native Rust agent loop, durable attempts, scoped tools, interruption, truthful capability profiles, user keys and operator allowance metering | S02 | Native task substrate |
| [S04](stage-04-focus-vertical-slice.md) | Small real focus change in the browser through visual, hybrid or autonomous modes, with review, commit, GitHub pull request and restore | S03 | First useful demonstration |
| [S05](stage-05-context-and-recovery-hardening.md) | Cross-session and cross-model continuity, freshness, approved canon and reviewed runtime lessons | S04 | Required before any pilot |
| [S06](stage-06-collaboration-and-content.md) | Invited collaborators with project roles, dependency tasks, localization and assets, controlled concurrency | S05 | Required for the first public release |
| [S07](stage-07-maps-and-structured-import.md) | Map inspection, bounded structured import and one supported source-preserving edit | S05, plus S06 only for reused content facilities | Expanded map scope, deferrable by review |
| [S08](stage-08-packaging-and-pilot.md) | Docker deployment on staging, tenant and security validation, backup and upgrade, provider comparison, rights and license gate, observed pilot | S04, S05, S06, and S07 unless explicitly deferred | Reviewable public release decision |

Every stage contains six work packets with conceptual owners, deliverables, dependencies, explicit non-goals and evidence gates. There are 48 packets in this baseline. Owners describe responsibilities. They are not a prescription to run 48 agents.

## Smallest credible product

The first useful demonstration is S04. Start with a small three-to-five-focus branch, one supported source profile, an owned branch in a server workspace and a bounded model task. Create one fresh mod and import one established synthetic fixture from a disposable GitHub repository through reused Mod Setup. Run a native model call, generate and approve a plan, apply a supported change, inspect source and visual evidence in the browser, commit owned changes, push the branch and open a pull request under a separate grant, then restore or continue externally.

The visual-first path includes a supported direct node or scalar edit. The hybrid path combines a prompt and a human adjustment. The autonomous path completes with the browser closed. They use the same source preconditions, grants, evidence and history. “Equivalent” means equivalent accepted semantics and preservation, not identical generated prose or pixel positions.

S04 is not enough for a public release. S05 supplies stronger model handoff, context, stale-evidence and long-sequence recovery guarantees. S06 supplies invited collaborators, which the maintainer requires in the first public release. S08 supplies deployment, tenant isolation, backup, security, rights review, support claims and pilot evidence. A reviewed release may defer S07, but it must explicitly exclude map capabilities and still pass applicable S08 gates. This is a scope decision, not a skipped test disguised as success.

## Reuse and integration milestones

S01 resolves the observed Setup recovery failure on server workspace paths, establishes the usable parser subset inside the Linux image and measures the open Rust runtime questions before selecting pins. S02 adapts Setup's existing desktop scanner and transaction services to server paths alongside its portable core. The current Tauri commands are inspected interfaces only. A reusable Rust facade is proposed work.

S03 connects the Rust agent loop to host-owned contracts C-01 through C-10, including C-10 RepositoryLink for GitHub operations and reconciliation. The Rust server owns grants and effects. S04 proves that native files, browser projections, autonomous work and GitHub publication converge through that authority. S05 proves that source and task truth survive replacement of the model and the chat.

S06 adds human collaborators and agent concurrency only after single-writer evidence exists. S07 starts with existing structured files and bounded edits because the audited geography-model synthesis path is unavailable. Neither a renderer nor a reference-image generator supplies missing playable-world generation.

## Parallel work without split authority

Within S01, Setup, source-fidelity and runtime probes may run independently after the environment and guidance are established. Their owners must use separate disposable fixtures and return evidence to one decision owner.

Within later stages, browser projections, deterministic fixtures and documentation can be developed in parallel against a frozen contract. Operations that alter the shared host journal, grant semantics, account isolation, parser API or Git and GitHub publication path require coordinated ownership and integration before acceptance. Never merge two competing authoritative implementations of the same contract.

After S05, S06 and S07 may progress in parallel when they have separate modules and source fixtures. A map packet that depends on shared asset provenance or review code waits for that specific S06 packet. S08 pilot execution waits for a frozen advertised capability set.

## Gate semantics

A stage is complete only when every mandatory exit gate has a result against its final revision. A known source-loss, cross-account or unauthorized-write failure blocks the affected capability. Missing credentials, hardware, a GitHub test repository or a user-operated game produce an explicit unavailable result. They do not count as a pass.

The [acceptance matrix](../05-verification/REQUIREMENTS-ACCEPTANCE-MATRIX.md) maps all 48 requirements and all 47 stage gates to planned checks. The [evaluation matrix](../05-verification/EVALUATION-MATRIX.md) defines model tasks and measurement. Stage evidence uses the six-file handoff convention in [the handoff template](../06-agent-handoffs/STAGE-HANDOFF-TEMPLATE.md).

Review a stage's actual result before authorizing the next. The future agent reports a concrete next-stage scope and stops. No handoff grants automatic progression.

## Effort and uncertainty

No calendar or person-day estimate is claimed. The dominant uncertainties are source-preserving adapter coverage, recovery correctness, the maintenance cost of a project-owned Rust agent loop, tenant isolation and containment of model-directed tools on shared servers, and operating cost of a free public service. S01 turns the technical ones into bounded experiments. The rights review and hosting choice are maintainer decisions recorded in S08.

Use [the backlog](BACKLOG-AND-DEFERRED-SCOPE.md) to preserve aspirations without inflating the first slice. Every substantive packet, debugging fix and stage concludes with the learning and configuration reviews. A justified no-change outcome is valid.
