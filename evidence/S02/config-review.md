# S02 configuration review

Recorded 2026-10-25 on HOI4 Factory `main` after the S02 merges. No secret values appear here.

## Runtime and build configuration

| File | Change in S02 |
|---|---|
| `compose.yaml` | Server, PostgreSQL 17.6 and Mailpit (digest-pinned). Read-only bind of `HOI4_GAME_DIR` to `/game`. Read-only GitHub App key mount from `FACTORY_GITHUB_APP_KEY_FILE`. `FACTORY_ANTHROPIC_*` passed through from the host. Server root filesystem read-only with a `/tmp` tmpfs, all capabilities dropped, `no-new-privileges` |
| `docker/server.Dockerfile` | Build stage for the Rust server, a pinned Node stage that builds the browser client, and a Debian runtime with `git`, `ca-certificates` and Debian `nodejs` 18.20.4 (Setup accepts only that package's `/usr/bin/node`). Runs as uid 10001. No coding CLI, game file or credential in the image (checked by listing the image filesystem) |
| `docker/db-init/01-roles.sql` | Creates the non-superuser login role used by the server, a member of `factory_app`, so row-level security applies in development too |
| `docker/test-runner.Dockerfile` | New. The Linux test image used by `scripts/linux-tests.sh` (Rust 1.97.1, git, Debian Node) |
| `.env.example` | New. Lists the local settings, tracked, with no values |
| `.gitignore` | Ignores `.work/`, `target/`, `node_modules/` and `.env` files, keeps `.env.example` |
| `Cargo.toml` | Workspace members accounts, workspace, game-ref, intake, setup-adapter, server, agent and the runtime probe. HOI4-Mod-Setup pinned to merged main `c28198bb9b4d3fd90cea294cf136cad357ed5bbc`, which contains #105 to #108, with `default-features = false` (no OS keyring) |
| `migrations/` | 0100-0101 accounts, 0300-0303 intake, 0400-0402 Setup adapter. Reversible pairs, row-level security forced on tenant tables |

## Environment variables

Server: `FACTORY_ENV`, `DATABASE_URL`, `DATABASE_MIGRATION_URL`, `FACTORY_LISTEN`, `FACTORY_DATA_ROOT`, `FACTORY_GAME_REF`, `FACTORY_WEB_DIST`, `FACTORY_NODE_EXECUTABLE`, `FACTORY_MAX_BODY_BYTES`, `FACTORY_UPLOAD_MAX_BYTES`, `FACTORY_PUBLIC_URL`, `FACTORY_ALLOWED_ORIGINS`, `FACTORY_SMTP_URL`, `FACTORY_MAIL_FROM`, `FACTORY_SESSION_TTL_DAYS`, `FACTORY_SESSION_IDLE_DAYS`, the `FACTORY_DB_*` pool settings, `FACTORY_GITHUB_CLIENT_ID`, `FACTORY_GITHUB_CLIENT_SECRET`, `FACTORY_GITHUB_APP_ID`, `FACTORY_GITHUB_APP_SLUG`, `FACTORY_GITHUB_APP_PRIVATE_KEY_FILE`, `FACTORY_ANTHROPIC_API_KEY`, `FACTORY_ANTHROPIC_MODEL`, `FACTORY_ANTHROPIC_BASE_URL`. Tests: `FACTORY_TEST_ADMIN_URL`, `ANTHROPIC_API_KEY` for ignored live tests.

## External services

| Service | State |
|---|---|
| GitHub App HOI4 Factory Dev (ID 5453675) | Permissions: contents, issues and pull requests write, checks and statuses read, metadata. The email permission and the installation scope still need the maintainer (see [maintainer questions](../maintainer-questions.md)) |
| HOI4-Mod-Setup | PRs #105, #106, #107 and #108 merged by the agent with the maintainer's authorization, using the admin override only for the code-owner review rule after all required checks passed |
| hoi4-agent-tools | PR #42 squash-merged on 2026-10-25 as `731e64f5b5c964bd1b33adad9c085bd3168b1940` after all checks passed, with the maintainer's authorization |
| Anthropic | `claude-haiku-5-5` used for live Setup analysis tests |

## Development client configuration

CLAUDE.md is canonical and AGENTS.md is its generated identical copy (ADR-024). Skills: six, with `start-implementation-stage` and `audit-hoi4-integrations` maintainer-invoked and the other four agent-invocable. Project MCP servers stay disabled. A user-scope `hoi4_agent_tools` server was connected in sessions and not used for evidence.

## Rollback

Revert the S02 merges on `main`. Run `docker compose -p hoi4-factory down -v` to drop the development database. The upstream Setup merges are separate and are reverted in HOI4-Mod-Setup if ever needed.
