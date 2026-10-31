# Ecosystem requirements FR-1 to FR-4

Status: **Proposed for approval**. These four areas are established maintainer requirements, recorded in the [decision record](00-decision-record.md#established-follow-up-requirements-2026-10-31). They are kept apart from the optional Hagane Works ideas. Only the way to meet them went through rounds (D-09 to D-12 and round D). Following D-13, each design stays as simple as the requirement allows, and kit work happens directly in the kit checkout. Detailed findings are in the research appendices [12](research/12-imagegen-mcp.txt), [13](research/13-focus-tree-quality.txt) and [14](research/14-kit-sync-and-questions.txt).

## FR-1 Upfront questions rule

**Requirement.** At the start of a substantial task the agent inspects the project first, sorts every open point into established requirements, decisions it can make itself and genuine questions, asks the genuine questions once in an organized consultation with suggested answers, records the answers in the specs, and then continues without avoidable interruptions. Routine implementation choices are never asked. The rule respects the autonomy levels and the idea-selection rounds.

**Findings.** The kit's rule at `CLAUDE_template.md:365`, copied verbatim into Chaos Redux `CLAUDE.md:328`, tells agents to ask a focused question only when something indispensable is missing. That is reactive and mid-task. The fallback-approval rule at `:357` and the world-iteration stop at `:268-270` also stall autonomous runs, and several skills (super-events, frame animation, feature assets, save inspection) ask late questions that could be settled at intake. The template has 55 bytes of headroom under its 60,000 byte cap.

**Design.**
1. One short canonical subsection, "Upfront consultation", in `CLAUDE_template.md` after "Specs and Plans", about 500 bytes. It replaces the sentence at `:365`. The detail lives in a new reference file `hoi4-feature-planning/references/consultation-questions.md`, so the template stays under its cap without waiting for the P11 slimming.
2. Content of the rule:
   - **When:** a task that creates or reshapes a feature, system, country, route set or asset set, spans three or more surfaces, or has no accepted spec. Bug fixes and follow-ups inside an accepted spec skip it.
   - **Inspect first:** project instructions, specs, the decisions record and the files the task touches. Never ask what a file can answer.
   - **Sort:** established, agent's own decision (including every routine choice), genuine question.
   - **Ask once:** one message grouped by surface, each question with a suggested answer and a one-line reason, plus the list of decisions the agent will take, which the user may overrule. Large tasks announce later batches up front.
   - **Record:** answers and defaults go to `docs/specs/<feature_slug>/decisions.md` with origin (user, established, agent default), using the P13 tags.
   - **Continue:** a later question is allowed only when it was not foreseeable or hits a hard gate (budget, a permission-gated action, wider scope). It is asked once, batched, with a default.
   - **Never ask** about naming, file layout inside the established layout, localisation keys, code factoring, tool choice, ordering, check selection or retries, and never for logs or in-game testing.
   - **Subagents** have no user channel. They put open items in their handoff.
3. Skills get one-line pointers instead of restating the rule: `hoi4-feature-planning`, `hoi4-improvement-loop` (one round per tranche, Adopt, Adapt, Defer and Reject mapped onto the six existing dispositions), `hoi4-subagents`, and the five skills with late asks.
4. Projections pick it up through the sync tools (FR-4): `AGENTS_template.md`, then `AGENTS.md` after install, and the skill mirrors.
5. HOI4 Factory implements the same rule in its own runtime policy, because a project's `AGENTS.md` is only labelled data there (A4). The intake consultation is the first checkpoint kind in the [autonomy specification](04-autonomy-modes.md).
6. Fully autonomous jobs ask once at intake. If no human is reachable, the agent applies its suggested answers, tags them as agent defaults and lists them in the completion notice.

**Defaults to approve.** The stub plus reference layout, `decisions.md` as the record, the fallback policy (stop and report, or substitute and report) as a standard intake question with "stop and report" as the default, and no backport to Chaos Redux `CLAUDE.md` in this phase (it is at its own size cap). Chaos Redux can adopt it in a later change.

**Acceptance.** The template stays under 60,000 bytes. `sync_instructions.py --check` and the guidance size check pass. A planning run on a sample feature produces one consultation and a `decisions.md`, and asks no routine questions. This last check needs a model run and is recorded as unavailable until one is made.

## FR-2 hoi4-imagegen-mcp

**Requirement.** A properly attributed derivative of [Connected-Mate/gptimage](https://github.com/Connected-Mate/gptimage) (MIT), named `hoi4-imagegen-mcp`, specialised for HOI4 graphical assets. It must use documented, authorized image APIs, load category conventions from verified configuration, support reference-guided generation and editing, processing, transparency, deterministic validation, naming, placement, preview, approval, versioned outputs and provenance. It integrates with the kit and HOI4 Factory without duplicating existing image infrastructure.

**Findings.**
- **Upstream:** 1,420 lines of JavaScript, 8 commits by one author, no tests. Its authentication impersonates the OpenAI Codex CLI against an undocumented ChatGPT endpoint and writes refreshed tokens into `~/.codex/auth.json`. Its saves have no project containment and its overwrite check is racy. Worth keeping: the tool shape, the reference-first workflow and automatic `-vN` versioning.
- **Kit:** already vendors gptimage almost verbatim as its default image provider, and already ships a hardened Python provider runtime with about 20 routes, project-contained saves and no overwrites. What is missing is the HOI4 layer: category profiles, processing, DDS export, sprite registration, validation and a provenance ledger.
- **Game:** measured from 1.19.3.0 headers: 95.8 percent of 15,126 DDS files are uncompressed A8R8G8B8 and 93 percent have one mip level, which confirms the kit's DDS rule. Several kit canvas conventions differ from vanilla: focus icons are 100x88 in 94.1 percent of files (kit 94x86), decision icons mostly 33x32 (kit 32x32), idea pictures 65x67 or 60x68 (kit 64x64). Vanilla pairs every focus icon with a `_shine` sprite, which no kit skill mentions.
- **Providers (vendor docs, 2026-10-31):** OpenAI's current image models are `gpt-image-2.5-sunburst` and `gpt-image-2.5-flare`. `gpt-image-1` was shut down on 2026-10-23 and Imagen is discontinued, so the kit's `vertex_imagen` route points at a dead model. Native transparency is documented only for OpenAI's image models.

**Design.**
- **Repository:** a GitHub fork of the upstream into `klimPaskov/hoi4-imagegen-mcp`, public, MIT, with the upstream notices kept (D-09). The fork keeps upstream history, then the code is rewritten in Rust (D-07) with an `UPSTREAM.md` mapping each upstream file to its replacement.
- **Crates:**
  - `imagegen-core`: categories, profiles, the processing pipeline, validation, DDS and TGA writers, naming, ledger. No network.
  - `imagegen-providers`: OpenAI, Gemini, ComfyUI and fal in phase one, Stability and BFL later.
  - `imagegen-mcp`: an `rmcp` stdio server.
  - `imagegen-cli`: doctor, calibrate, plan, generate, validate and install.
  - `provider-chatgpt-login`: a separate crate that is not a default member. It is enabled only by a feature flag, a config flag, an environment acknowledgement and local stdio mode, and CI asserts it never appears in default or HOI4 Factory builds (D-12).
- **One profile source:** the kit's `.tools/gfx/profiles.json` stays the owner of canvas sizes, margins, frames and evidence. Each row gains an optional `imagegen` object for export and naming fields. New rows cover focus icons with their shine sprites, decisions, ideas, technologies, military industrial organisations, agencies and super-events. Default canvas sizes come from the vanilla measurements and are overridable per project. That is a default to approve, because it changes the kit's focus icon convention from 94x86 to 100x88.
- **Thirteen tools** named `hoi4.image_*`: status, profiles, references, plan (dry run with cost estimate), generate, edit, process, validate, preview, review, install, ledger, cancel. Install refuses unless validation passed, review approved and hashes are unchanged. Outputs are versioned and never overwritten. A ledger records which references, settings and outputs belong together.
- **Deterministic and visual checks are separate.** Dimensions, alpha, margins, frames, ghosting, header fields and sprite registration are deterministic. Composition, style consistency and readability at game size go to visual review.

**Kit integration.**
- **Install:** through HOI4 Mod Setup as a pinned binary component, verified by SHA-256.
- **Clients:** MCP entries for all supported clients with no secrets.
- **Skill:** a `hoi4-imagegen` skill under 8 KB, with references and examples (five focus icons with shine sprites, a national spirit set, a report and a news picture, a flag set, a refinement edit).
- **Transition:** the Python routes and the vendored Node copy stay until the new MCP covers OpenAI, Gemini, ComfyUI and fal. Then the catalog default switches and the dead routes are removed, so there is no lasting duplicate.

**HOI4 Factory integration** (design only in this phase, D-13).
- **Adapter:** Factory links `imagegen-core` and `imagegen-providers` as an audited built-in adapter, with no subprocess.
- **Broker:** an "assets" tool profile, a `generate_image` grant class with provider and model allow-lists and a cost ceiling, and installs as ordinary change proposals.
- **Funding:** user keys or the monthly allowance (D-11). Operator-funded calls are limited to a cheap-tier allowlist with a default per-task cap of 0.25 USD. One finished asset costs between about 0.015 and 1.06 USD depending on provider and tier.

**Maintainer policies applied.**
- **D3:** agents may call a ComfyUI endpoint the maintainer configured for any asset, including portraits of real people. Chaos Redux keeps its own asset policy as an overlay.
- **D4:** vanilla art may be sent as a reference only when a project opts in, and it is off by default.

**Acceptance.** The test layers from the research: unit and parity tests against the kit's Python DDS writer, provider contract tests with recorded real responses, the pipeline into a temporary mod, the MCP protocol over stdio, real-provider end-to-end runs, a user-operated in-game check including the shine sprite question, cross-platform runs, plugin isolation and license checks, and a model-retirement drift check. Real-provider recordings are unavailable until provider keys with balance exist.

## FR-3 Focus tree quality in hoi4-agent-tools

**Requirement.** Find out why agent-made focus trees look poor although structural checks pass. Separate structural, layout and design quality. Improve the agent workflow and the MCP's diagnostics, scoring, comparison and partial layout, and test on real trees. No separate focus MCP. Deterministic tools arrange and evaluate geometry and never invent content.

**Findings on the supplied tree** (reconstructed from the screenshot, accurate to about half a column, source unknown per D-10):
- **Drift:** the main line steps left by 12 columns over six generations.
- **Lopsided:** one branch holds 10 of 58 focuses and floats top right.
- **Empty space:** the largest empty rectangle is 42.5 percent of the canvas, against a vanilla maximum of 30 percent for trees of this size.
- **Imbalance:** side balance 0.64 (vanilla maximum 0.34) and mass skew 0.40 (vanilla maximum 0.22).
- **Long chain:** seven single-successor focuses in a row (vanilla maximum 4).

**Why the current tools let it through.**
- **Symmetry checks skipped:** sibling symmetry checks run only for automatic positions, so fixed coordinates report zero cohorts.
- **Balance check too loose:** the only global balance check is bounding-box centring, which this tree passes.
- **Wrong connector model:** inspection returns `ok` with 14 detour and 20 crossing warnings, and most crossings are artefacts. The tool's curved connector model reports 766 crossings on 54 vanilla trees, where an orthogonal in-game model finds 86.
- **Repair path refused:** the documented compact repair was refused on all six real trees tried, taking 21 to 70 seconds each.
- **No image shown:** renders return links, not inline images.

**Root causes, ranked.**
1. Coordinates were almost certainly hand-typed. The tool's own solver with automatic positions gives width 18 instead of 27, side balance 0.19 and empty rectangle 0.23 in under a quarter of a second.
2. No lane plan exists before placement, and the kit's layout rules are slogans without numbers.
3. Nothing gates completion on layout. The focus auditor is a Haiku agent and no viewed image is required.
4. Part of the imbalance is in the design itself, since one subtree holds 46 of 58 focuses.

**Design.**
- **Rubric:**
  - **Hard defects:** duplicate coordinates, parent not above child, in-game overlap, connector through a plaque.
  - **Soft metrics, calibrated on all 67 vanilla 1.19.3.0 trees in two size buckets:** largest empty rectangle, side balance, mass skew, fill, row-centroid wander, linear detours, branch interleaving, cohort centring, vertical slack.
  - **Design flags, always for review:** long single-successor chains, leaf fraction, exclusive choices that fully reconverge.
- **Verdict in every inspect and rewrite response:** `blocked`, `revise`, `review` or `pass`, with a scorecard, fix hints and a layout delta. "Structure ok" can no longer read as done.
  - **Strictness (D2):** `blocked` and `revise` block the commit at every autonomy level, with a recorded override. `review` is advisory and goes to visual review.
- **Placement (D1):** agent-made trees are placed by the solver per branch lane. The source uses `relative_position_id` anchors per branch, as vanilla does.
- **TypeScript now:** metrics, verdict, scorecard, calibration file, inline images, a bundle-aware compact gate, an orthogonal connector model, fix hints and clearer tool descriptions.
- **Rust engine later (C1):** the real layout algorithm, partial relayout and new-branch insertion into free corridors that keep untouched coordinates identical, shared geometry for render and metrics, and vanilla `offset` blocks. The current compact solver is too slow on real trees, which the Rust engine also fixes.
- **Kit workflow**, from establishing the design to in-game verification:
  1. Establish the design (intake and brief).
  2. Write a lane plan.
  3. Define prerequisites, choices and convergence.
  4. Lay out with the solver.
  5. Render inline.
  6. Read the scorecard and answer fixed visual questions.
  7. Revise in bounded iterations.
  8. Validate structure and gameplay.
  9. Get approval at the visual review checkpoint.
  10. Verify in game.

  The slogan list in the focus skill's layout rules becomes the rubric. The visual judgement moves from the Haiku auditor to a stronger model or the human checkpoint, and Haiku keeps the mechanical tables.

**Acceptance.**
- **Metrics:** unit vectors with hand-computed values and invariance tests.
- **Calibration:** a calibration file recomputed from the installed game.
- **Supplied tree:** used as a fixture once the maintainer agrees to publish the reconstruction. It moves from today's `ok` to `revise`, and to `pass` after solver placement.
- **Real trees:** untouched coordinates stay identical across repairs and insertions.
- **Before and after:** agent runs with the old and new workflow are compared.
- **Human calibration:** blind human ratings of 15 to 20 trees calibrate the limits.

Maintainer inputs requested: in-game screenshots of a multi-row connector, close same-row focuses, an exclusive pair and an OR group to confirm the connector model, and the blind ratings.

## FR-4 Workflow synchronization

**Requirement.** Changes to canonical instructions, skills or agent definitions can never leave generated runtime projections silently outdated. Execution is targeted and dependency-aware, CI detects stale projections, Setup and runtime configurations stay compatible, and failures are visible. The seven acceptance criteria from the request apply.

**Findings.**
- **Nothing runs automatically:** no Claude Code, Codex or git hooks and no pre-commit configuration. The triggers are prose instructions, `check.py`, two CI workflows and Setup's finishing step.
- **Vacuous skill check:** `sync_skills.py --check` passes with "No skill mirrors exist" in the source repository and in CI, because the mirrors are git-ignored.
- **Hand edits overwritten silently:** a hand edit to a generated file looks like canonical drift and is overwritten by the next sync.
- **Missing generators skipped:** `check.py` skips a generator whose script is missing.
- **Setup failures ignored:** `check.py setup` ignores a failing synchronizer in its exit status.
- **No branch protection:** `main` is not protected, so CI results block nothing.
- **Chaos Redux drift:** its Codex configuration has 20 agent rows for 23 agents, a live stale projection.
- **Criteria today:** none of the seven is fully met.

**Design (the smallest set that meets all seven).**
1. **Registry:** one declarative file, `.tools/sync/generators.json`, listing each generator and check with its inputs, outputs and order. The Chaos Redux engine-skill generator from P10 is one more entry.
2. **Shared runner:** one runner in the shared `.tools/lib/` used by `check.py`, Setup and the hook. It maps changed paths to generators and runs only those.
3. **Hook:** one Claude Code `PostToolUse` hook on edit and write. An edited canonical file regenerates its outputs at once and tells the agent to commit them together. An edited output is refused with "edit the source instead". Any other file starts nothing. The hook finds the repository from the edited path, so it works inside worktrees and subagents.
4. **Deny rules:** permission deny rules in the shipped `.claude/settings.json` for generated paths.
5. **Tool fixes:** `--root` and `--skill` options, a source-versus-installed mode for the skill mirror check with a real round-trip test, Chaos Redux's stale-replacement check ported into the kit's instruction sync, and a nonzero exit whenever a synchronizer fails.
6. **CI:** one unfiltered CI step running the registry in check mode. The maintainer adds a ruleset that requires it on `main`.
7. **Release gate:** the release script also runs the registry in check mode.
8. **Optional pre-commit hook:** written by `check.py setup --git-hook` for people and non-Claude clients, off by default, never via `core.hooksPath` (Setup rejects it).
9. **HOI4 Factory:** runs a pinned copy of the runner in check mode at job completion for projects that carry the kit (design only in this phase).
10. **Launcher:** a small `.tools/py.sh` that finds a working Python, because this machine's default `python` is a Store stub.

**Defaults to approve.**
- **Behaviour:** the hook regenerates rather than only flagging.
- **Launcher:** add `.tools/py.sh`.
- **Ownership:** the kit owns the sync tools and Chaos Redux adopts them (P10).
- **Setup:** a shared test keeps Setup's allow-list and the registry in step, and format-6 clients enter the registry as data.
- **Provenance:** generated Chaos Redux skills carry a provenance comment.

**Acceptance.** One demonstration per criterion:
1. Edit the template and see `AGENTS.md` regenerate or get flagged.
2. Edit a skill and see its mirrors.
3. Add, modify and remove an agent and see its projections.
4. Edit a mod file and see no generator start.
5. Push a stale projection and see CI fail.
6. Run a Setup install and update round trip on a fixture mod.
7. Break a generator and see a visible failure.

All of these run on Windows and macOS.
