# S03 scope

Stage: S03, native agent, durable attempts and scoped tools. Packet: [stage-03-native-agent-and-durability.md](../../docs/04-implementation-stages/stage-03-native-agent-and-durability.md).

## Authorization and entry condition

The maintainer invoked `/start-implementation-stage S03` on 2026-10-24 and then asked for S02 to be finished and S03 to continue autonomously in the same session, collecting questions in [maintainer-questions.md](../maintainer-questions.md) and stopping only for a direct blocker. S04 is not authorized.

Entry condition: S02 accepted with per-account workspace identity, owned Git isolation and setup recovery. Implementation starts only after the S02 handoff is written and pushed. Until then this file is a draft and only read-only preparation happens.

## Maintainer inputs

From the [decision record](../../docs/02-architecture/maintainer-decisions-2026-10-24.md) S03 inputs: $1 operator allowance per verified user per month, default model `claude-haiku-5-5`, the self-hosted profile verified by connection, model listing and destination policy only (no local inference), and OpenAI live inference unavailable (adapter tested against recorded and refused responses). Live Anthropic use is pre-approved and recorded in evidence.

## Requirements and gates

Primary: REQ-001, REQ-002, REQ-003, REQ-014 (bounded autonomous work, backend), REQ-015, REQ-016, REQ-026, REQ-033. Gates S03-G1 to S03-G6. Expected partial results: G6 records OpenAI live as unavailable and the self-hosted profile as connection-only.

## Packets and ownership

| Packet | Crate or path | Migrations | Content |
|---|---|---|---|
| W1 provider adapters | `crates/agent` | none | Production Anthropic and OpenAI-compatible adapters from S01 (thin, own SSE), capability probing per profile, structured output handling with local validation and one repair attempt, normalized errors and retry classes, usage accounting, outbound destination policy for custom endpoint URLs |
| W2 and W4 task ledger and receipts | `crates/tasks` (new) | 0600-0699 | C-03 ordered events with gap detection, expected-version transitions, host incarnation and lease fencing, durable pause, attempts versus task completion, immutable C-06 bundles with manifest hash and typed narrowing error, C-04 intent before effect and observed result after, effect classes, replay identity bound to workspace, root, source and tool version, reconciliation of unknown effects |
| W3 grants, broker and payment source | `crates/broker` (new) | 0700-0799 | C-08 grants with operation class, paths, destination, budget, expiry and revocation epoch, closed final schema validation after hooks, BYOK keys encrypted per owner, operator allowance and user-key metering attributed separately, budget stop per task and account |
| W5 HOI4 adapter negotiation | `crates/hoi4-tools` (new) | none | Configured tool profiles only, Agent Tools MCP over stdio in its pinned container, schema and version discovery, health separate from feature presence, read-only mode, timeouts and bounded retries |
| Integration | `crates/server`, `web/`, `compose.yaml`, `docker/` | none | Task, attempt, pause, revoke, event stream and provider key routes, minimal task and API key screens, the agent worker in the server image |
| W6 restart and intervention | tests across crates | none | Process death, disconnect, a worker ignoring cancellation, stale results, reopening a task without repeated writes |

Shared contract owners: `crates/agent` owns provider capability and model request types, `crates/tasks` owns C-03, C-04 and C-06 types, `crates/broker` owns C-08 and metering types. A packet needing another packet's type depends on that crate.

## Closing deliverables

Besides the usual evidence files, S03 ends with a comprehensive handoff for S04 and a self-contained goal prompt for a fresh chat (`docs/06-agent-handoffs/GOAL-S04.md`), as the maintainer asked. The S02 close likewise produces `docs/06-agent-handoffs/GOAL-S03.md` as a fallback.

## Excluded

A fleet or multi-node scheduler, an unrestricted shell, visual editors, credit purchasing and payment providers, the full S05 memory and retrieval system, local model inference, and S04.

## Stop conditions

A write outside owned paths, a live provider call outside Anthropic, a tool write outside an owned fixture workspace, an unknown effect retried before reconciliation, a secret or user key appearing in logs, evidence or tracked config.
