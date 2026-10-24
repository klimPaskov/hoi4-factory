# HOI4 Factory

A free, open-source, hosted HOI4 modding workspace with optional visual editing, bounded agent work, GitHub integration and collaboration over real mod source files.

This repository holds the research and planning baseline, revised on 2026-10-24 by the [maintainer decisions](docs/02-architecture/maintainer-decisions-2026-10-24.md). **Product implementation has not started** and is deferred until the maintainer authorizes S01.

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

1. Clone this repository, open it in your editor and launch Claude Code from its root.
2. Read the [documentation index](docs/README.md), [master goal](docs/MASTER_GOAL.md) and [current status](docs/PACKAGE_STATUS.md).
3. Review the [architecture options](docs/02-architecture/options-and-decisions.md) and [implementation roadmap](docs/04-implementation-stages/ROADMAP.md).
4. When S01 is authorized, use the [Claude Code S01 prompt](docs/06-agent-handoffs/CLAUDE-CODE-START-STAGE-01.md).

The shared [AGENTS.md](AGENTS.md), importing [CLAUDE.md](CLAUDE.md), project settings and skills are already in their working locations.

## Contributing and license

HOI4 Factory is licensed under the GNU Affero General Public License version 3 with two additional terms under its section 7. Modified versions must keep the author attributions in [NOTICE](NOTICE), and must be marked as changed and not presented as the original. If you run a modified version as a network service, you must offer its source to its users. Hearts of Iron IV and its DLC belong to Paradox Interactive. No game files are in this repository.

## Project organization

| Location | Purpose |
|---|---|
| `docs/` | All research, goals, requirements, architecture, specifications, stage packets, verification plans, handoffs and engineering documentation |
| `scripts/` | Planning validation and deterministic skill synchronization utilities |
| `AGENTS.md` | Shared project instructions |
| `CLAUDE.md` | Claude Code entry point with the shared instructions imported |
| `.claude/` | Claude project settings and four reusable skills |
| `.codex/` | Codex project settings and portable skill copies |
| `.agents/` | Codex discovery copies of the same four skills |
| `.mcp.json` | Claude MCP definitions, disabled until their setup is reviewed |

Future application source belongs alongside `docs/`, in the structure chosen by the implementation stage. Stage evidence is written to project-root `evidence/SNN/` when work actually occurs. Detailed navigation and the full file inventory are in [docs/README.md](docs/README.md) and [docs/MANIFEST.md](docs/MANIFEST.md).

## Check the planning and guidance files

Run commands from this project root with Python 3.11 or newer. On Windows, set `PYTHONUTF8=1` first, because the validator otherwise reads files in the locale encoding and fails on UTF-8 text.

```bash
python scripts/sync_skills.py --check
python scripts/validate_package.py --root . --working-tree
```

The working-tree check validates the maintained planning and guidance files without comparing them to the frozen distribution hashes. The frozen hashes in `docs/MANIFEST.json` describe the original v02 package, which is the first commit of this repository, so the default distribution mode no longer matches the revised files. Run the stage's own tests for product behavior. See the [quality review](docs/05-verification/PLANNING-PACKAGE-QUALITY-REVIEW.md) for exact validation scope.

When an approved workflow-body change is needed, update its canonical source in `docs/07-developer-workflow/skill-workflows/`, then run `python scripts/sync_skills.py --write` and the check again. The generator preserves correct links in all three client locations.
