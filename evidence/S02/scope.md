# S02 scope

Stage: S02, accounts, server workspaces, Mod Setup and GitHub import. Packet: [stage-02-onboarding-and-workspaces.md](../../docs/04-implementation-stages/stage-02-onboarding-and-workspaces.md) at revision `4634d93`.

## Authorization and entry condition

The maintainer invoked `/start-implementation-stage S02` on 2026-10-24 after reviewing the S01 result ("Yes, then continue" to push S01). S03 is not authorized.

The entry condition requires no unresolved preservation defect on the intended apply path. S01 left the Setup placement-temp defect open. By the maintainer's decision in the [S01 handoff](../S01/handoff.md), S02 resolves it first through a pull request to HOI4-Mod-Setup. Development uses the pull request head. Gates 2 and 4 count only against a merged Setup revision that contains the fix. Until the maintainer merges, those gates stay pending, not passed.

## Maintainer decisions for S02

| Topic | Decision |
|---|---|
| Setup fix and facade | Pull requests to HOI4-Mod-Setup. The facade is a library API with per-session state, an injected credential store, an explicit state root, a public root identity and an optional `keyring` feature |
| Agent Tools defects | Pull request to hoi4-agent-tools. Factory host guards land with the tool integration in S04 |
| Upstream pull requests | Standing permission to open them on those two repositories. Feature branches may be pushed there. Never push to a default branch, never merge |
| Dev game reference | The local Steam install, mounted read-only. Identity: HOI4 1.19.3.0 "Operation Postern" (c01a, 5632), Steam build 25205862, 12 DLC folders. Only metadata is read. No game file enters the repository, an image or an export |
| Email sign-in | Email and password with Argon2id and a verification link. Development mail goes to a local Mailpit container |
| Production mail | Decided at S08 |

## Requirements and gates

Primary: REQ-004, REQ-005, REQ-006, REQ-007, REQ-021, REQ-027, REQ-046 (accounts and isolation, sign-in and roles basics), REQ-047 (connect and import only, no push) and REQ-048. Gates S02-G1 to S02-G5 from the packet.

## Implementation design

| Area | Owner path | Notes |
|---|---|---|
| Accounts, sessions, projects, tenancy | `crates/accounts` | sqlx 0.8 on PostgreSQL. Argon2id passwords. GitHub sign-in through the HOI4 Factory Dev App's user authorization. Opaque session cookie `hf_session` (HttpOnly, SameSite=Lax, Secure outside development). Every query goes through a tenant-scoped repository type, and PostgreSQL row-level security keyed on `app.account_id` set per transaction |
| Workspace storage, snapshots, Git baseline | `crates/workspace` | Root `/data/workspaces/<account_id>/<project_id>/`, created 0750 with readable ancestors (S01-L22). Snapshot inventory of every file with mode and sha256, dirty and untracked state. Owned branches and worktrees through the `git` CLI with `core.autocrlf=false` forced (S01-L03) |
| Upload and GitHub intake | `crates/intake` | ZIP and folder upload with size, count and path limits, traversal and symlink rejection, original upload kept as received, Git initialised only after explicit consent. GitHub import through installation tokens restricted to the selected repository, host allowlist per project (S01-L04), C-10 RepositoryLink rows. No push in S02 |
| Setup binding | `crates/setup-adapter` | Binds the HOI4-Mod-Setup facade from the pull request branch, then the merged revision. C-09 operations, receipts in PostgreSQL |
| Game reference | `crates/game-ref` | Read-only mount at `/game`, manifest of build, DLC set and checksum file hash, served to tools only as derived data |
| HTTP API and glue | `crates/server` | axum routes under `/api/v1`, common envelope, error codes and retry classes from the contracts. Migrations under `migrations/` with ranges 0100 accounts, 0200 workspace, 0300 intake, 0400 setup, 0500 game-ref |
| Browser client | `web/` | React with TypeScript on Vite. Sign-in, project list, create, upload, GitHub import, scan review, dry run, apply and recovery status. Translatable strings from the start (ADR-021) |
| Dev services | `compose.yaml` | Adds Mailpit and the read-only game mount |

## Excluded

Autonomous model writes, the focus canvas, collaborator invitations, push or pull request creation from the factory, public deployment, payments, the memory system and S03.

## Stop conditions

A write outside owned paths, any push to a default branch, a GitHub write outside `hoi4-factory-test-mod` or the two upstream repositories' feature branches, reading game file contents beyond metadata, an unknown effect outcome until reconciled, or a gate claimed against an unmerged Setup revision.
