# Focus editor round trip: HOI4 Factory against Hagane Studio (S04 gate 10)

Recorded on 2026-11-13 on the Windows PC (AMD Ryzen 5 7500F, 12 threads, 32 GB). Evidence kind: tool execution of Hagane Studio built from source, compared with HOI4 Factory's own measurement of 2026-11-08 on the same PC and the same tree. The probe is in [probes/hagane-round-trip](../../../probes/hagane-round-trip/README.md).

## Inputs

| Item | Value |
|---|---|
| Tree | F-FOCUS-SMALL, `crates/server/tests/fixtures/focus-small/common/national_focus/exp_basic.txt`, SHA-256 `68bd406d428bcc8add0a35b616904611bded565fe9e987d7bb0287fdfd15fcf9`, 4 focuses |
| Gesture | Move `exp_right` to x 7 or 8, y 1, five times |
| Hagane Studio | `hagane-works/hagane-studio` commit `191c2fe54e1d2ddc76ab2dc4fa699e96f3766c25` (2026-11-10), Go 1.25.14, linux/amd64 in Docker, 4 CPUs, image `sha256:e3c53f25…` |
| HOI4 Factory | [measure-2026-11-08.json](../measure/measure-2026-11-08.json): debug build, Windows, Agent Tools 4.12.0 |
| Game | HOI4 1.19.4.2 install mounted read-only for Hagane's base-game check. Not started |

## Results

| Surface | What one round trip covers | Times (ms) | p95 |
|---|---|---|---|
| Hagane Studio, editor | Drag request saved to Hagane's JSON project, position read back | run 1: 102, 81, 85, 94, 80. Run 2: 52, 101, 48, 48, 48 | about 100 |
| Hagane Studio, native files | The same move, then `export_project` writes the native mod as a ZIP | Warm: 2,031 to 2,836. The first export of each run took 113 s and 119 s while Hagane built its game index | about 2.8 s warm |
| HOI4 Factory, editor | Gesture as a C-02 proposal, Agent Tools rewrites the native file under hash guards, new projection returned | 26,830, 19,707, 24,777, 92,668, 39,698 (2026-11-08) | 92.7 s |

Both surfaces end with the moved position in a native focus file. The exported Hagane file has `x = 7` for `exp_right` on the last move, as requested.

HOI4 Factory's round trip is much slower than Hagane's. Its median of 26.8 s is about 13 times Hagane's warm native-file export (median 2.03 s) and about 300 times Hagane's in-editor save. At p95 the gap is about 33 times (92.7 s against 2.8 s). HOI4 Factory misses the Q-1 target of 1 s p95. The cause is the Agent Tools rewrite path, tracked in hoi4-agent-tools#137.

## Source preservation, seen in the same run

The two tools answer different questions about the file. HOI4 Factory changes only the moved focus and keeps every other byte (T-EDIT-01 and the hash-guard tests in `tests/focus.rs`). Hagane regenerates the whole tree on export. The diff of [the exported file](hagane-export-exp_basic.txt) against the input shows:

- Every comment is gone: the file header, the trailing comment on the tree id, the comment before the root focus and the one after `cost`.
- Inline blocks are expanded onto several lines, and `cancel_if_invalid`, `continue_if_invalid`, `available_if_capitulated` and `continuous_focus_position` are added.
- Every icon became `GFX_goal_unknown`. This run imported the focus file alone, without `interface/exp02_icons.gfx`, which may explain it.
- The tree's country modifier `add = 10` was exported as `add = 100`. That changes the tree's meaning. It is reported as observed on this input, without checking Hagane's code for the reason.

## Limits

- The two surfaces ran on different days and in different environments (HOI4 Factory as a Windows debug build on 2026-11-08, Hagane in a Linux container on 2026-11-13). Same machine and same tree. HOI4 Factory's number is far outside any gap that this difference could explain.
- Hagane ran without its React interface. The probe sends the same HTTP requests the interface sends on a drag, so browser rendering time is not included for either tool.
- Hagane's whole-mod import (`/api/go/import/jobs`) was not used. A whole-mod import might keep the icons.
- The T-BENCH-01 half of gate 10 (no regression from the gate 12 baseline) is not in this record. It waits for the Agent Tools re-pin of gate 11.
