# Delegate parallel work

## Trigger and inputs

Use this workflow when a stage splits into independent packets that subagents can build at the same time, or when one packet is large enough to hand off. Inputs are the stage scope, the packet list with owned paths, the current `main` revision and the [instructions](../../../../CLAUDE.md).

## Procedure

1. Split by ownership. Each packet owns disjoint paths, normally one crate or one top-level folder, plus its own migration number range. Shared files such as the root `Cargo.toml`, `Cargo.lock` and `compose.yaml` belong to the parent unless one packet clearly owns them. Packets that depend on another packet's API start only after that API is merged.
2. Create each worktree yourself from the repository root: `cd "$(git rev-parse --show-toplevel)" && git worktree add -q -b <stage>/<name> ../hoi4-factory-wt-<name> main`, run from inside the main checkout and not from a nested clone. Confirm with `git worktree list`. Do not rely on automatic isolation, which follows the session's current directory and once created worktrees of a nested clone. Work in an upstream clone uses its own `git worktree add` inside `.work/`.
3. Write a self-contained brief: the absolute worktree path and a rule to stay inside it, files to read first, the exact deliverable and public API, owned paths, tests to run and how (`CARGO_TARGET_DIR` set to the shared cache, disposable digest-pinned Postgres on a random 127.0.0.1 port, Docker images pinned by digest), the commit trailer, "do not push", and what to return. Subagents cannot create new Markdown files, so ask for report text in the final message.
4. Pick the model by task: Haiku for mechanical aggregation and simple edits, Sonnet for code, drafting and research, Opus only when a task clearly needs it.
5. Run independent agents in parallel in the background. Send corrections with SendMessage as soon as scope changes, such as a maintainer decision that removes a feature. Never poll their transcripts.
6. When an agent reports, review before merging: list files changed outside its owned paths (`git diff --name-only main...<branch>`), read the summary critically, and re-run its key tests yourself on the merged tree.
7. Merge into main right away. Resolve `Cargo.toml` conflicts by unioning `members` and workspace dependencies, take one side of `Cargo.lock` and let `cargo build` regenerate it. Build the whole workspace, run the affected tests, commit, then remove the worktree and delete the branch. Never remove a worktree with uncommitted changes.
8. Save report text the agent returned into the evidence folder, and record lessons the round taught in CLAUDE.md or a skill.

## Outputs and evidence

Merged commits on `main` with the agent's commit trailer, the agent's report saved where the stage expects it, no leftover worktrees or branches, and stopped test containers. Note in the stage evidence which agent produced which packet and which checks the parent re-ran.

## Stop and error paths

If an agent was given the wrong repository or path, stop it with SendMessage before it writes, recreate the worktree correctly and relaunch. If Docker or the disk becomes the bottleneck, move agents to the shared build cache and ask the maintainer before restarting Docker Desktop, because it hosts their other projects. If an agent edits outside its owned paths, revert or move those edits before merging. If two packets need the same file, sequence them instead of running them in parallel.
