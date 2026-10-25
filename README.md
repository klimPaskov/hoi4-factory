# HOI4 Factory

A free, open-source, hosted HOI4 modding workspace with optional visual editing, bounded agent work, GitHub integration and collaboration over real mod source files.

**Status, 2026-10-25:** stages S01 (bootstrap and architecture probes) and S02 (accounts, server workspaces, Mod Setup and GitHub import) are implemented. A user can sign up, create or upload a mod or import it from GitHub, and run HOI4 Mod Setup on it in a per-account server workspace. The next stage is S03, the native agent loop. See [package status](docs/PACKAGE_STATUS.md) and the [S02 handoff](evidence/S02/handoff.md). The planning baseline and the [maintainer decisions](docs/02-architecture/maintainer-decisions-2026-10-24.md) still define the full product.

## Planned product

| Part | Choice |
|---|---|
| Delivery | Public web application, used in a browser |
| Server and agent | Rust web server with a native Rust agent loop, no Node worker |
| Client | React with TypeScript |
| Deployment | Docker images and Compose, portable to any Linux host |
| Source of truth | Native mod files and Git history in server workspaces, downloadable or pushed to GitHub |
| Accounts | Sign in with GitHub or verified email, invite collaborators, private or public projects |
| Collaboration | GitHub issues, pull requests, reviews and merges fully inside the app, no need to open GitHub |
| Models | Claude and OpenAI, a free monthly allowance, your own API key, or any reachable OpenAI-compatible endpoint |
| Game data | A server-side read-only reference copy, latest build with all DLC, never redistributed |
| License | AGPL-3.0 with attribution terms, see [LICENSE](LICENSE) and [NOTICE](NOTICE) |

## Start here

1. Follow the [developer setup](docs/07-developer-workflow/DEVELOPER-SETUP.md) to install the tools, create the local configuration and start the stack with `docker compose up -d --build --wait`. The app runs at http://127.0.0.1:8080.
2. Read the [documentation index](docs/README.md), [master goal](docs/MASTER_GOAL.md) and [current status](docs/PACKAGE_STATUS.md).
3. Review the [architecture options](docs/02-architecture/options-and-decisions.md) and [implementation roadmap](docs/04-implementation-stages/ROADMAP.md).
4. To continue development, launch Claude Code from the repository root, run `/start-implementation-stage S03` and use the [S03 goal prompt](docs/06-agent-handoffs/GOAL-S03.md).

[CLAUDE.md](CLAUDE.md) holds the project instructions. [AGENTS.md](AGENTS.md) is its generated identical copy for clients that read that name. Project settings and skills are already in their working locations.

## Contributing and license

HOI4 Factory is licensed under the GNU Affero General Public License version 3 with two additional terms under its section 7. Modified versions must keep the author attributions in [NOTICE](NOTICE), and must be marked as changed and not presented as the original. If you run a modified version as a network service, you must offer its source to its users. Hearts of Iron IV and its DLC belong to Paradox Interactive. No game files are in this repository.

## Project organization

| Location | Purpose |
|---|---|
| `docs/` | All research, goals, requirements, architecture, specifications, stage packets, verification plans, handoffs and engineering documentation |
| `crates/` | Rust workspace: `accounts`, `workspace`, `game-ref`, `intake`, `setup-adapter`, `server` and `agent` |
| `web/` | React and TypeScript browser client, built with Vite |
| `migrations/` | PostgreSQL migrations, numbered by owning crate |
| `docker/`, `compose.yaml` | Server image, Linux test image, database setup and the development stack |
| `probes/` | S01 architecture probes, kept byte-exact because evidence records their hashes |
| `evidence/` | Stage evidence, handoffs and the maintainer question list |
| `scripts/` | Package validator, skill synchronization, Linux test runner and the live onboarding flow |
| `CLAUDE.md` | Canonical project instructions |
| `AGENTS.md` | Generated identical copy of CLAUDE.md |
| `.claude/` | Claude project settings and six skills |
| `.codex/` | Codex project settings and portable skill copies |
| `.agents/` | Codex discovery copies of the same six skills |
| `.mcp.json` | Claude MCP definitions, disabled until their setup is reviewed |

Stage evidence is written to project-root `evidence/SNN/`. Detailed navigation and the full file inventory are in [docs/README.md](docs/README.md) and [docs/MANIFEST.md](docs/MANIFEST.md).

## Check the planning and guidance files

Run commands from this project root with Python 3.11 or newer. On Windows, set `PYTHONUTF8=1` first, because the validator otherwise reads files in the locale encoding and fails on UTF-8 text.

```bash
python scripts/sync_skills.py --check
python scripts/validate_package.py --root . --working-tree
```

The working-tree check validates the maintained planning and guidance files without comparing them to the frozen distribution hashes. The frozen hashes in `docs/MANIFEST.json` describe the original v02 package, which is the first commit of this repository, so the default distribution mode no longer matches the revised files. Run the stage's own tests for product behavior. See the [quality review](docs/05-verification/PLANNING-PACKAGE-QUALITY-REVIEW.md) for exact validation scope.

When an approved workflow-body change is needed, update its canonical source in `docs/07-developer-workflow/skill-workflows/`, then run `python scripts/sync_skills.py --write` and the check again. The generator preserves correct links in all three client locations.
