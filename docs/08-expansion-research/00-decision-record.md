# Expansion research decision record

Status: **Plan approved on 2026-10-31, Phase D under way**. Started 2026-10-31. Every maintainer decision in the expansion research session is recorded here when it is made, with the date and the question it answers. Decisions that change product scope are also copied into the [maintainer decision record](../02-architecture/maintainer-decisions-2026-10-24.md). Nothing in this record authorizes implementation. Phase D starts only after the maintainer approves the consolidated plan.

Choices offered for each proposal: **Adopt** (take the recommended approach), **Adapt** (keep the idea, change it for our workflow), **Defer** (save for later), **Reject** (do not take it).

## Session setup decisions

| ID | Date | Question | Decision |
|---|---|---|---|
| D-01 | 2026-10-31 | How may material from the maintainer's private repositories appear in committed documents? | Keep it out entirely. Private repositories are not used as evidence in committed documents. Private-derived working notes stay in the git-ignored `.work/` folder |
| D-02 | 2026-10-31 | How should desktop use be framed, given that ADR-012 made the product web-only? | Plan an optional local companion that links the user's PC (game install, HOI4 Live Tools, local mod folder) to a hosted project. ADR-012 stands |
| D-03 | 2026-10-31 | How are decisions made in Phase B? | Full rounds for the Hagane Works workflow ideas first, then a few shorter rounds only for product choices that change architecture or scope. Other product choices become recommended defaults in the plan |
| D-04 | 2026-10-31 | How is the Hagane Works workbench covered, now that the hosted service has closed registrations? | Read the GPL-3.0 Hagane Studio source and the public documentation. No account is used |
| D-05 | 2026-10-31 | What reuse policy applies to Hagane Works material? | The maintainer discussed the project with the Hagane Works author, who agreed that the maintainer takes over development and may use the project in any way. The permission is in writing, held by the maintainer, and covers both `hagane-works/hagane-studio` and `haganeworks/hoi4-modmaking-skills`. This meets the written-permission reversal trigger of ADR-010 for these two repositories. Reuse is still judged on quality and fit, and the original request still says not to copy the skills wholesale |
| D-06 | 2026-10-31 | What strategic role should Hagane Studio play? | Decide with evidence in Phase C, after the studio source analysis. Options: harvest editors into HOI4 Factory, make the studio the local or desktop edition, converge into one product, or keep them separate |
| D-08 | 2026-10-31 | The maintainer asked for a separate worker that thoroughly improves the offline Paradox wiki snapshot | Authorized as its own session in the Agentic HOI4 Modding Kit, outside this planning task. Every page is reviewed and updated against HOI4 1.19.3.0, empty or unfinished pages are completed, missing articles are added, local improvements survive future syncs, and a refresh workflow keeps the snapshot current. Edits to the snapshot are allowed. Work stays on a feature branch until the maintainer approves a push |
| D-07 | 2026-10-31 | The maintainer asked to use Rust wherever possible so everything is fast. How far does that go? | One shared Rust HOI4 engine (parser, symbol index, reference closure, lints) is built first for P1 to P3. HOI4 Factory links it as a library, the kit and Chaos Redux call its command-line interface without needing the MCP server, and hoi4-agent-tools moves its tools onto it one at a time behind the same MCP names until the TypeScript index is retired. New backend and tool code is Rust where practical |

## Established follow-up requirements (2026-10-31)

The maintainer added four requirement areas to the same plan, relayed verbatim from the parent session. They are established requirements, not optional ideas, and are kept apart from the Hagane Works rounds. Only the choice of how to meet them goes to interactive rounds. Implementation still waits for plan approval.

| ID | Requirement | Repositories |
|---|---|---|
| FR-1 | Upfront questions rule: inspect first, separate established requirements, independent decisions and genuine questions, ask once in an organized initial consultation with suggested answers, record the answers in specifications, then continue without avoidable interruptions. Respects the autonomy levels and the idea-selection rounds | Kit canonical instructions, skills and runtime projections, HOI4 Factory runtime policy |
| FR-2 | Derivative project `hoi4-imagegen-mcp` from `Connected-Mate/gptimage` (MIT, notices preserved) for HOI4 graphical assets, using documented and authorized image APIs only, never the upstream's unofficial subscription backend. Category profiles from verified configuration, reference-guided generation and editing, preprocessing, transparency, deterministic validation, naming and placement, preview and approval, versioned outputs with provenance. Integrated into the kit and HOI4 Factory without duplicating existing image infrastructure | New `hoi4-imagegen-mcp`, kit, HOI4 Factory |
| FR-3 | Focus-tree quality in hoi4-agent-tools: investigate why agent-made trees look poor despite structural checks, separate structural, visual layout and design quality, improve the agent workflow (plan, layout, render, rubric review, revise, validate, approve, verify in game) and the MCP's diagnostics, scoring, comparison and partial layout, tested on real trees. No separate focus MCP. A real report was supplied: `.work/focus-tree-reports/2026-10-31-soviet-tree-weird-branching.webp` (git-ignored) | hoi4-agent-tools, kit focus skill, HOI4 Factory checkpoints |
| FR-4 | Audit and automate the kit's workflow synchronization so canonical instruction, skill and agent changes can never leave generated projections silently stale, with targeted dependency-aware execution, CI checks and the seven acceptance criteria | Kit, Chaos Redux generation (P10), HOI4 Factory job validation |
| FR-5 | Two-way visual editing between people and agents, added 2026-11-01 (see below): the MCP's read and render tools double as a visual editor, and edits by people and agents go through the same operations on the native mod files | HOI4 Factory editors (focus first), hoi4-agent-tools |

### Intake answers for the follow-up requirements (2026-10-31)

| ID | Question | Decision |
|---|---|---|
| D-09 | How is the `hoi4-imagegen-mcp` repository set up? | Public, `klimPaskov/hoi4-imagegen-mcp`, MIT like the upstream, with the upstream notices kept |
| D-10 | Where does the supplied Soviet focus-tree screenshot come from? | Unknown. A search of the maintainer's local mod folders found no match. The FR-3 investigation works from the screenshot alone |
| D-11 | In hosted HOI4 Factory, who pays for image generation? | The monthly operator allowance may be spent on images too, alongside bring-your-own provider keys. Per-user and per-task budgets still apply, and the plan must show the cost exposure |
| D-13 | What does the first implementation phase cover, and where does kit work happen? | Relayed from the maintainer through the parent session: research and workflow updates only, with no full HOI4 Factory stage development yet. Proposals stay simple and useful. Kit work happens directly in the kit checkout, and worktrees, scripts or other extra layers are added only when they really help. HOI4 Factory product features in this plan stay designs until a stage is authorized |
| D-12 | Which image providers does `hoi4-imagegen-mcp` target first, and what happens to the ChatGPT login backend? | Documented, authorized APIs are the main providers: the OpenAI API with an API key, Google Gemini or Imagen, a ComfyUI endpoint (local GPU or RunPod), and other hosted APIs chosen from the research (for example FLUX, Stability, fal.ai or Replicate). The maintainer amended FR-2 for the ChatGPT login: it stays available only as an opt-in, local-only, personal-use provider plugin, off by default, never shipped in hosted HOI4 Factory, with its terms and breakage risk documented. The maintainer was told it uses an undocumented backend that may break and may not be covered by the provider's terms |
| D-12a | How does image generation work on the maintainer's own machine? | Amends D-12 on 2026-11-01. On a local install the ChatGPT-login provider is a first-class route, not an opt-in plugin. It ships in the normal local build of `hoi4-imagegen-mcp` with no source build, feature flag or extra environment variable. The user signs in once, explicitly, in the browser, and that step shows the risk note once: the backend is undocumented and may break or hit account limits. When the user is signed in it is the default local provider, and the documented APIs stay available as alternatives. It is never part of hosted HOI4 Factory, which uses documented APIs only. W5 is finished only when this route is built, tested end to end with the real login, integrated into the kit, Chaos Redux, HOI4 Mod Setup and the HOI4 Factory plans, and documented, with only maintainer-only checks left open. Maintainer decision, 2026-11-01 |

## Phase B rounds

Rounds are added below as they are decided. Proposal details, source references and trade-offs are in the [Hagane Works analysis](03-hagane-works-analysis.md).

### Round 1: validation (2026-10-31)

| ID | Proposal | Recommendation | Decision | Home |
|---|---|---|---|---|
| P1 | Whole-mod reference closure: every reference marked resolved, unresolved or unknown, unknown never a pass, coverage listed per kind including zeros | Adopt | **Adopt** | hoi4-agent-tools, called by the kit, Chaos Redux and HOI4 Factory |
| P2 | Lint vocabulary generated from the installed game's documentation, plus reciprocal `mutually_exclusive`, vanilla namespace collision, inert MTTH on triggered-only events, duplicate loc keys in one event and unknown modifier keys | Adopt | **Adopt** | hoi4-agent-tools |
| P3 | Override and load-order audit: definitions removed by same-path files, emptied `replace_path` directories, exact shadow filenames | Adopt | **Adopt** | hoi4-agent-tools, shown by HOI4 Factory on import, run by `check.py full` |
| P4 | Import preservation report per path, and a visible shrinking findings baseline so checks fail only on new findings | Adopt | **Adopt** | HOI4 Factory and Mod Setup for the report, the kit and Chaos Redux for `check.py --baseline` |

Home revised by D-07 later the same day: P1 to P3 are built in the shared Rust HOI4 engine, not in the TypeScript code of hoi4-agent-tools. The agent-tools MCP routes call the engine.

### Round 2: investigation and debugging (2026-10-31)

| ID | Proposal | Recommendation | Decision | Home |
|---|---|---|---|---|
| P5 | Static bug investigation protocol: root cause before any edit, premise tripwire phrases including in the agent's own reports, evidence ladder with the reached rung reported, surprising fixes treated as red flags, no edits only to silence a validator | Adopt | **Adopt** | New small kit skill `hoi4-investigate` with a Chaos Redux mirror. Shared root-cause rules move out of debug-playtest into it. HOI4 Factory uses it as the policy for fix jobs |
| P6 | Log reading order (`game.log` last line, crash `exception.txt`, `dlc_load.json`, filtered `error.log`), first-error parser cascade rule, launcher `.mod` drift check, symptom table. Used only when a log is already available. The rule against asking for logs stays | Adopt | **Adopt** | P5 skill, and a HOI4 Factory "Report a game problem" action where the local companion attaches fresh logs |
| P7 | Short in-game test checklist with console commands | Adapt | **Adapt** | Not in chat. A "Test plan" section in Chaos Redux event docs, and an optional "Test in game" card per finished HOI4 Factory job whose results are recorded as user-operated game evidence |

### Round 3: knowledge and agent workflow (2026-10-31)

| ID | Proposal | Recommendation | Decision | Home |
|---|---|---|---|---|
| P8 | Import Hagane traps that are confirmed on HOI4 1.19.3.0 and missing from the kit, reworded and evidence-labelled. The contradicted claims are excluded | Adopt | **Adopt** | Kit system skills, then Chaos Redux and HOI4 Factory global lessons |
| P9 | Evidence tag on every rule (`[game-tested 1.19.3]`, `[vanilla-confirmed 1.19.3]`, `[docs]`, `[unverified]`) and a `LESSONS.md` incident ledger tying each new rule to its incident and regression fixture | Adopt | **Adopt** | Kit and Chaos Redux, and HOI4 Factory global lessons |
| P10 | Kit as the single authoring home for generic rules, with generated Chaos Redux engine skills | Adopt | **Clarification requested**, re-asked and decided in round 4 | Kit and Chaos Redux |
| P11 | Slim the always-loaded template and the largest skills | Adopt | **Adopt with a constraint**: not too aggressive, and every instruction must stay. Text is deduplicated or moved into references, never dropped, and a diff audit proves that no instruction was lost | Kit, then Chaos Redux |

### Round 4: design and planning (2026-10-31)

| ID | Proposal | Recommendation | Decision | Home |
|---|---|---|---|---|
| P10 | General rules are written once in the kit. Chaos Redux engine-system skills (ideas, characters, equipment, map, technology and similar) are generated as the kit text plus a Chaos Redux section, with a check that fails on direct edits of the generated copy. Events, event planning and event assets stay owned by Chaos Redux, with a backport audit that lists generic rules missing from the kit | Generate (option A) | **Adopt option A** | Kit and Chaos Redux |
| P12 | Design brief at a checkpoint: the agent decides everything derivable and asks only open items, tiered as story gaps, design calls with a recommendation, and quick confirms with defaults. Settled items are never asked again | Adopt for HOI4 Factory | **Adopt for HOI4 Factory only** | HOI4 Factory semi-autonomous design checkpoint. No change to Chaos Redux planning |
| P13 | Stable IDs and locked, derived or open origin tags on spec items, cited by implementation handoffs, with a gap audit in both directions | Adopt | **Adopt** | New Chaos Redux spec packages (no retrofitting) and HOI4 Factory specs. Basis for Drift Bot |
| P14 | Feasibility check before a spec commits to a mechanic: the hook, effect or scope exists and fires in the loaded mod, and the real setters of key flags are traced | Adopt | **Adopt** | Kit `hoi4-feature-planning`, Chaos Redux import review, automatic in HOI4 Factory design checkpoints |

### Round 5: ideas recommended for rejection (2026-10-31)

| ID | Proposal | Recommendation | Decision | Reason |
|---|---|---|---|---|
| R1 | Auto-update skills with `git pull` at every session start | Reject | **Reject** | Unpinned remote instructions are a supply-chain risk. Pinned sync with checks stays |
| R2 | Platform-owned IDs with automatic prefixes and `_1` suffixes | Reject | **Reject** | Native files must keep the author's IDs. `next_id` allocation and the P1 and P2 collision checks cover the need |
| R3 | Hidden startup-event overlays to modify vanilla characters and leaders | Reject | **Reject** | Changes take effect at day 0 instead of in the definition and are invisible in Git. Direct edits or shadow files, audited by P3, are used instead |
| R4 | Hagane's field reference as our HOI4 reference | Reject | **Reject** | It contains contradicted claims. Vocabulary is generated from the game documentation (P2) and confirmed facts arrive through P8 |

Ideas not put to a vote because existing practice already covers them or Chaos Redux has its own approach are listed with reasons in the [Hagane Works analysis](03-hagane-works-analysis.md#ideas-not-proposed).

### Product round A: architecture (2026-10-31)

| ID | Proposal | Recommendation | Decision |
|---|---|---|---|
| A1 | Hagane Studio: harvest the useful parts (focus-graph shell, event and decision preview cards, hash-guarded span repair plans, per-edit finding delta, draw.io importer, finding-code catalog as fixtures, and game discovery, safe install and log parsing for the companion, rewritten in Rust where they are Go). Tag a final release, keep it maintenance-only, then archive it once HOI4 Factory imports Hagane exports, the companion syncs local folders, and the focus, event, decision and character editors exist | Harvest, then archive | **Harvest, then archive**, narrowed by A1-1 on 2026-11-01 |
| A2 | One job engine. Autonomy levels are presets of a per-project checkpoint policy over design brief, visual review, plan, diff, commit and merge or publish, each set to auto, ask or notify. Switchable mid-job at the next checkpoint without widening grants. Review queue and decision history. The existing visual, hybrid and autonomous modes are renamed editing styles | Adopt | **Adopt with a change**: users may set the merge checkpoint to auto if they want. The fully autonomous preset still asks before merge by default, and an owner can switch merge to auto as a standing project rule. Branch protection and required checks still apply and merges are never forced |
| A3 | Timing of parallel writer agents | Keep the ADR-007 order | **Clarification requested**, re-asked and decided in product round B |
| A4 | HOI4 Factory's runtime agent loads the kit's text-only skills and `LESSONS.md` ledger as versioned HOI4 knowledge, and a project's own `CLAUDE.md` or `AGENTS.md` as labelled data with no authority. Executable skills wait for the sandbox | Adopt | **Adopt** |

### Product round B: data and features (2026-10-31)

| ID | Proposal | Recommendation | Decision |
|---|---|---|---|
| A3 | Parallel writer agents, each in its own worktree with declared paths and serialized integration. Read-only helper agents are allowed at once | After the S06 conflict and recovery tests | **After the S06 tests**. ADR-007 order stays |
| B1 | Shared notes as Obsidian-compatible Markdown in `.factory/notes/` of the project repository, database only as a rebuildable index, `confirmed`, `proposal`, `assumption` and `outdated` status with `confirmed` set only by a human, agent edits to human notes as proposed diffs, never shipped in the mod, separate from ADR-022 memory. Public mods may use a separate private notes repository | Adopt | **Adopt** |
| B2 | Native jobs board with many fields, linked one-to-one to GitHub issues | Adopt | **Adapt: keep it simple.** The maintainer said there is no need to make it unnecessarily complicated. The plan specifies a minimal board and adds fields only when real use needs them |
| B3 | Drift Bot | Adopt | **Reject**. No Drift Bot is built. P13 (stable spec item IDs, origin tags, citations and the gap audit) stays adopted on its own merits |

### Product round C: engine, validation, dependencies, companion (2026-10-31)

| ID | Proposal | Recommendation | Decision |
|---|---|---|---|
| C1 | Home of the shared Rust HOI4 engine | A Cargo workspace inside hoi4-agent-tools, Apache-2.0 | **Inside hoi4-agent-tools**. MCP tool names stay stable while tools move onto the engine |
| C2 | CWTools MD Edition (MIT Rust rules engine) as the rules-validation layer | Bounded SHA-pinned spike, then integrate behind the engine's interface | **Delegated** ("whatever makes the MCP better"). Plan: run the spike and integrate only if it measurably improves the MCP's validation (more true findings on the fixture corpus, acceptable false positives on vanilla, acceptable startup time and memory on amd64 and arm64). Otherwise the engine keeps its own closure and lints and the F# worker plan is dropped. It is never the write path |
| C3 | New runtime dependencies | Approve all four | **Approved**: `@xyflow/react` (after a CSP spike), `@dagrejs/dagre` (when event-chain layout starts), `image_dds` (with legacy DDS headers until an in-game test passes), CodeMirror 6 (optional, lazy-loaded) |
| C4 | Local companion v1 in Rust | Full v1 | **Full v1**: pairing, game detection, mod folder sync with local edits returned as proposals, log upload on request, HOI4 Live Tools test cards |

### Round D: choices inside the established requirements (2026-10-31)

| ID | Proposal | Recommendation | Decision |
|---|---|---|---|
| D1 | Position storage for agent-made focus trees | Solver places each branch lane, source uses `relative_position_id` anchors per branch | **Solver plus branch anchors** |
| D2 | Strictness of the focus layout verdict | `blocked` and `revise` block the commit at every level with a recorded override, `review` is advisory | **`blocked` and `revise` gate, `review` advisory** |
| D3 | Agents calling a ComfyUI endpoint the maintainer configured (local GPU or a RunPod URL) | Allowed except portraits of real historical people | **Allowed for everything**, including portraits of real people. This changes the kit's rule that agents never operate RunPod. Chaos Redux keeps its own asset policy (historical leaders are never generated) as a project overlay unless the maintainer changes it separately |
| D4 | Vanilla Paradox art sent to image providers as reference images | Opt-in for local kit use only, never in hosted HOI4 Factory | **Opt-in everywhere**, per project, off by default. In hosted HOI4 Factory this amends ADR-015, which keeps raw game files on the server, for opt-in reference images only. It stays behind the game-rights review that already gates public exposure |

## Maintainer decisions after plan approval (2026-11-01)

Relayed from the maintainer through the parent session. They are documentation decisions and authorize no building.

| ID | Question | Decision |
|---|---|---|
| A1-1 | What does HOI4 Factory take from Hagane Studio? | **Harvest narrowed to hard capabilities.** HOI4 Factory builds its own interface through its own design track (ADR-025, `ui-design-iteration`), and none of Hagane Studio's visual design is reused. From Hagane only capabilities that would be costly to rebuild from scratch are taken: span repair plans, per-edit finding changes, lints, the draw.io importer, the finding-code catalog, and game discovery, safe install and log parsing. Each is adapted to our architecture and design. UI pieces our own tools already cover, such as the event and decision preview cards, are dropped. The xyflow focus-graph wiring is taken only if it saves real effort when S04 is designed. Maintainer decision, 2026-11-01 |
| FR-5 | How do people and agents edit the same thing visually? | **Two-way visual editing between people and agents.** The MCP's read and render tools double as a visual editor for people, who can view, drag, snap and rearrange elements such as focus nodes directly. Edits by people and by agents go through the same operations and are saved to the native mod files, so each side sees the other's changes at once. An agent's edit appears in the editor immediately, and a person's layout change is visible to the agent through its normal inspect and read tools, so the agent continues from the current state instead of overwriting it. Met by the adopted pattern of one operation layer for the editor and the agent, with parity tests ([Hagane analysis](03-hagane-works-analysis.md)), with no new subsystem. Applies to the S04 focus editor and every later editor, and builds on W3 (verdict and inline renders) and W7 (layout engine). Maintainer decision, 2026-11-01 |
| Q-1 | What quality bar do HOI4 Factory and hoi4-agent-tools set against comparable tools? | **Decisive quality lead.** HOI4 Factory and hoi4-agent-tools must be decisively ahead of comparable tools such as Hagane Studio and `cpntodd/HOI4-MCP`: faster, more accurate, more reliable and more deeply integrated with coding agents, so that the lead is never in doubt. The lead is proven with measured head-to-head benchmarks on shared mod files and questions, and those benchmarks run in CI so no change quietly makes a tool worse. Anything harvested from Hagane must end up better than the original once it is integrated with our tools. Linked to A1 and A1-1, FR-5, P1 to P4, D-07 and W7. Maintainer decision, 2026-11-01 |

## MCP improvement decisions (M-1 to M-15), 2026-11-01

Made by the maintainer in an interactive review of hoi4-agent-tools, after an analysis of `cpntodd/HOI4-MCP` found it weak. They apply Q-1 to the MCP. Documentation only. Where each lands is in the [stage roadmap](../04-implementation-stages/ROADMAP.md#mcp-improvement-decisions-m-1-to-m-15).

| ID | Topic | Decision |
|---|---|---|
| M-1 | Skills served by the MCP | hoi4-agent-tools serves the kit's skills through a `hoi4.skill` tool, which lists them and loads one by name, and as MCP resources. The text served is the kit version pinned for the mod, with its hash. A check fails when the served text drifts from the source, so every client reads the same skills reliably. Maintainer decision, 2026-11-01 |
| M-2 | Approved rules, one format at two levels | Kit-wide rules live in the kit and mod rules in the mod's repository, in one format. Agents propose rules, and a rule takes effect only after the maintainer approves it. Each rule carries its reason, its evidence (an `error.log` line, game documentation or vanilla) and its regression test, and duplicate and conflict checks run before approval. `LESSONS.md` is folded into this format, and the MCP serves both levels. Maintainer decision, 2026-11-01 |
| M-3 | Recurring errors | `hoi4.error_log` groups errors repeated across launches into signatures, counts them, and links each to the matching documentation and any approved rule. Maintainer decision, 2026-11-01 |
| M-4 | No redundant safety checks | The tools are accurate enough that extra guards such as an edit-size limit are not needed, and checks that slow the tools without clear value are not added. The existing drift guards stay, because they stop concurrent edits by people and agents from overwriting each other, which FR-5 needs. Maintainer decision, 2026-11-01 |
| M-5 | Generated tool documentation | Tool documentation is generated from the tool definitions. CI fails on drift, duplicate names or an undocumented tool. Maintainer decision, 2026-11-01 |
| M-6 | Benchmark mods | The benchmark set is vanilla, Chaos Redux, ZIN and Ireland Total Overhaul, plus two or three of the largest public Workshop mods (Kaiserreich, Old World Blues, The New Order) pinned at fixed versions. They run locally, because their files cannot be shipped. Maintainer decision, 2026-11-01 |
| M-7 | Role of `cpntodd/HOI4-MCP` | It is not a base for anything: its parser loses information, several of its HOI4 facts are wrong, it has no renderers, its write tool is unsafe and most of its files are copies of the kit. Its ideas may be adapted and improved. A one-time head-to-head run in an isolated container (pinned commit, read-only mod copies, no network) is the Q-1 proof against it. Ongoing benchmarks measure against our own stored baselines. Maintainer decision, 2026-11-01 |
| M-8 | Rust move order | Refines D-07. First ship the engine binaries. Then move the shared symbol index (`hoi4.mod_index`, then `hoi4.source_lookup`), add the whole-mod reference report to `hoi4.script_validate`, move the map hot paths and then the remaining analyzers, and finally retire the TypeScript index. The ZIN MCP worker owns this sequence. Maintainer decision, 2026-11-01 |
| M-9 | Coverage | As much of HOI4 modding as possible is covered, reliably. Each area gets an inspect tool, plus render and a safe rewrite where they help, in this order. Localisation in every language comes first, then ideas and characters, then on_actions, military industrial organisations, balance of power, special projects, operations and raids, and then division templates, order of battle and equipment, plus GFX and sound validation. Maintainer decision, 2026-11-01 |
| M-10 | Lookup vocabulary | The lookup vocabulary comes mostly from the game's own localisation, plus a short hand-checked list. A measured experiment with a small local embedding model is also approved, because the remaining misses are meaning gaps that the documentation never phrases the way players do, such as "access to the sea" for `is_coastal` or "share a border" for `is_neighbor_of`. The model is an optional layer, pinned by hash and fetched only when switched on, and word ranking stays the fallback. Word-only and word-plus-model ranking are scored once on an untouched blind test set, and the model is kept only if it clearly wins without hurting exact-name or negative queries. This fits ADR-009, which allows vectors once a measured recall gap remains after simpler improvements, and applies to hoi4-agent-tools reference search only. Maintainer decision, 2026-11-01, amended the same day |
| M-11 | Writers | Event, decision and technology rewrite tools arrive together with their visual editors, in stage-plan order. The same operations serve agents and FR-5 two-way editing. Maintainer decision, 2026-11-01 |
| M-12 | Real-game checks | A nightly benchmark runs on whichever maintainer machine has the game, over vanilla and the pinned mods. Results stay local with their hashes, and any regression is flagged. Maintainer decision, 2026-11-01 |
| M-13 | macOS CI | A macOS arm64 job joins the hoi4-agent-tools Node CI. Maintainer decision, 2026-11-01 |
| M-14 | Chaos Redux tools | `chaosx.visual_revision` and `chaosx.focus_country_assets` move out of the public repository into a private plugin that the server loads only when it is present. Maintainer decision, 2026-11-01 |
| M-15 | HOI4-MCP licensing | No action is needed, because nothing of it is used. Maintainer decision, 2026-11-01 |

## Plan approval (2026-10-31)

| ID | Question | Decision |
|---|---|---|
| E1 | Approve the implementation plan (Phase D) as written, starting with W1 kit sync automation | **Approved** ("Approve plan"). Work follows the roadmap order. W1 runs in the planning session directly in the kit checkout. W3 and W4 were offered to the maintainer as separate sessions because they live in hoi4-agent-tools and depend on nothing in W1 |
