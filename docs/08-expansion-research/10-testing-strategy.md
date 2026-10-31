# Testing strategy

Status: **Proposed for approval**. This strategy covers the work packages in the [roadmap](09-implementation-roadmap.md) and the HOI4 Factory designs. It follows the existing [test strategy](../05-verification/TEST-STRATEGY.md) and the evidence rules in CLAUDE.md: real providers for live calls, recorded-real replay labelled as such for faults a live API cannot produce, and an explicit "unavailable" with a reason for anything that could not run.

## Evidence kinds stay separate

| Kind | Example | Never claimed as |
|---|---|---|
| Source parse and static check | Reference closure, lints, layout metrics | Game behaviour |
| Offline render | Focus tree render, event card | What the game draws |
| Offline simulation | Agent-tools scenario and mechanic tests | Observed play |
| User-operated game observation | Test-in-game card results, screenshots, logs from the companion | Automated proof |
| Maintainer approval | Visual review answers, blind ratings | Mechanical correctness |

## Test corpus

| Corpus | Use | Rights |
|---|---|---|
| Vanilla HOI4 1.19.3.0 | Calibration of lints and layout limits, false-alarm checks, header measurements | Read in place, never copied into fixtures |
| Chaos Redux (public repository) | Real event mod: reference closure on the B001 error classes, override audit, generated skills | Public repository, read at a pinned commit |
| ZIN and Ireland Total Overhaul (public repositories) | Large focus trees, many fixed coordinates, existing-mod import and baseline | Public repositories |
| The supplied Soviet focus tree | Layout verdict before and after | Reconstruction published as a fixture only with the maintainer's consent |
| Synthetic fixtures | One valid and one invalid case per rule, parser round trips, DDS and TGA writers | Ours |
| `klimPaskov/hoi4-factory-test-mod` | Every automated GitHub write | The only GitHub write target |

## Realistic workflows

| Workflow | What is tested | Pass condition |
|---|---|---|
| Existing mod import | Setup import of a public mod, preservation report, findings baseline | Every unapproved byte unchanged. The report lists every path. Existing findings do not fail later checks |
| Agent fix of a broken event | `hoi4-investigate` on a fixture where an option calls an undefined scripted effect | The root cause is named with the evidence rung before any edit. Only the missing call is fixed |
| New focus branch, semi-autonomous | Intake, brief, solver layout, render, scorecard, visual review, commit | One consultation, verdict `pass` or a recorded override, untouched focuses identical, approved layout stored |
| Human edits while the agent works | A human proposal and an agent proposal on overlapping and non-overlapping spans | Non-overlapping changes both land. Overlapping ones are rebased or re-proposed, never silently overwritten |
| Approval and mode switch | Switching from fully autonomous to human-in-the-loop mid-job | The next checkpoint asks. Nothing waiting is auto-approved. The grant is unchanged |
| Interruption | Process killed during a write, browser closed, budget reached | Resumption from the last durable checkpoint, no duplicate effect, unknown effects reconciled first |
| Git | Save point, undo, hunk staging, conflict detection, pull request and merge with required checks | Unrelated user changes never staged by an agent. Merges respect branch protection |
| Graphics | Five focus icons from a style profile through plan, generate, validate, review and install | Versions never overwritten, shine sprites registered, ledger complete, cost within the estimate |
| Kit synchronization | The seven FR-4 demonstrations | All seven pass on Windows and macOS |
| Upfront consultation | A planning run on a sample feature | One consultation, a `decisions.md`, no routine questions (needs a live model run) |
| Game testing | Test-in-game card through the companion and HOI4 Live Tools on Windows | Results recorded as user-operated evidence with build and DLC set |

## Per work package

| Package | Required tests |
|---|---|
| W1 Kit sync | Seven criteria demonstrations, round-trip skill mirror test in a temporary root, CI failure on a stale projection, Setup round trip |
| W2 Kit guidance | Sync and size checks, diff audit for P11 proving no instruction was dropped, evidence tag lint for new rules, contradicted Hagane claims absent |
| W3 Focus metrics | Metric unit vectors, invariance and determinism, calibration drift test, supplied tree fixture, the six previously refused trees |
| W4 Rust engine | Byte-identical parse round trips, valid and invalid fixture per rule, zero false alarms on vanilla, B001-class fixture, existing agent-tools tests per moved route, CWTools spike comparison |
| W5 Imagegen | Layers L0 to L9 from [research report 12](research/12-imagegen-mcp.txt): unit and parity, provider contracts with recorded real responses, pipeline, MCP protocol, real-provider runs, in-game check, cross-platform, Factory gates, supply chain and plugin isolation, model-retirement drift |
| W6 Chaos Redux | Generated skills byte-stable, `--check` fails on a hand edit, stale Codex rows fixed, `check.py full` green with the engine checks |
| W7 Focus layout | Determinism across Windows, macOS and Linux, idempotence, untouched-focus hash, branch insertion property fuzz, runtime budget |

## Human review where metrics cannot decide

- **Focus trees:** blind ratings by the maintainer of 15 to 20 trees on a three-point scale (weird, acceptable, good), correlated with the layout verdict to adjust limits.
- **Graphics:** visual review of composition and style consistency at game size, recorded in the asset ledger.
- **Design quality:** branch purpose, choices and pacing are judged at the design brief and visual review checkpoints, never by a metric.

## Agent workflow evaluation

For FR-1, FR-3 and P5, run the same small prompt set with the old and the new guidance on live providers, and record spend. Example prompts: a new 40-focus branch, a repair of the supplied tree, adding a branch to an existing tree, fixing an event that never fires. Measure the verdict distribution, iterations, images actually viewed, questions asked and whether a routine question was asked.

## Platforms

Every script and check runs on Windows 11 (Git Bash and PowerShell) and macOS on Apple Silicon. Server-side and engine tests also run on Linux amd64 and arm64 in Docker. Platform-sensitive changes are re-run on the other machine before they are trusted.
