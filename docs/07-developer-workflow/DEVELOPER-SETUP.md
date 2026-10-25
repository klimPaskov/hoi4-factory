# Developer setup on a new machine

This guide takes a fresh computer from nothing to a running HOI4 Factory stack, with every check green and the repository ready for the next stage. It was written at the end of S02 on 2026-10-25, updated for S03 the same day, and covers Windows 11 (Git Bash) and macOS (checked on Apple Silicon). Run every command below from a POSIX shell: Git Bash on Windows, the default zsh on macOS. Project rules live in [CLAUDE.md](../../CLAUDE.md), and the current stage state is in the latest `evidence/SNN/handoff.md`.

## 1. Install the prerequisites

| Tool | Version used | Notes |
|---|---|---|
| Git | 2.54 | On Windows, Git for Windows sets `core.autocrlf=true` system-wide. The repository's `.gitattributes` handles checkouts, and tools pass `-c core.autocrlf=false` |
| Rust via rustup | 1.97.1 | `rust-toolchain.toml` installs the pinned toolchain on first build. On Windows the MSVC build tools are required, on macOS the Xcode Command Line Tools (`xcode-select --install`) |
| Docker with Compose v2 | Docker 28.5, Compose 2.40 | Docker Desktop on Windows and macOS, Docker Engine on Linux. Give it at least 30 GB of disk. Every pinned image has amd64 and arm64 builds, so Apple Silicon runs them natively |
| Python | 3.11 or newer | Only for the package validator and helper scripts. Run them through `scripts/py.sh`, which sets `PYTHONUTF8=1` and finds `py -3.13` on Windows, a `python3.11`+ on macOS and Linux, or falls back to `uv run --python 3.13`. The default `python` is 3.9 on both machines, so install 3.13 (Windows: python.org installer with the `py` launcher, macOS: `brew install python@3.13` or `uv python install 3.13`) |
| GitHub CLI `gh` | 2.87 | Signed in to the maintainer's account, for upstream pull requests and GitHub checks |
| Claude Code | current | The development client. Launch it from the repository root |

Node is not needed on the host for the stack. The browser client builds inside a pinned Node image (`web/scripts/docker-npm.sh`), and the server image installs Debian's Node package itself (for Setup) plus Node 22 for HOI4 Agent Tools. Node 22 or 24 on the host is needed only to run the Agent Tools integration tests outside Docker (section 5).

Keep at least 30 GB free. Rust build caches grow quickly, and a full disk causes misleading test failures. CLAUDE.md explains how to prune them.

## 2. Clone the repository

```bash
git clone https://github.com/klimPaskov/hoi4-factory.git
cd hoi4-factory
git config core.autocrlf false
```

The clone contains everything tracked: code, evidence, the eight skills under `.claude`, `.codex` and `.agents`, CLAUDE.md and its generated copy AGENTS.md. Nothing under `.work/` is tracked. It holds local clones, credentials and scratch output.

## 3. Create the local configuration

Three things never go into Git. Create them on each machine. The quickest way is one command, which asks for each secret (paste it, or press Enter to keep a saved one), saves it in every place below, checks the Anthropic key with a free API call, then starts the stack and checks `/readyz`:

```bash
scripts/setup-local.sh
```

It never prints a secret, and running it again is safe. The rest of this section describes what it does.

| Item | Where it goes | How to get it |
|---|---|---|
| Hearts of Iron IV game folder | Found automatically by `scripts/compose.sh`, or `HOI4_GAME_DIR` in `.env` | A local Steam installation. The script looks in this machine's Steam libraries (Windows `C:/Program Files (x86)/Steam`, macOS `~/Library/Application Support/Steam`, Linux `~/.local/share/Steam`, plus any extra library in `libraryfolders.vdf`). Set `HOI4_GAME_DIR` only for an install outside Steam. A value that does not exist on the current machine is ignored, so one `.env` works on both. It is mounted read-only and only its metadata is read |
| Anthropic API key | `FACTORY_ANTHROPIC_API_KEY` in `.env`, and `ANTHROPIC_API_KEY` in the shell for live tests | The maintainer's Anthropic Console, using a key created inside a workspace. Without it, Setup analysis and live model tests are unavailable and everything else works |
| GitHub App private key | `.work/github-app/private-key.pem` | The app is **HOI4 Factory Dev** (App ID `5453675`, slug `hoi4-factory-dev`). At https://github.com/settings/apps/hoi4-factory-dev choose **Generate a private key**, save the downloaded file under that path, and revoke older keys you no longer use. Without it, GitHub import is reported as unavailable |
| Key encryption key (S03) | `.work/secrets/factory-secret.key` | Created automatically by `scripts/compose.sh` on first use: 32 random bytes in base64, used to encrypt the model API keys users save. Each machine has its own, so keys saved in one machine's database cannot be opened with another's. Keep it if you want saved keys to survive, and delete it only together with the database volume |
| GitHub App client secret | `FACTORY_GITHUB_CLIENT_SECRET` in `.env` | On the same App's settings, open the **Credentials** tab in the left sidebar (since October 2026 client secrets are no longer on the General page), choose **Generate a new client secret** and copy it at once, because GitHub shows it only once. If the tab shows a loading error, reload once. The App's **Callback URL**, on the General page under **Identifying and authorizing users**, must be `http://127.0.0.1:8080/api/v1/auth/github/callback` (use your port if you changed `FACTORY_HOST_PORT`). Without the secret, "Sign in with GitHub" is off and email sign-in works |

Copy `.env.example` to `.env` and fill it in. The same `.env` can be copied between Windows and macOS unchanged. If your copy of `.env.example` has no `FACTORY_GITHUB_CLIENT_SECRET=` line, add it. `.env` is git-ignored, and Claude Code is configured not to read it.

The S01 GitHub probe scripts additionally expect `.work/github-app/app.json` with the fields `id` and `pem`. Create it from the key file when you need those probes:

```bash
scripts/py.sh -c "import json; json.dump({'id': 5453675, 'pem': open('.work/github-app/private-key.pem').read()}, open('.work/github-app/app.json', 'w'))"
```

## 4. Start the stack

```bash
scripts/compose.sh up -d --build --wait
```

`scripts/compose.sh` resolves the game folder for this machine and passes every argument to `docker compose`. Plain `docker compose` also works when `HOI4_GAME_DIR` is set for this machine. This builds the server image (Rust server, browser client and Debian Node) and starts PostgreSQL, Mailpit and the server. If the first start reports `No such image` for Mailpit or PostgreSQL, run `scripts/compose.sh pull --ignore-buildable` once and start again (seen on a fresh macOS machine on 2026-10-25).

| Address | What |
|---|---|
| http://127.0.0.1:8080 | The app: sign up, projects, upload or GitHub import, the Setup wizard |
| http://127.0.0.1:8080/readyz | Database, migrations, storage and game reference status |
| http://127.0.0.1:8025 | Mailpit, where verification and reset emails arrive in development |

The database creates its roles only on a fresh volume. After changing `docker/db-init/`, run `scripts/compose.sh -p hoi4-factory down -v` (this project's own volume only) and start again.

## 5. Run the checks

```bash
scripts/py.sh scripts/validate_package.py --root . --working-tree
scripts/py.sh scripts/sync_skills.py --check
```

Rust tests need a PostgreSQL superuser URL in `FACTORY_TEST_ADMIN_URL`. Start a disposable one:

```bash
docker run -d --rm --name hf-test-pg -p 127.0.0.1:55433:5432 -e POSTGRES_HOST_AUTH_METHOD=trust postgres:17.6-bookworm@sha256:f3bd19c606e442c3d7bdfa8002e03fe260a1023351e0ea4598032022b68dd6e3
FACTORY_TEST_ADMIN_URL=postgres://postgres@127.0.0.1:55433/postgres cargo test --no-fail-fast --workspace --exclude factory-runtime-probe
docker stop hf-test-pg
```

The S02 Setup gate tests also run on Linux, because Setup's readiness rules differ by platform. `scripts/linux-tests.sh` builds its own test image and database on first use:

```bash
scripts/linux-tests.sh -p factory-server --test setup_gates
```

The browser client checks run in the pinned Node image:

```bash
web/scripts/docker-npm.sh install
web/scripts/docker-npm.sh run check
```

The S03 HOI4 Agent Tools tests run against a local build of the pinned Agent Tools when these variables are set, and print `SKIPPED` otherwise. Build it once into `.work/` (Node 22 or 24 on the host):

```bash
git clone https://github.com/klimPaskov/hoi4-agent-tools.git .work/hoi4-agent-tools
git -C .work/hoi4-agent-tools -c core.autocrlf=false checkout 731e64f5b5c964bd1b33adad9c085bd3168b1940
(cd .work/hoi4-agent-tools && npm ci && npm run build)
export FACTORY_AGENT_TOOLS_NODE="$(command -v node)" FACTORY_AGENT_TOOLS_ENTRY="$PWD/.work/hoi4-agent-tools/dist/bin/stdio.js" FACTORY_AGENT_TOOLS_FIXTURE="$PWD/.work/hoi4-agent-tools/fixtures/focus/workspace"
```

The agent tests (`cargo test -p factory-server --test agent`) start the real worker process and replay recorded real Anthropic streams from a local server, so they need no key. `scripts/live_agent_flow.py` runs one real agent task against a running stack. The self-hosted endpoint check uses `compose.selfhosted.yaml` (an Ollama container with no model, about 2.8 GB to pull).

Tests marked `#[ignore]` call the real Anthropic API. Run them with `ANTHROPIC_API_KEY` set and `cargo test -- --ignored`. `scripts/live_flow.py` drives the whole onboarding flow against a running stack, including one real analysis call.

## 6. Optional upstream clones

Work on HOI4-Mod-Setup or hoi4-agent-tools happens in local clones under `.work/`:

```bash
git clone https://github.com/klimPaskov/HOI4-Mod-Setup.git .work/HOI4-Mod-Setup
git clone https://github.com/klimPaskov/hoi4-agent-tools.git .work/hoi4-agent-tools
```

The factory pins HOI4-Mod-Setup by commit in the root `Cargo.toml`, so these clones are needed only for upstream pull requests.

## 7. Ready for the next stage

The machine is ready when all of these hold:

- [ ] `scripts/compose.sh up -d --build --wait` reports the server healthy, and `/readyz` shows `status: ok` with the game reference available and read-only, and `checks.agent` with `available`, `user_keys`, `agent_tools` and `operator_allowance` all true
- [ ] Signing up at http://127.0.0.1:8080, confirming the email from Mailpit and creating a project works
- [ ] The package validator and `sync_skills.py --check` pass
- [ ] The workspace Rust tests and the web checks pass
- [ ] Claude Code, launched from the repository root, lists the eight project skills
- [ ] You have read the latest stage handoff and its goal prompt in `docs/06-agent-handoffs/`
