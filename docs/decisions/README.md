# Decisions in force

Start here to find what the maintainer has decided. Each file below covers one project or area. It opens with the decisions in force, one line each with a link to the full entry, then holds the new decisions in full and lists the replaced and expired ones apart. This layout was chosen on 2026-11-08 ([D-35](operations.md#decisions)).

| File | Covers |
|---|---|
| [Operations](operations.md) | How the work is run: approvals and autonomy, game launches, sessions and coordination, usage, releases and promotion, money and keys, private material, issues, machines and priorities |
| [HOI4 Factory](factory.md) | The hosted product, its architecture and its stages |
| [HOI4 Agent Tools](agent-tools.md) | The MCP server and the shared Rust engine |
| [The kit](kit.md) | The Agentic HOI4 Modding kit, its skills and its wiki |
| [HOI4 Mod Setup](setup.md) | The installer app |
| [HOI4 Live Tools](live-tools.md) | In-game testing |
| [hoi4-imagegen](imagegen.md) | Image generation for HOI4 assets |
| [3D route](3d.md) | The kit's 3D route and 3D production |
| [Mods](mods.md) | Decisions recorded here that govern Chaos Redux and ZIN. Each mod keeps its own internal decisions in its own repository |

## How to record a decision

1. Write it in the file of the project it governs, in the Decisions table, with the next free ID. IDs form one global sequence (`D-33`, `D-34` and so on) across all files, so search every file for the highest ID first. Older prefixes such as `3D-`, `W5-`, `M-` and `R-` stay valid for older entries.
2. Write the decision as a clear and complete statement of the maintainer's intent in your own words, not a quote. Give the date, the question it answers, and say whether it is a maintainer decision or a technical default an agent chose under D-18.
3. Add a one-line row to that file's In force table. A decision that governs several projects is written in full in [operations](operations.md), with a one-line row in each affected file.
4. When a decision replaces or amends an older one, update the older one's In force row so it describes the rule as it stands now, or move it to Replaced or expired with the new ID.
5. When a dated window or deadline has passed, move its row to Replaced or expired.
6. Keep private material out: the maintainer's own ratings and review verdicts, private repository content (D-01) and personal preferences that are not project rules.

Then update CLAUDE.md or a skill if the decision changes how agents work, and run the package validator.

## History records

These hold the full text of every decision made before 2026-11-08. They are not rewritten. The In force tables point into them.

- [Expansion research decision record](../08-expansion-research/00-decision-record.md), 2026-10-31 to 2026-11-08.
- [Maintainer decisions](../02-architecture/maintainer-decisions-2026-10-24.md), ADR-011 to ADR-028 and the stage answers.
- [Options and decisions](../02-architecture/options-and-decisions.md), ADR-001 to ADR-010.
- [UI design decisions](../design/DESIGN-DECISIONS.md), the page designs the maintainer chose.

## Plans

The current plan is [next steps, November 2026](../09-planning/NEXT-STEPS-2026-11.md).
