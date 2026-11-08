# S04 scope

Stage: S04, the focus authoring vertical slice in all three modes. Packet: [stage-04-focus-vertical-slice.md](../../docs/04-implementation-stages/stage-04-focus-vertical-slice.md). Goal prompt: [GOAL-S04](../../docs/06-agent-handoffs/GOAL-S04.md).

Status: **working scope**, written on 2026-11-08 before any S04 code. It is revised in place as packets land, and the handoff records the final disposition.

## Authorization and entry condition

The maintainer authorized S04 on 2026-11-08 on the Windows PC. The decision record holds it as "S04 authorized (2026-11-08)", and `docs/PACKAGE_STATUS.md` says the same. The coordinating session started this session with the stage goal. The entry items still open on that date finish during the stage:

- The engine binary in the Factory server image (T-ENG-01). This session owns it.
- M-14, the rest of M-8 (`source_lookup` and the `script_validate` closure on the engine), the M-6 benchmark baselines and the nightly vanilla run. The Agent Tools session owns them. This session integrates against its releases and never edits hoi4-agent-tools itself.

Development stays on the Windows PC (D-25). Routine merges and pushes need no approval (D-18, D-23). 3D unit production is paused under 3D-11 and is outside S04.

## Requirements and gates

Exit gates 1 to 12 from the packet. Primary requirements: REQ-004 to REQ-008 (three modes, focus editing, preview), REQ-010 and REQ-011 (review and restore), REQ-047 (GitHub publication), plus the FR-5 two-way editor rule and Q-1. Issues `stage:S04`: #5 (artifacts readable by the agent), #6 (the full Agent Tools surface behind the guarded proposal path) and #7 (probed self-hosted endpoints as agent profiles).

## Packets and ownership

| Packet | Paths | Content |
|---|---|---|
| E1 engine in the image (T-ENG-01) | `docker/server.Dockerfile`, `crates/hoi4-tools/src/profile.rs` | Pin the latest Agent Tools release. Take `engine/bin` from the release tarball, checked against the release SHA-256 and the bundle manifest. Build a test image without the binary and show the engine-backed tool answers unavailable |
| E2 linked engine | `crates/engine-check` (new) | Link `hoi4-engine` from the same Agent Tools commit as a Cargo git dependency. Closure, lints, override audit and the findings baseline on a worktree. Unknown counts never pass |
| W1 focus projection | `crates/server/src/focus/`, `web/src/focus/` | `hoi4.focus_inspect` through the broker, bound to file hashes, stored with tool name, version and projection hash. Canvas on `@xyflow/react` with diagnostics and the layout verdict |
| W2 two-way mutation | same, plus the profile | `hoi4.focus_rewrite` as the only rewrite tool. Gestures become the same rewrite request with expected hashes, routed as a C-02 proposal with the person as actor. `source_changed` events redraw open editors |
| W3 autonomous task | `crates/server/src/agent/` | The agent creates three to five nodes with effects and localization through the same tool, with the tab closed. Changed IDs since its last read go into its next tool result. The engine check runs before every agent commit |
| W4 change card | `crates/server/src/proposals*`, `web/src/review/` | Semantic summary, raw diff, source-linked preview, finding delta, closure verdict, layout verdict, checks performed, limits, restore point. Approval bound to the proposal hash |
| W5 commit, publish, restore | `crates/workspace`, `crates/server/src/github*` | Commit owned files under the expected HEAD. Push and pull request through C-10 on `klimPaskov/hoi4-factory-test-mod` under separate publication grants, with lost replies reconciled by reading state. Restore. Preservation report on import |
| W6 equivalence and parity | `crates/server/tests/` | Same intent in visual, hybrid and autonomous modes. T-EDIT-01 across gesture, native agent and coding-client paths. T-BENCH-01 and the editor round trip |

New browser pages go through the `ui-design-iteration` skill. Variants are batched so the maintainer answers several at once, and non-UI work continues while waiting.

## Excluded

Event, map and technology editors, general visual editors, unrestricted autonomous plans, a many-agent factory, pushes or merges to a default branch, Hagane Studio's visual design and preview cards, claims that a preview proves game behavior, the S05 memory system, and anything in S05 to S08.

## Stop conditions

- A tool result with an unknown effect stops that operation until it is reconciled by reading state.
- A failing required gate is reported, never skipped. Scope is narrowed only by a recorded decision.
- Credentials, money, maintainer-only access, the Mac and in-game checks wait for the maintainer and are batched in [maintainer questions](../maintainer-questions.md). Work that does not depend on them continues.
- No game is started. Docker Desktop is never restarted, and shared images and volumes are never pruned.
- One heavy job at a time on this PC (Rust builds, Docker builds, test suites), because the desktop app crashes when memory runs out.
