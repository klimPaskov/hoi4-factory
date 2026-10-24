# S01 scope

Stage: S01, Claude Code bootstrap and architectural probes. Packet: [stage-01-bootstrap-and-probes.md](../../docs/04-implementation-stages/stage-01-bootstrap-and-probes.md) at repository revision `290af7c48ce160d4f7461b90aedc92528295330b`.

## Authorization

The maintainer invoked `/start-implementation-stage S01` in Claude Code on 2026-10-24, after answering the S01 input questions in the same session. That authorizes S01 only. S02 is not authorized.

## Maintainer amendments to the packet

These were given in chat before the stage started and override the packet where they differ.

| Packet text | Amendment | Effect |
|---|---|---|
| "Start with fake transports" for runtime probes | "Nothing must be fake." Use the maintainer's OpenAI API key | Runtime probes call the live OpenAI API. Faults that a live API cannot produce on demand (truncation at a byte offset, missing finish marker, crash after a tool effect) are injected into recorded real OpenAI response streams and replayed. Every result records whether it was live or recorded-real replay. No synthetic model output is used |
| Two API profiles and one self-hosted endpoint | Anthropic key to be set by the maintainer. Ollama probe skipped | M-API-A (OpenAI) runs live. M-API-B (Anthropic) runs only if `ANTHROPIC_API_KEY` is present when the probe runs, otherwise unavailable. M-LOCAL-C is unavailable by maintainer choice |
| 10 USD live budget | Do not purchase credits. Use the existing OpenAI key | Self-imposed cap of 2 USD of estimated OpenAI usage for the stage, recorded per call. Stop live calls if the key has no balance |
| GitHub App probe | Claude prepares a disposable private repository and an App manifest, the maintainer clicks to create and install it | The App is installed only on the disposable repository. If the maintainer has not completed the click, the App probe is unavailable and the gh-token path is not substituted |

## Requirements and gates in scope

S01 owns REQ-036, REQ-037, REQ-038, REQ-039, REQ-040 and REQ-045 at planning level. It probes REQ-001, REQ-002, REQ-004 to REQ-007, REQ-011, REQ-013, REQ-033 and REQ-047 without accepting them. Exit gates S01-G1 to S01-G5 from the packet apply, with the amendments above.

## Outputs

| Packet | Output | Owned paths |
|---|---|---|
| S01-W1 | environment.json, source closure | `evidence/S01/` |
| S01-W2 | guidance checks, Cargo workspace and Compose skeleton | `Cargo.toml`, `crates/`, `compose.yaml`, `docker/`, `.gitignore`, `evidence/S01/` |
| S01-W3 | EXP-01 Setup binding and recovery evidence | `probes/setup/`, `evidence/S01/exp01/`, `.work/` |
| S01-W4 | EXP-02 focus fidelity evidence | `probes/focus/`, `evidence/S01/exp02/`, `.work/` |
| S01-W5 | EXP-03 and EXP-04 runtime, container and GitHub evidence | `crates/agent/`, `probes/runtime/`, `evidence/S01/exp03/`, `evidence/S01/exp04/` |
| S01-W6 | checks.json, config-review.md, learning-review.md, handoff.md | `evidence/S01/` |

`.work/` holds disposable upstream clones and run directories and is git-ignored.

## Excluded

Production editors, accounts, sign-in, the browser client, a broad task platform, public deployment, payments, gVisor containment (EXP-07 belongs to S06/S08), changes to real mods, reading a local game install, writes to any non-disposable GitHub repository, and S02.

## Required tools and known gaps

Rust 1.97.1 stable plus 1.88.0 (Setup's pinned toolchain), Docker 28.5.2 with Compose 2.40.3 on a WSL2 Linux engine, Node 24.15.0, Python 3.13.5, Git 2.54, gh 2.87.3, Claude Code 2.1.304. Gaps at start: no `ANTHROPIC_API_KEY`, no gVisor runtime, pnpm not on PATH, no HOI4 game copy (not needed for S01 fixtures).

## Stop conditions

Stop an operation on missing authority, scope expansion, a write outside owned paths, a GitHub write outside the disposable repository, live spend reaching the cap, or an unknown effect outcome until it is reconciled. Unavailable optional checks are recorded, not substituted.
