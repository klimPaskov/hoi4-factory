# S03 configuration review

Recorded 2026-10-25 on HOI4 Factory `main` at the end of S03, on the macOS machine. No secret values appear here.

## Runtime and build configuration

| File | Change in S03 |
|---|---|
| `docker/server.Dockerfile` | New `agent-tools` stage: hoi4-agent-tools fetched at commit `731e64f5b5c964bd1b33adad9c085bd3168b1940` (the build fails unless `git rev-parse HEAD` equals the pin and `package.json` says 4.4.0), built with `npm ci`, pruned to production dependencies. The runtime image gets it at `/opt/hoi4-agent-tools` and the stage's Node 22 binary at `/opt/node22/bin/node`, with `FACTORY_AGENT_TOOLS_NODE` and `FACTORY_AGENT_TOOLS_ENTRY` set. Setup keeps Debian's `/usr/bin/node`. npm and npx are not copied. The image scan found no coding CLI, npm, npx or credential file |
| `compose.yaml` | Read-only mount of `./.work/secrets/factory-secret.key` (override with `FACTORY_SECRET_KEY_SOURCE`) at `/run/secrets/factory-secret.key`. New server variables `FACTORY_SECRET_KEY_FILE`, `FACTORY_OUTBOUND_ALLOW_CIDRS` (empty by default) and `FACTORY_ALLOWANCE_MICROS` (1000000). The server keeps its read-only root filesystem, dropped capabilities and `no-new-privileges` |
| `compose.selfhosted.yaml` | New, development only: an Ollama 0.40.0 container pinned by digest, no published port, no model, used for the self-hosted profile check |
| `scripts/compose.sh` | Creates `.work/secrets/factory-secret.key` (32 random bytes, base64, mode 600) on first use when no override is set |
| `Cargo.toml` | Workspace members `crates/tasks`, `crates/broker`, `crates/hoi4-tools`. New dependencies: `jsonschema` 0.60.1 (broker, hoi4-tools), `ipnet`, `url` and `libc` (agent), `futures-util` (server) |
| `migrations/` | 0600 tasks ledger (tasks, attempts, events, bundles, intents, receipts, incarnations, leases, recovery function), 0700 grants, 0701 user API keys, 0702 model usage. Reversible pairs, RLS enabled and forced, no DELETE except a user's own API key |
| `.gitattributes` | Recorded model streams under `crates/agent/tests/fixtures/` and `crates/server/tests/fixtures/recorded/agent-*` are kept byte-exact |

## Environment variables added

Server: `FACTORY_SECRET_KEY_FILE`, `FACTORY_OUTBOUND_ALLOW_CIDRS`, `FACTORY_ALLOWANCE_MICROS`, `FACTORY_OPENAI_API_KEY`, `FACTORY_OPENAI_BASE_URL`, `FACTORY_AGENT_TOOLS_NODE`, `FACTORY_AGENT_TOOLS_ENTRY`, `FACTORY_AGENT_MAX_RUNNING`. The operator allowance uses the existing `FACTORY_ANTHROPIC_API_KEY` and `FACTORY_ANTHROPIC_BASE_URL`. Worker test faults: `FACTORY_AGENT_WORKER_TEST_FAULT`, read only by debug builds. Tests: `FACTORY_AGENT_TOOLS_NODE`, `FACTORY_AGENT_TOOLS_ENTRY` and `FACTORY_AGENT_TOOLS_FIXTURE` enable the real Agent Tools tests.

## Checks from the packet's configuration checkpoint

| Check | Result |
|---|---|
| Secrets and user API keys absent from logs, evidence and tracked config | Pass. User keys are sealed with AES-256-GCM bound to owner, key ID and provider. A test shows the stored bytes do not contain the key, and no route returns it. The worker receives the provider key only inside a frame over a pipe, never in argv or the environment, and the frame's Debug output redacts it. Evidence JSON files were searched for `sk-` (none) |
| Provider destinations match grants | Pass. Every grant names the profile's destination (`anthropic:https://api.anthropic.com` in the live run). The worker connects only to the profile's base URL |
| Outbound destination policy active | Pass. Default policy refuses private, loopback, link-local, metadata, CGNAT and unique-local addresses (also when embedded in IPv6 forms), checks every address a name resolves to at connect time, follows no redirects and ignores system proxies. The self-hosted check shows a refusal by default and an allowed range only by explicit operator setting |
| Absent MCP servers remain disabled | Pass. `.mcp.json` and `.claude/settings.json` unchanged. The product's own MCP client starts only the configured HOI4 Agent Tools read-only profile, with an environment allowlist |
| Runtime config separate from Claude and Codex development config | Pass. No product setting lives in `.claude`, `.codex` or `.agents`. Claude Code is not in the image |

## External services

| Service | State |
|---|---|
| Anthropic | `claude-haiku-5-5` used for recordings, the agent crate's live tests and one live run in Compose. Total S03 spend about 0.004 USD (see the handoff) |
| OpenAI | Not called. No key on this machine; the adapter is tested against recorded and refused responses |
| hoi4-agent-tools | Pinned at `731e64f` in the image and in local tests. No upstream change in S03 |
| GitHub | No S03 change. App and installation as left by the maintainer after S02 |

## Development client configuration

CLAUDE.md gained the "Agent runtime rules (learned in S03)" section and a delegation note, and the `delegate-parallel-work` skill gained two notes. Copies regenerated with `scripts/sync_skills.py --write`; `--check` reports no drift. Seven skills, invocation modes unchanged. A user-scope REA MCP server was connected in the session and not used.

## Rollback

Revert the S03 commits on `main`. The 0600 and 0700 migrations have down scripts (run them, or drop the development volume with `scripts/compose.sh -p hoi4-factory down -v`). Delete `.work/secrets/factory-secret.key` only together with the database, or saved user keys become unreadable.
