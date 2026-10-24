# S01 configuration review

Recorded 2026-10-24 against base revision `290af7c48ce160d4f7461b90aedc92528295330b` plus the uncommitted S01 changes. No secret values, tokens or private paths are recorded here.

## Client and runtime versions

| Item | Version or identity | Evidence |
|---|---|---|
| Claude Code | 2.1.304, desktop app 3.1623.0, Agent SDK 0.3.304, model `claude-opus-5-5` | `$CLAUDE_CODE_EXECPATH --version` and environment variable names |
| Codex CLI | 0.142.3 | `codex --version`. Live Codex loading trial not performed |
| Rust | 1.97.1 pinned by `rust-toolchain.toml`. rustup installed that channel locally on first build | `cargo --version` |
| Docker | 28.5.2 client and server, Linux engine on WSL2, Compose v2.40.3 | `docker version` |
| Python for package checks | 3.13.5 with `PYTHONUTF8=1`. The default `python` on PATH is 3.9.12 and lacks `tomllib` | validator runs |

## Instruction and skill loading

CLAUDE.md loaded and imported AGENTS.md once, observed in this session's instruction context. The four project skills exist in `.claude/skills`, `.codex/skills` and `.agents/skills`, and the generated projections match their canonical bodies (`sync_skills.py --check` pass). `/start-implementation-stage` was invoked by the maintainer and loaded.

ADR-023 changed invocation modes during S01. `start-implementation-stage` and `audit-hoi4-integrations` keep `disable-model-invocation: true` and `allow_implicit_invocation: false`. `validate-stage` and `capture-engineering-lessons` no longer carry the flag, and their Codex sidecars allow implicit invocation. The change is in `scripts/sync_skills.py` (one `MANUAL_ONLY` set), the descriptions in `docs/07-developer-workflow/skills.json`, and the matching check in `scripts/validate_package.py`. After regeneration the client listed both skills as available to the model.

## MCP connections

Both project MCP servers stay disabled: `.codex/config.toml` has `enabled = false` for `hoi4_agent_tools` and `comfy_cloud_portraits`, and `.claude/settings.json` lists both in `disabledMcpjsonServers`. No project MCP connection was enabled in S01.

Finding: a `hoi4_agent_tools` server and a `gptimage` server were connected in this session from user-scope Claude configuration, outside the project. The project's disable list does not cover user-scope servers. S01 probes did not use them. EXP-02 exercised the pinned Agent Tools source inside its own Linux container instead.

## Files added or changed

| File | Purpose | sha256 |
|---|---|---|
| `rust-toolchain.toml` | Pin Rust 1.97.1 with rustfmt and clippy | `559f488c...0d7a25` |
| `Cargo.toml`, `Cargo.lock` | Workspace with `factory-server`, `factory-agent`, `factory-setup-adapter` and the runtime probe crate | lock `c6b83d94...346` |
| `compose.yaml` | Development server and PostgreSQL 17.6, database not published outside the Compose network | `74518c41...7a25` |
| `docker/server.Dockerfile` | Server image on digest-pinned `rust:1.97.1-slim-bookworm` and `debian:bookworm-slim` | `312a9896...795` (rebuilt during validation) |
| `docker/agent-probe.Dockerfile` | EXP-04 probe image on the same bases plus `ca-certificates` | `d39c8aae...030` |
| `.dockerignore` | Keeps target, work clones, Git, evidence, docs and env files out of build contexts | `d09ebd9b...0e3` |
| `.gitignore` | Ignores `.work/`, `target/`, `node_modules/` and env files | `c2f43037...321d` |
| `.gitattributes` | Keeps text files LF, leaves the CSV byte-identical | `a2aef040...b5dd7b3` |
| `probes/runtime/compose.db-port.yaml` | Probe-only override publishing PostgreSQL on 127.0.0.1:55432. The Compose `db` was recreated without it after EXP-03 | not hashed |

Base image digests: `rust@sha256:2775a09d208ff0d7c1f50490c45b62db929e87ba1dcbc3f2132ac71a704bcdd3`, `debian@sha256:7ff6ba31527b9e336162fa061746c366c4b2032f4d2cff2078bea43f78efe3f8`, `postgres@sha256:f3bd19c606e442c3d7bdfa8002e03fe260a1023351e0ea4598032022b68dd6e3`. The `ca-certificates` apt package is not pinned by digest.

## Credentials and external services

| Item | Location | State |
|---|---|---|
| Anthropic API key | Windows user environment `ANTHROPIC_API_KEY`, read per command and passed to Docker by name | Working with `claude-haiku-5-5`. The maintainer gave standing approval to use it |
| OpenAI API keys | Session environment and Windows user environment `OPENAI_API_KEY` | Both refused with 429 `credit_balance_exhausted`. OpenAI live probes unavailable |
| GitHub App `HOI4 Factory Dev` (ID 5453675) | Credentials in git-ignored `.work/github-app/` | Installed on `klimPaskov/hoi4-factory-test-mod` only, selection `selected` |
| First probe App (ID 5453381) | Local key removed | Uninstalled everywhere through its own credentials. The maintainer deletes the registration |
| `gh` CLI | User keyring login, scopes `delete_repo gist read:org repo workflow` | Used for repository creation and reads only |

## Unsupported or deferred

Codex live loading, gVisor (`runsc` is not installed on Docker Desktop), an OS sandbox outside-root denial test, OpenAI live inference, and the self-hosted endpoint (skipped by the maintainer).

## Rollback

Revert the S01 commit to restore the planning-only tree. Remove the probe containers and images with `docker compose down -v` and `docker image rm hoi4-factory-agent-probe:s01 hoi4f-exp02-agent-tools:25f5641`. Delete `.work/`. Uninstall or delete the GitHub App and the throwaway repositories from GitHub settings, which only the maintainer can do. To make all four skills maintainer-only again, add every skill name to `MANUAL_ONLY` in `sync_skills.py` and to the matching set in `validate_package.py`, then regenerate.
