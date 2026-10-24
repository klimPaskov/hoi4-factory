# SPEC-004: Context, memory and runtime learning

Status: **Proposed**. Primary requirements: REQ-008, REQ-018, REQ-019 and REQ-020. Delivery: foundational source identity in S02, initial context in S03, complete handoff and learning workflow in S05.

Read first: [architecture and authority](../02-architecture/data-context-and-state.md), [runtime](002-native-runtime-and-models.md), [recovery](003-orchestration-and-recovery.md), [engineering learning](012-engineering-guidance-skills-and-learning.md) and [evaluation matrix](../05-verification/EVALUATION-MATRIX.md).

## Purpose and scope

A lore writer must be able to say what a country believes, approve a design and return later without finding that a model summary has silently become canon. A scripter must be able to see why an agent selected a particular effect or source file. A different model must recover the task from durable facts and visible evidence. This subsystem supplies those abilities. It does not promise perfect recall, preserve a provider's private reasoning, or make every conversation sentence a permanent rule.

The first version uses exact identifiers, paths, dependency lookup and full-text retrieval. Semantic retrieval is an optional experiment after failures on a measured task set. The context budget belongs to each model profile and request. The source config's document-loading byte limit is unrelated to the model's total context window.

## Evidence and reuse decision

[SRC-R01, Beacon](https://github.com/Asymptote-Labs/agent-beacon/blob/cfbd7b1d721664e3b1b5ca2fca5eac4327532c07/cli/beacon/internal/learning/candidate.go) separates a candidate from an approved memory and writes both review records in a transaction. Its [failure and concurrent review tests](https://github.com/Asymptote-Labs/agent-beacon/blob/cfbd7b1d721664e3b1b5ca2fca5eac4327532c07/cli/beacon/internal/learning/candidate_transaction_test.go) are useful test designs. These are source observations, with no execution claimed.

[SRC-R02, Entire](https://github.com/entireio/cli/blob/ee57ff55142e5078b625dd963fc398d49a8ddda1/api/checkpoint/interfaces.go) offers a useful distinction between checkpoint, session and task records. [SRC-R04, MFS](https://github.com/zilliztech/mfs/blob/78352894a0a826ee24db852411bf9a3c49884922/server/python/tests/test_engine_cancel_reconcile.py) illustrates why cancelled derived indexing needs reconciliation. Adopt the invariants and test shapes. Directly embedding these entire systems would add stores, services and CLI assumptions that the first product has no demonstrated need for.

## Authority and data ownership

| Information | Authority | Storage proposal | Mutation rule |
|---|---|---|---|
| Editable scripts, localization and original assets | Mod repository | Native paths in Git, optional Git LFS policy for large owned assets | Reviewed source proposal |
| Approved lore and requirements | Named project owner | Versioned Markdown or JSON under a configurable canon root | Explicit canon approval |
| Accepted engineering policy | Repository maintainer | AGENTS, skills, architecture decisions and tests | SPEC-012 promotion policy |
| Task state, grants and attempt outcomes | Native host | Local transactional database | Validated state transition |
| Candidate runtime memories | Evidence-backed suggestion | Local database with optional review export | Candidate review |
| Search index, summaries and dependency graph | Derived projection | Rebuildable local cache | Recompute from identified inputs |

Default proposed project metadata is `.hoi4factory/project.json`. If that path already exists with an unknown schema, onboarding produces a conflict instead of replacing it. Runtime databases and credentials live in user application data, outside the mod's normal Git history. An explicit export may include redacted run evidence and a manifest of omitted private fields.

## ContextBundle contract, C-06

Input is workspace ID, task ID, exact source snapshot, model capability profile, selected canon/rule versions and maximum token budget. Output contains:

- Approved requirement IDs and source citations, including owner and approval revision.
- Current task objective, bounded scope, completed operations and unresolved outcomes.
- Relevant source locators with relative path, content hash, byte or line region and retrieval reason.
- Tool capabilities actually available, with version and checks that remain unavailable.
- Recent visible conversation needed for intent, plus a concise factual handoff summary.
- Candidate memories with an explicit unapproved label when the task permits consulting them.
- A context manifest listing included, excluded and truncated sources and the budget calculation.

A bundle's identity hashes its manifest and source versions. It is immutable once attached to an attempt. A provider request uses a derived rendering of that bundle. Provider adapters may change serialization, tokenization and feature conditioning while preserving the same approved facts. Provider-private reasoning blocks, credentials and stale tool-call identifiers are excluded from cross-provider handoffs.

The context builder reserves space for output and tool results. When required material does not fit, it removes lower-priority derived summaries first, then offers a narrower task or retrieval step. It must never silently truncate the approved scope, critical safety constraints or unresolved side effects.

## Operations and state transitions

`buildContext` returns a bundle or a typed error such as `required_context_too_large`, `source_missing` or `workspace_changed`. `explainContext` exposes why each item was included without exposing hidden model reasoning. `invalidate` takes changed source hashes, a dependency set or a game/tool version change and marks affected projections stale. `refresh` builds replacement projections and atomically changes the current index pointer only after all required artifacts are ready.

A `KnowledgeCandidate`, C-07, carries a domain, statement, scope, evidence IDs, source/version bounds, author, applicable task classes, counterexample and proposed destination. Its states are `candidate`, `in_review`, `approved`, `rejected`, `superseded` and `stale`. A review records reviewer, reason, candidate hash, resulting memory version and any superseded item. Competing approval and rejection requests have one winner through an expected-version check.

Approval of a runtime lesson does not approve a canon change. A suggestion that alters lore becomes a separate design proposal with its own accountable owner. A lesson about the factory's development belongs in the engineering workflow, with a Git diff and regression evidence. The UI names these destinations clearly.

## Freshness and recovery

Each derived item records its source set, extractor version, profile and build timestamp. A source edit, renamed identifier, changed load order, DLC change or tool upgrade invalidates only the affected closure where that closure is known. Unknown dependencies invalidate the broader relevant index. Time-to-live alone is inadequate because source can change immediately after a successful lookup.

If indexing dies mid-build, the previous complete index remains readable but is labelled stale where inputs changed. Partial new rows cannot appear as current. A cancelled embedding or conversion job must not publish orphan search results. Reopening the app checks index manifest hashes against current source before serving authoritative-looking results.

When switching models, pause at an attempt boundary, reconcile in-flight effects, persist a new provider profile and construct a fresh bundle. Preserve tool receipts, approval scope and task identity. If the new provider cannot handle images, tools or the required context, expose a specific degraded mode or stop that operation. A model switch cannot broaden file scope or send local-only material to a remote endpoint.

## Observable acceptance tests

| Test | Fixture and operation | Required result |
|---|---|---|
| CTX-01 | Approved lore says monarchy, an unreviewed conversation proposes republic | Planning can show both, generated accepted changes follow approved monarchy until canon review |
| CTX-02 | Modify a scripted effect used by a focus after indexing | Its prior semantic evidence becomes stale before the next proposal is applied |
| CTX-03 | Switch from API profile A to B after a tool result | New request contains task facts and receipt references, no provider-private reasoning or repeated write |
| CTX-04 | Switch to local profile with a smaller context limit | Builder stays within the measured profile budget or returns a specific narrowing request |
| CTX-05 | Inject failure between candidate review writes | No approved memory appears without its matching review transition |
| CTX-06 | Two reviewers approve and reject the same candidate concurrently | Exactly one expected-version transition succeeds and both actions remain attributable |
| CTX-07 | Cancel index build after derived records are produced | Previous index remains consistent, cancelled generation contributes no current search hit |
| CTX-08 | Remove source or revoke its data-sharing permission | Next context excludes it, cached provider-ready payloads are invalidated |
| CTX-09 | Change project root or use a linked worktree | Stable workspace identity remains explicit, unrelated project memory is never selected |
| CTX-10 | A plausible lesson lacks repeatable evidence | It remains a candidate or is rejected, with no instruction or canon mutation |

S05 must run these against production storage and context assembly. Mocks may replace remote transport and clocks, not the authority checks being evaluated. Report source coverage, context omissions, retrieval precision on fixture answers and human corrections. Proposed targets are in the [evaluation matrix](../05-verification/EVALUATION-MATRIX.md). No product acceptance test has run in this planning assignment.

## Open experiments

EXP-05 measures retrieval quality and stale-answer prevention before introducing embeddings. EXP-03 verifies model switches and cancelled tool outcomes. EXP-08 checks a restored database against Git and artifact manifests. Results may change caching or retrieval implementation without changing canon ownership. Adding a hosted shared memory service requires a new privacy, conflict and access-control design.

