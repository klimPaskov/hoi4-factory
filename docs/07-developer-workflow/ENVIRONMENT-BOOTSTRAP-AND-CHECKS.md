# Environment bootstrap and checks

Status: Proposed S01 procedure. Owner: developer environment maintainer. Read the [S01 packet](../04-implementation-stages/stage-01-bootstrap-and-probes.md), [config migration](CONFIG-MIGRATION-REPORT.md), [instruction map](SKILLS-AND-INSTRUCTION-MAP.md) and [Claude first-stage prompt](../06-agent-handoffs/CLAUDE-CODE-START-STAGE-01.md).

## Open the project folder

Use the extracted `hoi4-factory/` folder as the project root. Its `AGENTS.md`, `CLAUDE.md`, client settings and skill directories are already in their working locations, and planning documents live under `docs/`. Confirm that the user authorized S01, read applicable instructions and inspect Git initialization, branch, worktree, dirty paths, dependencies and package management conventions. Record absent Git metadata or an absent initial commit honestly. The planning archive itself does not authorize product implementation. If integrating these files into a different existing repository, preserve its files and merge the starter as a reviewed diff. Do not write into user-level or system-level client config.

All `evidence/SNN/` and `engineering/knowledge/` paths in this documentation are relative to the project root. Create those records when actual authorized work produces evidence. Do not create a second nested `hoi4-factory/` directory or move client settings under `docs/`.

Record `evidence/S01/environment.json` with OS and architecture, actual client versions, tool versions, selected working root, repository revision, dependency pins, available capabilities and unavailable checks. Use nonsecret summaries. Do not dump an entire environment, credential file or auth response into evidence. The client executable path may be recorded locally when needed, but shared templates must not contain private user paths.

The package requires Python 3.11 or newer for the portable TOML-based validation helper. It does not install Python, Node, Rust, Claude, Codex or HOI4 tools. Select runtime versions for probes from the approved stage packet and actual upstream manifests. Do not convert a repository CI matrix into a universal product minimum without testing.

## Check the files before client loading

Before changing a fresh extraction, the default validator verifies the frozen distribution, including its manifest:

```bash
scripts/py.sh scripts/validate_package.py --root .
```

During work, run the documentation and skill checks from the project root:

```bash
scripts/py.sh scripts/validate_package.py --root . --working-tree
scripts/py.sh scripts/sync_skills.py --check
```

Working-tree mode checks the authored planning and guidance files while allowing later Git metadata, evidence and product files. It skips frozen distribution hashes and cannot be combined with archive verification. It does not test the future application. Use the approved stage's own checks for product behavior.

Inspect `.codex/config.toml`, `.claude/settings.json` and `.mcp.json` with real parsers. Validate documented keys against the pinned schemas and current installed versions. Do not merely search text for a setting name. The planning package's own checks are documented as structural checks, not a full client integration test.

Confirm that both Codex MCP entries are false and both Claude project server names are in `disabledMcpjsonServers`. The latter list is the intentional project-server rejection mechanism. Do not remove it merely because a server executable is missing. Check that no tokens, personal trust tables, private absolute paths or unsupported keys entered the merge.

## Claude Code first

1. Launch Claude Code from `hoi4-factory/`, or the explicitly selected existing repository root, with normal permissions.
2. Confirm the installed version and inspect which instruction files loaded. Confirm `CLAUDE.md` imported AGENTS exactly once.
3. Confirm all four project skills are visible and callable explicitly. Use a read-only preparation invocation before enabling any external tool.
4. Inspect effective project, local, user and managed settings without copying secrets. A project file may be overridden or supplemented by other scopes.
5. Confirm the two shipped MCP connections remain disabled. No connection or tool call is required for instruction loading.
6. Record actual permission and sandbox behavior on this OS. Use a disposable fixture for an outside-root denial test. Do not test against valuable user files.

Current Claude supports the explicit import, project skills and native JSON configuration. These are documented conventions whose effective loading still needs the installed-client check. [Memory](https://code.claude.com/docs/en/memory) [Skills](https://code.claude.com/docs/en/skills) [Settings](https://code.claude.com/docs/en/settings)

For parallel work, create a reviewed Git worktree only after a commit exists. Record the actual base revision and ownership. `claude --worktree <name>` and `claude --resume` are convenience features, not durable product recovery or sandbox enforcement. Do not automatically copy `.env` files between worktrees. On resume, re-read the current scope and handoff, inspect dirty state, and reconcile any interrupted effect. [Worktrees](https://code.claude.com/docs/en/worktrees) [Sessions](https://code.claude.com/docs/en/sessions)

## Optional Codex interoperability check

Launch the installed Codex client from the reviewed repository. Verify project trust through its normal user interface. Confirm that project `.codex/config.toml` is effective, AGENTS is loaded, and the four `.agents/skills` names are available. The archive also includes `.codex/skills` copies, but they are not asserted as current automatic discovery paths.

Check canonical concurrency support, effective workspace-write sandbox and on-request approval. Do not use a permissive global setting to make a failing project check pass. Record the selected multi-agent backend because max_depth behavior differs. Select a model using the team's established account mechanism and record its actual capabilities. [Basic config](https://developers.openai.com/codex/config-basic) [Config reference](https://developers.openai.com/codex/config-reference)

Claude completion of S01 does not require a Codex license or account if the stage acceptance explicitly permits the optional trial to remain unavailable. Mark portability as structurally checked and live loading unverified. Never claim a model or provider benchmark without access and actual measured runs.

## Enabling an inherited MCP connection

The starter retains HOI4 Agent Tools and Comfy Cloud definitions for traceable reuse. Activation is an explicit project change after the selected task needs that dependency. For HOI4 tools, follow the pinned Mod Setup and tool audit rather than installing an arbitrary latest package. The Agentic manifest pins 4.0.3. The independently audited 4.3.0 tool release requires a separate compatibility migration. For Comfy, verify account authorization, external data destination, service cost and required authentication. A chat subscription does not establish API access.

Do not run the upstream bootstrap as an environment inspection. Its [inspected implementation](https://github.com/klimPaskov/Agentic-HOI4-Modding/blob/856940f2a629e0e2f3e03fc484cb0c0309c96734/.tools/mcp/bootstrap_hoi4_agent_tools.py) can install Node on Windows, remove a mismatched package and install into a user npm prefix. The quiet option only suppresses output. Installation requires a concrete reviewed action and applicable authorization. This package performs no installation and changes no global configuration.

For the selected connection, resolve the installed executable or endpoint, inspect its exact version and tool inventory, identify effectful operations, and verify the workspace root. Windows uses the observed `hoi4-agent-tools.cmd` command. Linux needs a verified installed command name and a reviewed local config diff. Codex cwd is explicit. Claude must be launched from the intended repository and its server-reported root checked.

After review, remove only the selected name from Claude's rejected list or set only the selected Codex connection enabled. Preserve the previous config revision. Start with a harmless capability or validation request, record the tool receipt and classify any unknown outcome. Do not retry an effect after a timeout until the result is reconciled. Claude timeout is milliseconds, while the Codex fields are seconds. [MCP configuration](https://code.claude.com/docs/en/mcp)

Missing executables, unsupported auth, unavailable game files or unreachable servers produce specific capability gaps. Continue source inspection and other authorized probes where useful. A dependent acceptance check remains unavailable until it has the required evidence. Do not fabricate a path, make an automatic global installation, or substitute an unreviewed server.

## Context, recovery and completion evidence

The source instruction budget 184320 is bytes. Root model context and compaction keys are absent. Verify actual context and compaction behavior for the selected client and model, identifying whether a value is documented, reported or measured. Keep handoffs focused on task state and evidence, and exclude hidden thought artifacts when changing providers.

S01 records `scope.md`, `environment.json`, `checks.json`, `config-review.md`, `learning-review.md` and `handoff.md` under `evidence/S01/`. The config review lists preserved settings, adaptations, unavailable dependencies, actual checks, remaining risks and rollback. The stage packet controls architectural probes and their exact exit criteria, including the known Setup recovery uncertainty. No production implementation beyond those bounds is authorized by this guide.

When configuration changes later, repeat the checks affected by the change and update provenance. Avoid rerunning unrelated expensive tests. S08 rechecks installed versions, supported environments, stale paths, licenses and disabled optional dependencies before release.
