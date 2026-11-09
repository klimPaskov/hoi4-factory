# S04-W3 live run: the agent adds a focus branch with no browser

Evidence kind: live model calls with real spend, against the real HOI4 Agent Tools 4.11.0 and a real PostgreSQL with row-level security. No game was run.

| Item | Value |
|---|---|
| Test | `crates/server/tests/focus.rs`, `live_deepseek_agent_adds_a_branch_through_the_editor_path` (ignored by default, run with `--ignored`) |
| Date | 2026-11-08, on the Windows PC |
| Model | `deepseek-flash` at `https://api.deepseek.com`, through the OpenAI-compatible adapter as the owner's probed endpoint profile `user:<key id>` (issue #7, D-21, D-31). The Anthropic key had no balance |
| Probe | inference probe observed text, tool calls, streaming, usage accounting and model listing. Parallel tool calls, strict JSON schema, cancellation and image input stayed unknown. The context window was declared as 64,000 tokens with the probe |
| Fixture | F-FOCUS-SMALL, the four-focus EXP-02 tree, uploaded as a ZIP |
| Browser | none connected. The server alone drove the attempt |
| Result | attempt `succeeded` in 72.6 s with six model turns. One `focus_projection`, one `focus_edit` with three `create` operations, one more read and `finish`. One change proposal by `native_agent` created `exp_reform_a`, `exp_reform_b` and `exp_reform_c` with costs, prerequisites, icons, rewards and English localisation |
| Bytes | the focus file went from `68bd406d…` (the fixture's exact bytes) to `31334a0d…`. The test checked that every original byte is kept and that each new focus has its title and description keys |
| Spend | 6,045 micro-dollars (about US$0.006), estimated from reported usage at DeepSeek's peak rate, the upper bound. Off-peak is half |

Files: `detail.json` (the task and attempt), `events.json` (the C-03 stream), `proposals.json` (the proposal with its operations, hashes and diffs) and `usage.json` (the account's usage after the run). The operations in `proposals.json` are the recorded real output that the T-EDIT-01 parity test replays.
