# Context, protocols and evaluation research

Research date: 2026-10-23. All repository observations are source inspection, with no candidate tests or game runs executed. The collaborative supplied-source gate finished before external research began. The exact source records are in [evidence-root.json](evidence-root.json) and [the registry](01-research-sources.md).

## Context and memory recommendation

Use a hybrid authority model. Keep editable mod source, approved canon and accepted engineering guidance in Git. Keep execution leases, task transitions, approval grants, attempt receipts, usage and recovery journals in a local transactional database. Keep renders and logs in a content-addressed artifact store with hashes recorded in the database. Make domain indexes and search projections rebuildable from authoritative inputs. This is a proposed synthesis based on the distinct responsibilities seen in SRC-R01 through SRC-R04.

Beacon supplies concrete evidence for separate candidates and reviewed memory. Its review tests inject failures between memory and candidate writes and race approval against rejection. Its retrieval excludes superseded memory. Adopt these invariants, without importing the external evaluator or allowing an inferred lesson to rewrite approved lore. The store's project identity and relayed-path boundary also show why a workspace ID needs more care than a raw directory string.

Entire supplies a useful checkpoint interface and native Git ref compare-and-swap pattern. Native lock errors, stale ref values, symbolic refs and permanent namespace errors are distinct outcomes. Retrying all of them as transient conflicts would be wrong. The proposed factory should publish small redacted evidence links with a commit while keeping credentials, raw provider responses and personal logs out of Git.

agentglass provides read-only session projections, pagination, redaction and explicit unknown usage/cost states. Its types and read model are suitable observation patterns. It is distinct from the Glass Town factory project and does not establish a durable approved project-memory layer.

MFS makes the cost of a retrieval service concrete. Metadata, vectors, artifacts and job state must remain consistent, including when cancellation wins after vector writes. The small first version should start with paths, exact identifiers, dependency lookup and full-text search. Add vectors only after a retrieval benchmark shows a material gap. Source freshness must come from file/revision fingerprints and connector state, not from a plausible answer.

## Protocol boundaries

MCP connects the native host to tools and resources. The actual server protocol, schema and capabilities must be discovered and pinned per session profile. The latest official docs redirected to 2026-07-28 and describe a changed request capability model. Existing HOI4 integrations may speak older versions, so the plan must not hardcode the newest wording as a universal transport contract. Tasks support is optional. A task handle does not by itself prove that a write is idempotent or safe to retry.

Agent Skills carry reusable instructions. Their frontmatter or allowed-tools text must never be treated as authority to grant process, network or file access. ACP can expose the product's own agent to another editor later. It is not required between internal services and it must not turn the product into an external coding-CLI launcher. LiteLLM remains a possible gateway adapter if deployment or provider needs justify its additional service boundary.

## Research and legal interpretation limits

The two harness papers are useful design checklists and evaluation ideas. Neither is a head-to-head HOI4 result. The source-code study explicitly distinguishes inspection from execution and documents limitations in its Claude Code source basis. This research used official Claude documentation, not a circulated source leak. The runtime-substrate paper's worked case is intentionally narrow. Its evidence structure is useful even where generalized outcome improvements remain unknown.

byProduct and Warp are workflow inspirations with vendor-described capabilities. Their claims are not proof that many agents, cloud hosting or a long lifecycle is necessary for the first HOI4 use case. A single bounded writer with reliable restore is the first success condition.

The supplied Paradox agreement supports preserving rights and provenance boundaries. It does not authorize redistribution of game or DLC assets, and its commercial application must be checked against the intended distribution and business model. Keep legal conclusions qualified and turn unresolved commercial reuse into an explicit release decision, rather than asserting that all independent tools are either permitted or prohibited.

## Repository guidance and maturity boundaries

The supplementary [root-guidance audit](context-guidance-audit.json) fully read nine present guidance bodies across Beacon, Entire and MFS, with two resolved AGENTS symlinks recorded separately from their original Git blobs. Agentglass's complete pinned tree has none of the three named root guidance files. Guidance declarations are not executed runtime results.

| Reference | Additional boundary from guidance | Reuse consequence |
|---|---|---|
| [Beacon guidance](https://github.com/Asymptote-Labs/agent-beacon/blob/cfbd7b1d721664e3b1b5ca2fca5eac4327532c07/CLAUDE.md) | Normal hooks have a local posture, while onboarding/cloud forwarding are separate opt-in flows. An optional policy seam fails open. Detailed test guidance has embedded-hook build prerequisites | Borrow provenance and review patterns. Do not use the optional seam as the factory's mandatory write authority or assume all Beacon flows remain local |
| [Entire guidance](https://github.com/entireio/cli/blob/ee57ff55142e5078b625dd963fc398d49a8ddda1/CLAUDE.md) | Checkpoints are commit-time metadata refs, with source identity and trusted-root constraints. They are not worktree restoration | Adapt ref publication and trace links, keep factory restore in its own tested contract and do not publish private session data automatically |
| [MFS guidance](https://github.com/zilliztech/mfs/blob/78352894a0a826ee24db852411bf9a3c49884922/CLAUDE.md) | Python server owns retrieval and indexing, the Rust client is thin, stores need reconciliation and guidance says tests are not CI-gated | Treat full adoption as an added service, measure the need first and do not equate formatting CI with retrieval correctness |
| [agentglass source tree](https://github.com/BjoernSchotte/agentglass/tree/3fdd14eafdc873f23b009a273580b4a0ff9b5ca0) | No AGENTS, CLAUDE or CONTRIBUTING path in the complete inspected tree | Absence is recorded. Source types and read projections remain the evidence basis |

All four repositories were observed unarchived on their main branch. Audited commit dates were 2026-10-22 for Beacon, 2026-10-23 for Entire, 2026-10-19 for agentglass and 2026-07-31 for MFS. Observed latest release tags were ext-v0.1.0, v0.12.0, v2026.10.13 and v0.4.6 respectively. Tag names are identities, not inferred release dates. Exact timestamps and metadata URLs are preserved in [SRC-R01 through R04](evidence-root.json). These observations do not establish future maintenance or production maturity.

## Actual research limitations

Direct outbound urllib and git access were blocked by the environment. Approved GitHub retrieval and web tools were used. This is an access limitation, not a repository defect. No installed HOI4, Clausewitz, Meshy or Blender tool appeared in the available tool inventory. No model APIs, providers, game sessions, native setup binaries or external test suites were executed. The final package can validate its own files, links, frontmatter, configs and ZIP, while runtime gates remain planned.
