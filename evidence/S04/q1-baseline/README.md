# Q-1 baseline for S04 (gate 12)

`baseline-2026-11-06.json` is the real-game baseline of T-BENCH-01, the hoi4-agent-tools benchmark, as accepted on this Windows PC on 2026-11-06 at 23:10 UTC and still current when S04 started on 2026-11-08. It was copied unchanged from the benchmark's machine-local folder. SHA-256 `1168af60ab60945a596417d7fbd4790216bacb2aea816215865f33bfbddd3731`.

| Item | Value |
|---|---|
| Suite | `real` |
| Cases | 99, of which 93 pass and 92 answer inline |
| Corpora | vanilla, Chaos Redux, ZIN, Ireland Total Overhaul, Atlantis, Kaiserreich 1.7 and Old World Blues 5.1.12, each pinned by version, commit or Workshop manifest and a SHA-256 over its script, interface and localisation files (the `corpora` field) |
| Hardware | AMD Ryzen 5 7500F, 12 threads, 32 GB, Windows x64, Node v24.15.0 (the `machine` field) |
| Engine | the `hoi4-engine` binary named in `machine.engineSha256` |
| Comparator | the one-time head-to-head with `cpntodd/HOI4-MCP` of 2026-11-01, recorded in hoi4-agent-tools `docs/research/hoi4-mcp-head-to-head-2026-11-01.md` (M-7, pull request #113). It predates the accuracy fixes |
| Latency | per case, in milliseconds (the `latency` field) |

Surfaces that could not run are named in the benchmark's own report. The file holds identifiers, hashes and verdicts only, never mod or game files. The nightly run that compares later releases with this baseline is owned by the Agent Tools session (S04-4).
