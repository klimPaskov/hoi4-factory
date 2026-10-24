# Maintainer decisions, 2026-10-24

Status: **Recorded maintainer decisions**. Approving owner: the project maintainer, in a Claude Code planning session on 2026-10-24. Evidence base: the v02 planning package, imported as the first commit of this repository. These are product direction decisions, not experimental results. No probe, build, container or model call was run to support them.

Implementation remains deferred. The maintainer stated that the application will be developed later. No stage is authorized by this record. The planning documents were revised in the same change so a future session can begin from a consistent baseline.

## Summary

| ADR | Decision | Replaces |
|---|---|---|
| ADR-011 | Native Rust agent runtime, React with TypeScript user interface | Pi or OpenCode Node worker in ADR-002 |
| ADR-012 | Public hosted web application with accounts | Local desktop application in ADR-002 and deployment |
| ADR-013 | Docker images and Compose as the deployment unit, portable across Linux hosts | Per-platform desktop installers |
| ADR-014 | Native GitHub integration for sign-in, import, publication and pull requests | Generic Git remotes only |
| ADR-015 | Server-held full game reference copy, latest build with all DLC | Game files read only from the user's machine |
| ADR-016 | Free use with bring-your-own model keys and an operator credit allowance | Local credentials only |
| ADR-017 | AGPL-3.0 open-source license with attribution terms | No license chosen |
| ADR-018 | GitHub issues, pull requests, reviews and merges fully inside the app | Opening GitHub for collaboration |
| ADR-019 | Claude and OpenAI adapters, monthly allowance on any model, Stripe for credits | Unchosen providers and payments |
| ADR-020 | gVisor runtime for per-task tool containers | Unchosen sandbox runtime |
| ADR-021 | Verified email, 30-day deletion grace, private or public projects, license-respecting copies, English with translatable strings | Unchosen account policies |
| ADR-022 | Automatic personal, project and global memory with verified lessons and no authority | Review-every-lesson runtime learning |
| ADR-023 | Automatic engineering learning gated by regression tests | Review-every-lesson engineering learning |
| ADR-024 | Agent edits canonical CLAUDE.md and the skills directly, AGENTS.md is a generated identical copy | Test-gated guidance promotion and the AGENTS import |

ADR-001 and ADR-003 through ADR-010 remain in force, with the scope changes noted below. ADR-002 is superseded by ADR-011 and ADR-012.

## ADR-011: native Rust agent runtime

**Decision.** Write the model loop in Rust inside the same Cargo workspace as the server. That covers provider adapters, stream assembly, tool-call parsing, final argument validation, cancellation, usage accounting and retry classification. Do not package a Node worker. Do not embed Pi, pi-durable or OpenCode. The browser user interface is React with TypeScript.

**Why.** Maintainer preference. It also removes the Rust/Node process boundary, two-language debugging and Node packaging that the [candidate comparison](options-and-decisions.md#comparison) listed as Candidate A costs. Host-side revalidation of final tool arguments was already required for every candidate, so the runtime library never held that authority.

**Alternatives preserved.** Candidate A (Pi worker), Candidate B (OpenCode) and the OpenHands Python service remain documented reference designs. Their source and tests stay useful as behavioral references, for example Pi's mutable pre-tool hooks and OpenCode's at-least-once restart behavior. They are no longer planned dependencies.

**Costs accepted.** The [small custom loop](options-and-decisions.md#other-approaches-considered) warning now applies directly. The project owns stream assembly, malformed tool calls, provider error classes, context conditioning, cancellation, usage accounting and recovery. Each provider adapter must be probed and maintained by this project.

**Constraints.** Prefer vetted Rust crates for HTTP, server-sent events, JSON Schema validation and serialization over hand-written substitutes, following master goal principle 1. The agent crate holds no filesystem, Git or grant authority. It requests effects through the host tool broker exactly as [SPEC-002](../03-subsystem-specs/002-native-runtime-and-models.md) requires of any runtime. Keep the provider adapter boundary narrow so a different runtime can still replace it.

**Unresolved.** Whether a third-party Rust provider abstraction is adopted or thin project adapters are written. Whether the loop runs inside the server process or in a supervised worker process or container, which affects the worker-generation fencing tests in SPEC-002. S01 measures both questions.

**Reconsideration trigger.** Probe evidence shows that adapter maintenance or fault handling costs materially more than an adapted library, or a required provider capability cannot be supported at reasonable cost.

## ADR-012: public hosted web application

**Decision.** Deliver the product as a public web service used in a browser, not as an installed desktop application. Anyone may sign up. The backend is a Rust web server. Tauri and Electron shells are dropped.

**Accounts.** Sign-in uses GitHub or email. GitHub sign-in also offers repository linking under ADR-014. Email accounts can link GitHub later. Each account owns its projects, secrets, budgets and agent runs. Users, workspaces, tasks and secrets are isolated per account and per project.

**Collaboration.** A project owner can invite collaborators into the same server workspace from the first public release. Invited members receive explicit project roles. All existing ownership, lease, fencing and review rules in SPEC-003 and SPEC-005 apply between human collaborators and agents. Real-time co-editing of one file remains deferred. Concurrent work still uses owned branches or worktrees and reviewed integration.

**Source of truth.** Native mod files and Git history remain the creative source, now held in server-side workspaces. The escape hatch (REQ-044) becomes download as ZIP, clone or push to the user's GitHub repository. The application never stores mod content only in a proprietary format.

**Mod Setup reuse.** ADR-001 stands with a narrower binding. The portable Rust planning core runs on the server against server workspaces. The desktop scanner and native transaction services are adapted to operate on server paths. Launcher integration and the user's local Documents folder are out of scope. Installing a finished mod for play happens on the user's machine through normal HOI4 means, outside this application. EXP-01 verifies this binding.

**Model endpoints.** Hosted API providers and any OpenAI-compatible endpoint URL the server can reach are supported. A user's own localhost is not reachable and is not promised. Custom endpoint URLs are an outbound request risk, so the server blocks private, loopback and link-local destinations by default. A self-hosting operator may allow them explicitly.

**Why.** Maintainer requirement.

**Consequences.** A public multi-tenant service adds authentication, tenant isolation, abuse and rate limits, privacy obligations, data retention, backups and operations work that the desktop plan did not have. Agent tool execution now runs on shared infrastructure, so RISK-06 containment becomes a server security requirement before public launch.

## ADR-013: Docker deployment

**Decision.** Ship the Rust server and its bundled tool adapters as Docker images. Development, self-hosting and the public service start from the same images. Docker Compose describes the service set. The design stays portable to any Linux host. No hosting provider is chosen yet.

**Defaults chosen in this record.** PostgreSQL replaces local SQLite for multi-user operational state. Artifact bytes go to an S3-compatible object store or a mounted volume behind one interface. Git repositories live on a persistent volume. These defaults may be revised by S01 evidence.

**Constraints.** A container shares the host kernel and is not proof of isolation by itself, in the same way the baseline already says a Git worktree or separate process is not a sandbox. If containers separate users' workspaces or agent tools, that containment needs its own evidence under SPEC-011 and EXP-07. A public service running model-directed tools should evaluate a stronger runtime boundary than plain containers. Images must not contain game or DLC files, credentials or user mod data. Pin base images and record their digests.

**Unresolved.** Whether each workspace or task gets its own container, and the production orchestrator.

**Reconsideration trigger.** A chosen hosting platform cannot run the images, or measured startup and isolation costs make per-task containers impractical.

## ADR-014: native GitHub integration

**Decision.** A user can connect a factory project to GitHub. That covers signing in with GitHub, granting access to chosen repositories, importing an existing mod from a GitHub repository, pushing owned branches, opening pull requests for review and creating a new repository for a mod that has none. A mod with no repository can also be uploaded as a ZIP or folder into a server workspace with server-side Git history, and published to GitHub later.

**Coverage in the existing package.** [SPEC-005](../03-subsystem-specs/005-git-collaboration-and-review.md) already defined generic Git remotes. Publication there is an explicit, separately authorized operation with a remote receipt, and a lost push response is reconciled before any retry (GIT-09). It did not define GitHub accounts, repository import, pull requests or repository creation. Those parts are new scope recorded as REQ-047.

**Constraints.** Approval to edit files does not grant push, pull request creation or repository creation, matching SPEC-005. Request only the repositories the user selects. Prefer a GitHub App with short-lived installation tokens over long-lived personal tokens. Tokens live in the server secret store, never in mod history, evidence or logs. Pushing to a shared default branch stays a separately authorized action. Pull request and repository creation, like push, reconcile an uncertain outcome before retrying.

**Defaults chosen in this record.** GitHub is the only forge in the first release. Other forges are a backlog item.

**Extended by ADR-018**, which makes GitHub issues, pull requests and reviews fully usable inside the application.

**Reconsideration trigger.** GitHub terms or rate limits block the intended flow, or users need a forge GitHub cannot represent.

## ADR-015: server-held game reference

**Decision.** The server holds a full copy of Hearts of Iron IV, the latest build with all DLC, as a shared read-only reference. Validation, reference lookups, previews and asset resolution read it. Mods targeting an older game version get a visible compatibility warning rather than a matching older build.

**Constraints.** Game files are mounted read-only into the processes that need them. They are never baked into images, never included in user exports or pull requests, and never served to users as raw files. Only derived results such as diagnostics, rendered previews of a user's own mod and reference excerpts needed for a check leave the server. Each evidence record names the exact game build and DLC set used.

**Rights.** The maintainer deferred the game-rights review. It is a release gate. The review must be completed and recorded before the service is exposed on a public domain. The maintainer expects free use, with optional paid credits, to fit the publisher's terms. This record does not decide that question.

**Game verification.** The server reference copy supports source, static and rendering evidence. Running the game on the server is not planned. Actual game observations remain user-operated and are labeled as such.

## ADR-016: free use, bring-your-own keys and credits

**Decision.** The application is free to use. A user can bring their own model API key, stored encrypted in the server secret store and used only for that user's requests. The operator can also fund model use through a credit allowance. Buying additional credits from the operator is a later monetization feature recorded in the backlog.

**Constraints.** Budgets, metering and the unknown-cost rules in SPEC-002 apply per user and per task regardless of who pays. A user's key is never used for another user. A consumer chat subscription is still not assumed to grant API access.

## ADR-017: open-source license

**Decision.** Publish the repository as open source under the GNU Affero General Public License version 3. Under its section 7, add two terms. Modified versions must preserve the author attributions in the NOTICE file. Modified versions must be marked as different from the original and must not misrepresent their origin.

**Why.** The maintainer wants to prevent others from taking the project, modifying it and presenting it as their own. No open-source license can forbid modified versions. The AGPL comes closest to this intent. Anyone who runs a modified version as a network service must offer its source under the same license, keep the notices and identify the change. Trademark protection of the project name is separate from the license and is not established by this record. This is not legal advice.

**Compatibility.** Reused components are Apache-2.0 (HOI4-Mod-Setup, hoi4-agent-tools, HOI4-Live-Tools, HOIIVUtils) and MIT (CWTools, its HOI4 rules, hoi4-modmaker). Both are compatible with inclusion in an AGPL-3.0 work, and their own notices must be retained. The S08 dependency inventory still verifies every transitive license. Nova remains comparison only.

## Follow-up decisions

The maintainer answered the remaining product questions later on 2026-10-24.

### ADR-018: GitHub as the in-app collaboration hub

**Decision.** Users never need to open GitHub. Everything the project needs from GitHub is available inside the application: issues, pull requests, reviews, comments, labels, assignees, branches, check results and merges. All collaboration on a project happens in one place.

**Scope.** Issues can be listed, filtered, created, edited, commented on, labeled, assigned, closed and reopened, and linked to factory tasks. Pull requests can be opened, viewed with the factory's semantic and raw diff, reviewed with line comments, approved or sent back with requested changes, updated and merged. Check and status results from GitHub are shown beside the factory's own evidence. A member can turn any issue or review comment into an agent task with one click. The task still needs normal approval before an agent edits anything.

**Source of truth.** GitHub remains authoritative for issues, pull requests, reviews and comments. The application mirrors them through GitHub App webhooks with signature verification, plus periodic reconciliation for missed deliveries. It never keeps a second, divergent copy of a discussion. Actions taken in the app are performed on GitHub as the acting user where GitHub supports user-to-server tokens, so authorship stays correct on GitHub.

**Constraints.** Merging, closing, and changing labels or assignees are effects with grants, like push. Merges respect branch protection and required checks and are never forced. Each GitHub write reconciles an uncertain outcome before retrying. App permissions widen to issues and pull requests read and write, checks and statuses read, and metadata, requested only for repositories the user selects. A project without a GitHub repository gets local tasks and reviews until it is published.

**Supersedes.** The narrower operation list in ADR-014 and the open question about mapping review comments to tasks.

### ADR-019: model providers, allowance and payments

**Decision.** The first release ships native adapters for Anthropic (Claude) and OpenAI. The OpenAI-compatible adapter also serves self-hosted endpoints. Each verified user receives a small monthly operator-funded allowance that can be spent on any supported model, after which they bring their own key or later buy credits. Credit purchases will use Stripe. The operator then handles tax registration and compliance, which the backlog item must cover.

### ADR-020: gVisor tool sandbox

**Decision.** Model-directed tools and allowlisted commands on the public service run in per-task containers under the gVisor runtime. EXP-07 still has to demonstrate the boundary with escape and cross-tenant tests before public launch. Plain Docker is acceptable for local development only.

### ADR-021: account and project policies

**Decision.**

| Policy | Rule |
|---|---|
| Sign-up abuse | Email must be verified, and GitHub sign-in counts as verified. Rate, task and spend limits apply per account. The free allowance starts only after verification |
| Account deletion | The account is disabled at once and permanently deleted after a 30-day grace period unless restored. The user's GitHub repositories are never touched. Projects that also have other owners are kept for them |
| Project visibility | Projects are private by default. An owner may make one public, giving a read-only view of its source, previews and evidence |
| Copying public projects | Another user may copy a public project into their account only when the project declares a license that permits it. Attribution is kept |
| Interface language | English at launch, with all interface text in translatable strings so languages can be added later |

### ADR-022: automatic memory, feedback and self-improvement

**Decision.** The agent learns automatically from feedback and outcomes so it becomes personal to each user and stops repeating mistakes. Three memory scopes exist. Learning runs in four lanes decided by fixed Rust rules, never by the model. Memory carries no authority.

| Scope | Holds | Visible to |
|---|---|---|
| Personal | Portable preferences and habits of one user, such as verbosity, explanation depth, review style and visual or hybrid mode. Never project names, paths, tags or lore | That user only |
| Project | Conventions, mistake-avoidance lessons and project facts, shared by all members | Project members by role. Not part of a public project view or a public copy |
| Global | General HOI4 scripting and tool lessons that help every user | Every task, as product knowledge |

| Lane | What | How it activates |
|---|---|---|
| A | Personal preferences stated by the user, or repeated in the same direction at least twice | Immediately. A single inferred signal is applied on probation and dropped on contradiction or a thumbs-down |
| B | Project conventions, mistake-avoidance lessons and facts | Automatically after verification: a convention must be observable in the accepted source, and a lesson must pass the replay gate below |
| C | A lesson that conflicts with another member's active item, or one member's preference that would bind the whole project | Owner review. Until then it applies only to its author's tasks if portable |
| D | Canon or lore changes, grants, permissions, tools, endpoints, budgets, statements about another user, secrets | Never stored as memory. Canon changes become design proposals. Permission changes appear at most as a settings suggestion the user applies |

**Verification gate.** A lesson from a failed validation followed by a working fix first tries a deterministic check: if it can be compiled into a validator rule or source query, that rule must flag the original failure and pass the fix. Otherwise the original case is replayed with and without the lesson (at least three samples each), the lesson-free run must still reproduce the failure, and the lesson must improve the outcome. A do-no-harm sample of recent successful tasks must not regress. After activation every use is attributed, and repeated harm pauses the lesson automatically. Thresholds are starting values that EXP-05 calibrates.

**Global lessons.** Learned automatically, with strict checks and no human review. Only de-identified signals of the form "validator rule R failed, then fix pattern F passed" are used, never raw user text, and a candidate needs the same signal from at least five unrelated projects. It must pass the verification gate across those projects' frozen cases plus a do-no-harm sample before it ships as versioned product knowledge. The maintainer can pause or remove any global lesson. A user's contribution of de-identified signals follows their learning switch.

**Feedback signals.** Thumbs up or down, "remember this", "don't do that again" and "forget that", user edits to an agent proposal before accepting it, rejected proposals with a reason, reverted commits, closed agent pull requests and requested changes, failed validations followed by fixes, user-reported game errors, repeated corrections and stopped runs.

**Writer and safety.** A background reflection job with a budget and a single tool that proposes memory. It never copies text from imported mods, game files, the web, uploads or GitHub content by non-members, and refers to those by ID. Memory reaches the model as a labelled, attributed data block that states it grants nothing, and the host still checks every action against grants. Writes and loads are scanned for injection patterns. Every write has a ledger row. Owners can pause or delete everything one contributor added.

**User control.** Personal learning is on by default with an off switch. Users can delete memories ("forget that", the Undo on each "memory updated" notice, or delete all) and switch learning off. There is no page to browse or edit memories, by maintainer choice. The research recommended one, so this is a reconsideration point if users ask what the agent remembers. Project owners can delete project memory and pause a contributor's items. Deletion removes the item, its index entries, cached payloads and anything derived only from it.

**Defaults chosen in this record.** Lane B activates project-wide by default. Verification replays and the background reflection job are paid by the operator, outside user budgets and allowances, under a daily cap. The off switch stops new learning and signal sharing and hides existing personal items until it is turned back on. Deleting is what forget, Undo and delete all do. Active project memory takes precedence over a global lesson, and such a conflict counts as feedback against the global lesson. Lessons from a member who leaves stay with the project, still credited to them, and the owner may delete them. No modeling of user traits beyond stated or repeated preferences, and sensitive personal traits are never stored. Exporting project memory into the mod repository is off unless the owner turns it on.

**Staleness and conflict.** An item goes stale when the files, identifiers, game build, DLC set or tool versions it refers to change, and is re-verified on its next matching task. Newer statements supersede older ones without erasing history. Conflicts between members are never resolved automatically. Unused items become stale after 60 days and are archived 60 days later.

**Isolation.** Every memory and feedback row carries its account and project. PostgreSQL row-level security and a scoped data-access layer mean no query can run without a scope. A task never sees another user's personal memory or another project's memory.

**Build or reuse.** Build natively in Rust on PostgreSQL with full-text search first. Borrow designs, not code, from Hermes Agent (background review, do-not-capture list, write ledger, frozen snapshots, character caps), Letta (commit per reflection), mem0 (supersede and merge), Graphiti (validity time), Honcho (evidence for each conclusion) and Agent Beacon (review state machine). Research: [Hermes memory](https://github.com/NousResearch/hermes-agent/blob/f7fb481502bd3ef32be19e7034ba566801d7bf85/website/docs/user-guide/features/memory.md), [Hermes background review](https://github.com/NousResearch/hermes-agent/blob/f7fb481502bd3ef32be19e7034ba566801d7bf85/agent/background_review.py), [Letta reflection](https://github.com/letta-ai/letta-code/blob/75d7921f1666b92b4a82acca64e5ef09001c1eb3/src/agent/subagents/builtin/reflection-v2.md), [mem0 supersede](https://github.com/mem0ai/mem0/blob/2c81af3bbe27d9bccb09c772d583df9e9be72ee5/docs/platform/features/dream.mdx), [Graphiti invalidation](https://github.com/getzep/graphiti/blob/8ce6d633bdcd3d178b3a47bc25b105dce106a754/graphiti_core/utils/maintenance/edge_operations.py), [Honcho deletion](https://github.com/plastic-labs/honcho/blob/5bf3b3489db4295d249c0fbe11797fee82e108ba/docs/v3/documentation/features/advanced/deleting-data.mdx), [Beacon memory](https://github.com/Asymptote-Labs/agent-beacon/blob/473c18434d7336cccb34ef9ca3694339088bbbc8/docs/concepts/cross-harness-memory.mdx) and the [MINJA memory poisoning paper](https://arxiv.org/abs/2503.03704). None of the systems checked verifies that a lesson prevents its original failure before activation, which is the gap the verification gate closes.

**Supersedes.** The review-every-lesson rule in REQ-020 and SPEC-004.

### ADR-023: automatic engineering learning with tests as the gate

**Decision.** The factory's own development process improves itself automatically. At each stage handoff the agent runs the validation and lesson-capture workflows itself. A lesson that is backed by a passing regression test, and that changes no authority, access root, external data transfer, paid operation, approved product scope or game design canon, is promoted into guidance automatically and recorded in the knowledge changelog. Anything outside that bound still needs owner review. No agent may widen its own permissions through a guidance or skill edit.

**Effect on skills.** `validate-stage` and `capture-engineering-lessons` become callable by the agent. `start-implementation-stage` stays invocable only by the maintainer, because starting a stage is an authorization step. `audit-hoi4-integrations` stays maintainer-invoked.

### S03 inputs (maintainer answers, 2026-10-24)

| Topic | Decision |
|---|---|
| Free allowance | $1 of model usage per verified user per month, funded by the operator |
| Default model | Claude Haiku 5.5 (`claude-haiku-5-5`) when the user has not chosen one |
| Self-hosted endpoint profile | Verify connection, model listing and the outbound destination policy only. Do not run inference on a local model |
| OpenAI profile | Live inference unavailable until an OpenAI organization with API balance exists. The adapter is tested against recorded and refused responses |

### ADR-024: the agent maintains its own guidance

**Decision.** CLAUDE.md is the canonical instruction file, and AGENTS.md is a byte-identical copy generated by `scripts/sync_skills.py`. The implementing agent edits CLAUDE.md and the skills itself whenever it learns something that would help future development, and creates new skills whenever a workflow repeats or a task type would benefit from a written procedure. No test gate or maintainer review is needed for these edits.

**Limit.** A guidance or skill edit must never widen authority, access roots, external data transfer, paid operations, approved product scope or game canon. Those still need a maintainer decision.

**Supersedes.** The regression-test gate of ADR-023 for guidance changes. Regression tests are still added where a lesson is testable. ADR-023's automatic validation and lesson capture at handoff stay.

## Effects on existing decisions

| ADR | Effect |
|---|---|
| ADR-001 | Kept. Desktop services adapted to server workspaces, launcher integration dropped |
| ADR-002 | Superseded by ADR-011 and ADR-012 |
| ADR-003 | Kept. The host is now the Rust server |
| ADR-004 | Kept in intent. PostgreSQL replaces SQLite for operational state |
| ADR-005, ADR-006, ADR-009, ADR-010 | Unchanged |
| ADR-007 | Kept for agents. Human collaborators are invited earlier, still under owned branches and reviewed integration |
| ADR-008 | Unchanged. Claude Code remains the development client |

## Requirement changes

Existing requirement IDs are kept. Wording changed for REQ-002, REQ-003, REQ-006, REQ-007, REQ-021, REQ-027, REQ-034, REQ-035, REQ-041 and REQ-044. Three requirements were added.

| ID | Requirement | Spec | First stage |
|---|---|---|---|
| REQ-046 | Accounts and tenant isolation. Public sign-up through GitHub or email, per-account and per-project isolation of workspaces, agent runs, secrets and budgets, project roles for invited collaborators, abuse and rate limits | SPEC-011 | S02 |
| REQ-047 | GitHub integration. Work with GitHub entirely inside the app, from repository connection, import and upload through issues, pull requests, reviews, comments, checks and merges, mirrored by verified webhooks, with every write separately authorized and reconciled (extended by ADR-018) | SPEC-005 | S02 |
| REQ-048 | Server game reference. A read-only full game copy, latest build with all DLC, used for validation and previews, never exported or baked into images, with the rights review completed before public exposure | SPEC-006 | S02 |

## Remaining open questions

| Question | Owner | Decision point |
|---|---|---|
| Hosting provider and production orchestrator | Maintainer | Before S08 deployment |
| gVisor containment evidence for agent tools | Security owner | EXP-07, before public launch |
| Game-rights review for a hosted service with free use and optional credits | Maintainer | Before public domain exposure |
| Credit pricing (the monthly allowance is set at $1) | Maintainer | When credits are scheduled |
| Privacy policy text and hosting jurisdiction | Maintainer | Before public launch |
