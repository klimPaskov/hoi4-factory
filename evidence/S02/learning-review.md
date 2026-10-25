# S02 engineering learning review

Recorded 2026-10-25. Under ADR-024 lessons were applied to CLAUDE.md and the skills during the stage, as they came up. This review lists them with where they went, plus what stays open.

## Applied during S02

| Lesson | Evidence | Destination |
|---|---|---|
| Automatic worktree isolation follows the session's current directory, and once created worktrees of a nested clone | Two agents received worktrees of `.work/hoi4-agent-tools` | CLAUDE.md delegation rules, `delegate-parallel-work` skill step 2 |
| Subagents here cannot create new Markdown files | Every report agent hit it | CLAUDE.md, `delegate-parallel-work` |
| A shared Cargo cache still stores a copy of every crate per worktree path and grew to 23 GB | `target/debug/deps` measured at 19 GB | CLAUDE.md working environment, `delegate-parallel-work` step 7 |
| The C: drive filling up hung Docker Desktop and made unrelated tests fail | Docker hung about 40 minutes, `game-ref` tests failed only on a nearly full disk | CLAUDE.md working environment and verification |
| Git Bash corrupts non-ASCII command-line payloads | First live sign-up returned 400 | CLAUDE.md working environment |
| The Compose database creates roles only on a fresh volume | Server login failed until the volume was recreated | CLAUDE.md working environment, developer setup guide |
| Heredocs passed to Python alter backslash escapes | Three failed edits | CLAUDE.md working environment |
| A squash merge of a base PR breaks a stacked PR's ancestry | PR #108 add/add conflicts after #106 merged | CLAUDE.md upstream rules |
| Setup accepts only Debian's packaged `/usr/bin/node` | PR #108 Linux CI and the server image | CLAUDE.md upstream rules, server and test-runner Dockerfiles |
| Agents must keep temporary output inside owned places | An agent created and deleted a folder in the user's home directory | CLAUDE.md delegation rules, `delegate-parallel-work` step 3 |
| HOI4-Mod-Setup merges need the admin override for the code-owner review and green required checks | Merges of #105 to #108 | CLAUDE.md upstream rules |
| `cargo test` stops at the first failing binary | A run reported 112 tests instead of the full suite | CLAUDE.md verification |
| Uploaded and imported `.gitattributes` with `eol` or `filter` rewrite bytes at baseline commit | Intake negative controls | Code: intake neutralises them, with tests |
| Tests that assert a fixed property of the pinned upstream revision go stale when the pin moves | Three setup-adapter tests still expected the pre-merge Setup (no #105 fix, the finalization dead end #107 removed) and failed on the final Windows run | CLAUDE.md working environment; the tests now assert against `SETUP_PIN` |
| Production readiness advice must stay proportionate for a free hobbyist tool | Maintainer feedback on the first checklist | CLAUDE.md purpose section, `production-readiness-review` skill |

## Upstream defects found and fixed

HOI4-Mod-Setup: placement temp files surviving rollback (#105), a recovery dead end at finalization and an Anthropic schema rejection (#107), and a test suite that depended on a Windows-only Node runtime (#108). hoi4-agent-tools: silent byte rewrites on invalid UTF-8 and stale-base reverts (#42, merged as `731e64f`).

## Open

- RustSec advisories against `rustls-webpki`, `webpki-roots` and `webpki-root-certs` fail HOI4-Mod-Setup's `cargo-audit`. The factory has the same crates through its TLS stack. Dependency scanning is a production readiness item for S08, and a version bump can happen earlier.
- `probes/runtime/src/replay_server.rs` is not rustfmt-clean. It stays unchanged because S01 evidence records its hash.

No authority-widening change was proposed.
