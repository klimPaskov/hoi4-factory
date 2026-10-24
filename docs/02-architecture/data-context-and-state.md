# Data, context and state

Status: **Proposed, revised for the hosted service** by [maintainer decisions of 2026-10-24](maintainer-decisions-2026-10-24.md). This is the common authority model for C-01 through C-10. Read [contracts](contracts-and-interfaces.md), [context](../03-subsystem-specs/004-context-memory-and-runtime-learning.md) and [recovery](../03-subsystem-specs/003-orchestration-and-recovery.md).

## Storage decisions

| Store | Owns | Does not own | Recovery |
|---|---|---|---|
| Native mod repository on the Git volume | Script, localization, source assets and accepted project documents | Credentials or transient model state | Git history, owned worktree snapshots and an optional GitHub remote |
| PostgreSQL database | Accounts, memberships, roles, tasks, attempts, grants, lease epochs, receipts, budgets, evidence index and candidate reviews | Game files or the only copy of mod source | Consistent backup, migrations and integrity checks |
| Content-addressed object store | Rendered previews, logs, uploaded archives and exported evidence bytes | Approval authority | Verify hash, regenerate derived items or restore backup |
| Derived indexes | Symbols, dependency graph, full-text retrieval and optional vectors, scoped per project | Canon or source truth | Rebuild from identified inputs |
| Server secret store | Bring-your-own model keys, GitHub App tokens and operator credentials, encrypted at rest | Project behavior | Reauthenticate through the owning provider or user |
| Read-only game reference | Full game copy, latest build with all DLC | Any tenant data | Reinstall the named build, never exported |

The default proposed metadata prefix is `.hoi4factory`, subject to collision detection at onboarding. Its Git-tracked project descriptor records schema version and nonsecret project identity. It does not contain server paths, account identifiers or tokens. Canon locations are configurable so existing project documents remain authoritative.

PostgreSQL provides transactional database operations. Its transactions do not make mod files on the Git volume, Git refs, object store writes, GitHub calls and provider effects one transaction. The SQLite [atomic commit documentation](https://sqlite.org/atomiccommit.html) consulted for the earlier local design makes the same boundary point. The product uses explicit journals and reconciliation for those boundaries.

## Core entities

An **Account** signs in through GitHub or email. A **Project** belongs to one owning account and has **Memberships** with roles for invited collaborators. A **Workspace** belongs to one project and has stable ID, server root handles, repository identity, an optional repository link and profile versions. A **SourceSnapshot** records commit where available, dirty file hashes, ordered content roots and game/DLC/tool closure. A **Task** holds approved objective, requirement links, dependency edges and ownership. A **Run** is one orchestration instance for a task, while an **Attempt** is a bounded model or tool execution.

An **ApprovalGrant** contains allowed operation classes, roots, budgets, expiry and revocation epoch. A **Proposal** contains an expected base, operations and intended postconditions. A **ToolReceipt** records intent and outcome. An **EvidenceRecord** identifies exact artifacts and checks. A **ReviewDecision** binds a reviewer and reason to a specific proposal hash.

A **KnowledgeCandidate** is separate from an **ApprovedCanonVersion**. Runtime lessons and engineering lessons have separate domains and destinations. Search projections can reference either, but an unapproved candidate cannot acquire authority by appearing in a retrieved context.

## Referential and ordering rules

IDs are globally unique within the service and carry no embedded privileges. Every task, proposal and receipt belongs to one workspace, and every workspace to one project and owning tenant. Queries filter by tenant at the data access layer, not only in request handlers. A missing tenant filter is a defect tested by T-ACCT-01. Cross-workspace operations require explicit separate authority and cannot reuse a grant by changing a path string.

Run events are append-only with a host-assigned sequence. The state transition and its event are committed in one database transaction where they share the same store. With several server replicas, the database assigns the sequence and lease epochs, not process memory. Browser streams can reconnect from a sequence cursor and detect gaps. Timestamps help users read the history, but sequence and expected version govern concurrency.

Artifacts are written to staging, hashed, durably published, then referenced by a database transaction. If the process dies between artifact publication and reference creation, recovery can remove or reattach an unreferenced object according to the pending intent. If a referenced artifact is missing, evidence becomes unavailable and cannot remain a passing proof.

## File apply journal

The proposed state sequence is `prepared -> authorized -> applying -> files_applied -> validated -> committed`, with `reconcile_required`, `rolled_back` and `failed` as explicit alternatives. These are product transaction states. Setup's own states retain their separate semantics through C-09.

Before apply, persist exact final arguments, source preconditions, staged output hashes, backup references and grant version. Recheck roots and hashes immediately before each effect. After apply, read the actual bytes and record observed hashes. A receipt cannot report success based only on the requested content.

A multi-file apply uses an ownership lock and journal. Partial application is visible to recovery and hidden from the UI's accepted snapshot pointer. Validation checks the new closure. A Git commit is a separate guarded operation that records expected HEAD and owned index content. If the ref advances but receipt finalization fails, recovery finds the exact commit and reconciles it instead of making a duplicate commit.

Rollback has its own preconditions. If a human has edited a file after the failed operation, preserve both versions and require a reviewed resolution. A backup is not permission to overwrite current work.

## Retry and effect classification

Pure reads can retry within bounded limits if the source identity and scope remain valid. Deterministic derived rendering can retry against the same input snapshot. Writes need an idempotency key and observed pre/postconditions. External jobs need a remote handle or lookup mechanism before safe retry can be claimed.

An unknown outcome is durable. Examples include a provider request that timed out after billing, an upload whose acknowledgement was lost, a GitHub push or pull request creation whose response was lost, or a Git ref update followed by process death. The next action is reconciliation, not an automatic duplicate. Missing logs, absent process listings and an expired lease are not interchangeable with proof that the effect did not happen.

## Context freshness

A context bundle includes the approved versions and source locators needed for a task. Its manifest records omissions and token budgeting. Source, tool, game or policy changes invalidate affected bundles and evidence. A changed tool schema invalidates pending calls even when the tool name stays identical.

Retain visible conversation needed for intent and factual handoffs. Cross-model conversion does not attempt to carry private reasoning. It reconstructs task state from accepted decisions and receipts. Failure to fit required constraints into a smaller model returns a specific narrowing decision.

## Backup, export and retention

A portable project export contains native mod files, accepted project documents and a redacted evidence manifest. Users obtain it as a ZIP download, a clone or a push to their GitHub repository. It never contains game reference files. A full service backup is an operator task covering PostgreSQL, the object store, the Git volume, pending journals and a consistent snapshot manifest. Secrets are backed up only in encrypted form, or excluded and restored by reauthentication.

Back up while writes are paused or through a tested database snapshot protocol with matching artifact references. Copying a live database file without its required journal state is not the planned backup method. EXP-08 tests restore after incomplete operations, missing artifacts and schema changes.

Retention policy separates optional raw provider payloads, redacted event history and accepted evidence. Users can inspect export contents before sharing. Account deletion removes the account's own projects, secrets and artifacts on a recorded schedule while preserving projects owned by others. Deletion may remove sensitive payloads while keeping a tombstone that a result once existed and is now unavailable. It must not leave an old pass badge that depends on deleted evidence.

## Migration and integrity checks

Every schema migration names the source and target version, backup requirement and downgrade policy. Unknown newer schemas open read-only or fail safely. A source manifest upgrade does not automatically upgrade tool binaries or rewrite project instructions.

On open, verify workspace identity, lease ownership, unresolved journals, referenced artifact existence and source snapshot divergence. Run deeper integrity checks only when needed by a detected inconsistency or a release gate. The product should remain useful for unaffected read-only work while a specific operation is blocked.

