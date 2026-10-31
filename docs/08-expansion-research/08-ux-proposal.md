# UX proposal

Status: **Proposed for approval, design only (D-13)**. This document lists the important screens, the user journeys that connect them and the review interfaces. Every screen goes through the ADR-025 design loop ([GOAL-UI](../06-agent-handoffs/GOAL-UI.md) and the `ui-design-iteration` skill) before it is built, so this is a content and flow proposal, not a visual design. The guiding rule is the one the request sets: one coherent product, not a collection of disconnected tools.

## Principles

1. **One place per job.** A job page shows its spec items, relevant notes, progress, the current checkpoint, renders and the diff, so a user never hunts across tools.
2. **Progressive detail.** Plain language first ("save point", "agent's draft"), technical detail (model, tool versions, grant, budget, evidence rungs) one click away.
3. **Every waiting decision is in one queue.** Notifications point to the review queue, never to scattered pages.
4. **Pictures next to source.** A render always sits beside the change it shows, and clicking a rendered region jumps to its source span.
5. **No surprise authority.** The page always shows the job's autonomy level, what the agent may do next without asking, and one clear pause button.
6. **Accessible and translatable.** Semantic elements, keyboard use, both themes, all strings in i18n files, no inline styles.

## Screens

| Screen | Purpose | Key content |
|---|---|---|
| Project home | Orientation | Current objective, jobs in progress, review queue count, recent save points, health of checks, quick actions |
| Jobs board | Plan and track work | Columns (to do, in progress, in review, done), the proposed lane for agent suggestions, filters, a ready-now view for agents |
| Job page | Everything about one job | Objective and acceptance checklist, linked spec items and notes, autonomy level and pause, progress timeline from job events, current checkpoint card, latest render, changes tab, decision history, handoff |
| Review queue | Answer waiting decisions | Cards for intake consultations, design briefs, visual reviews, plans, diffs and merges, each with a recommendation, batch answering, filters by job and kind |
| Visual review | Approve or adjust a render | The focus tree (or event window, icon set) with the proposal highlighted, before and after toggle, layout scorecard, accept, request changes with a note, or edit directly on the canvas |
| Changes | Review a diff | Semantic summary (focuses added, prerequisites changed, keys renamed), raw diff with hunk selection, finding delta, image diffs in two-up, swipe, onion skin and difference modes |
| Focus editor | Visual editing | Canvas on the game grid with prerequisites, exclusive links and lanes, side panel with typed fields and raw script fallback, live layout verdict, render preview |
| Event and decision editors | Visual editing | Form fields beside a live preview card. Clicking a preview element focuses its field |
| Notes | Shared knowledge | List with status and kind filters, Markdown editor (CodeMirror), backlinks, references to mod files, history from Git, "proposed by agent" diffs to accept |
| Save points | Git for beginners | Timeline of save points and agent drafts, undo to any recorded point, branch view behind an advanced switch, pull requests and reviews |
| Assets | Graphics | Asset sets by category, candidates with versions, validation results, approve and install, provenance of each asset |
| Project settings | Policy | Autonomy preset and the per-checkpoint table, budgets and provider keys, image reference opt-ins, companion pairing, DLC set, collaborators |
| Help and tours | Learning | Tour list (replayable), task tutorials on a sample mod, contextual help articles |

## User journeys

1. **First visit.** Sign up, optional two-minute tour, create a sample project, complete the "first focus" tutorial: add a focus, see its render, ask the agent to change it, review the diff, make a save point.
2. **Import an existing mod.** Upload or connect GitHub, the Setup wizard runs, the import preservation report (P4) shows what was parsed, what is kept byte-exact and which findings already existed, and the findings baseline is recorded.
3. **Semi-autonomous focus branch.** The walkthrough in the [autonomy specification](04-autonomy-modes.md#walkthrough-a-focus-branch-in-semi-autonomous-mode): intake questions, design brief, render with scorecard, visual review with a direct drag, notify-level diff, merge approval, optional test-in-game card.
4. **Fully autonomous overnight job.** The user approves an objective and budget, closes the browser, and returns to one completion notice: what was done, decisions taken as agent defaults, renders, checks and evidence rungs, and the pull request waiting for merge.
5. **Human-in-the-loop event editing.** The user edits event text in the form while the agent wires effects. Each of them sees the other's proposals as they arrive, and conflicting proposals are rebased or re-proposed, never overwritten.
6. **Graphics set.** The agent plans five focus icons from the project style profile, shows candidates with validation results, the user approves three and asks for changes to two, and the approved ones are installed with shine sprites and registered.
7. **Report a game problem.** In the companion or the web page, the user clicks "Report a game problem", the companion attaches fresh logs, the agent runs the P6 reading order and opens a fix job with its findings.
8. **Collaborator review.** A contributor's job reaches the merge checkpoint, the owner reviews the changes and the render, comments, approves, and the merge runs under branch protection.

## Review interfaces in detail

**Checkpoint card.** Title, kind, job, age, the agent's recommendation with a one-line reason, the evidence (render, diff, brief), and the available answers. Every answer can carry a note. Answers are recorded in decision history.

**Visual review for focus trees.**
- **Canvas:** the full tree with new and changed focuses highlighted, plus per-branch crops.
- **Scorecard:** shows only the failing metrics with their fix hints.
- **Before and after:** a toggle for changes to an existing tree.
- **Direct edits:** a drag moves a node or branch and becomes a source-bound proposal. The verdict is recomputed at once.
- **Answers:** accept, request changes with a note, or take over editing.

**Changes view.**
- **Semantic summary:** first, in plain language.
- **Raw diff:** with hunk and line selection for staging.
- **Finding delta:** introduced, resolved and remaining findings.
- **Graphics:** image diffs for every changed asset.
- **Save point:** a "keep this draft" button that respects the merge policy.

**Decision history.** A filterable list on the job page and in project settings: question, recommendation, answer, who answered or "agent default", time, policy version, linked spec items.
