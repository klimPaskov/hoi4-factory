# Prioritized backlog and deferred scope

Status: **Proposed**. Revised on 2026-10-24 for the [maintainer decisions](../02-architecture/maintainer-decisions-2026-10-24.md). Deferred features remain visible and traceable. A backlog entry is not implementation authorization. Review its evidence, dependency and acceptance gate before moving it into a stage.

## Priority order after the first useful slice

| Priority | Capability and value | Prerequisites and intended contract | Main uncertainty | Promotion test |
|---|---|---|---|---|
| P2 | Desktop companion app for macOS, Linux and Windows (ADR-026), synced with the web app, with local tools such as HOI4 Live Tools and a locally installed Agent Tools MCP | The web app complete enough to be the reference, with the server staying the single authority for projects, tasks, receipts, grants and Git history | Sync conflicts between web and desktop members, local tool trust, packaging and signing on three platforms | A web member and a desktop member edit one project and each sees the other's accepted changes with no lost or duplicated work |
| P2 | Usage priority and plan tiers (ADR-027): new users first in the queue, higher plans more priority and credits | S03 metering and admission, the credits backlog (Stripe) | Fairness under load, abuse of new accounts | Under a saturated runner pool, queued work is admitted in the specified order, and allowances follow the plan |
| P1 | Reliable event and decision authoring alongside focus branches | S05, selected S06 packets, C-02 source proposals and C-05 evidence | Audited event and technology MCP tools include read surfaces, not a general verified writer | A bounded writer preserves untouched bytes and catches broken references across a complete fixture |
| P1 | Localization and imported asset workflow | S05/S06, asset provenance and consumer wiring | Encoding, conversion fidelity, namespacing and rights | Missing-key, BOM, invalid-encoding, duplicate-destination and unused-asset fixtures pass |
| P1 | Readable existing maps and structured import | S05/S07, source-linked map projection and dependency closure | Malformed inputs, remapping and performance | F-MAP-STRUCTURED and F-MAP-BROKEN pass, unsupported synthesis remains explicit |
| P1 | Credit purchasing from the operator through Stripe | S03 metering, S08 release, Stripe integration, tax registration and pricing | Payment provider terms, refunds, tax, fraud and the game-rights position of paid use | Purchased credits are metered exactly once, failed or uncertain payments never grant credit, and the rights review covers paid use |
| P2 | Steam Workshop publishing | S04/S05 publication grants, Steam integration research | Workshop upload needs Steam identity and tooling that the server may not hold | A disposable item uploads under explicit user authority with an exact receipt, and a lost reply is reconciled before retry |
| P2 | Additional forges such as GitLab or Codeberg | S04 GitHub publication path behind a forge interface | Different permission models, rate limits and review objects | Import, push and merge request flows pass the same reconciliation tests as GitHub |
| P2 | Older game builds held on the server | S02 game reference mount and per-mod target version | Storage, update work and rights for multiple builds | A mod targeting an older build validates against that build with a recorded profile, and caches invalidate per build |
| P2 | Additional bounded map painting or boundary tools | S07, known topology and rollback envelope | A visual stroke can change many linked files | One operation has exact ownership, stale-source rejection, topology tests and declared game evidence |
| P2 | Larger focus trees and incremental projections | S04/S05, immutable projection generation | Renderer and parser scaling in the browser | 2,000-node fixture benchmark identifies the bottleneck and demonstrates a measured improvement |
| P2 | Two disjoint agents with dependency scheduling | S06, leases, fences and merge gates | More attempts may increase rework and cost | Accepted output improves over single-writer baseline without worse integrity or uncontrolled retries |
| P2 | Broader provider profiles | S03/S05/S08, capability probe and destination grants | Tool schemas, context limits, privacy and cost differ | Full recorded profile runs required scenarios without silent downgrade |
| P3 | Rich event, decision, idea, character, technology and diplomacy editors | Narrow existing domain adapters plus actual demand | A universal editor could hide unsupported semantics | Each proposed editor justifies user value and independently passes preservation and concurrency tests |
| P3 | Reference-to-world generation from geography and lore | S07 plus a separately reviewed synthesis design | Current audited geography-model write path is unavailable | A new implementation produces complete native data, passes topology checks and receives scoped actual game evidence |
| P3 | Multi-region or horizontally scaled hosting | S08 deployment and measured load | Distributed ownership, data residency and operating cost | Server outages and partition recovery preserve all accepted work, and leases stay single-owner across nodes |
| P3 | Real-time co-editing of one file | Stable source semantics and conflict model | Native files, binary maps and semantic constraints resist generic text CRDT assumptions | Concurrent editing proves explicit convergence with no loss of unknown source constructs |
| P3 | Advanced image and 3D generation | S06 rights/provenance and optional provider grants | Asset licensing, privacy, conversion and reliable consumer wiring | Generated asset is licensed, exported, wired, rendered and separately qualified in the intended game profile |
| P3 | Semantic or vector retrieval service | EXP-05 exact/domain/full-text baseline | Extra databases, stale indexes and operational cost | Measured missed-fact reduction exceeds agreed cost and correctness thresholds |
| P3 | External editor ACP and product MCP export | S05, separate public API threat model | Protocol versioning, user identity and write authority | Negotiation and revocation work with a named client, export remains read-only until a separate write review |
| P3 | Large swarms and fully automatic shared-branch integration | Repeated successful two-worker trials and explicit maintainer policy | Ownership ambiguity, compound retries, cost and review overload | Evidence supports accepted throughput and recovery at each increased worker count |

Invited collaborators and hosted operation moved out of this backlog and into S06 and S08, because the maintainer requires them for the first public release.

## Explicit first-slice exclusions

The first slice does not include a complete map suite, general-purpose script generation, every game editor, arbitrary executable extensions, credit purchasing, Steam Workshop publishing, other forges, automatic merging of pull requests, autonomous safety-policy changes or unattended writes to a shared default branch. It also does not promise that source previews reproduce every game layout, that a passing simulation proves gameplay or that the server runs the game.

These exclusions follow the reuse and integrity evidence in [the audit](../00-research/02-existing-assets-audit.md) and [architecture decisions](../02-architecture/options-and-decisions.md). They are scope controls, not a permanent rejection of the product vision.

## Backlog refinement rule

A promoted entry needs a named owner, user scenario, supported input subset, contract changes, migration effects, proposed fixtures, license position and a stopping condition. It must explain which existing capability it reuses and what missing behavior it adds. User demand alone does not establish that an external tool implements the required write operation.

Keep candidate designs in an ADR or stage proposal. Do not add active config, install services or create unused production abstractions solely to anticipate an unapproved backlog item.

## Retention and retirement

At every major stage review, check whether a deferred item has become unnecessary, is now supported upstream or should be narrowed. Preserve its decision history. A retired feature remains discoverable with the reason and replacement, if any. The maintenance review also checks whether old guidance accidentally describes a deferred feature as available.
