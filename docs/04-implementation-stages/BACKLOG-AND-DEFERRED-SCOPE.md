# Prioritized backlog and deferred scope

Status: **Proposed**. Deferred features remain visible and traceable. A backlog entry is not implementation authorization. Review its evidence, dependency and acceptance gate before moving it into a stage.

## Priority order after the first useful slice

| Priority | Capability and value | Prerequisites and intended contract | Main uncertainty | Promotion test |
|---|---|---|---|---|
| P1 | Reliable event and decision authoring alongside focus branches | S05, selected S06 packets, C-02 source proposals and C-05 evidence | Audited event and technology MCP tools include read surfaces, not a general verified writer | A bounded writer preserves untouched bytes and catches broken references across a complete fixture |
| P1 | Localization and imported asset workflow | S05/S06, asset provenance and consumer wiring | Encoding, conversion fidelity, namespacing and rights | Missing-key, BOM, invalid-encoding, duplicate-destination and unused-asset fixtures pass |
| P1 | Contributor reviews with one human and one agent | S05/S06, C-08 grants and owned Git workspaces | Semantic conflicts can evade text merge checks | Conflict trials catch shared IDs, focus logic, localization keys and binaries without losing either contribution |
| P1 | Readable existing maps and structured import | S05/S07, source-linked map projection and dependency closure | Malformed inputs, remapping and performance | F-MAP-STRUCTURED and F-MAP-BROKEN pass, unsupported synthesis remains explicit |
| P2 | Additional bounded map painting or boundary tools | S07, known topology and rollback envelope | A visual stroke can change many linked files | One operation has exact ownership, stale-source rejection, topology tests and declared game evidence |
| P2 | Larger focus trees and incremental projections | S04/S05, immutable projection generation | Renderer and parser scaling | 2,000-node fixture benchmark identifies the bottleneck and demonstrates a measured improvement |
| P2 | Two disjoint agents with dependency scheduling | S06, leases, fences and merge gates | More attempts may increase rework and cost | Accepted output improves over single-writer baseline without worse integrity or uncontrolled retries |
| P2 | Linux and macOS desktop support | S08 platform-specific packaging and recovery | Tool availability, permissions, filesystem and binary differences | Clean installation, native task, cancellation and restore pass on each claimed platform |
| P2 | Broader provider profiles | S03/S05/S08, capability probe and destination grants | Tool schemas, context limits, privacy and cost differ | Full recorded profile runs required scenarios without silent downgrade |
| P3 | Rich event, decision, idea, character, technology and diplomacy editors | Narrow existing domain adapters plus actual demand | A universal editor could hide unsupported semantics | Each proposed editor justifies user value and independently passes preservation and concurrency tests |
| P3 | Reference-to-world generation from geography and lore | S07 plus a separately reviewed synthesis design | Current audited geography-model write path is unavailable | A new implementation produces complete native data, passes topology checks and receives scoped actual game evidence |
| P3 | Hosted coordination and remote execution | S06/S08, privacy/data residency decisions and real remote sandbox | Distributed ownership, accounts, project confidentiality and operating cost | Offline local workflow remains usable, server outages and partition recovery preserve all accepted work |
| P3 | Real-time multi-user coediting | Stable source semantics and conflict model | Native files, binary maps and semantic constraints resist generic text CRDT assumptions | Concurrent editing proves explicit convergence with no loss of unknown source constructs |
| P3 | Advanced image and 3D generation | S06 rights/provenance and optional provider grants | Asset licensing, privacy, conversion and reliable consumer wiring | Generated asset is licensed, exported, wired, rendered and separately qualified in the intended game profile |
| P3 | Semantic or vector retrieval service | EXP-05 exact/domain/full-text baseline | Extra databases, stale indexes and operational cost | Measured missed-fact reduction exceeds agreed cost and correctness thresholds |
| P3 | External editor ACP and product MCP export | S05, separate public API threat model | Protocol versioning, user identity and write authority | Negotiation and revocation work with a named client, export remains read-only until a separate write review |
| P3 | Large swarms and fully automatic shared-branch integration | Repeated successful two-worker trials and explicit maintainer policy | Ownership ambiguity, compound retries, cost and review overload | Evidence supports accepted throughput and recovery at each increased worker count |

## Explicit first-slice exclusions

The first slice does not include a complete map suite, general-purpose script generation, every game editor, arbitrary executable extensions, cloud hosting, automatic publication, autonomous safety-policy changes or unattended writes to a shared main branch. It also does not promise that source previews reproduce every game layout or that a passing simulation proves gameplay.

These exclusions follow the reuse and integrity evidence in [the audit](../00-research/02-existing-assets-audit.md) and [architecture decisions](../02-architecture/options-and-decisions.md). They are scope controls, not a permanent rejection of the product vision.

## Backlog refinement rule

A promoted entry needs a named owner, user scenario, supported input subset, contract changes, migration effects, proposed fixtures, license position and a stopping condition. It must explain which existing capability it reuses and what missing behavior it adds. User demand alone does not establish that an external tool implements the required write operation.

Keep candidate designs in an ADR or stage proposal. Do not add active config, install services or create unused production abstractions solely to anticipate an unapproved backlog item.

## Retention and retirement

At every major stage review, check whether a deferred item has become unnecessary, is now supported upstream or should be narrowed. Preserve its decision history. A retired feature remains discoverable with the reason and replacement, if any. The maintenance review also checks whether old guidance accidentally describes a deferred feature as available.

