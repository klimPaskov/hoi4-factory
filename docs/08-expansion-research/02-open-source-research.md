# Open-source research

Status: **Research record, 2026-10-31**. This document compares the open-source projects the maintainer named and the ones the research found, by responsibility. Each project gets one verdict: **Integrate** (take it as a dependency), **Adapt** (reimplement a named idea) or **Reject**. The rule throughout is one dependency per responsibility, so overlapping frameworks are never combined. Commit SHAs, licenses and depth of reading for every source are in the research appendices: [orchestration, jobs and drift](research/08-oss-orchestration-jobs-drift.txt), [graphics, editors and modding tools](research/09-oss-graphics-editors-modding.txt) and [knowledge, onboarding and Git](research/10-oss-knowledge-onboarding-git.txt). Hagane Works is covered separately in the [Hagane Works analysis](03-hagane-works-analysis.md).

The maintainer's Rust preference (D-07) applies to every recommendation: new backend and tool code is Rust where practical, browser code stays React and TypeScript.

## Agent orchestration, jobs and parallel work

| Project | What it actually is | License | Verdict and the specific idea taken |
|---|---|---|---|
| [Beads](https://github.com/gastownhall/beads) (moved from `steveyegge/beads`), v1.3.2 | Agent issue tracker on a Dolt database with typed dependency edges and ready-work queries | MIT and Apache-2.0 notices | **Adapt.** Typed edges where only blocking kinds affect readiness, cycle rejection at write time, the ready-work rule, compare-and-swap claims, lease heartbeats with reclaim after about twice the TTL. Our lease and fencing design is stronger, so keep it. Do not copy its newest-timestamp-wins sync or hash IDs |
| [Gas Town](https://github.com/gastownhall/gastown) v1.2.1 and [Gas City](https://github.com/gastownhall/gascity) v1.5.1 | Multi-agent controller on tmux roles, and its successor controller | MIT | **Adapt.** Checkpoint fields, human-readable recovery handoff, stranded-work scan, skip after N failures, computed roll-ups. The earlier verdict (adapt invariants only) stands |
| [Blacksmith](https://github.com/ozten/blacksmith) | Single-author Unix-only supervisor around `bd` and the `claude` CLI, 30 commits, last commit 2026-03-05 | MIT | **Reject** as a dependency. **Adapt** three ideas: glob "affected sets" that default to conflicting with everything and are compared with the real diff afterwards, one integration at a time with a bounded fix-attempt breaker, and a full suite every N merges |
| [Pane](https://github.com/greenfield-inc/Pane) | Electron desktop agent workspace | AGPL-3.0 with extra attribution and naming terms | **Reject.** **Adapt** its nine-field handoff note and its check that the receiving agent really resumed |
| [vibe-kanban](https://github.com/BloopAI/vibe-kanban) | Rust and Postgres agent Kanban. The vendor announced a shutdown on 2026-04-10 and the community maintains it | Apache-2.0 | **Adapt.** Its issue schema is the closest model for our jobs data: floating sort order, task to pull request many-to-many. Its migration that renumbered every short ID is a warning to fix the ID scope once |
| [Backlog.md](https://github.com/MrLesk/Backlog.md) | Markdown task tracker for agents | MIT | **Adapt.** Task field list (acceptance checklist, definition of done, plan and notes sections) and its three review points (spec, plan, code) |
| [Plane](https://github.com/makeplane/plane) | Full project tracker | AGPL-3.0 | **Adapt.** Configurable state names mapped to fixed state categories, inverse relation pairs |
| [agtx](https://github.com/fynnfluegge/agtx) | One board with different autonomy policies | Apache-2.0 | **Adapt** the idea that autonomy levels are policies on one board. Avoid its rule that a missing dependency counts as satisfied |
| Claude Squad, container-use, Task Master, Crystal, uzi, Sculptor, OpenHands, Huly, Conductor | Session managers, sandboxes and task CLIs | Various (Task Master has a Commons Clause, Conductor is closed, Huly is frozen) | **Reject.** Each duplicates a responsibility the Rust host already owns or is unmaintained |

## Spec verification and drift

Drift Bot was rejected in decision B3. This section is kept as the research record.

| Project | What it actually is | License | Verdict |
|---|---|---|---|
| [8090 Software Factory Drift Bot](https://docs.8090.ai/codebase/overview) | Closed product. Reviews pull requests against blueprints and requirements, posts one summary comment with inline comments on the most important findings (capped at two since v0.43.0) and sets a pass or fail `drift-bot` status | Closed | **Adapt** the presentation: one summary, few inline anchors, one status. Its detection method, severity, false-positive handling and deviation approval are not documented, so our design fills those gaps |
| [spec-kit](https://github.com/github/spec-kit) | Spec-driven development CLI with `analyze` and `checklist` commands | MIT | **Adapt** the findings table with stable IDs per category and finding caps |
| [OpenSpec](https://github.com/Fission-AI/OpenSpec) | Change proposals with validation | MIT | **Adapt** the CRITICAL, WARNING and SUGGESTION severities |
| [Tracey](https://github.com/bearcove/tracey) | Requirement coverage tracker | Apache-2.0 | **Adapt** the rule that a link to an older requirement version is stale, not covered |
| [ArchUnit](https://github.com/TNG/ArchUnit) freezing and ratchets | Architecture rules with a frozen baseline of known violations | Apache-2.0 | **Adapt** baselines with justified exceptions, which become our deviation records |
| Requirement tools (OpenFastTrace, StrictDoc, Doorstop, sphinx-needs) and contract testers (oasdiff, Schemathesis, Pact) | Traceability and API contract tools | Various | **Reject** as dependencies. Their scope is enterprise traceability or HTTP APIs |

## Git and worktrees

| Option | Finding | Verdict |
|---|---|---|
| The `git` command line | Already used by the workspace crate. Supports everything needed, including `git merge-tree --write-tree` for conflict detection (git 2.38 and later, Debian bookworm ships 2.39.5) and `git apply --cached --unidiff-zero` for hunk and line staging, which GitHub Desktop uses | **Integrate** (already in use) |
| [gitoxide](https://github.com/GitoxideLabs/gitoxide) 0.61 | Its own status file lists checkout, merge, rebase, push and worktree removal as incomplete | **Reject** for now. Revisit when it covers push and merge |
| [git2-rs](https://github.com/rust-lang/git2-rs) | libgit2 bindings, a second Git engine and a C build surface | **Reject** |
| [jj](https://github.com/jj-vcs/jj) | Version control with an operation log and first-class conflicts | **Adapt** the operation log as an undo timeline built on our restore-point refs |
| [GitButler](https://github.com/gitbutlerapp/gitbutler) | Virtual branches and hunk assignment | **Adapt** the undo timeline idea only. Its FSL license bars competing use |
| [Mergiraf](https://codeberg.org/mergiraf/mergiraf) | Structural merge driver | **Defer.** GPL-3.0-only, languages compiled in, no Clausewitz grammar. A merge driver on our own HOI4 parser is the better later option |
| worktrunk, git-worktree-runner, git-absorb | Worktree and fixup helpers | **Adapt** branch rules: record the base SHA, never delete a branch that existed before the task, delete a task branch only when the merged head equals its tip |

## Visual editors and canvases

| Project | License | Verdict |
|---|---|---|
| [@xyflow/react (React Flow)](https://github.com/xyflow/xyflow) 12 | MIT. The paid Pro tier only removes an attribution link | **Integrate** for the focus tree and event chain canvas, after a spike that confirms it works under the strict CSP. Hagane Studio's focus editor already runs on it. Needs maintainer approval as a new runtime dependency |
| [dagre](https://github.com/dagrejs/dagre) 2.0 | MIT | **Integrate later** for event chain layout. Focus trees use the game's own grid and our solver |
| [elkjs](https://github.com/kieler/elkjs) | EPL-2.0 or GPL-3.0-or-later | **Reject** unless dagre proves insufficient |
| [tldraw](https://github.com/tldraw/tldraw) | Custom license that requires a key in production | **Reject** |
| Excalidraw, Rete.js, LiteGraph | MIT | **Reject.** Poor fit for a grid-bound tree, and LiteGraph is stale or merged into a GPL application |
| Konva, PixiJS, Fabric.js | MIT | **Defer** to the map and GUI stages |
| [Graphite](https://github.com/GraphiteEditor/Graphite) | MIT or Apache-2.0 | **Reject** as a dependency now: alpha, crates unpublished, no embedding API, no DDS output. **Adapt** three ideas: an asset recipe as a node graph, a content-addressed resource store, versioned document migrations |
| [OpenDesign](https://github.com/nexu-io/open-design) | Apache-2.0 | **Reject.** It drives third-party coding agents to produce HTML, slides and video, with no game asset code. **Adapt** its style-spec file, rule sheets and lint before accepting a design |

## Image and asset pipeline

| Responsibility | Choice | License | Notes |
|---|---|---|---|
| Raster operations | [image](https://github.com/image-rs/image) crate | MIT or Apache-2.0 | Rust, server and local companion |
| BC compression | [image_dds](https://github.com/ScanMountGoat/image_dds) with ddsfile and intel_tex_2 | MIT | It writes DX10-extension headers for BC1, BC3 and BC7. Whether HOI4 accepts those is unverified, so the pipeline writes legacy DXT1, DXT5 or A8R8G8B8 headers itself with ddsfile until an in-game acceptance test passes. Needs maintainer approval |
| TGA flags | Small in-house writer | Ours | The `image` encoder defaults to RLE and a top-left origin, while HOI4 flags are uncompressed 32-bit with a bottom-left origin |
| Layered sources | Master PNG plus a versioned recipe JSON and a receipt in Git, with OpenRaster as the exchange format for Krita and GIMP | Open format | No new dependency for the first version |
| AI generation | `hoi4-imagegen-mcp` (FR-2) with documented provider APIs and ComfyUI as a separate HTTP service | MIT for our MCP, ComfyUI stays a separate GPL-3.0 process | See [ecosystem requirements](06-ecosystem-requirements.md#fr-2-hoi4-imagegen-mcp) |
| Test oracles only | Pillow, ImageMagick, texconv (Windows), Compressonator (Linux x86_64 only) | Various | Not shipped |

## HOI4 and Paradox modding tools

| Project | What it is | License | Verdict |
|---|---|---|---|
| [CWTools MD Edition](https://github.com/MillenniumDawn/cwtools-vscode) (Rust engine, 17 crates) | HOI4-first rules validator, LSP and CLI with JSON and SARIF output and a `--since <git-ref>` mode. Parser keeps comments and ranges but stores numbers parsed, so it is not byte-lossless | MIT | **Integrate after a bounded spike** as the rules-based validation layer of the Rust engine, pinned by SHA and built from source. Very high churn and a small team, so the engine wraps it behind its own interface. Never the write path. Decided in a product round |
| [cwtools-hoi4-config](https://github.com/cwtools/cwtools-hoi4-config) | Versioned `.cwt` rules for HOI4 | MIT | **Integrate** as rules input |
| [jomini](https://github.com/rakaly/jomini) | Fast Paradox text and save parser in Rust | MIT | **Defer.** Not lossless. Possibly useful for fast read-only scans and saves |
| [RHoiScribe](https://github.com/czxieddan/RHoiScribe) and [hoi4skill](https://github.com/zhangxiaoyu66666/hoi4skill) | Rust agent toolchains for HOI4 with validation, safe edits and asset registration | AGPL-3.0 and GPL-3.0 | **Adapt** ideas only (approval-gated asset generation with a dry run, tri-state validation report, tool-call audit log, event chain lint). They overlap hoi4-agent-tools |
| [hoi4-focus-viewer](https://github.com/1725381449-cell/hoi4-focus-viewer) | Dependency-free focus viewer | MIT | **Adapt** the "why is this focus locked" panel, flag-setter tracing and exclusion preview |
| hoi4-focus-tree-designer | Browser focus editor that preserves unmanaged fields | No license | **Adapt** ideas only |
| [paradox-translation-toolkit](https://github.com/khoeos/paradox-translation-toolkit) | Key-by-key localisation coverage and markup-safe translation | GPL-3.0 | **Adapt** coverage and markup checks for S06 |
| tiger (`hoi4-tiger`), pdx-tools, hoi4treesnap, Nova | HOI4 validator still unfinished, EU4 save tools, stale screenshot tool, binary-only hub | Various | **Reject** |

## Knowledge and notes

| Option | Verdict |
|---|---|
| Obsidian-compatible Markdown: CommonMark and GFM, YAML frontmatter, `[[wikilinks]]`, tags, aliases, unknown syntax preserved, `.canvas` files passed through | **Integrate** as the format. It stays usable outside HOI4 Factory |
| [pulldown-cmark](https://github.com/pulldown-cmark/pulldown-cmark) with its wikilink support, `ammonia` for sanitizing, `serde-saphyr` for frontmatter | **Integrate** on the server. `serde_yaml` is deprecated and `serde_yml` has advisory RUSTSEC-2025-0068 |
| Karpathy's LLM wiki pattern (raw sources, an agent-maintained wiki, a schema file, ingest, query and lint operations, `index.md` and `log.md`) | **Adapt.** Generate `index.md` and `log.md` on the server, because hand-appended copies conflict across parallel worktrees |
| Retrieval with PostgreSQL full-text search, `pg_trgm` and one-hop link expansion | **Integrate.** No embeddings, consistent with ADR-009 |
| Outline (BSL), Logseq, Trilium, Joplin, basic-memory (AGPL applications or Python runtime), qmd (large local models), WYSIWYG editors that rewrite Markdown on save | **Reject** |
| CodeMirror 6 with Markdown and merge packages, lazy loaded | **Optional dependency** for the notes editor and the conflict edit pane, needs maintainer approval |

## Onboarding and tours

| Option | Verdict |
|---|---|
| In-house tour engine on the Popover API and CSS anchor positioning, tours as versioned data keyed to feature flags with `data-tour` targets, seen-state on the server, checklist items verified from server state, a CI test that fails on a missing target or string | **Recommended.** Zero dependencies, CSP-safe, follows the i18n lint |
| [Driver.js](https://github.com/kamranahmedse/driver.js) | **Fallback** if the in-house engine proves costly. MIT, zero dependencies, about 8 KB, has dialog roles, a focus trap and keyboard support |
| Shepherd.js, Intro.js | **Reject.** AGPL or paid commercial licenses |
| Reactour, Onborda, NextStep, OnboardJS, Usertour | **Reject.** Disabled focus lock or hard-coded English, Next.js orientation, release-candidate status, or a separate service |

## Diff and review UI

| Option | Verdict |
|---|---|
| In-house accessible diff table and per-region conflict chooser | **Recommended** |
| react-diff-view | **Fallback** dependency |
| `@codemirror/merge` | Only inside the optional CodeMirror package. It has no three-way mode and its default buttons are mouse-only |
| Monaco, diff2html | **Reject.** Too heavy, or injects HTML strings |
| Image diffs in four modes (two-up, swipe, onion skin, difference blend) as GitHub Desktop does | **Adapt** with CSS only. DDS and TGA are converted to PNG on the server |

## Consolidated dependency picture

| Responsibility | Dependency | Approval |
|---|---|---|
| Jobs, orchestration, autonomy, GitHub mirror | None new. Native Rust and PostgreSQL with the adapted invariants | Not needed |
| Git | The `git` CLI already in use | Not needed |
| Focus and event canvas | `@xyflow/react`, later `@dagrejs/dagre` | Needed |
| Rules-based script validation | CWTools MD Edition engine behind the shared Rust engine's interface, after a spike | Needed (replaces the F# CWTools fallback in SPEC-006) |
| Image pipeline | `image`, `image_dds`, `ddsfile` | Needed for `image_dds` |
| Notes and conflict editing | Optional CodeMirror 6 | Needed |
| Tours and diff UI | None | Not needed |
| Markdown on the server | `pulldown-cmark`, `ammonia`, `serde-saphyr` | Small Rust crates, recorded in the plan |

Approvals are collected in the product rounds of the [decision record](00-decision-record.md).
