# Expansion research and planning package

Status: **Plan approved by the maintainer on 2026-10-31**, prepared the same day. This package researches and plans the expansion of HOI4 Factory into the central environment of the maintainer's HOI4 ecosystem, together with improvements to the Agentic HOI4 Modding Kit, Chaos Redux, hoi4-agent-tools and a new `hoi4-imagegen-mcp`. No repository has been changed by it apart from this folder and the decision records. Implementation (Phase D) began after that approval, in the order of the [roadmap](09-implementation-roadmap.md).

## How the work was done

1. **Phase A, research.** Fourteen research reports read the actual source of HOI4 Factory, the kit, Chaos Redux, hoi4-agent-tools, HOI4 Live Tools, HOI4 Mod Setup, the full Hagane Works skills repository, Hagane Studio, gptimage and about sixty open-source projects, at pinned revisions.
2. **Phase B, consultation.** The maintainer decided 14 Hagane-inspired proposals and 4 rejections in five rounds, then the product and requirement choices in four more rounds. Every answer is recorded.
3. **Phase C, planning.** The decisions were combined with the four established follow-up requirements (FR-1 to FR-4) and the existing architecture into the documents below.

## Deliverables

| # | Deliverable | Document |
|---|---|---|
| 1 | Current-state audit of HOI4 Factory and related projects | [01-current-state-audit.md](01-current-state-audit.md) |
| 2 | Open-source research with licenses and integrate, adapt or reject verdicts | [02-open-source-research.md](02-open-source-research.md) |
| 3 | Hagane Works analysis with source references and proposals | [03-hagane-works-analysis.md](03-hagane-works-analysis.md) |
| 4 | Decision record: accepted, adapted, deferred and rejected ideas | [00-decision-record.md](00-decision-record.md) |
| 5 | Autonomy levels specification | [04-autonomy-modes.md](04-autonomy-modes.md) |
| 6 | Feature specifications: notes, jobs, Git, onboarding, graphics, and why there is no Drift Bot | [05-feature-specifications.md](05-feature-specifications.md) |
| 6a | Established requirements FR-1 upfront questions, FR-2 `hoi4-imagegen-mcp`, FR-3 focus tree quality, FR-4 workflow synchronization | [06-ecosystem-requirements.md](06-ecosystem-requirements.md) |
| 7 | Architecture proposal | [07-architecture.md](07-architecture.md) |
| 8 | UX proposal | [08-ux-proposal.md](08-ux-proposal.md) |
| 9 | Implementation roadmap with work packages, dependencies, acceptance criteria and Git considerations | [09-implementation-roadmap.md](09-implementation-roadmap.md) |
| 10 | Testing strategy | [10-testing-strategy.md](10-testing-strategy.md) |
| 11 | Risks and open questions | [11-risks-and-open-questions.md](11-risks-and-open-questions.md) |
| | Research appendices | [research/README.md](research/README.md) |

## The plan in brief

- **First phase (D-13):** research and workflow updates only, kept simple.
  - **Kit:** synchronization made automatic and enforced (FR-4), then the upfront questions rule (FR-1), careful slimming that keeps every instruction (P11), a small static investigation skill (P5, P6), verified traps with evidence tags and a lessons ledger (P8, P9) and a feasibility step (P14).
  - **hoi4-agent-tools:** gains a focus layout verdict calibrated on vanilla trees (FR-3), and hosts the new shared Rust HOI4 engine (D-07, C1) with reference closure, generated lints, an override audit and a findings baseline (P1 to P4).
  - **`hoi4-imagegen-mcp`:** forked from gptimage and rewritten in Rust with documented providers (FR-2).
  - **Chaos Redux:** adopts generated engine skills, spec item IDs and test plan sections (P10, P13, P7).
- **HOI4 Factory:** gets designs, not code, in this phase: autonomy presets over one checkpoint policy with optional auto-merge (A2), Obsidian-compatible notes (B1), a simple jobs board (B2), Git save points, tours, the asset pipeline and a full local companion (C4). Each lands in its stage once that stage is authorized.
- **Not built:** a Drift Bot (B3), a second tracker, a second copy of HOI4 knowledge, a separate focus MCP, or any hosted use of the ChatGPT login.

## Related records

- Maintainer decisions that change product scope are also recorded in the [maintainer decision record](../02-architecture/maintainer-decisions-2026-10-24.md#expansion-research-decisions-2026-10-31).
- A separate session is overhauling the kit's offline wiki snapshot (D-08).
