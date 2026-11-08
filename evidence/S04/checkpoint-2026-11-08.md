# S04 checkpoint, 2026-11-08

S04 runs at low priority (D-36, D-46). This note says where it stands so a resumed session can continue without the chat. It is not the stage handoff.

## Done and pushed

| Packet or gate | State | Evidence |
|---|---|---|
| T-ENG-01, gate 11 | Agent Tools 4.12.0 (`73a5d71`) installed in the image from its release tarball by SHA-256, the engine binary checked against the bundle manifest, `hoi4-engine` linked from the same commit. A no-engine image answers `FOCUS_ENGINE_UNAVAILABLE` and writes nothing | [t-eng-01](t-eng-01/run.log), `scripts/t-eng-01.sh` |
| W1, W2, gate 3 | Projection and focus operations as C-02 proposals, hash guards, `source_changed`, discard | `crates/server/tests/focus.rs` |
| W3 | Agent `focus_projection` and `focus_edit` on the same apply path. Live DeepSeek run, $0.006 | [live-w3](live-w3/README.md) |
| W5, gates 5, 6 and 8 | Commit check against the project baseline, commit, restore, publication grants, push, pull request, lost replies reconciled, ZIP download, sync of outside changes, import preservation report | `tests/focus.rs`, `tests/publish.rs` |
| T-EDIT-01, gate 9 | Gesture, recorded native agent and coding-client paths give identical bytes and projection | `gesture_agent_and_coding_client_paths_give_identical_bytes` |
| Gate 12 | Q-1 baseline of 2026-11-06 stored with pins and hardware | [q1-baseline](q1-baseline/README.md) |
| Gate 7 | Warm preview p95 131 ms (target 3 s). Gesture round trip 20 to 93 s (target 1 s), tracked in hoi4-agent-tools#137 | [measure](measure/measure-2026-11-08.json) |
| Issues | #5 and #7 closed. #6 has a progress note | GitHub |

## Waiting

- The maintainer's choice of the focus editor and change card designs ([maintainer questions](../maintainer-questions.md) items 14 and 15). The variants are in the design lab and `web/src/api/focus.ts` already maps the server to the lab's types.
- The rest of M-8 in a later Agent Tools release, then one more re-pin. From 4.13.0 the stdio server lists only the core profile unless `HOI4_AGENT_TOOLS_PROFILES=all` is set. At that re-pin, add the variable to `agent_tools_spec` and `ENV_ALLOWLIST` (or a narrower profile list that covers every tool of `agent_tools_focus_editor`), or the profile's tools are reported missing.

## Still to do

The two browser pages from the chosen designs (gates 1, 2 and 4 in the browser), the Hagane round-trip comparison (gate 10, unavailable unless Hagane is run on the same tree), T-IMPORT-01 on a GitHub import, then `validate-stage`, the handoff and GOAL-S05.
