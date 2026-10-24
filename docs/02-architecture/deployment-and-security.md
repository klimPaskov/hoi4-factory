# Deployment and security

Status: **Proposed**. Read [permissions](../03-subsystem-specs/011-extensibility-sandbox-and-permissions.md), [config migration](../07-developer-workflow/CONFIG-MIGRATION-REPORT.md) and [release stage](../04-implementation-stages/stage-08-packaging-and-pilot.md).

## Deployment target

The recommended first product is a local desktop application. It packages its native model worker and required tool adapters. End users should not install Claude Code or Codex. They supply permitted API credentials or a local inference endpoint and their own legally obtained game installation where game-aware operations require it.

Windows is the initial candidate release platform, subject to S01 packaging and recovery results. Linux and macOS support is earned through their own fixtures, including Unicode paths, permissions, process trees, filesystem semantics and updates. A browser-only app may offer reference viewing or ZIP planning later, but cannot silently claim the same native game access.

Tauri documents bundled external binaries by target architecture. This is a feasible packaging mechanism to test, not proof that this particular Rust/Node product has been packaged successfully. The [official sidecar documentation](https://v2.tauri.app/develop/sidecar/) is the reference for that experiment.

## Trust boundaries

The renderer, model output, source comments, imported skills, retrieved documents and tool descriptions are untrusted inputs. The trusted host enforces exact schemas and grants. Bundled worker code is part of the trusted software supply chain, but it should receive minimal operating-system authority. Third-party extensions require isolation before execution.

Tauri capabilities limit frontend exposure. Its documentation also notes that overlapping capabilities merge permissions and registered application commands need explicit attention. Therefore command authorization must be tested, not inferred from the use of Tauri. [Source](https://v2.tauri.app/security/capabilities/)

If Candidate B is selected, follow Electron's documented context isolation, renderer sandbox and restricted IPC model. The runtime worker must not share privileged Node APIs with untrusted rendered content. [Source](https://www.electronjs.org/docs/latest/tutorial/security)

## File and process access

Resolve roots through host-owned handles. Reject traversal, symlink escapes, unexpected volume changes and source precondition mismatches. Preserve existing files outside owned paths. A Git worktree provides change isolation, not an operating-system sandbox.

The first content tools are typed operations and allowlisted validation commands. A generic shell is unnecessary for the first end-user task. If later workflows require it, define executable, argument, working-directory, environment, network and timeout constraints with OS enforcement. Skill prose cannot expand those constraints.

Process cancellation must cover children and grandchildren on the supported platform. Record accepted cancellation separately from confirmed termination. A worker that ignores a soft termination signal must be stopped through a tested fallback. Late results cannot pass the current fencing token.

## Credentials, privacy and models

Store secrets in the platform credential mechanism or a reviewed equivalent. Pass only required secret references to the appropriate adapter. Redact before log persistence and before evidence export. Do not put API keys, provider cookies or private machine paths in mod Git history.

A provider profile identifies data destination, endpoint, authentication, model identity, terms/version reference and capability probe. A subscription to a chat product is not assumed to grant API entitlement. Automatic fallback only uses destinations already authorized for that task and data class. Local-only content remains local.

Usage reporting distinguishes provider-reported tokens, locally estimated tokens, known prices, billed cost and unknown values. Unknown is not zero. Budget limits stop new dispatch. A lost acknowledgement can leave billing uncertain and is reported without automatic duplicate generation.

## Supply chain and update policy

Pin the setup manifest and resolved component closure. Verify downloaded hashes and expected signatures where available. Avoid mutable latest URLs for reproducible setup. A new manifest can change tool behavior even if the main application is unchanged.

Maintain an inventory of direct and transitive licenses, native binaries, model licenses and bundled assets. Record the exact tested runtime and tool revisions. Update in a disposable fixture first, run source-preservation and recovery regressions, then publish a migration receipt. Preserve an uninstall and rollback path that leaves the user's mod files intact.

## Game and asset rights

The [supplied Paradox agreement](https://store.steampowered.com/eula/3493540_eula_0) describes noncommercial service and UGC use, free UGC and limits on third-party rights. Verify its applicability to the intended HOI4 distribution and business model before release. This planning review does not decide every commercial-tool question.

Game and DLC bytes stay on the user's machine and out of the application package. A repository's permissive code license does not automatically cover game references, wiki exports, music or third-party art. Asset provenance records original source, rights basis, transformations and intended distribution. Restricted editor code remains comparison-only unless permission changes.

## Release gates

The release package needs reproducible build provenance, license inventory, supported-platform smoke results, crash/recovery evidence, upgrade/uninstall preservation, secret-redaction checks and the declared model matrix. Actual game claims require observations on the named build and DLC profile with authorization.

Known critical source-loss or unauthorized-write failures block the affected write capability. An unavailable optional renderer or model does not block unrelated read-only use. The UI must make the limitation explicit. No “safe” label is earned solely by running a static package check or adopting a mature upstream framework.

