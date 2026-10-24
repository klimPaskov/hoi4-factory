# SPEC-013: Configuration reuse and developer environment

Status: Proposed, revised for the Rust, Docker and hosted-service decisions in [maintainer decisions of 2026-10-24](../02-architecture/maintainer-decisions-2026-10-24.md). Owner: developer environment maintainer. Primary stages: S01 bootstrap and S08 maintenance audit. Requirements: REQ-038 and REQ-043. Acceptance: AC-038 and AC-043.

Read first: [shared instructions](../../AGENTS.md), [configuration migration report](../07-developer-workflow/CONFIG-MIGRATION-REPORT.md), [complete field map](../07-developer-workflow/CONFIG-FIELD-MAP.csv), [source inventory](../07-developer-workflow/CONFIG-SOURCE-INVENTORY.json), [bootstrap checks](../07-developer-workflow/ENVIRONMENT-BOOTSTRAP-AND-CHECKS.md), and [permissions specification](011-extensibility-sandbox-and-permissions.md).

## User value and scope

A maintainer should be able to adopt the planning starter without inheriting another project's secrets, machine paths, broad tool permissions or unsupported client options. The environment must support Claude Code first, preserve a usable Codex alternative and make unresolved dependencies visible. Configuration should explain behavior instead of silently relying on a developer's global setup.

This subsystem owns project instruction loading, skill discovery, developer-client settings, MCP connection definitions, configuration provenance and version checks. It does not define the hosted service's runtime configuration. The Rust server and agent crate need their own provider capabilities, account authentication, tenant-scoped roots, secret store references, C-08 ApprovalGrants and C-04 tool receipts. Developer configuration can supply integration evidence, but it cannot become the product's authorization database or a universal context limit.

The extracted `hoi4-factory/` folder is the working project root, with client files already in place and documentation under `docs/`. S01 verifies and adapts those files in place. Adoption into a different existing repository uses a reviewable merge that preserves team configuration. Extraction does not install tools, trust a directory, connect remote services or modify global client settings. Once a user authorizes a concrete bounded bootstrap stage, normal inspections and reversible in-scope edits proceed without redundant confirmation.

## Product build and container environment

The product is a Rust Cargo workspace for the server and agent crate plus a React and TypeScript browser client, built into Docker images under [ADR-013](../02-architecture/maintainer-decisions-2026-10-24.md#adr-013-docker-deployment). The developer environment therefore pins the Rust toolchain through a toolchain file, commits `Cargo.lock` and the frontend lockfile, and pins base images by digest. Local development runs the same images through Docker Compose with PostgreSQL, an object store or volume, a Git volume and a read-only game reference mount supplied by the developer.

Development configuration is separate from production configuration. A Compose override for development may expose debugging ports and seed fixture accounts. It never contains real provider keys, GitHub App private keys or game files. Those are supplied at run time from the developer's own secret source and are excluded from Git by ignore rules and from images by the build context exclusions.

The environment inventory records Rust, Cargo, Node and the frontend package manager, Docker Engine and Compose versions, the container runtime, and whether a game reference build is available to the developer. A developer without a game copy can still run source-only fixtures, with game-dependent checks reported unavailable.

## Verified source and compatibility findings

The audited upstream is Agentic-HOI4-Modding at commit `856940f2a629e0e2f3e03fc484cb0c0309c96734`, dated 2026-10-18. Its actual `.codex/config.toml` contains 58 scalar leaves, including 46 role registry leaves. All 23 referenced role files were fetched and parsed, with six fields each. The exhaustive map accounts for 196 observed fields. The separate supplied ChaosRedux configuration is historical evidence and is not presented as this repository's config. [SRC-C02](https://github.com/klimPaskov/Agentic-HOI4-Modding/blob/856940f2a629e0e2f3e03fc484cb0c0309c96734/.codex/config.toml)

The root source sets never approval, danger-full-access, 184320 instruction bytes, 24 legacy concurrent threads and V1 depth two. It defines HOI4 Agent Tools stdio and Comfy Cloud HTTP connections. It does not set a root model, provider, model context window, auto-compaction threshold, private trust path or credential. Referenced roles select reasoning effort and sandbox mode but do not fix a model. None of these absent fields may be invented as observed source values.

Current Codex documentation names `max_concurrent_threads_per_session` as the canonical concurrency key and documents `max_threads` as a legacy alias. Its current schema says `max_depth` is ignored by V2. The starter uses the canonical key with four threads and omits a depth guarantee. This is an explicit resource-policy adaptation, not a faithful copy of every default. [SRC-C10](https://developers.openai.com/codex/config-reference) [SRC-C11](https://github.com/openai/codex/blob/f71c6afc30031011fe0b4d757b981fd0301b2536/codex-rs/core/config.schema.json)

## Active starter and migration contract

The Codex starter retains the instruction byte ceiling and nonsecret server settings, switches to on-request approval and workspace-write sandbox, and sets both MCP servers disabled. The role registry is omitted from active configuration because its 23 domain production roles require separate scope and dependency review. Their source values and complete paths remain in the field map and inventory, so omission is explicit and reversible.

Claude has native JSON settings and MCP files. Its starter uses default permissions, preserves the source root .env read-deny patterns, rejects both project MCP server names, and disables blanket project-server approval. These patterns are a useful narrow precaution, not a complete secret boundary. No broad shell or MCP allow rules, hooks, tokens or user paths are included. A stricter OS sandbox is a target-machine validation choice with documented support requirements.

The HOI4 connection retains its Windows `.cmd` executable name. On another OS, the owner resolves an actually installed command and reviews the local diff before enabling it. Codex's 600-second tool timeout maps to Claude's documented 600000-millisecond per-server timeout. Codex's per-server startup timeout has only a wider client-level Claude environment equivalent. No undocumented Claude cwd or startup field is invented. The server must report the intended workspace root before any effect. [SRC-C18](https://code.claude.com/docs/en/mcp)

Every mapping row records source path and revision, key, observed value category, classification, target setting, preserved or changed behavior, dependency, compatibility, confidence, verification and future owner. Categories distinguish direct reuse, Claude adaptation, intentionally model-specific behavior, dangerous defaults, absent optional settings and no equivalent. The full CSV covers observed fields. The report separately records absent fields and proposed additions, preventing a new default from appearing to originate upstream.

## Operations and state transitions

`inspect_environment` receives the repository root and approved stage. It returns a nonsecret inventory of OS, architecture, executable paths relative to the selected environment, versions including Rust, Docker and Compose, current branch and dirtiness, loaded guidance, available skills and effective configuration sources. It never dumps credential environment variables. Missing optional tools are unavailable capabilities, not an automatic project failure.

`plan_config_merge` receives current project settings, the pinned starter and the field map. It produces a diff classified as preserved, adapted, added, removed or locally unresolved. It detects conflicting keys, broadened permissions and existing instruction ownership. A schema mismatch or unsupported field blocks that setting's activation, while unrelated checks may continue. A merge must not replace a whole file merely because it parses.

`validate_config` parses TOML and JSON, validates known fields against pinned schemas and current primary documentation, checks referenced paths and redacts sensitive values. It then runs a real installed-client loading check in a disposable project. Syntax success, schema success and client success are distinct evidence. The Claude SchemaStore root permits additional properties, so unfamiliar keys still require documented support. [SRC-C20](https://github.com/SchemaStore/schemastore/blob/bf7c6745505ccace5568dc0d7f4ae29647e9b41a/src/schemas/json/claude-code-settings.json)

`activate_connection` starts from a reviewed dependency and exact connection definition. The owner confirms installation, auth source, data destination, cost implications, resolved workspace and allowed effects. Activation changes only the chosen project connection and records the previous configuration. A harmless capability inventory precedes any domain operation. A timeout with uncertain effects enters reconciliation, not blind retry. Deactivation restores the disabled state without deleting evidence.

`upgrade_environment` records old and new versions, including base image digests and the Rust toolchain, compatibility claims, schema changes, meaningful regression cases and rollback. A model change also rechecks actual context and tool capabilities. An accepted engineering knowledge change updates setup instructions and the changelog. No upgrade silently alters user-level configuration or approved game design.

## Tests, evidence and failure recovery

Normal tests parse the shipped files, verify all observed fields are mapped, check the two disabled connections, resolve every enabled role path, and confirm instruction and skill discovery. A live S01 check records the installed client versions and effective configuration without exposing auth. A model report must identify whether context limits are documented, client-reported or measured.

Negative tests include a missing executable, stale command path, credential mistakenly embedded in a config diff, unsupported key, project config not trusted, wrong server root, absent remote authentication, and a role that can still mutate through shell tools despite a textual read-only label. Expected results are precise unavailability or denial, no hidden fallback to broad permissions, and no false completion claim. Existing user files remain unchanged.

Recovery tests interrupt activation, change the server binary after validation, and revoke an approved tool scope. The next effect must compare the relevant identity and policy again. An unknown outcome needs inspection and a receipt before retry. Reverting a Git config commit restores the prior project settings, but does not undo remote side effects. Those require their own operation-specific reconciliation.

This planning package can perform syntax, known-shape, path and mirror checks. Neither developer client, a full schema engine nor the MCP services are available here. Accordingly live client loading, actual server inventory, OS containment and context measurements remain explicit S01 experiments. The source projection comparison that did run is not described as the upstream test suite or a client integration test.

Run `python scripts/validate_package.py --root . --working-tree` from the project root during development, and `python scripts/sync_skills.py --check` for deterministic skill projections. The validator needs Python 3.11 or newer. On Windows with a non-UTF-8 locale, set `PYTHONUTF8=1`, as observed on 2026-10-24 when the default code page failed to decode the package text. The validator's default mode is the frozen distribution check against `docs/MANIFEST.json`. Working-tree mode skips distribution hashes and permits later evidence, Git metadata and product files while preserving authored planning and guidance checks. Product checks remain the approved stage's responsibility.

## Ownership and roadmap

S01 owns environment adoption and EXP-01 through EXP-04 as bounded by its stage packet. A failed critical architectural probe must be resolved or explicitly change the decision before production work proceeds. S02 and S03 add only the tools required for setup and native runtime integration. S08 reviews version pins, image digests, licenses, stale settings, optional client support and rollback documentation. The developer environment maintainer owns configuration integrity, the security owner reviews broadened effects, and the product architecture owner decides changes to the future runtime contract.
