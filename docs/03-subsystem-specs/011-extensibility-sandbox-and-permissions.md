# SPEC-011: Extensibility, sandbox and permissions

Status: **Proposed, revised for the public hosted service** by [maintainer decisions of 2026-10-24](../02-architecture/maintainer-decisions-2026-10-24.md). Primary requirements: REQ-015, REQ-033, REQ-034, REQ-042 and REQ-046. S02 delivers accounts and tenant isolation, S03 delivers scoped built-in tools, S05 expands interoperable adapters and S08 gates public launch.

Read first: [deployment and security](../02-architecture/deployment-and-security.md), [contracts](../02-architecture/contracts-and-interfaces.md), [runtime](002-native-runtime-and-models.md), [recovery](003-orchestration-and-recovery.md), [HOI4 adapters](006-hoi4-tools-and-validation.md) and [engineering guidance](012-engineering-guidance-skills-and-learning.md).

## Purpose and scope

Extensions should add HOI4 validators, importers, generators and model providers without taking control of the workspace or reaching another tenant's data. A user should understand the capability being granted and stop further effects without losing completed work. Normal operations within an existing bounded grant should proceed without repeated approval dialogs.

The initial release uses audited built-in adapters and narrowly wrapped external tools, all inside the server's Docker images. Arbitrary plugins, unrestricted shell access and user-supplied MCP executables are not required for the first focus workflow. On a public multi-tenant service, user-supplied executables would run on shared infrastructure, so they are out of scope until a demonstrated isolation boundary exists. Unreviewed executables remain disabled. A reviewed third-party executable requires demonstrated isolation before activation, with no first-release trusted-code bypass.

## Evidence and reusable boundaries

[SRC-B01, Pi tool execution](https://github.com/earendil-works/pi/blob/8a2ebd38549c3521a02db0e5d758a24320e97f94/packages/agent/src/agent-loop.ts) permits a hook to alter already-validated arguments. Its [durable tool path](https://github.com/earendil-works/pi/blob/8a2ebd38549c3521a02db0e5d758a24320e97f94/packages/durable/src/harness/tool.ts) validates again before intent. [SRC-B02, OpenCode](https://github.com/anomalyco/opencode/blob/02671e66454260d77fe04d63cf5eb94e76921dee/packages/core/src/tool/runtime.ts) has raw-schema branches that cannot substitute for closed host validation. Pi and OpenCode are no longer planned dependencies under [ADR-011](../02-architecture/maintainer-decisions-2026-10-24.md#adr-011-native-rust-agent-runtime). These findings remain counterexamples for the Rust agent crate and require enforcement after every extension hook.

[Pi MCP runtime](https://github.com/earendil-works/pi/blob/8a2ebd38549c3521a02db0e5d758a24320e97f94/packages/coding-agent/src/extensions/mcp/runtime.ts) and [OpenCode MCP client](https://github.com/anomalyco/opencode/blob/02671e66454260d77fe04d63cf5eb94e76921dee/packages/core/src/mcp/client.ts) provide transport patterns. Their existence does not establish containment of the server executable. [SRC-B08, Mastra configuration](https://github.com/mastra-ai/softwarefactory-template/blob/ebde794a8656278cf26bbb2b8764be7070b325d9/src/mastra/index.ts) distinguishes local and isolated sandbox providers. All observations here are from source, with no sandbox or permission tests executed.

## Trust and execution boundaries

| Component | Default trust and access | Required boundary |
|---|---|---|
| Rust web server and host | Trusted application authority | Own accounts, tenant checks, grants, handles, journal and accepted snapshots |
| Rust agent crate | Trusted project code, model input and output untrusted | No generic mutation tools, broker-only effects and generation checks in process or in a worker |
| Browser client | Untrusted. Any user may modify it | Authenticated session, server-side authorization, no paths, secrets or game files delivered |
| Other tenants | Untrusted to each other | Tenant filter on every query, path, artifact and run, tested by T-ACCT-01 |
| Built-in validator or converter | Audited executable with narrow input/output contract | Owned staging directory, resource limits and observed outputs |
| Third-party MCP server executable | Untrusted executable until reviewed at an exact version | Verified identity and demonstrated sandbox on shared infrastructure before activation. No first-release trusted-code bypass |
| Remote MCP service | External service with declared data flow | Per-service auth, approved outbound data and unknown-effect reconciliation |
| UI extension or preview | Untrusted presentation content unless bundled and audited | No direct server bridge, escaped rendering, content security policy and typed proposals |
| Custom model endpoint URL | User-supplied outbound destination | Private, loopback, link-local and metadata addresses blocked by default, rechecked after redirects and DNS changes |
| Repository instructions and assets | Task data, including possibly hostile text | Cannot create grants, change policy or execute automation |

A separate process provides lifecycle isolation but not necessarily filesystem isolation. A container shares the host kernel and is not proof of isolation by itself. Model-directed tools on a public service run beside other tenants' work, so a tool escape is a cross-tenant breach, not only a local risk. The architecture cannot call any component sandboxed merely because the Rust host is logically authoritative. The maintainer selected per-task containers under the gVisor runtime in [ADR-020](../02-architecture/maintainer-decisions-2026-10-24.md#adr-020-gvisor-tool-sandbox). EXP-07 must demonstrate that boundary with escape and cross-tenant tests before public launch. Plain Docker is for local development only. Until then execution is limited to trusted bundled code with constrained callable tools.

For a reviewed third-party extension, the sandbox must enforce roots, process creation, network access, tenant separation and resource limits at the kernel, container runtime or virtual machine layer. If the selected platform cannot enforce a requested restriction, the extension remains disabled. A trusted-code exception is deferred to a separately reviewed architecture decision and an explicit operator trust decision. It is not an enabled fallback in the first release. No silent downgrade from isolated to host-user execution is allowed.

## Accounts, roles and tenant isolation

Sign-in uses GitHub or email under REQ-046. Each account owns its projects, secrets, budgets and agent runs. A project owner may invite collaborators with explicit roles. Roles bound what a member may read, propose, apply, approve and publish. A grant can never exceed its grantor's role.

The server derives account and role from the authenticated session on every request. The data access layer applies the tenant filter, so a forgotten check in one handler cannot return another tenant's rows. Workspace paths resolve inside the owning tenant's root. Object store keys and Git volume paths include the project identity and are never accepted verbatim from the client.

Email must be verified before an account can run agent tasks, and GitHub sign-in counts as verified. The monthly allowance starts only after verification. Projects are private by default. An owner may make a project public, which grants a read-only view of source, previews and evidence to anyone. Copying a public project into another account is offered only when the project declares a license that permits it, and the copy keeps attribution. Abuse and rate limits apply per account, per project and per source address. Sign-up throttling, task admission limits and model budget limits are separate controls. A suspended account stops new dispatch immediately while in-flight effects settle under SPEC-003.

Removing a collaborator revokes their sessions and increments the epoch on grants they issued for that project. Their completed contributions stay attributed in Git history and receipts.

## ApprovalGrant, C-08

A grant identifies workspace/run scope, operation categories, roots, limits, expiry, policy version and revocation epoch. Categories include reading indexed source, creating a proposal, applying owned changes, executing a named validator, making a local commit, pushing an owned branch, opening a pull request, creating a repository and integrating into a shared default branch. A proposal or tool ID never grants authority.

The host evaluates the final call against the current grant immediately before an effect. It checks normalized arguments, canonical root identity, expected source hashes, executable/tool version and worker generation. Symlinks, junctions, relative traversal, case variation and replacement of a root during execution belong to the path-integrity test set.

The approval interface shows the concrete action, affected paths or destination, budget and recovery limits. A standing grant can authorize multiple bounded operations. New scope, a changed executable or a wider outbound data policy requires a revised grant. Revocation increments the epoch and stops future admission. Already-started effects are handled through SPEC-003, with no false claim that revocation undid them.

## Extension manifest and operations

An extension manifest declares identity, publisher, version, code hash, license, protocol version, executable or endpoint, input/output schemas, effect category, required roots/network destinations, resource limits and supported cancellation/reconciliation behavior. Declared read-only or idempotent hints are claims to verify, not permissions.

| Operation | Input and output | Error and recovery |
|---|---|---|
| Inspect extension | Manifest and pinned artifact return compatibility and permission plan | Missing schema, unknown license or incompatible protocol affects the relevant activation decision |
| Activate | Reviewed identity, capabilities and grant return registered adapter | Hash or capability drift rejects activation |
| Discover MCP | Selected server returns tool catalog with schema hashes | Catalog changes require reclassification before new tools execute |
| Execute | Final normalized request and C-08 return C-04 plus output references | Invalid input, scope violation, timeout or unknown effect is explicit |
| Reconcile | Operation ID and declared observation query return observed state | Unverifiable result remains unknown and cannot auto-replay |
| Update | New version and manifest diff return upgrade proposal | Old in-flight operations remain bound to old identity until settled |
| Disable | Adapter ID returns revocation and cleanup record | Running effects remain visible until settled or reconciled |

Registration rejects schemas the host cannot enforce for mutating operations. Output validation applies after hooks, with the host retaining an immutable authoritative receipt. An extension may add a comment or derived result, but cannot rewrite the recorded outcome.

## MCP and other adapters

MCP adapters negotiate the selected protocol version and capabilities, with transport-specific timeouts and explicit lifecycle ownership. Initial candidates are stdio and Streamable HTTP. A service's prompts and resource text remain untrusted context. Tool names are namespaced by registered identity, preventing a remote tool from impersonating a built-in operation.

A stdio server receives an explicit environment allowlist and owned working directory. Credentials are resolved through the host and supplied only to the component that needs them. Do not inherit the entire host environment. Remote authentication and endpoint identity are bound to the configured service, and redirects or changed hosts require the corresponding policy check.

Tool output has size and media limits. Large results become referenced artifacts with hashes and selected excerpts. Treat protocol errors, transport loss and service-side errors separately. Reconnecting a transport may be safe while retrying its previous mutating call is not.

An ACP or other agent adapter may be added behind the same contracts, but external agent products remain optional integrations. The native product's task state, approvals and source authority must survive removing any such adapter.

## Files, processes and network

Converters operate in staging directories and return output manifests. The host validates file types, declared paths, size limits and postconditions before applying C-02. Archive import checks traversal, links and resource expansion before extraction into an owned staging root. No model-produced path can override root policy.

Process supervision records executable hash/version, arguments, working directory, environment keys, start time, exit status and captured output. Arguments containing secrets are redacted while retaining a verifiable non-secret operation identity. Cancellation targets the owned process tree where supported and verifies settlement. A remote accepted job may need a service query even after the local transport stops.

Reviewed Setup post-install commands and other allowlisted validation commands run only inside the controlled tool environment for the owning project. That environment has the project workspace as its only writable root, no access to other tenants' workspaces or the server secret store, a read-only game reference mount when needed, a default-deny network policy and the same time and output limits as any tool. A command that needs anything outside that environment is refused and shown in the dry run, never run in the server process itself.

Provider requests are also outbound data operations. Project policy identifies which sources may leave the server and to which provider destinations. A task restricted to one provider cannot silently fail over to another. Custom endpoint URLs pass the outbound destination check before every request. Telemetry is separately configured and must not export raw mod content, prompts or credentials by default.

## Licensing and distribution

The project is licensed AGPL-3.0 with section 7 attribution and modification-marking terms under [ADR-017](../02-architecture/maintainer-decisions-2026-10-24.md#adr-017-open-source-license). The public service links users to its corresponding source. Maintain a release bill of materials for container base images, native binaries, Rust crates, frontend packages, extensions and generated assets. Record source revision, license text, notices, modifications and distribution decision. Reused Apache-2.0 and MIT components are compatible with inclusion and keep their notices. The MIT licenses inspected for Pi, OpenCode, OpenHands, Gas City, Gas Town, Beads and Fusion apply only if code from them is later copied. Beads also carries Apache-2.0 third-party notices. The Mastra template declares Apache-2.0 in its manifest but lacks a root license file at the inspected revision, so direct copying needs the missing distribution evidence resolved.

Tool reuse does not authorize redistribution of game data, DLC content, third-party mod assets or model weights. The server's game reference copy is mounted read-only, never baked into images and never exported, as REQ-048 requires. The game-rights review for the hosted service is a release gate before public domain exposure. Asset provenance, generated-content permissions and contributor rights are recorded by SPEC-010 and the release policy. An unknown optional dependency can remain excluded while the rest of the product proceeds.

## Verification and acceptance

| Test | Scenario | Required observation |
|---|---|---|
| EXT-01 | Tool schema or pre-execution hook changes path/type | Final host validation rejects unsafe call with no effect |
| EXT-02 | Symlink/junction or root replacement races apply | Operation cannot escape the granted root |
| EXT-03 | Revocation occurs after queueing | New effect is refused under updated epoch |
| EXT-04 | Local executable ignores cancellation | Process outcome is observed or marked unknown, never assumed rolled back |
| EXT-05 | MCP tool catalog changes during a session | Changed capability is withheld until classified |
| EXT-06 | Server returns oversized, malformed or hostile rendering content | Result is bounded and cannot execute UI code |
| EXT-07 | Remote mutation completes but response is lost | Reconciliation runs before any repeat |
| EXT-08 | Plugin attempts direct filesystem access | Actual sandbox denies it, or untrusted plugin feature is unavailable |
| EXT-09 | Task restricted to one provider triggers fallback to another | Policy blocks outbound request |
| EXT-10 | Archive contains traversal and excessive expansion | Staging rejects entries before workspace apply |
| EXT-11 | Package license or executable hash changes | Release/activation evidence becomes stale and is reviewed |
| EXT-12 | Export logs and extension diagnostics | Credential values are absent, redaction coverage is recorded |
| EXT-13 | One tenant requests another tenant's workspace, artifact, run or secret by guessed ID | Request fails as not found and no data or existence signal leaks |
| EXT-14 | Custom endpoint resolves to a private or metadata address, directly or after redirect | Outbound request is blocked before any data is sent |
| EXT-15 | A tool or export path attempts to include game reference files | Request is refused and no game bytes leave the server |
| EXT-16 | Collaborator with edit role attempts to push or approve beyond role | Host refuses, and any grant they issued cannot exceed the role |

AC-015 requires usable intervention and scoped grants. AC-033 requires demonstrated boundaries for the shipped extension surface. AC-034 requires complete distribution evidence for included items. AC-042 requires conformance fixtures for selected protocols without making them the authority. AC-046 requires tenant isolation and role enforcement under T-ACCT-01 and T-ACCT-02. Record negative-test traces, image digests, container runtime and host kernel, executable identities, source hashes and C-05 limitations. S08 must test the deployed images, since development-process permissions do not prove release containment.

