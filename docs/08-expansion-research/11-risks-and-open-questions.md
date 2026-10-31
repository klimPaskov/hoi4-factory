# Risks and open questions

Status: **Proposed for approval**. Risks are grouped by the four concerns the request names: licensing, maintenance, compatibility and architectural complexity, plus cost and security. Open questions that need the maintainer are listed last, so they can be answered in one batch.

## Licensing and rights

| Risk | Mitigation |
|---|---|
| The Hagane permission is written but its exact scope (relicensing into Apache-2.0 tools, the Hagane name and icon) is held privately | Before code moves, the maintainer confirms that the permission covers relicensing and branding. Each adapted file carries a provenance note. Screenshots with game map data, game-derived runtime paths and avatars of unknown origin are never moved |
| Vanilla art sent to image providers (D4) is a transfer of Paradox material, and in hosted HOI4 Factory it amends ADR-015 | Off by default, opt-in per project, recorded in the asset ledger, and blocked in hosted use until the game-rights review that already gates public exposure is complete |
| Rules for AI-generated assets on Steam Workshop and in Paradox's terms, and the output terms of some providers, were not researched | A short rights research task before `hoi4-imagegen-mcp` is promoted for public use. Documentation promises nothing until then |
| The kit's vanilla reference image shelf ships Paradox-derived images inside an Apache-2.0 repository | Raised as a separate question. It exists independently of this plan |
| The ChatGPT-login plugin uses an undocumented backend | Local-only, off by default, never in hosted builds, documented risk, CI check on default builds (D-12) |
| Paradox wiki text is edited in the snapshot | The wiki worker keeps the wiki's license notice and attribution |

## Maintenance

| Risk | Mitigation |
|---|---|
| The Rust engine is a large new component in a TypeScript repository | Tools move one at a time behind the same MCP names, and the existing tests are the regression suite. The TypeScript index is retired only when every route has moved |
| CWTools MD Edition changes very quickly | Integration only if the spike passes, pinned by SHA, behind the engine's own interface, never the write path |
| Image models retire often (three retirements in eight months) | Model tables are data with dates, a drift check runs on a schedule, skills never name model IDs |
| Kit guidance sits at its size caps | Slimming by deduplication and moves only, with a diff audit (P11), and the upfront rule as a short stub plus a reference file |
| Generated Chaos Redux skills can surprise someone who edits the generated copy | Deny rules and the hook refuse such edits and name the source file |
| Hagane Studio becomes an orphan if the wind-down stalls | Archive conditions are written down (A1) |

## Compatibility

| Risk | Mitigation |
|---|---|
| A new game build changes effects, triggers or modifiers | Vocabulary is generated from the installed game's `documentation/` files, and the wiki refresh flags affected pages |
| The connector and footprint model for focus layout is inferred from one screenshot | Orthogonal metrics stay advisory until in-game screenshots confirm the model |
| Setup's allow-lists and the kit's registry drift apart | A shared test in the Setup repository reads the kit registry |
| Hooks and tooling behave differently on Windows and macOS | Every script is POSIX `sh` or Python through a launcher, and is tested on both machines |
| The DX10 DDS headers written by `image_dds` may not load in HOI4 | Legacy headers until an in-game acceptance test passes |

## Architectural complexity

| Risk | Mitigation |
|---|---|
| Two implementations of one responsibility during transitions (TypeScript and Rust index, Python and Rust image QA) | Parity tests and shared finding codes during the transition, and a written retirement step for each old implementation |
| Too many new pieces at once | D-13 limits the first phase to workflow updates. HOI4 Factory features wait for their stages. The jobs board starts minimal (B2). Drift Bot is not built (B3) |
| The autonomy policy grows into a configuration maze | Three presets, six checkpoint kinds, three values, narrower rules only where a project asks |

## Cost and security

| Risk | Mitigation |
|---|---|
| Hosting costs outrun funding, as they did for Hagane Works | Heavy work stays local, model and image use are metered with per-task and per-account budgets, operator-funded images use a cheap-tier allowlist with a default per-task cap of 0.25 USD, and background model jobs are opt-in and capped |
| The allowance now also covers images (D-11) | The cost dashboard shows image spend separately. The allowlist and cap are configuration that the operator can tighten |
| A live-looking API key was found in a prompt note of a private repository during research | Reported to the maintainer for rotation. It was not copied anywhere |
| The local companion adds a new trust boundary | Outbound-only connection, pairing per project, writes limited to named folders, its own security review before it ships |

## Open questions for the maintainer

These do not block approval of the plan. Each has a default that applies if it is not answered.

| Question | Default if unanswered |
|---|---|
| Does the written Hagane permission cover relicensing into Apache-2.0 tools and use of the Hagane name and icon? | Port only into AGPL HOI4 Factory, and write new code from the rule specs for Apache-2.0 tools, until confirmed |
| Canvas sizes for image profiles: vanilla-measured (focus 100x88, idea 65x67, decision 33x32) or the kit's current conventions (94x86, 64x64, 32x32)? | Vanilla-measured, overridable per project |
| May the reconstruction of the supplied focus tree be published as a test fixture? | No, it stays in the git-ignored work folder |
| Will you provide in-game screenshots of a multi-row connector, close same-row focuses, an exclusive pair and an OR group? | Orthogonal metrics stay advisory |
| Will you rate 15 to 20 focus trees blind for calibration? | Limits stay at vanilla percentiles |
| Which provider keys with balance will exist for real recordings (OpenAI API, Gemini, fal, a ComfyUI or RunPod endpoint)? | Real-provider tests are reported as unavailable |
| May a ruleset require the Validate job on the kit's `main`? | CI reports but does not block |
| When will the uncommitted edits in the Chaos Redux tree be committed or set aside, so W6 can start? | W6 waits |
| Should the upfront rule later be backported into Chaos Redux `CLAUDE.md`? | Not in this phase |
| Distribution of the imagegen binary: a Setup release-binary component or npm platform packages? | Setup release-binary component |
| Should a short rights research task (Paradox and Steam rules on AI assets, provider output terms) run before the imagegen MCP is promoted? | Yes, before public promotion |
