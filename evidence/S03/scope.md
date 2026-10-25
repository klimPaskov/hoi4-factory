# S03 scope

Stage: S03, native agent, durable attempts and scoped tools. Packet: [stage-03-native-agent-and-durability.md](../../docs/04-implementation-stages/stage-03-native-agent-and-durability.md). Goal prompt: [GOAL-S03](../../docs/06-agent-handoffs/GOAL-S03.md).

Status: **final**, confirmed against the packet on 2026-10-25 before any S03 code was written.

## Authorization and entry condition

The maintainer invoked `/start-implementation-stage S03` on 2026-10-25 on the macOS machine, with the instruction "I authorize S03 now", and asked for autonomous implementation to the end of the goal: S03 handoff, GOAL-S04, an updated GOAL-UI and everything pushed to `origin main`. Questions go to [maintainer-questions.md](../maintainer-questions.md). S04 and the UI design track are not authorized.

Entry condition met: the [S02 handoff](../S02/handoff.md) is complete and pushed (`main` at `7c70b0c`). The readiness check on this Mac passed: `scripts/compose.sh up -d --build --wait` healthy, `/readyz` `status: ok` with the game reference available and read-only, the package validator 11 of 11 and the skill check with no drift.

## Maintainer inputs

From the [decision record](../../docs/02-architecture/maintainer-decisions-2026-10-24.md) S03 inputs and the goal prompt:

- $1 of operator-funded model usage per verified user per month. Users may bring their own API key, stored encrypted per owner and never used for anyone else.
- Default model `claude-haiku-5-5`. Live Anthropic calls are pre-approved and their spend is recorded.
- OpenAI live inference is unavailable (no API balance). The OpenAI adapter is tested against recorded and refused responses only.
- The self-hosted OpenAI-compatible profile is verified by connection, model listing and the outbound destination policy only. No local inference.
- Low friction: no MFA, lockouts, extra sign-up steps or enterprise features.

## Requirements and gates

Primary: REQ-001, REQ-002, REQ-003, REQ-014 (bounded autonomous work, backend), REQ-015, REQ-016, REQ-026, REQ-033. Gates S03-G1 to S03-G6 from the packet. Expected partial result: G6, because OpenAI live is unavailable and the self-hosted profile is connection-only.

## Packets and ownership

| Packet | Crate or path | Migrations | Content |
|---|---|---|---|
| W1 provider adapters | `crates/agent` | none | Production Anthropic and OpenAI-compatible adapters from S01 (thin, own SSE), a provider-neutral request, capability probing per profile, structured output with local validation and at most one repair attempt, normalized errors and C-envelope retry classes, usage accounting, outbound destination policy |
| W2 and W4 task ledger and receipts | `crates/tasks` (new) | 0600-0699 | C-03 ordered events with gap detection, expected-version transitions, host incarnation and lease fencing, durable pause, attempts versus task completion, immutable C-06 bundles with manifest hash and typed narrowing error, C-04 intent before effect and observed result after, effect classes, replay identity, reconciliation of unknown effects |
| W3 grants, broker and payment source | `crates/broker` (new) | 0700-0799 | C-08 grants with operation class, paths, destination, budget, expiry and revocation epoch, closed final schema validation after any hook or repair, user API keys encrypted per owner, metering that keeps operator allowance and user-key usage apart, budget stops per task and per account |
| W5 HOI4 adapter negotiation | `crates/hoi4-tools` (new) | none | Configured tool profiles only, Agent Tools over MCP stdio, schema and version discovery, health separate from feature presence, read-only mode, timeouts and bounded retries |
| Integration | `crates/server`, `web/`, `compose.yaml`, `docker/` | none | Agent host (tool execution through broker and ledger), the worker supervisor, task, attempt, pause, revoke, event stream and API key routes, minimal task and API key screens, the agent worker and Agent Tools in the server image |
| W6 restart and intervention | tests across crates | none | Process death, disconnect, a worker that ignores cancellation, stale results, reopening a task without repeated writes |

Shared contract owners: `crates/agent` owns provider, capability and model request types. `crates/tasks` owns C-03, C-04 and C-06 types. `crates/broker` owns C-08 and metering types. The composition of grant check, fence check and intent record into one transaction lives in the server's agent host, so `tasks` and `broker` do not depend on each other.

## Technical defaults chosen for S03

Recorded here as the implementing agent's defaults. Each is revisable by the maintainer.

1. **Agent loop placement.** A supervised worker process of the same binary (`factory-server agent-worker`), as S01 measured. The host drives the loop turn by turn over the S01 framed protocol: the worker makes one provider call and assembles the stream, the host validates, persists, meters and executes tools. A worker generation comes from the durable lease, and the host drops frames from any other generation.
2. **Where agent writes go.** Only into the task's own linked Git worktree on its owned branch (`tasks/<task_id>/`, created by the S02 workspace crate), and only under the paths the task's grant names. The mod's main `source/` tree is never written by the agent. The resulting branch commit is the proposal S04 reviews.
3. **Built-in tools.** `list_files`, `read_file` (pure reads), `write_file` (idempotent write keyed by operation, with an expected-before hash and an observable after-hash postcondition) and `finish` (the structured final report, validated locally with one repair attempt). Agent Tools tools are added by a configured profile.
4. **Agent Tools packaging.** hoi4-agent-tools `731e64f5b5c964bd1b33adad9c085bd3168b1940` (package 4.4.0, contains the #42 guards) is built from its source archive, verified by SHA-256, in a separate stage of the server image and installed at `/opt/hoi4-agent-tools` with Node 22 from the digest-pinned `node:22-bookworm-slim` image at `/opt/node22`. Setup keeps Debian's `/usr/bin/node`. The server starts Agent Tools per task as a supervised stdio child with an environment allowlist and the task worktree as its working directory. The gVisor per-task container of ADR-020 stays an EXP-07 launch gate, as the decision record says plain Docker is for local development.
5. **S03 Agent Tools profile.** Read-only only: `source_lookup`, `mod_index`, `script_validate`, `reference_search`, `reference_read`, `reference_context`, `focus_inspect`, `event_inspect`, `decision_inspect`. Rewrite tools are withheld in S03 (S04 decides the writable subset with EXP-02's evidence). Any tool not in the profile, or whose schema hash changes during a session, is withheld.
6. **MCP client.** A thin JSON-RPC client over newline-delimited stdio in `crates/hoi4-tools`, like the thin provider adapters, because the factory needs bounded frames, an explicit environment, process-group termination and its own durable job wrapper. Recorded as a reconsideration point if the official Rust SDK becomes necessary.
7. **User key encryption.** The accounts crate's existing `AesGcmSecretStore` (AES-256-GCM, written in S02 for exactly this use) with a server master key file (`FACTORY_SECRET_KEY_FILE`, 32 random bytes in base64, a read-only mount like the GitHub App key). The owner account ID, key ID and provider are the authenticated context, so a ciphertext copied to another account or row does not decrypt. Without the master key the key routes report unavailable and only the operator allowance works.
8. **Operator allowance.** Paid with the server's own provider key (`FACTORY_ANTHROPIC_API_KEY`, already used by Setup analysis). $1 per verified account per calendar month (UTC), counted in micro-dollars, reserving the conservative bound before each call and settling to the estimate from reported usage. Missing usage keeps the reservation and is marked `usage_unknown`.
9. **Task defaults.** Budget $0.20 per task, at most 20 model turns per attempt, `claude-haiku-5-5` on the operator allowance unless the user picks a saved key.
10. **Self-hosted profile check.** An Ollama container pinned by digest, started only for the probe on the Compose network, with no model pulled and no inference. It exercises connection, model listing and the destination policy: a private Docker address is refused by default and accepted only when the operator lists its range.
11. **Event stream.** C-03 events are read by after-sequence cursor from PostgreSQL. The live stream is server-sent events fed by an in-process notifier and always backed by the database read, so a reconnect cannot skip or duplicate events.

## Closing deliverables

`evidence/S03/environment.json`, `checks.json`, `config-review.md`, `learning-review.md`, `readiness.json`, `handoff.md`, a serialized C-06 fixture with its manifest hash, the updated package status and README, [GOAL-S04](../../docs/06-agent-handoffs/GOAL-S04.md) and an updated [GOAL-UI](../../docs/06-agent-handoffs/GOAL-UI.md) screen inventory.

## Excluded

A fleet or multi-node scheduler, an unrestricted shell, visual editors, credit purchasing and payment providers, the full S05 memory and retrieval system, local model inference, Agent Tools rewrite tools, merging or publishing a task branch, the gVisor sandbox (EXP-07), the UI design track and S04.

## Stop conditions

A write outside owned paths, a live provider call outside Anthropic, a tool write outside an owned fixture workspace or task worktree, an unknown effect retried before reconciliation, a secret or user key appearing in logs, evidence or tracked config.
