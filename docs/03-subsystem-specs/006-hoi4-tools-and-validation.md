# SPEC-006: HOI4 tools and layered validation

Status: **Proposed**. Primary requirements: REQ-025, REQ-026, REQ-027, REQ-040 and REQ-048. Shared requirements: REQ-006, REQ-011, REQ-024, REQ-028, REQ-034 and REQ-046. Inspect compatibility in S01, provision the server game reference and record profiles in S02, establish adapters in S03 and prove the first useful validation loop in S04. The game reference follows [ADR-015](../02-architecture/maintainer-decisions-2026-10-24.md#adr-015-server-held-game-reference).

Read first: [existing assets audit](../00-research/02-existing-assets-audit.md), [contracts](../02-architecture/contracts-and-interfaces.md), [native runtime](002-native-runtime-and-models.md), [permissions](011-extensibility-sandbox-and-permissions.md), [focus editors](009-focus-and-other-editors.md) and [maps](008-maps.md).

## Purpose and evidence boundary

The user needs useful tools that understand HOI4 source and honest evidence about what was checked. A file can parse correctly while a reference is missing, a modeled effect can pass while the game behaves differently, and an attractive preview can omit a conditional state. Each result therefore identifies its inputs, tool version, checks, coverage and limitations. The product never collapses these layers into a single claim that the mod works.

The leading adapters are HOI4 Agent Tools for source, rendering and bounded mutation, CWTools with its independently pinned HOI4 rule pack for static analysis, and optional HOI4 Live Tools for supported actual-game observations. These are separable capabilities. An unavailable live adapter does not prevent a source-only workflow, and an unsupported source construct does not authorize a lossy rewrite. [SRC-A10 to SRC-A22, audit](../00-research/02-existing-assets-audit.md)

## Source-backed reusable components

Agent Tools `25f5641bbc07c65a822ef576ac6e92bd12666880` has typed MCP tools, stdio and authenticated HTTP transports. Its parser retains original bytes and source ranges. No-op serialization returns original bytes, and targeted replacements preserve supported encoding and untouched text. Unsafe parse diagnostics block rewriting. The transaction executor validates before and after effects and can roll back failures. [SRC-A11, source implementation](https://github.com/klimPaskov/hoi4-agent-tools/blob/25f5641bbc07c65a822ef576ac6e92bd12666880/src/hoi4_agent_tools/core/transaction-execution.ts)

CWTools `b377453dee803f9258be92cfc49896d09039702d` exposes HOI4 parser, semantic, localization, reference and scope services. Its pretty-printer normalizes formatting, so this proposal uses it as an independent analyzer. Its rule pack `297dd69c42aabe10078a8b7ab7f2d07ee631d4ef` is a separate compatibility input and acknowledges variable/array false positives. [SRC-A20 to SRC-A22, HOI4 engine](https://github.com/cwtools/cwtools/blob/b377453dee803f9258be92cfc49896d09039702d/CWTools/Game/HOI4/HOI4Game.fs)

Live Tools `6b91a2a43edfeab60a415ec2ddd9b71d215b8a56` has a native Linux path bound to an exact executable hash and build ID. It reports unavailable and unverified capabilities, with native Windows/macOS adapters and native mod QA explicitly unavailable in the inspected runtime. The published Python release is a different target. Installation of the package is not sufficient to qualify a game observation. The hosted service does not run the game, so Live Tools is not part of the server deployment. It remains a reference for user-operated observation on the user's own machine. [SRC-A17, capability response](https://github.com/klimPaskov/HOI4-Live-Tools/blob/6b91a2a43edfeab60a415ec2ddd9b71d215b8a56/native/src/protocol.rs)

## Server game reference

The server holds one full copy of Hearts of Iron IV, the latest build with all DLC, as a shared read-only reference for every project. Validation, reference lookups, previews and asset resolution read it. It replaces the user's local installation root that the desktop plan assumed.

| Rule | Requirement |
|---|---|
| Mounting | Mounted read-only into the tool processes or containers that need it. No tool receives write access to it |
| Images and exports | Never baked into Docker images, never included in user downloads, pushes or pull requests |
| User access | Never served as raw files. Only derived results leave the server, such as diagnostics, rendered previews of a user's own mod and short reference excerpts needed to explain a check |
| Identity | A reference manifest records build ID, version string, DLC set, file count and a content hash. Every evidence record names that manifest |
| Updates | A new game build is installed beside the old one, verified, then switched atomically. Diagnostics and previews tied to the old manifest become stale |
| Rights | The game-rights review is a release gate. It is completed and recorded before the service is exposed on a public domain |

Mods that declare an older supported game version are validated against the current build with a visible compatibility warning. The factory does not keep older builds. A diagnostic that may depend on that version difference is labelled with it.

## Capability and compatibility inventory

At workspace activation, collect tool executable or package identity, source revision, installed dependency closure, protocol version, schemas, supported operations, platform constraints and health result. Bind this inventory to C-01. The actual setup manifest may install Agent Tools 4.0.3 even though the standalone audit examined 4.3.0. The adapter supports the installed contract and treats an upgrade as a reviewed compatibility change.

The game profile records the server reference manifest, the mod's declared supported game version and any compatibility warning, the DLC set assumed for the check, language, dependency mods, load order and descriptor `replace_path` behavior. Distinguish observed values from declarations. A dependency mod is resolved from another project the user can read or from an uploaded copy, never fetched implicitly. A different profile or reference manifest invalidates affected diagnostics and previews. Capability probing may initialize and list tools but must not spend image-generation credits or edit source.

| Adapter operation | Inputs | Outputs and important errors |
|---|---|---|
| Discover and health | Image-pinned tool identity, workspace roots, game reference manifest and bounded timeout | Versioned capability inventory, unsupported protocol, absent runtime or auth failure |
| Lookup source | Snapshot, symbol or source location and layer selection | Exact definitions, usages, override provenance and truncated-count metadata |
| Inspect or render | Snapshot, domain selector, scenario and render budget | C-05 artifacts, diagnostics, fidelity and continuation references |
| Validate | Snapshot, changed paths, rule version and named checks | Diagnostic set, baseline delta, coverage, unavailable checks and evidence hashes |
| Prepare a mutation | C-02 intent, allowed paths and source hashes | Candidate bytes and semantic diff in an isolated proposal workspace, or explicit unsupported operation |
| Execute a mutation | Final normalized arguments, C-08 grant and C-04 operation identity | Applied, unchanged, blocked, error or unknown-effect receipt |
| Import observation | A user-operated observation report from the user's own machine with build, DLC, load order and scenario | Recorded as user-reported game evidence with its scope, never as server-verified behavior |
| Reconcile | Existing operation or job identity | Durable outcome, pending state or unresolved effect, without replay by default |

## Validation layers

| Layer | Examples | What a successful result establishes |
|---|---|---|
| Source integrity | Parse, encoding, duplicate IDs, path containment, unchanged spans | The selected source representation and write operation satisfy named structural rules |
| Static domain checks | CWTools rules, Agent Tools reference and scope checks, localization keys | The selected static checks found no new classified blocking problem in their coverage |
| Package completeness | Definitions, registrations, asset links and declared test cases | Required named package members are present or explicitly unresolved |
| Modeled scenarios | Declared effect steps and assertions | A bounded interpreter produced the expected result for the declared model and inputs |
| Offline visual evidence | Focus, GUI, technology or map rendering | The renderer's interpretation of the exact source snapshot |
| Actual game observation | User-operated run on the user's own machine with a recorded scenario | The reported behavior for that build, DLC profile, state and time |

The Agent Tools mechanic interpreter treats unsupported effects and unknown write sets as unresolved. Advancing modeled days only expires modeled timed flags, not all game on-actions. Package-check case presence is distinct from executing the case. Preserve those distinctions in C-05 and in user-facing labels. [SRC-A14, modeled semantics](https://github.com/klimPaskov/hoi4-agent-tools/blob/25f5641bbc07c65a822ef576ac6e92bd12666880/docs/mechanics.md)

A check has pass, fail, unresolved, unavailable or not-run status. A proposal gate evaluates required checks for its actual operation category. Newly introduced syntax corruption blocks a source mutation. A known baseline rule warning may be retained with a reviewed reason. Optional game observation can remain not run for a source-only task. The product must not treat every unknown as global failure or silently upgrade it to pass.

## Mutation and source-preservation contract

Use C-02 to bind semantic intent, base snapshot, per-path hashes, source ranges, dependencies and expected postconditions. The host checks the final normalized tool arguments after model and extension hooks. The tool's own transaction and the factory's task state have distinct responsibilities. The host owns policy, grants and published workspace state. The upstream tool owns its internal atomic mutation and recovery within the authorized task root.

Public Agent Tools domain rewrites keep planning and application within one call. Do not expose fictional upstream transaction IDs or bypass its workflow. For a required preview, run the call in an isolated proposal workspace or use a proven read-only core preparation adapter. Re-read candidate bytes and diff them before publication. A broader autonomous grant can authorize bounded task-root changes directly, while the receipt still records their scope and effects.

Preserve comments, duplicate and unknown keys, raw unsupported blocks, ordering, BOM and newline style where the selected writer proves support. Unsupported encodings or unsafe source remain inspectable and read-only. No pretty-printer should rewrite an imported whole file merely to expose one scalar edit. A failed after-check invokes supported rollback and records any conflict if concurrent user work prevents exact restoration.

## Transport, artifacts and long work

Run each tool adapter in a supervised worker inside the deployment, reached over stdio or an internal network interface, never exposed to the public internet. Cap process resources, artifact storage and operation duration per account and per task. Each tool invocation sees only its project's workspace and the read-only game reference. The tool protocol must negotiate version and task support rather than assume every provider client accepts background tasks. A UI disconnect does not mean a tool did not finish. Persist its request identity and reconcile its job or receipt before retrying.

If an adapter's HTTP transport is used between containers, enforce explicit origin and host configuration, authentication, workspace grants and scope. The source's static bearer mode grants both read and write. A token must not be advertised as read-only because the UI hides mutation buttons. Inspected tests reject read-scope writes and session takeover with a replacement credential. [SRC-A15, authorization tests](https://github.com/klimPaskov/hoi4-agent-tools/blob/25f5641bbc07c65a822ef576ac6e92bd12666880/tests/integration/http-authorization.test.ts)

Store linked artifacts by content hash in the object store with provenance, source snapshot, tool version and game reference manifest. Fetch continuation ranges explicitly and record omissions. Sanitized HTML/SVG previews are served to the browser from an isolated origin or sandboxed frame with no script access to the application session. A render operation that writes review artifacts is distinct from a mod-source mutation. C-08 permissions represent that distinction even if a generic MCP read-only hint is false.

## Live effects and uncertainty

The hosted service sends no input to a running game. These rules apply if a later reviewed feature lets a user connect a game session on their own machine. Use live tools only for supported owned sessions. Recheck executable, process and campaign identity immediately before an effect. The upstream pause/speed journal writes dispatch intent durably and quarantines an uncertain dispatch, including attempts under a fresh request ID. A verified receipt is historical evidence and does not guarantee the game still has that state. [SRC-A18, action journal](https://github.com/klimPaskov/HOI4-Live-Tools/blob/6b91a2a43edfeab60a415ec2ddd9b71d215b8a56/native/src/actions.rs)

A timeout after dispatch produces `unknown_effect` and a reconciliation requirement. No automatic input replay, debug fallback or unsupported memory-command path is allowed. An unsupported game build returns a useful limitation and keeps source work available. Actual-game evidence must name the scenario and cannot certify every branch, language or DLC combination.

## Acceptance tests and stage ownership

| Test | Observable expectation |
|---|---|
| BOM/CRLF/comment/unknown-key corpus | No-op preserves exact bytes, scalar edit changes only approved spans or refuses |
| Malformed and unsupported source | No destructive fallback, explicit location and unsupported reason |
| Baseline rule disagreement | Tool-specific diagnostics and rule revisions retained, with no fabricated consensus |
| Tool replaced after discovery | Hash or version mismatch invalidates capability and requires rediscovery |
| Stale path or dependency changed | Apply refuses before source publication |
| Connection lost after mutation request | Host reconciles one operation, never duplicates effects by blindly retrying |
| Optional live runtime absent | Source workflow remains usable with unrun game evidence |
| Server game reference updated (T-GAME-01) | Old diagnostics and previews become stale, new evidence names the new manifest |
| Request for raw game files through any API or export | Refused, and the fixture export contains no game bytes |
| Mod declares an older game version | Validation runs with a visible compatibility warning on affected results |
| Unsupported game executable | Live adapter refuses and records exact limitation |
| Late observation | Historical result cannot authorize current input or count as fresh success |
| Artifact truncation or hostile markup | Explicit continuation and isolated display, with no hidden omitted failure |

S01 records versions, license inventory, source-preservation results and measured adapter feasibility. S03 establishes C-04 supervision and final-argument authorization. S04 demonstrates source edit, independent checks, fresh preview and review against one small focus task. S07 adds map-specific checks. S08 verifies the deployed images and game reference handling, with actual evidence files and all not-run capabilities declared.
