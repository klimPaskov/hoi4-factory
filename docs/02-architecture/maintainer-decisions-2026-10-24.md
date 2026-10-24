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

**Unresolved.** How pull request review comments map to factory tasks.

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
| REQ-047 | GitHub integration. Connect selected repositories, import a mod from GitHub, upload a mod without a repository, push owned branches, open pull requests and create repositories, each separately authorized and reconciled | SPEC-005 | S02 |
| REQ-048 | Server game reference. A read-only full game copy, latest build with all DLC, used for validation and previews, never exported or baked into images, with the rights review completed before public exposure | SPEC-006 | S02 |

## Remaining open questions

| Question | Owner | Decision point |
|---|---|---|
| Hosting provider and production orchestrator | Maintainer | Before S08 deployment |
| Per-task containers or a stronger sandbox runtime for agent tools | Security owner | EXP-07, before public launch |
| Game-rights review for a hosted service with free use and optional credits | Maintainer | Before public domain exposure |
| Payment provider and credit pricing | Maintainer | When the credits backlog item is scheduled |
| Mapping pull request review comments to tasks | Collaboration owner | S06 |
| Privacy policy, data retention and account deletion | Maintainer | Before public launch |
