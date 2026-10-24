# Users and workflows

Status: **Proposed**. Read [requirements](product-requirements.md), [visual/autonomy model](visual-and-autonomy-model.md) and [scenario evaluation](../05-verification/EVALUATION-MATRIX.md).

## Contributor roles

| Contributor | Starts with | Needs from the factory | Typical authority |
|---|---|---|---|
| Lore writer | Narrative, dates, political intent and references | Reviewable canon and previews with plain-language consequences | Propose and approve owned lore |
| Scripter | Exact IDs, effects and implementation constraints | Source editor, diagnostics, dependency lookup and raw diff | Bounded code changes and technical review |
| Artist | Original art, icons, audio or model assets | Provenance, conversion receipts and visible consumer wiring | Owned asset proposals |
| Map designer | Geographic intent, structured map or reference image | Layered inspection, topology diagnostics and bounded edits | Owned map change proposals |
| Balancing contributor | Expected behavior and scenario assumptions | Static checks, simulations and clearly scoped game observations | Proposal and scenario review |
| Maintainer | Project intent, version compatibility and merge policy | Consolidated evidence, conflicts, approvals and restore | Canon, integration and release authority |

These are collaboration responsibilities, not mandatory always-running agent personas. One person may hold several roles. The first public release uses server workspaces, GitHub exchange and invited project members with explicit roles. Real-time co-editing of one file remains later work.

## Scenario 0: Create or import a mod

After sign-in, the start screen offers three entry paths through the Setup adapter: create a new mod, upload a mod as a ZIP or folder, or import a repository from a connected GitHub account. It inspects the server workspace, the server game reference build, generated descriptors, instruction files and the resolved bootstrap manifest. The user sees discovered values with the ability to correct them.

For a new mod, the app uses Setup's actual planning and generation functions. For an import, it inventories existing source, binary assets, Git state and partial prior setup before suggesting modifications. The app separates mandatory project preparation from optional assistants and integrations. A missing optional dependency produces an actionable limited-capability result.

The review names files to create, preserve, merge or replace and displays the resolved component versions. Applying requires authorization for that concrete plan. The adapter follows the actual transaction state. Verified staging can resume. An interrupted partial apply that Setup marks unsafe to resume requires rollback. Successful-lock finalization has its own reconciliation path. No generic Resume button may conceal those differences.

The workspace handoff contains roots, profile, setup receipt, current source identity and unresolved checks. A pre-existing mod remains usable outside the app, and the user can download it or push it to GitHub at any time. Re-running onboarding should recognize already completed work and refuse to clobber changed generated files.

## Scenario 1: Lore writer describes a country

A writer supplies an outline and references. The native agent asks only for missing decisions that materially affect implementation, otherwise it presents a bounded design proposal. The writer approves lore independently from implementation detail.

The task plan links approved statements to focus, localization, event or asset work. A preview uses plain language and permits source inspection. If an inferred suggestion conflicts with canon, the conflict is visible and creates a separate canon proposal. An unsupported historical claim remains a reference question rather than silently becoming a fact.

For the first slice, the task stays within a small focus branch. Later domain adapters can add decisions and events using the same change and evidence contracts. The acceptance record distinguishes design approval, static checks and game observations.

## Scenario 2: Human and agents change related files

A scripter owns one branch or worktree while a bounded agent task owns another. Each records its base revision and identifiers. An artist may propose a replacement icon at the same time.

Before integration, the host checks text conflicts, duplicate IDs, localization keys, symbol references and binary destination collisions. It does not assume two clean textual merges are semantically compatible. A conflict card shows the common base, both intents, changed native files and affected validation. A contributor can keep one version, manually combine them or split the work into new tasks.

The merged proposal is validated and previewed again from its final bytes. Evidence from either original branch is stale if its input closure changed. No task can force-push a shared branch or erase unrelated staged work under a routine write grant.

## Scenario 3: Map intent and edit

A designer can open an existing map, inspect provinces, states, regions, terrain and related metadata, then select an intended change. The first map milestone prioritizes read-only understanding and structured import. A bounded paint operation is offered only where topology and dependent-file validation support a reliable write.

A structured map import includes known file formats, exact dimensions, identifiers and relationships. It creates a dry-run proposal with a dependency closure and collisions. A reference image is stored as a reference with attribution, scale assumptions and design notes. It does not become valid province data by being uploaded.

Creating or generating a new world is a later workflow. It requires separate geometry, ID allocation, terrain, state, strategic-region, supply and content decisions. A model can propose geography, but topology validation and actual game observation remain independent evidence. The UI labels unimplemented editors and unsupported operations honestly.

## Scenario 3B: Focus tree through visual and autonomous paths

In visual mode, the contributor selects a node, drags it within supported layout semantics, edits a supported field and reviews the resulting source diff. Unknown blocks and comments remain intact. An ambiguous relative-position construct stays read-only until a supported operation exists.

In hybrid mode, the agent proposes a branch, the person adjusts its layout, and the combined result receives fresh validation. In autonomous mode, the task proceeds under a bounded grant and can finish a reviewable branch commit without an open canvas. All three routes use the same repository, proposal and evidence machinery.

A useful preview displays focus IDs, localized labels, prerequisites, mutually exclusive links and missing dependencies. It states the source revision and fidelity. It never claims to execute rewards or reproduce engine behavior merely because it can draw the graph.

## Scenario 3C: Unattended work with intervention

The maintainer grants a task permission to read selected roots, create proposals and apply bounded changes in an owned worktree. The grant includes operation classes, budget, expiry and stop conditions. Broader permissions remain unavailable until separately authorized.

The task works while the user is absent. Its status shows current phase, progress, cost where known and any blocked dependency. It pauses for an uncertain side effect, a budget limit, a scope violation or a meaningful required approval. It does not ask the user to approve every harmless read.

The user can pause, narrow or revoke the grant. The host records the revocation epoch before allowing subsequent effects. In-flight external operations may complete despite cancellation, so the result remains pending reconciliation until an authoritative receipt resolves it. Resume starts from the durable boundary, not from a guessed conversation summary.

## Scenario 4: Switch model or return after interruption

The user changes provider or returns to the app later. The host first reconciles active attempts and verifies source state. It builds a fresh context bundle from approved intent, task state, source pointers and evidence.

If the new provider lacks required tools, images or context capacity, the UI gives a precise option such as text-only inspection or a narrower task. Data restricted to one provider destination cannot move to another provider through automatic fallback. Provider-specific private reasoning and old tool-call identifiers are not transplanted.

A resumed task presents what is confirmed, what is stale and what remains unresolved. It preserves the same grant or asks for a new grant only if authorization expired or scope changed. It must not repeat a commit, upload or external generation merely because its last response was lost.

## Scenario 5: A result is broken

The contributor reports the visible defect and, where available, supplies a game log or screenshot with game version and DLC profile. The app links the report to the exact generated artifact. It separates parser errors, missing references, unsupported tool coverage, game-specific behavior and intent mismatch.

The agent reproduces the failure at the cheapest relevant layer, proposes a scoped correction, and reruns affected checks plus preservation checks. A game-only issue remains unverified until the corresponding game scenario is observed. The server game reference supports source and static checks only. Without a user-operated game observation, analysis continues with explicit limits.

Restore is an ordinary reviewed operation. It returns owned content to the selected accepted snapshot or creates a revert on a shared branch. It preserves subsequent human work and leaves the failed result, evidence and recovery decision attributable. The learning review can produce a tested reusable lesson, a narrow bug regression or a justified no-change outcome.

## User experience acceptance

The first usability session asks a non-scripter to create a small focus branch and understand its validation limits without editing configuration. A technical contributor must reach exact source and raw diff within one deliberate navigation action from a change card. A maintainer must identify what an unattended task may do and stop further writes. These are proposed usability tests, not completed user research.

