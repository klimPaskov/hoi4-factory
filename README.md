# HOI4 Factory

A native, model-agnostic HOI4 modding workspace with optional visual editing, bounded agent work and collaboration over real mod source files.

This is the **v02 project-folder edition** of the completed research and planning baseline. Extract the ZIP and use **`hoi4-factory/` as your working project folder**. The application is ready to enter its reviewed implementation stages. Product implementation has not started.

## Start here

1. Open this `hoi4-factory/` folder in your editor and launch Claude Code from this root.
2. Read the [documentation index](docs/README.md), [master goal](docs/MASTER_GOAL.md) and [current status](docs/PACKAGE_STATUS.md).
3. Review the [architecture options](docs/02-architecture/options-and-decisions.md) and [implementation roadmap](docs/04-implementation-stages/ROADMAP.md).
4. For the first authorized implementation stage, use the [Claude Code S01 prompt](docs/06-agent-handoffs/CLAUDE-CODE-START-STAGE-01.md).

The shared [AGENTS.md](AGENTS.md), importing [CLAUDE.md](CLAUDE.md), project settings and skills are already in their working locations. When using this folder directly, no second starter-copy step is needed. If integrating it into an existing repository, merge its guidance with that repository's instructions.

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

Run commands from this project root with Python 3.11 or newer.

For the freshly extracted distribution:

```bash
python scripts/sync_skills.py --check
python scripts/validate_package.py --root .
```

Once development has added files or changed the baseline, use:

```bash
python scripts/validate_package.py --root . --working-tree
```

The working-tree check validates the maintained planning and guidance files without comparing them to the frozen distribution hashes or treating unrelated product files as package errors. Run the stage's own tests for product behavior. See the [quality review](docs/05-verification/PLANNING-PACKAGE-QUALITY-REVIEW.md) for exact validation scope.

When an approved workflow-body change is needed, update its canonical source in `docs/07-developer-workflow/skill-workflows/`, then run `python scripts/sync_skills.py --write` and the check again. The generator preserves correct links in all three client locations.
