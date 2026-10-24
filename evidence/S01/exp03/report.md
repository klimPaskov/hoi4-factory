# EXP-03 report: Rust agent runtime probes (S01-W5)

Recorded 2026-10-24. Stage S01 is authorized by the maintainer. S02 is not started. Base revision `290af7c48ce160d4f7461b90aedc92528295330b` with uncommitted S01 changes on `main`. Machine-readable results are in [results.json](results.json). Its `source_manifest` hashes every input file.

## Summary

The Rust loop rejects every listed fault on recorded real provider streams. It also rejects hook-mutated and malformed final tool arguments at the host boundary with zero filesystem effect. Stale fencing tokens, crash-after-effect cases and changed replay identity are handled as specified, and all of that state survives a PostgreSQL container restart. OpenAI inference could not run because both OpenAI keys returned HTTP 429 `credit_balance_exhausted`. Live model evidence therefore comes from Anthropic Messages with `claude-haiku-5-5`. The OpenAI adapter has real evidence for authentication and quota refusal handling and for model listing only.

Recommendation: keep thin project adapters and run the model loop in a supervised worker process of the same binary. The numbers are below.

## Amendment compliance

No synthetic model text was used anywhere. Every fault was injected into a stream captured byte-for-byte from a real Anthropic response. When no recording existed, the probes reported the case as unavailable instead of substituting data. Each result is labelled with one of four evidence kinds.

| Kind | Meaning |
|---|---|
| live | A real provider request |
| recorded-real-replay | Replay of bytes captured from a real provider stream, with the recorded chunk boundaries |
| local-execution | Real PostgreSQL, processes, pipes and filesystem, fed with recorded real tool arguments |
| static | Source inspection, build metrics and unit tests |

## What was built

| Path | Content |
|---|---|
| `crates/agent/src/sse.rs` | Own incremental SSE parser that reports a partial line or an undispatched event at end of stream |
| `crates/agent/src/openai.rs` | Chat Completions adapter, tool-call delta assembly by index, HTTP refusal classes, model listing |
| `crates/agent/src/anthropic.rs` | Messages adapter (`anthropic-version: 2026-10-21`), `input_json_delta` assembly by block index, thinking blocks tolerated and never surfaced |
| `crates/agent/src/transport.rs` | Live byte source with a tee for recordings, replay source with truncate, disconnect and drop-event faults |
| `crates/agent/src/protocol.rs` | Length-prefixed JSON frames, 1 MiB limit, handshake with protocol version, runtime build and adapters |
| `probes/runtime/src/broker.rs` | Closed JSON Schema validation after all hooks (jsonschema 0.60.1, no remote refs), workspace root policy |
| `probes/runtime/src/journal.rs` | PostgreSQL effect journal with lease fencing, intent before effect, recovery to `unknown_effect` |
| `probes/runtime/src/worker.rs` | Host supervisor and worker child, generation fencing, overhead benchmark |
| `probes/runtime/recordings/` | Four real Anthropic streams with metadata, key and request headers never stored |

The agent crate holds no filesystem, Git or grant authority. It receives byte sinks and sources from its caller. The broker and journal are host code in the probe crate.

## Models and spend

| Provider | Model ID | Result |
|---|---|---|
| OpenAI | `gpt-5.4-nano-2026-03-17` requested, chosen from the free model listing | Three requests refused with 429 `insufficient_quota`, code `credit_balance_exhausted`. No inference |
| Anthropic | `claude-haiku-5-5` requested, `claude-haiku-5-5` reported in `message_start` | Six billed requests succeeded |

Estimated spend is 0.00 USD on OpenAI and about 0.00068 USD on Anthropic. The figure comes from reported usage at 0.10 USD per million input tokens and 0.50 USD per million output tokens, from the [Anthropic pricing page](https://platform.claude.com/docs/en/about-claude/pricing) retrieved 2026-10-24. The cancelled turn is counted at its full 600-token output allowance as an upper bound. The coordinator's own key check call is not included. Per-call entries are under `spend` in results.json.

## Results by requirement

| Requirement | Check | Kind | Verdict |
|---|---|---|---|
| RT-03 | Truncation mid tool arguments, truncation at an event boundary, missing stop reason, missing `message_stop`, text truncation, text missing stop reason | recorded-real-replay | pass. Each returns an incomplete-output class, no effect, error row persisted |
| RT-03, T-RUN-02 | Malformed tool JSON (one argument fragment dropped) | recorded-real-replay | pass. `malformed_tool_arguments`, no effect |
| RT-07 | Disconnect after the tool call deltas | recorded-real-replay | pass. Transport error with acceptance `accepted`, no effect, no automatic retry |
| RT-07 | Follow-up stream disconnects after the tool was settled | recorded-real-replay | pass. Recovery reads the C-04 receipt (`already_applied`), executes nothing, one settled op in total |
| RT-04 | Hook mutates the path to `../`, absolute, nested traversal, the host marker file, or a symlink escape | recorded-real-replay | pass. Initial validation passed, final call refused, tree snapshot unchanged |
| RT-04, T-RUN-02 | Hook changes `content` to a number, adds an unknown field, drops a required field, renames the tool, or turns the arguments into a string | recorded-real-replay | pass. Closed schema or unknown tool refusal, no effect |
| T-RUN-03 | A stale fence cannot publish a result or start an effect, pause and revocation stop dispatch, and both persist across a database restart | local-execution | pass |
| T-RUN-04 | Real process aborts before intent, during the effect and after the effect | local-execution | pass. No op and no file for the first case. `unknown_effect` with observed hash for the other two. Replay blocked until explicit reconciliation |
| T-RUN-04, ORCH-09 | Changed workspace root or tool version | local-execution | pass. Replay refused with the changed field named |
| RT-08 | Late frame from a superseded worker generation, frame stamped with an old generation, worker killed mid-request, incompatible protocol version, junk on the frame channel, in-process task outliving a restart | recorded-real-replay | pass. Durable event order unchanged |
| RT-10 | Live cancellation after 9 events, request accepted | live | pass. `cancelled_after_acceptance`, automatic retry not allowed, no retry made |
| RT-02, T-RUN-05 | Live tool-call turn and follow-up turn on Anthropic | live | pass |
| RT-06 | Invalid key on OpenAI and Anthropic | live | pass. Class `auth`, no retry |
| RT-06 | OpenAI 429 `credit_balance_exhausted` | live | defect found and fixed, live recheck not run |
| RT-03, RT-07, RT-10 on OpenAI | Stream faults, live tool turn, cancellation | none | unavailable. No OpenAI stream can be recorded without credit |
| Anthropic adapter | Unit tests for request shape and HTTP classes | static | pass |

## Defect found by the live run

The first OpenAI classifier treated 429 as a rate limit unless the error code was `insufficient_quota`. OpenAI actually returned type `insufficient_quota` with code `credit_balance_exhausted`. The probe therefore labelled the refusal `rate_limited` with automatic retry allowed, which would have caused repeated billed attempts against an empty account. The classifier now checks the error type and the known quota codes. The unit test `live_credit_exhausted_is_quota_not_retryable` uses the provider's values. The artifact `runs/live-record-quota-refusal.json` still shows the original wrong classification as evidence.

## Provider crate comparison

Both comparison crates send the same request from `probes/runtime/compare/body.json`. They were built with `cargo build --release -j 12` into a fresh target directory, three times each in alternating order.

| Measure | Thin adapter (`factory-agent`, `openai` feature) | async-openai 0.42.2 (`rustls`, `chat-completion`) |
|---|---|---|
| Unique normal dependencies, `cargo tree` | 90 | 126 |
| Normal closure, `cargo metadata` Linux | 106 | 139 |
| Stripped release binary | 4,584,960 bytes | 5,629,440 bytes (plus 1,044,480, plus 22.8 percent) |
| Clean release build, three runs | 342.5, 106.7, 203.0 s (median 203.0) | 257.1, 302.6, 168.9 s (median 257.1) |
| Raw byte capture for replay | Yes, through the tee | No public hook. Needs the optional middleware feature and a custom tower service |
| Missing `[DONE]` marker | `incomplete_output` | Same as success, the stream simply ends |
| Truncated partial event at EOF | `incomplete_output` with the byte count | Discarded without error by eventsource-stream 0.2.3 |
| Tool-call deltas | Assembled by index, parsed only after a clean finish | Raw chunks exposed, assembly and parsing left to the caller |
| Malformed arguments | `malformed_tool_arguments` class | Not detected |
| Anthropic | Own adapter in the same crate | Not supported |
| License | AGPL-3.0-only (project) | MIT |

Build time differs by more between identical runs than between the crates, so it does not decide anything. The async-openai fault rows come from reading its source (`src/client.rs` and eventsource-stream `src/event_stream.rs` lines 288 to 290). The replay comparison through the local server is unavailable because it needs an OpenAI-format recording. The details are in [compare/comparison.json](compare/comparison.json).

## Execution placement comparison

The same binary runs as host or as worker. The release build ran on the Windows host, with 200 iterations over the recorded real Anthropic tool-call stream (2,666 bytes).

| Measure | Median | p95 |
|---|---|---|
| In-process assembly | 33.0 µs | 47.3 µs |
| Worker round trip, including assembly | 224.0 µs | 358.5 µs |
| Assembly inside the worker | 43.0 µs | 57.2 µs |
| Worker spawn and handshake (10 spawns) | 12.2 ms | 17.6 ms |

The worker adds about 0.19 ms per turn. A live turn took 0.9 to 4.6 s end to end in this probe, so the overhead is below 0.03 percent. An earlier worker run had a 216 ms median spawn time while clean builds were loading the CPU. Spawn time is load sensitive. Both placements reject late results through the same generation check (W3 for the process, W7 for the in-process task). Only the process can be terminated with evidence while it is stuck (W5).

## Licenses

`python -I probes/runtime/licenses.py . factory-agent factory-runtime-probe` lists every normal transitive dependency for the Linux container target and the Windows host target. There are 107 crates under `factory-agent` and 188 under the probe on Linux. No crate falls outside the AGPL-3.0 allow list. The cryptography stack is aws-lc-sys 0.46.0 and ring 0.17.14, both under ISC, Apache-2.0, MIT or BSD-3-Clause terms. The async-openai closure also has no flag. The list is a planning heuristic and not legal advice. NOTICE retention is still owed at S08.

## Limitations

- OpenAI inference, OpenAI stream faults and OpenAI cancellation are unavailable until an OpenAI account has credit.
- The Anthropic stream assembler has evidence from one model and one fixture shape. Parallel tool calls, thinking blocks, refusals and server tool blocks were not recorded.
- Cancellation cost is an upper bound. Provider-side billing after a client disconnect is not observable.
- All runtime numbers come from one Windows host. The worker was not measured inside a container or under gVisor.
- The journal runs on one host. Clock skew between replicas and network partitions were not exercised.
- Clippy reports a large error variant on `TurnError` (it carries the partial state) and one collapsible `if`. Neither is fixed yet.

## Recommendation

Adopt thin project adapters. They own SSE parsing and completion detection, capture raw bytes for replay, report incompleteness explicitly, need 36 fewer crates, and serve both ADR-019 providers from one crate. Keep using vetted crates for HTTP, TLS, JSON and JSON Schema as ADR-011 requires.

Run the loop in a supervised worker process of the same binary. Fencing is equal in both placements, but the worker gives a killable boundary for 0.19 ms per turn and a 12 ms start. Keep the in-process path for tests.

## Maintainer decisions needed

1. Whether to fund OpenAI later to complete the OpenAI stream fault, cancellation and live retest evidence, and the replay-based fault comparison (`probes/runtime/compare/fault-compare.sh` is ready).
2. Accept or override the adapter and placement recommendations above.
3. Confirm the Anthropic request profile: `anthropic-version: 2026-10-21`, thinking disabled, `tool_choice: auto` with a prompt instruction, strict tool schema.

## Reproduction

```
docker compose -f compose.yaml -f probes/runtime/compose.db-port.yaml up -d --no-deps db
ANTHROPIC_API_KEY="$(powershell.exe -NoProfile -Command "[Environment]::GetEnvironmentVariable('ANTHROPIC_API_KEY','User')" | tr -d '\r')" target/release/runtime-probe record-anthropic
ANTHROPIC_API_KEY="..." RUNTIME_PROBE_LIVE=1 bash probes/runtime/run-exp03.sh
bash probes/runtime/compare/measure.sh <scratch dir>
python -I probes/runtime/licenses.py . factory-agent factory-runtime-probe
python -I probes/runtime/collect_results.py
```

Running `record-anthropic` again replaces the recordings and changes their hashes.

## Engineering learning candidates

- A live refusal can carry the quota signal in the error type rather than the code. Classify on both and add the observed values as a regression test.
- Anthropic stream data lines carry whitespace padding inside the JSON object. Byte-exact replay must keep it.
- A stream that ends without its completion marker must be an error. An SSE library that returns end of stream in both cases hides that fault.
