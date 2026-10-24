@AGENTS.md

# Claude Code development entry point

Launch Claude Code from this repository root so its project settings and MCP definitions are resolved here. This file imports the shared rules once. Do not maintain a competing policy copy in this file. Current Claude documentation supports this import even where native AGENTS discovery depends on client version or other instruction files. See [the instruction map](docs/07-developer-workflow/SKILLS-AND-INSTRUCTION-MAP.md).

Use `/start-implementation-stage` for a stage already authorized by the user, `/validate-stage` for evidence review, `/capture-engineering-lessons` for the handoff review, and `/audit-hoi4-integrations` for a bounded integration inspection. Only the maintainer invokes `/start-implementation-stage` and `/audit-hoi4-integrations`. Under ADR-023 the agent runs `/validate-stage` and `/capture-engineering-lessons` itself at each handoff. No skill grants implementation, paid-operation or next-stage authority. Before first use, confirm all four names appear in the installed client's skill list.

Give subagents a small task, approved roots, owned paths and required evidence. Send only relevant context. Do not assume a subagent inherits skill content or that denying editing tools constrains shell and MCP effects. Use actual permission and sandbox controls supported by the installed version and OS. No custom subagents are preinstalled by this package.

For parallel work, use a reviewed Git branch or worktree after the repository has a commit. Preserve dirty work and record the actual base revision. Before cleanup, confirm which paths and branch are owned by the task. Resume with `claude --continue` or `claude --resume` only when the selected session belongs to this repository and task. Re-read the stage handoff and verify artifact hashes after resuming. A transcript is useful context, not proof that a stage or effect completed.

Keep root guidance concise and load relevant specs when needed. Record measured context constraints for the chosen model and client instead of transplanting another provider's token limits. If context becomes scarce, write a factual handoff with completed checks and pending operations before compacting or switching sessions.

The shipped `.claude/settings.json` and `.mcp.json` use Claude's documented JSON conventions. Both inherited MCP connections are rejected by default. Follow [environment bootstrap](docs/07-developer-workflow/ENVIRONMENT-BOOTSTRAP-AND-CHECKS.md) before enabling a connection. Claude does not read `.codex/config.toml`.
