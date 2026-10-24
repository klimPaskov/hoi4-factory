# Deployment and security

Status: **Proposed, revised by [maintainer decisions of 2026-10-24](maintainer-decisions-2026-10-24.md)**. Read [permissions](../03-subsystem-specs/011-extensibility-sandbox-and-permissions.md), [config migration](../07-developer-workflow/CONFIG-MIGRATION-REPORT.md) and [release stage](../04-implementation-stages/stage-08-packaging-and-pilot.md).

## Deployment target

The product is a public web service. Anyone may sign up through GitHub or email. The Rust server, its bundled tool adapters and the agent crate ship as Docker images. Docker Compose describes the service set, at minimum the web server, PostgreSQL and any tool workers. Development, self-hosting and the public service start from the same images. The design stays portable to any Linux host, and no hosting provider is chosen yet.

End users need only a browser. They do not install Claude Code, Codex, a desktop shell or a local model. They may bring their own model API key, use the operator's credit allowance, or configure any OpenAI-compatible endpoint the server can reach.

Images must be reproducible. Pin base images and record their digests. Record the exact Rust toolchain, crate lockfile, frontend lockfile and bundled tool versions in the build provenance. Images must not contain game or DLC files, credentials, user mod data or machine-specific configuration. T-DEPLOY-01 verifies this.

## Persistent data

| Store | Contents | Placement |
|---|---|---|
| PostgreSQL | Accounts, memberships, grants, tasks, events, receipts, budgets | Managed or containerized database with backups |
| Object store | Content-addressed artifacts, previews, evidence bytes | S3-compatible service or mounted volume behind one interface |
| Git volume | One repository per mod workspace, with owned worktrees | Persistent volume, never inside an image |
| Secret store | Bring-your-own model keys, GitHub App tokens, operator credentials | Encrypted at rest, scoped references only |
| Game reference | Full game copy, latest build with all DLC | Read-only mount, never exported |

Backups cover PostgreSQL, the object store and the Git volume together with a recorded consistency point. The game reference is reinstalled rather than backed up. Restore is tested under EXP-08.

## Trust boundaries

The browser, model output, source comments, imported repositories, uploaded archives, retrieved documents and tool descriptions are untrusted inputs. The trusted host enforces exact schemas, tenant membership and grants on every request. Bundled adapter code is part of the trusted supply chain, but it receives minimal authority.

A public multi-tenant service makes tenant isolation the first boundary. Every query, file path resolution, Git operation, artifact read and agent run is bound to an account and project. A collaborator's role limits what they may approve or publish. Tests T-ACCT-01 and T-ACCT-02 cover isolation, sign-in and project roles.

Browser sessions use secure, HTTP-only cookies or equivalent tokens, cross-site request forgery protection, a restrictive content security policy and strict origin checks. Rendered mod content, localization text and model output are escaped before display. Uploaded archives are checked for path traversal, symlinks, size bombs and unexpected file types before extraction into a workspace.

## File and process access

Resolve roots through host-owned handles scoped to one tenant's workspace. Reject traversal, symlink escapes, cross-tenant paths and source precondition mismatches. Preserve existing files outside owned paths. A Git worktree provides change isolation, not an operating-system sandbox.

The first content tools are typed operations and allowlisted validation commands. A generic shell is unnecessary for the first end-user task. If later workflows require it, define executable, argument, working-directory, environment, network and timeout constraints with enforced containment. Skill prose cannot expand those constraints.

Model-directed tools run on shared infrastructure. A container shares the host kernel and is not proof of isolation by itself. Per-task containers, a stronger sandbox runtime or a separate virtual machine boundary must be selected through EXP-07 evidence before public launch. Until then only trusted bundled adapters execute.

Process cancellation must cover children and grandchildren inside the selected boundary. Record accepted cancellation separately from confirmed termination. A worker that ignores a soft termination signal is stopped through a tested fallback. Late results cannot pass the current fencing token.

## Credentials, privacy and models

Store secrets in the server secret store, encrypted at rest. Pass only a scoped secret reference to the adapter for one request. A user's model key is used only for that user's requests. Redact before log persistence and before evidence export. Never put API keys, GitHub tokens, session cookies or private paths in mod Git history.

GitHub access uses a GitHub App with short-lived installation tokens for the repositories the user selects. Long-lived personal tokens are not the default. Push, pull request creation and repository creation are separately authorized and reconciled operations. T-GH-01 and T-GH-02 cover import preservation and publication reconciliation.

Custom model endpoint URLs are an outbound request risk. The server resolves the destination and blocks private, loopback, link-local and metadata-service addresses by default, including after redirects and DNS changes. A self-hosting operator may allow internal destinations explicitly. A user's localhost is not reachable and is not promised.

A provider profile identifies data destination, endpoint, authentication, model identity, terms or version reference and capability probe. A consumer chat subscription is not assumed to grant API entitlement. Automatic fallback only uses destinations already authorized for that task and data class.

Usage reporting distinguishes provider-reported tokens, locally estimated tokens, known prices, billed cost and unknown values. Unknown is not zero. Budget limits stop new dispatch per user and per task, whether the user's own key or operator credits pay. A lost acknowledgement can leave billing uncertain and is reported without automatic duplicate generation.

Privacy policy, data retention, account deletion and project export are required before public launch. Account deletion disables the account at once and permanently removes its workspaces, secrets and artifacts after a 30-day grace period unless the user restores it. The user's GitHub repositories are never touched, and projects that also have other owners are kept for them. GitHub webhooks arrive at a public endpoint, so every delivery signature is verified before processing and deliveries are de-duplicated.

## Supply chain and update policy

Pin the setup manifest and resolved component closure. Verify downloaded hashes and expected signatures where available. Avoid mutable latest tags for base images and tools. A new manifest can change tool behavior even if the server is unchanged.

Maintain an inventory of direct and transitive licenses, container base images, native binaries, model licenses and bundled assets. Record the exact tested runtime and tool revisions. Update in a disposable environment first, run source-preservation and recovery regressions, then publish a migration receipt. Schema migrations for PostgreSQL are reversible or backed by a tested restore point.

## Licensing, game and asset rights

The repository is open source under AGPL-3.0 with section 7 terms requiring preserved author attributions and marking of modified versions under [ADR-017](maintainer-decisions-2026-10-24.md#adr-017-open-source-license). Operating a modified version as a network service obliges the operator to offer its source. The service must link its own corresponding source to users. Reused Apache-2.0 and MIT components keep their notices. This is not legal advice.

The server holds a full game reference copy under [ADR-015](maintainer-decisions-2026-10-24.md#adr-015-server-held-game-reference). It is mounted read-only, never baked into images, never exported in ZIPs, pushes or pull requests, and never served as raw files. Only derived diagnostics, previews of the user's own mod and excerpts needed for a check leave the server. T-GAME-01 verifies this.

The [Paradox agreement](https://store.steampowered.com/eula/3493540_eula_0) describes noncommercial service and UGC use, free UGC and limits on third-party rights. The maintainer deferred the rights review for a hosted service with free use and optional paid credits. It is a release gate. The review is completed and recorded before the service is exposed on a public domain.

A repository's permissive code license does not automatically cover game references, wiki exports, music or third-party art. Asset provenance records original source, rights basis, transformations and intended distribution. Restricted editor code remains comparison-only unless permission changes.

## Release gates

The public release needs reproducible image provenance, a license inventory, tenant isolation results, containment evidence for agent tools, backup and restore results, secret-redaction checks, outbound endpoint blocking checks, the declared model matrix and the recorded game-rights review. Actual game claims require user-operated observations on the named build and DLC profile.

Known critical source-loss, cross-tenant access or unauthorized-write failures block the affected capability and the public launch. An unavailable optional renderer or model does not block unrelated read-only use. The UI makes the limitation explicit. No safe label is earned solely by running a static package check, adopting containers or using a mature upstream framework.
