# macOS checks across the HOI4 tools

This is a self-contained brief for a Claude Code agent on the maintainer's Mac (D-47). To start it, give the agent this prompt:

```text
Clone or update github.com/klimPaskov/hoi4-factory, read its CLAUDE.md, then carry out docs/06-agent-handoffs/GOAL-MACOS-CHECKS.md from start to finish. Work on your own and ask me only for sign-ins, credentials and in-game checks, in one batch.
```

## Goal

Run every check that only runs on macOS across the HOI4 tools, fix what can be fixed, and record the results. Development stays on the Windows PC (D-25), so these checks were recorded as macOS gaps instead of blockers. The next promotion covers the whole modder workflow (D-43) and will reach Mac modders, so this is the proof that a Mac modder can install and use it.

## Rules

- Read hoi4-factory's CLAUDE.md first, including its macOS section, then each repository's own CLAUDE.md or AGENTS.md before you touch that repository. Read the [decisions in force](../decisions/README.md) for operations, Setup, the kit, Agent Tools, hoi4-imagegen and Live Tools.
- Never start Hearts of Iron IV on the Mac. The standing game windows of D-33 cover only the Windows PC and its WSL copy. You may read the Mac's game folder, if one is installed, but never launch it. List every check that needs the running game for the maintainer.
- Spend nothing. The Anthropic and OpenAI API keys have no balance, and no paid key or paid step is used. Image checks use hoi4-imagegen's ChatGPT sign-in route with one test image only, never the general `gptimage` tool. Before that image, ask the maintainer to confirm no Windows session is generating images at the same time, because the sign-in limit is per account.
- Sign-ins and credentials belong to the maintainer. When a step needs a browser sign-in, ask the maintainer to complete it. Never type passwords or keys, and never print a token.
- Stop starting new work above about 80% of the weekly usage limit (D-39).
- Docker Desktop is shared with other projects. Never restart it, prune images, build cache or volumes, or stop containers you did not start. If it hangs, ask the maintainer.
- Put temporary files only in a scratch folder or a repository's ignored work area, and keep paths short. Before you finish, remove your clones, build output, test containers and any image you built for a check.
- Fix problems on feature branches with pull requests, and merge them when every required check is green (D-18, D-23). Sessions on the Windows PC work in the same repositories at the same time, so never push to a tool repository's default branch, update your branch onto the latest main before merging, and before any push to hoi4-factory `main` check `git log origin/main..main` lists only your commits. Keep line endings LF.
- File every failure you cannot fix as a GitHub issue in the repository it belongs to, with evidence, the reason and an acceptance check (the `track-issues` skill). Issues are public, so no secrets, private paths or user data.

## Before you start

1. Confirm `uname -s` prints `Darwin`. Record the macOS version, the chip, Xcode command line tools, Homebrew, Node, Python through `scripts/py.sh`, the Rust toolchain, Docker Desktop and free disk (`df -h`).
2. Clone or update these repositories from github.com/klimPaskov: hoi4-factory, hoi4-agent-tools, Agentic-HOI4-Modding, HOI4-Mod-Setup, HOI4-Live-Tools and hoi4-imagegen-mcp.
3. Note each repository's latest release tag, so the results name exact versions.

## Checks

### A. HOI4 Mod Setup, fresh install (the most important check)

1. Download the latest macOS arm64 Setup release from GitHub Releases and verify it as Setup's release guide describes.
2. Open it the way a modder would. Setup builds are not paid-signed (R-2), so record the "Open anyway" steps with screenshots and compare them with the guide linked from Setup's README. Fix the guide if it is wrong or missing steps.
3. Create a new test mod project in a scratch folder. Pick the kit and every MCP server Setup offers (HOI4 Agent Tools, hoi4-imagegen, the Meshy MCP and the Blender Lab MCP where offered), with the default image route. Enter no paid keys.
4. Confirm that the kit came from the current kit `main`, that installed versions match the kit's pins, that Setup's own checks pass, and that the generated Claude Code and Codex configurations start every MCP server. From the project folder, `claude mcp list` shows each server's state. Record each server's start result and its tool count.
5. Run Setup's update path once on the same project and record the result.
6. Run Setup's own offline acceptance on macOS if its testing docs describe a macOS run.

### B. HOI4 Agent Tools

1. In the Setup-made project, call a few read tools on the test mod: `hoi4.mod_index`, `hoi4.source_lookup`, `hoi4.script_validate` and documentation search through the docs pack with no mod open. Record cold and warm times.
2. Confirm the bundled engine binary for macOS arm64 matches its checksum manifest and that the engine client reports it available.
3. If the Mac has the game installed, point the tools at its folder read-only and repeat one lookup that needs vanilla files.

### C. hoi4-imagegen (layer L6 in its `docs/testing.md`)

1. Run `doctor` and record what it reports for the platform and the credential store.
2. Ask the maintainer to sign in to the ChatGPT route in the browser with `hoi4-imagegen login`. From 0.4.0 the tokens are a private file, not a Keychain item ([issue #2](https://github.com/klimPaskov/hoi4-imagegen-mcp/issues/2)). Without printing the tokens, confirm these points:
   - The file is `~/Library/Application Support/hoi4-imagegen/chatgpt-login/auth.json`, with mode 0600, in a folder with mode 0700.
   - `login` reports that it removed the Keychain item of an older sign-in, when a 0.3.x sign-in left one.
   - Start `hoi4-imagegen-mcp` twice, and call `hoi4.image_status` each time. It reports `signed_in: true`, and macOS shows no Keychain password dialog. Note the version tested.
   - After the next release, update the binaries and start the MCP server once more. It still shows no dialog and reports `signed_in: true`.
   - `hoi4-imagegen logout` removes the file and leaves no `hoi4-imagegen.chatgpt-login` item in Keychain Access.
   - With `HOI4_IMAGEGEN_CHATGPT_STORE=keychain` set for both programs, the tokens go to the Keychain instead. This is optional and expected to prompt.

   When every point passes, close issue #2 with the result.
3. Generate one small test image of a simple family, such as an idea icon, into the test mod, then run the audit tool on it. Look at the image yourself before calling it a pass.
4. Update the L6 row in `docs/testing.md` with the result, by pull request.

### D. The kit

1. In a kit checkout, run every step of the kit's Validate workflow locally, as hoi4-factory's CLAUDE.md requires before a push, and record any failure that only happens on macOS, such as BSD tool flags or paths.
2. Check that the Agent Tools bootstrap finds Node on macOS, including Homebrew's paths.

### E. HOI4 Live Tools, offline only

1. Run its offline test suite on macOS. Do not start the game.
2. List the live checks that would need the running game, for the maintainer.

### F. HOI4 Factory

Every script and command must work on both machines, but development has stayed on Windows since 2026-11-06, so check that it still does. Follow [developer setup](../07-developer-workflow/DEVELOPER-SETUP.md) only as far as these checks need, and make no live model calls.

1. `scripts/py.sh scripts/validate_package.py --root . --working-tree` and `scripts/py.sh scripts/sync_skills.py --check`.
2. `cargo test --no-fail-fast` for the workspace, with free disk checked first.
3. `scripts/linux-tests.sh -p factory-server --test setup_gates`, which exercises the arm64 images.
4. `scripts/compose.sh up -d --build --wait`, a quick look that the server and client answer, then `scripts/compose.sh down`.
5. Any committed script that fails on zsh or BSD tools is a bug. Fix it so it still works in Git Bash on Windows.

## Record the results

1. Write `evidence/macos/2026-11-checks.md` in hoi4-factory: the date, the machine facts, and one row per check with pass, fail or unavailable, the command, the exit code, the versions and the evidence. Name the issues filed and the pull requests merged. Keep screenshots out of the repository unless they show nothing private.
2. Update the macOS lines in `docs/PACKAGE_STATUS.md` with the outcome.
3. Run the package validator and `sync_skills.py --check`, commit, check `git log origin/main..main`, and push.
4. Report to the maintainer in one short message: the results table, what you fixed, the issues left, and the exact list of in-game checks that need the running game.
