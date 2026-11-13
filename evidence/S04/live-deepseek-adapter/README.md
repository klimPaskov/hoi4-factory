# Live DeepSeek check of the compatible adapter fixes (issue #12)

Recorded on 2026-11-13 under D-21 (S04 live model checks use DeepSeek only). Evidence kind: live. The model is `deepseek-flash` at `https://api.deepseek.com`, called through the production `ProviderClient` with the default destination policy. The key came from the Windows user environment, read per command and never printed.

## What changed in the adapter

- The OpenAI-compatible kind sends the output bound as `max_tokens` as well as `max_completion_tokens`. The OpenAI kind still sends only `max_completion_tokens`, because OpenAI's newer models refuse `max_tokens`.
- `ModelRequest` has an optional `reasoning` effort. The Chat Completions adapters send it as `reasoning_effort`, with `Off` sent as `"none"`. Without it, no reasoning field is sent and the provider default applies.
- HTTP 402 is classified as quota exhaustion, terminal and never retried.
- The DeepSeek price record in `crates/agent/src/cost.rs` was already in place before this change.

## Calls

Command, run from the worktree with `DEEPSEEK_API_KEY` and `DEEPSEEK_EVIDENCE_DIR` set:

```text
cargo test -p factory-agent --test live_deepseek -- --ignored --test-threads=1 --nocapture live_deepseek_output_bound_holds live_deepseek_reasoning_off_is_honoured
```

| Files | Captured (UTC) | Request | Result |
|---|---|---|---|
| `deepseek-bound-8-run1.*` | 18:13:20 | "Say ok.", bound 8, thinking at default | `finish_reason: length`, 8 completion tokens (all 8 reasoning). The adapter reported incomplete output, as it should. The test itself failed because it expected a finished turn. The test was corrected to read the usage from the incomplete outcome too |
| `deepseek-bound-8.*` | 18:13:42 | Same, corrected test | Same result, 8 completion tokens, test passed |
| `deepseek-reasoning-off.*` | 18:13:21 | Short arithmetic question, bound 200, `reasoning_effort: "none"` | `finish_reason: stop`, 11 completion tokens, 0 reasoning tokens, answer "The trip is 3 hours and 25 minutes." Test passed |

On 2026-11-02, with only `max_completion_tokens` sent, the same "Say ok." request with bound 8 returned 15 completion tokens (`evidence/S03/live-deepseek-compat/exploratory-max-completion-tokens.response.json`). With both fields the bound now holds.

## Spend

Three calls, 97 input tokens and 27 output tokens in total. At DeepSeek's off-peak price (the calls were outside the peak windows) that is about US$0.00003, and at most US$0.00006 at the peak price. The outcome files of the two bound runs say `usage_unknown` for cost only because the test helper prices finished turns. The usage is in the final chunk of each `.sse` file.

## Limits

- One model on one day. DeepSeek also documents a `thinking` field. This adapter relies on `reasoning_effort: "none"`, which DeepSeek honoured here.
- The 402 classification is covered by a labelled unit test built from DeepSeek's documentation, not by a recorded refusal. A live 402 needs an account with no balance.
- The `openai:gpt-5.4-nano` profile is still unverified live (issue #2).
