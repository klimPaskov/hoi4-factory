# Configuration migration report

Status: Verified source inspection with Proposed starter adaptations. Review date: 2026-10-23. Owner: developer environment maintainer. Read with [SPEC-013](../03-subsystem-specs/013-config-reuse-and-developer-environment.md) and [bootstrap checks](ENVIRONMENT-BOOTSTRAP-AND-CHECKS.md).

## Exact source and coverage

The source is [Agentic-HOI4-Modding at 856940f2a629e0e2f3e03fc484cb0c0309c96734](https://github.com/klimPaskov/Agentic-HOI4-Modding/commit/856940f2a629e0e2f3e03fc484cb0c0309c96734), committed 2026-10-18T19:35:06Z. The actual file is [`.codex/config.toml`](https://github.com/klimPaskov/Agentic-HOI4-Modding/blob/856940f2a629e0e2f3e03fc484cb0c0309c96734/.codex/config.toml). Its SHA256 is `cd1746b7ef551d361c8c34d872a13e3c91b0f920941adca9c2f16e4008a59e8f`.

The inspection covered all 58 scalar leaves of the root config and all 138 fields across its 23 referenced role layers. There are no unresolved config-file paths. No include or profile layer is declared in this root config. The [complete CSV](CONFIG-FIELD-MAP.csv) provides one row for every observed field, including each role description, path and instruction layer. Long instructions are identified by length and SHA256 rather than copied as active policy. The [inventory](CONFIG-SOURCE-INVENTORY.json) records each layer's path, hash, effort and sandbox intent.

The separately supplied ChaosRedux TOML is a different Project Source. It is not the Agentic repository config. Its model names, local paths and context settings must not be attributed to this source.

The original repository uses Apache-2.0 for its own code and guidance. Its [NOTICE](https://github.com/klimPaskov/Agentic-HOI4-Modding/blob/856940f2a629e0e2f3e03fc484cb0c0309c96734/NOTICE) excludes third-party material and records an inherited art licensing uncertainty. This starter contains newly authored instructions. Adopting upstream assets or full skills later requires their applicable notices and dependency review.

## Root setting decisions

| Observed source key and value | Classification | Codex starter | Claude adaptation | Reason and check |
|---|---|---|---|---|
| `approval_policy = never` | Dangerous default for general adoption | `on-request` | `permissions.defaultMode = default` | Preserve scoped authority with normal client review. The two permission systems are not identical. Inspect effective policy in a disposable project. |
| `sandbox_mode = danger-full-access` | Dangerous default | `workspace-write` | No invented equivalent key | Review actual Claude OS sandbox support. Default permission mode alone does not contain every filesystem effect. |
| `project_doc_max_bytes = 184320` | Direct Codex reuse | Retained | No direct byte-limit key asserted | This is an instruction byte ceiling, not model context tokens. Keep root instructions substantially smaller. |
| `agents.max_threads = 24` | Legacy client key | Canonical `max_concurrent_threads_per_session = 4` | Task packet bounds concurrent workers | Four is a proposed starter resource cap. It is not an upstream value or a cross-model performance result. |
| `agents.max_depth = 2` | Backend-specific | Omitted | No verified numeric equivalent | Current Codex schema says V1 only and ignored by V2. Do not promise depth enforcement. |
| `agents.<role>.description` and `.config_file`, 23 roles | Optional role registry | All accounted for, none activated | Optional reviewed `.claude/agents` definitions | Product engineering does not need all mod-production roles during bootstrap. Adopt only a role with approved scope, tools and review. |
| `mcp_servers.hoi4_agent_tools.command = hoi4-agent-tools.cmd` | Reusable platform-specific connection | Retained, disabled | Same stdio command, rejected | Resolve the actual installed executable. The Windows command is not a fabricated absolute path. |
| `mcp_servers.hoi4_agent_tools.cwd = .` | Partial portability | Retained | Launch Claude from repository root | No Claude MCP cwd field was verified. Check server-reported roots before effects. |
| `startup_timeout_sec = 120` | Partial Claude equivalent | Retained | Optional session `MCP_TIMEOUT=120000`, not shipped | Claude startup setting is client-wide. It is not an exact per-server translation. |
| `tool_timeout_sec = 600` | Claude equivalent | Retained | Server `timeout = 600000` | Convert seconds to milliseconds. Timeout does not prove an effect was canceled. |
| `default_tools_approval_mode = auto` | Dangerous inherited automatic approval | `prompt` | Default permissions, no broad allow rule | Inspect exact effective tool policy before activation. |
| `env.HOI4_AGENT_STDIO_IDLE_EXIT_MINUTES = 30` | Shared server setting | Retained as string | Retained as string | This is a server lifecycle setting, not model compaction or durable product scheduling. |
| `mcp_servers.comfy_cloud_portraits.url` | Optional external dependency | Known URL retained, disabled | HTTP definition retained, rejected | Authentication, costs, data destination and capabilities remain unverified until activation. |

All root details are supported by the [pinned source](https://github.com/klimPaskov/Agentic-HOI4-Modding/blob/856940f2a629e0e2f3e03fc484cb0c0309c96734/.codex/config.toml), current [Codex reference](https://developers.openai.com/codex/config-reference) and current [Claude MCP documentation](https://code.claude.com/docs/en/mcp). The detailed CSV adds dependency, compatibility, confidence, verification procedure and future owner to each row.

## Referenced layers and intentionally omitted roles

Each of the 23 inspected Codex role files has exactly six fields. Their mappings are repeated per actual file in the CSV so no field is silently omitted.

| Role field | Decision |
|---|---|
| `name` | Preserve stable role identity if adopted, adapt naming to the target client's convention. |
| `description` | Reuse routing intent after reviewing engineering applicability. Do not make a mod role appear generally capable. |
| `model_reasoning_effort` | Codex-specific. The source projects Claude tiers to effort, which is not measured capability equivalence. |
| `sandbox_mode` | Retain intent with actual client enforcement. Claude tool exclusions alone may leave shell or MCP mutation possible. |
| `nickname_candidates` | Optional display metadata. No exact Claude field is claimed. |
| `developer_instructions` | Complete content inspected and hashed. Re-author a bounded engineering role rather than activating project-specific canon, workflows and dependencies wholesale. |

Source tiers are one xhigh improvement planner, five high roles and seventeen medium roles. Four roles are read-only and nineteen workspace-write. None has a fixed model. The canonical Claude agents use Opus, Sonnet and Haiku tiers respectively. A source-only projection check reproduced all 23 Codex role files and the registry without differences. This was an isolated pure-function comparison, not an upstream test suite run, installed-client trial or custom-agent execution.

The source sync library and guidance are pinned [here](https://github.com/klimPaskov/Agentic-HOI4-Modding/blob/856940f2a629e0e2f3e03fc484cb0c0309c96734/.tools/sync/README.md). Its committed tree has 32 `.claude` skills and no committed `.codex/skills` or `.agents/skills`. Setup projects canonical skills into `.agents/skills` for Codex.

## Absent settings and proposed additions

| Setting or concern | Observed status | Starter decision and future verification |
|---|---|---|
| Root `model` and `model_provider` | Absent | Do not invent a fixed provider or claim a source default. Select the implementation model locally after verifying account capability. |
| `model_context_window` | Absent | No universal token number. Record actual model/client capabilities and observed budget behavior. |
| `model_auto_compact_token_limit` | Absent | Do not carry a historical ChaosRedux value into every model. Validate compaction behavior separately. |
| Profiles, include files and feature switches | Absent in inspected root | Do not introduce extra config layers merely to resemble another source. |
| Private project paths, trust entries, API keys and tokens | Absent | Keep credentials in an approved local auth mechanism. Never write private paths into shared templates. |
| Meshy and Blender MCP definitions | Intentionally absent in source comments | Keep absent until a scoped asset task requires a reviewed installation and tool capability. |
| MCP `enabled` | Added | Both Codex entries are explicitly false. This is a safety adaptation. |
| Claude `disabledMcpjsonServers` | Added | Reject both named connections until dependency review and scoped activation. |
| Claude `enableAllProjectMcpServers` | Added false | Avoid blanket project-server approval. Do not use this setting as a replacement for checking managed or local policy. |
| Claude per-server `timeout` | Added equivalent | HOI4 value 600000 ms preserves the observed Codex tool timeout. |

Optional does not mean inaccessible tools can be called by implication. The user or maintainer selects the concrete connection and scope. A request for a source audit does not authorize a paid generation or external write.

## Three configuration domains

**Codex development config** is project `.codex/config.toml`. It loads under the installed client's trust and precedence rules. **Claude development config** is `.claude/settings.json` plus `.mcp.json`, with its own local, user and managed overrides. **Future application runtime config** is a separate versioned product contract with capability, credential and approval handling. It must not ingest a developer TOML or JSON file as trusted runtime policy. [Codex basic config](https://developers.openai.com/codex/config-basic) [Claude settings](https://code.claude.com/docs/en/settings)

The source manifest identifies five coding environments and Claude Code as its default primary. Its manifest schema is 5.0.0, and its generated source revision differs from the later manifest refresh commit. It pins HOI4 Agent Tools 4.0.3. The independently audited tool repository is 4.3.0, a separate compatibility target that requires a reviewed migration. The new application must reuse the setup adapter's own compatibility and receipt rules instead of interpreting a developer-client preference as an onboarding runtime decision. [Pinned manifest](https://github.com/klimPaskov/Agentic-HOI4-Modding/blob/856940f2a629e0e2f3e03fc484cb0c0309c96734/hoi4-mod-setup.manifest.json) [Audited tool package](https://github.com/klimPaskov/hoi4-agent-tools/blob/25f5641bbc07c65a822ef576ac6e92bd12666880/package.json)

The complete upstream [HOI4 tool bootstrap](https://github.com/klimPaskov/Agentic-HOI4-Modding/blob/856940f2a629e0e2f3e03fc484cb0c0309c96734/.tools/mcp/bootstrap_hoi4_agent_tools.py) and its [contract tests](https://github.com/klimPaskov/Agentic-HOI4-Modding/blob/856940f2a629e0e2f3e03fc484cb0c0309c96734/scripts/test_mcp_bootstrap_contract.py) were inspected as source. The bootstrap validates exact package and runtime identities but is effectful. It can install Node through Windows winget, remove a mismatched package and install into a user npm prefix. Its quiet flag does not make it a diagnostic. Neither the installer nor those tests ran here. The starter does not ship or invoke the installer.

## Validation confidence and maintenance

The [Codex schema](https://github.com/openai/codex/blob/f71c6afc30031011fe0b4d757b981fd0301b2536/codex-rs/core/config.schema.json) is pinned to f71c6afc30031011fe0b4d757b981fd0301b2536, dated 2026-10-23. The [Claude settings schema](https://github.com/SchemaStore/schemastore/blob/bf7c6745505ccace5568dc0d7f4ae29647e9b41a/src/schemas/json/claude-code-settings.json) is pinned to bf7c6745505ccace5568dc0d7f4ae29647e9b41a, dated 2026-10-19. Both pinned schemas matched their initial retrieved content.

The current Codex schema omits the documented legacy max_threads alias. That conflict is why the starter uses the canonical key. The Claude schema permits additional root properties. Therefore a schema pass alone cannot prove that a new key works. This package limits active keys to those corroborated by current documentation and inspected schemas.

Planning checks parse JSON and TOML, inspect documented shapes, check disabled connections and verify generated skill consistency. No full JSON-schema engine or live client exists in this environment. S01 must validate the installed versions, actual instruction loading, skills and effective permissions. The package does not claim those checks passed.

For each future change, record the exact source version, nonsecret diff, behavior change, owner, test evidence and rollback. Run a focused regression when permissions, tools, models, context behavior or discovery changes. Update this report, the instruction map, bootstrap guide and changelog together. Do not edit user or system config automatically. Reverting project config restores settings, but remote effects still need their own receipts and recovery.
