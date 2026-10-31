# Autonomy levels specification

Status: **Proposed for approval**. Based on decision A2 (presets of one checkpoint policy, merge may be set to auto by the owner), FR-1 (upfront questions) and P12 (design brief). It extends [the visual and autonomy model](../01-product/visual-and-autonomy-model.md), [SPEC-003](../03-subsystem-specs/003-orchestration-and-recovery.md) and contract C-08 rather than replacing them.

## Two separate ideas

| Term | Meaning | Values |
|---|---|---|
| **Editing style** | How a person works on content. This is the existing three-mode model, renamed | Visual (canvas and forms), hybrid (prompt plus adjustment), chat only (the canvas may stay closed) |
| **Autonomy level** | How often a job stops for a human decision | Fully autonomous, semi-autonomous, human-in-the-loop |

Neither grants privilege. Authority always comes from the job's grant (C-08) and the project role of the person who started or approved it. Changing an editing style or an autonomy level never widens a grant.

## One job engine

Every job, whatever its level, runs on the same SPEC-003 task and attempt machinery: durable events, leases, fencing, intent before effect, reconciliation of uncertain effects, pause and revoke. The autonomy level only decides what happens at **checkpoints**.

### Checkpoint kinds

| Kind | When it occurs | What the reviewer sees |
|---|---|---|
| Intake consultation | At the start of a substantial job (FR-1) | Established requirements, decisions the agent will make itself with its defaults, and the questions that genuinely need an answer, each with a suggested answer |
| Design brief | Before content design is fixed (P12) | The filled brief and tiered open items: story gaps, design calls with a recommendation, quick confirms with defaults |
| Visual review | When a renderable artifact exists, for example a focus tree layout, an event window, an icon set | The render next to the proposal, with accept, request changes, or edit directly on the canvas |
| Plan | Before the first write | Ordered steps, files to touch, checks to run, estimated budget |
| Diff | Before commit | Semantic diff, raw diff, finding delta, P13 gap audit, evidence rungs reached |
| Commit | Before writing to the job branch history | Commit message and grouping of changes |
| Merge | Before integrating into the project's main branch | Pull request state, required checks, branch protection |
| Publish | Before any outward effect beyond the project, such as creating a public repository or a release | Exact effect and target |

### Policy values

Each checkpoint kind is set per project to **ask** (the job waits for an answer), **notify** (the job continues and the result appears in the review queue for later review) or **auto** (the job continues and the decision is only logged).

### Presets

| Checkpoint | Fully autonomous | Semi-autonomous | Human-in-the-loop |
|---|---|---|---|
| Intake consultation | Ask once, then default and log | Ask | Ask |
| Design brief | Notify | Ask | Ask |
| Visual review | Notify | Ask | Ask |
| Plan | Auto | Notify | Ask |
| Diff | Auto | Notify | Ask |
| Commit | Auto | Auto | Notify |
| Merge | Ask by default. The owner may set it to auto as a standing project rule (A2) | Ask | Ask |
| Publish | Ask | Ask | Ask |

A preset is only a starting point. Owners edit any cell, and can add narrower rules such as "always ask for focus tree layouts" or "auto-accept icon batches that pass validation". Rules are stored per project and versioned, so a job records which policy version it ran under.

Human-in-the-loop never means approving trivial operations. Routine choices inside an approved plan (file names that follow conventions, helper effect structure, ordering of edits) are made by the agent and logged.

## Rules that no level changes

1. **Destructive operations always ask**, and some are never available to agents: force push, history rewrite of shared branches, deleting a branch with unmerged work, deleting a repository, deleting files outside the job's declared scope, and permanent deletion of project data.
2. **Merges respect branch protection and required checks** and are never forced, even when merge is set to auto.
3. **Uncertain effects stop the job.** An effect in `unknown_effect` or `reconciling` is never retried blindly and is shown as a blocker.
4. **Budgets stop the job.** Reaching a per-task or per-account budget pauses the job with a clear reason.
5. **Scope changes ask.** If the agent finds that the objective needs files, systems or authority outside the approved scope, it raises a checkpoint instead of widening scope.
6. **Credentials, permissions, endpoints and canon** are never changed by a job. Canon changes become design proposals.

## Switching levels during a job

- A switch takes effect at the next checkpoint. Checkpoints already waiting keep their state.
- Moving to more asking (for example from fully autonomous to semi-autonomous) applies to every later checkpoint at once.
- Moving to less asking never widens the grant and never approves anything already waiting.
- Every switch is recorded in the job's decision history with who switched and why.

## Review queue and decision history

**Review queue.** One project-wide queue lists every waiting or notify-level checkpoint with its job, kind, age, preview (render, diff or brief) and available answers. Answers can be given in batches. A checkpoint that waits longer than a configurable time sends one notification, not a stream.

**Decision history.** Every answer, default and automatic decision becomes a record with: job, checkpoint kind, question, options offered, the recommendation, the answer, who answered or "agent default", time, policy version, and links to evidence. Answers that settle a design question are also written into the relevant specification item (FR-1, P13), so they are never asked again. Personal preferences inferred from repeated answers follow ADR-022 lane A, and nothing in decision history grants authority.

## Interruption and resumption

- A paused, revoked, crashed or budget-stopped job keeps its durable state. On resumption the agent rebuilds its context from the job's events, the decision history and the C-06 context bundle, not from a chat transcript.
- After feedback ("move the economy branch left", "make option B cost more"), the job resumes from its last durable checkpoint with the feedback attached as a new requirement on the affected spec item. Only the affected steps run again.
- A proposal that became stale because the source changed in the meantime is detected by its expected hash and rebased or re-proposed, never silently applied.

## Parallel agents

Read-only helper agents (research, review, lookups) may run in parallel at every level. When parallel writers arrive is decision A3, still open in the [decision record](00-decision-record.md). The design assumed for writers: each writer works in its own worktree with a declared set of paths, integration is serialized, the real diff is compared with the declared paths, and a job that declares no paths runs alone.

## Walkthrough: a focus branch in semi-autonomous mode

1. **Intake.** The user asks for a five-focus economic branch for Ireland. The agent reads the existing tree, the project's design notes and the relevant spec, then asks two questions with suggestions: should the branch join the existing industry branch or stand alone (suggested: join it), and should rewards use the mod's existing economy ideas (suggested: yes).
2. **Design brief.** The agent fills the brief: branch purpose, five focuses with names and rewards, prerequisites, one exclusive choice, AI weights. It lists one design call with a recommendation. The user accepts with one change.
3. **Feasibility check** (P14). The agent confirms that every effect it plans exists in the 1.19.3 documentation and that the hook it relies on fires in this mod.
4. **Layout and render.** The agent writes the focuses, lays them out with the focus layout tools and renders the tree. It evaluates the render against the focus quality rubric (FR-3) and revises until the rubric passes.
5. **Visual review checkpoint.** The user sees the render with the new branch highlighted, drags one node one column right, and approves. The edit becomes a source-bound proposal and is revalidated.
6. **Validation.** Reference closure, the focus layout verdict and the P13 gap audit pass. The diff checkpoint is at notify level, so the job continues and the diff appears in the queue.
7. **Commit.** Automatic, with a message citing the spec items.
8. **Merge checkpoint.** The job opens a pull request and waits for approval, because merge is ask in this project.
9. **Test in game card** (P7). The user optionally runs the listed checks, or the local companion runs them through HOI4 Live Tools, and the results become user-operated game evidence.

## Data model additions

| Record | Key fields |
|---|---|
| `checkpoint_policy` | project, version, checkpoint kind, value (ask, notify, auto), narrower rules, author, time |
| `checkpoint` | job, attempt, kind, state (waiting, answered, defaulted, expired), payload reference, policy version |
| `decision` | checkpoint or intake item, question, options, recommendation, answer, actor or agent default, time, linked spec items, evidence links |

All three carry account and project columns under the existing row-level security, and every state change emits a C-03 event in the same transaction.
