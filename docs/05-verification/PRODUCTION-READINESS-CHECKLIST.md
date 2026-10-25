# Production readiness checklist

Status: **Maintainer-supplied on 2026-10-24, a menu of considerations and not a requirements list.** The maintainer decides what to do and when. HOI4 Factory is a free tool for hobbyist modders, so this list favors work that is cheap, invisible to users and protects against data loss, cross-account leaks, leaked secrets or runaway cost. Enterprise, compliance and friction-adding items were removed by the maintainer on 2026-10-24 and are added back only if a real need appears. The production-readiness-review skill reads this file at each stage handoff and before any release to keep the Fit, Owner stage and Status now columns true. Release gates stay in [deployment and security](../02-architecture/deployment-and-security.md), and nothing here blocks a release by itself.

## How to use

Fit says when an item is worth doing. **now** means cheap, invisible to users and protective of data, tenants, secrets or spend, and **when needed** names the concrete trigger that makes it worth doing. The Friction column says what a user would feel, with none being the goal. Status now is one of done with an evidence path, in progress (work exists but no committed test or receipt is cited yet), planned (specified in a document but not built) or gap (nothing specified or built). Owner stage is S02 to S08 as in the [roadmap](../04-implementation-stages/ROADMAP.md) or backlog, and Where specified cites the plan or says gap.

## Identity and access

| ID | Item | Fit | Friction | Owner stage | Where specified | Status now |
|---|---|---|---|---|---|---|
| PR-001 | Login works with plus sign in email | now | none | S02 | [S02 scope](../../evidence/S02/scope.md) names email sign-in | done: crates/accounts tests, evidence/S02/live-compose.json (o'brien+s02 address) |
| PR-002 | Password reset links expire and work once | now | none | S02 | gap | done: crates/accounts password reset tests (30 minutes, single use, ends sessions) |
| PR-003 | Sessions time out and kill every session for one user | now | sign in again after a long idle period | S02 | [SPEC-011](../03-subsystem-specs/011-extensibility-sandbox-and-permissions.md) (collaborator removal revokes sessions), [S02 scope](../../evidence/S02/scope.md) (opaque session cookie) | done: crates/accounts session expiry and sign-out-everywhere tests |
| PR-004 | Roles and permissions deeper than admin/everyone | now | owners pick a role when inviting | S06 | [REQ-046](../01-product/product-requirements.md), [SPEC-011](../03-subsystem-specs/011-extensibility-sandbox-and-permissions.md), [T-ACCT-02](TEST-STRATEGY.md), [S06-W5](../04-implementation-stages/stage-06-collaboration-and-content.md) | planned |
| PR-005 | Checks stopping user A reading user B's data by changing a URL number | now | none | S02 | [SPEC-011](../03-subsystem-specs/011-extensibility-sandbox-and-permissions.md) (EXT-13), [T-ACCT-01](TEST-STRATEGY.md), [S02-W2](../04-implementation-stages/stage-02-onboarding-and-workspaces.md) | done: crates/accounts and crates/server cross-account 404 tests |
| PR-006 | Tenant isolation | now | none | S02 | [REQ-046](../01-product/product-requirements.md), [RISK-20](RISKS-DECISIONS-AND-OPEN-QUESTIONS.md), [T-ACCT-01](TEST-STRATEGY.md), [S02 scope](../../evidence/S02/scope.md) (row-level security) | done: forced row-level security, crates/accounts RLS tests, crates/server isolation tests |
| PR-007 | Login rate limits | now | none in normal use | S02 | [REQ-046](../01-product/product-requirements.md), [SPEC-011](../03-subsystem-specs/011-extensibility-sandbox-and-permissions.md) | done: crates/accounts rate limit tests |

## Data and database

| ID | Item | Fit | Friction | Owner stage | Where specified | Status now |
|---|---|---|---|---|---|---|
| PR-008 | Schema someone designed instead of improvised | now | none | S02 | [Data, context and state](../02-architecture/data-context-and-state.md), [Contracts](../02-architecture/contracts-and-interfaces.md), [S02 scope](../../evidence/S02/scope.md) (migration ranges) | done: per-crate migrations designed against the contracts, migrations/ |
| PR-009 | Migrations forward and rollback | now | none | S02 | [Deployment and security](../02-architecture/deployment-and-security.md) (reversible or restore point), [Data, context and state](../02-architecture/data-context-and-state.md) (migration section) | done: reversible .up and .down migrations, crates/server migration reversal test |
| PR-010 | Migrations that don't lock the main table at peak | when needed: when real traffic or a table big enough to block writes for seconds appears | none | backlog | gap | gap |
| PR-011 | Indexes for 5 million rows | now, on tenant and lookup keys, with a 5 million row test left for when real traffic appears | none | S02 | gap | done: indexes on tenant and lookup keys with schema tests (5 million row test left for later) |
| PR-012 | Connection pooling | now | none | S02 | [S02 scope](../../evidence/S02/scope.md) (sqlx on PostgreSQL) | done: sized sqlx pool with acquire timeout, FACTORY_DB_* settings |
| PR-013 | Nightly backups | now | none | S08 | [Deployment and security](../02-architecture/deployment-and-security.md), [Data, context and state](../02-architecture/data-context-and-state.md), [S08-W3](../04-implementation-stages/stage-08-packaging-and-pilot.md) | planned |
| PR-014 | Tested restore | now | none | S08 | [Deployment and security](../02-architecture/deployment-and-security.md) (EXP-08), [T-RELEASE-02](TEST-STRATEGY.md), [S08-W3](../04-implementation-stages/stage-08-packaging-and-pilot.md) | planned |
| PR-015 | Point-in-time recovery | when needed: when paid credits launch or losing a day of data becomes unacceptable | none | backlog | gap | gap |
| PR-016 | Soft deletes | now | deleted accounts stay restorable for 30 days | S02 | [ADR-021](../02-architecture/maintainer-decisions-2026-10-24.md), [Data, context and state](../02-architecture/data-context-and-state.md) (retention) | done: soft delete and 30-day restore for projects, accounts deletion columns |

## Data correctness

| ID | Item | Fit | Friction | Owner stage | Where specified | Status now |
|---|---|---|---|---|---|---|
| PR-017 | Server-side validation | now | none | S02 | [Contracts](../02-architecture/contracts-and-interfaces.md), [Deployment and security](../02-architecture/deployment-and-security.md) (trust boundaries) | done: server-side validation with field names in error envelopes |
| PR-018 | Timezones stored right | now | none | S02 | gap | done: timestamptz everywhere in migrations/ |
| PR-019 | Apostrophes in names | now | none | S02 | gap | done: evidence/S02/live-compose.json (apostrophe in email and name) |
| PR-020 | Emoji, accents, RTL in every field | now, as storing and showing text exactly, with RTL layout polish left for a user who needs it | none | S02 | gap (stage 2 Unicode fixtures cover paths only) | done: evidence/S02/live-compose.json (accent, emoji and Arabic stored exactly) |
| PR-021 | Money as integers | when needed: when paid credits launch, but cheap to get right in the S03 allowance ledger | none | S03 | gap | gap |
| PR-022 | Currency, rounding and tax per country | when needed: when paid credits launch | prices shown in one currency | backlog | [Backlog](../04-implementation-stages/BACKLOG-AND-DEFERRED-SCOPE.md) (Stripe row names tax registration) | gap |
| PR-023 | Data model that survives "one small change" | now | none | S02 | [Contracts](../02-architecture/contracts-and-interfaces.md) (versioned schemas), [Data, context and state](../02-architecture/data-context-and-state.md) (migrations) | planned |
| PR-024 | Idempotency keys on payments | when needed: when paid credits launch | none | backlog | [Backlog](../04-implementation-stages/BACKLOG-AND-DEFERRED-SCOPE.md) (metered exactly once), [Contracts](../02-architecture/contracts-and-interfaces.md) (idempotency on remote mutations) | planned |
| PR-025 | Dedup of repeated submits | now | none, a double click starts one run | S03 | [Contracts](../02-architecture/contracts-and-interfaces.md), [S03-W2](../04-implementation-stages/stage-03-native-agent-and-durability.md), [S03-W4](../04-implementation-stages/stage-03-native-agent-and-durability.md) | planned |
| PR-026 | Pagination on page 400 | when needed: when events or task lists pass a few hundred rows | none | S04 | gap | gap |
| PR-027 | Imports surviving a 2009 CSV | now, read as old mod uploads with odd encodings, BOMs and CRLF | none | S02 | [Test strategy](TEST-STRATEGY.md) (F-SETUP-IMPORT, F-LOCALIZATION), [SPEC-001](../03-subsystem-specs/001-onboarding-and-mod-setup.md) | done: crates/intake upload tests keep BOM, CRLF and binary bytes |
| PR-028 | Retention rules and deletion code | now | none | S08 | [Deployment and security](../02-architecture/deployment-and-security.md), [Data, context and state](../02-architecture/data-context-and-state.md), [ADR-021](../02-architecture/maintainer-decisions-2026-10-24.md), [RISK-25](RISKS-DECISIONS-AND-OPEN-QUESTIONS.md), [S08-W3](../04-implementation-stages/stage-08-packaging-and-pilot.md) | planned |

## Security

| ID | Item | Fit | Friction | Owner stage | Where specified | Status now |
|---|---|---|---|---|---|---|
| PR-029 | Secrets out of code, repo and AI chat history | now | none | S02 | [Deployment and security](../02-architecture/deployment-and-security.md) (credentials), [AGENTS.md](../../AGENTS.md), .gitignore excludes .env, no secret scanner yet | done: .gitignore, .env.example, key file mounts, image and repository scans in S01 and S02 |
| PR-030 | Rotating every key an AI pasted into config | now | none | S08 | gap | gap |
| PR-031 | Confirming every package the AI installed is real and intended | now | none | S02 | [Deployment and security](../02-architecture/deployment-and-security.md) (supply chain), [T-RELEASE-03](TEST-STRATEGY.md) | planned |
| PR-032 | Dependency scanning | now | none | S08 | gap | gap |
| PR-033 | Input sanitization | now | none | S02 | [Deployment and security](../02-architecture/deployment-and-security.md), [SPEC-011](../03-subsystem-specs/011-extensibility-sandbox-and-permissions.md) | in progress: typed validation on every S02 route, review again when the agent writes source in S03 |
| PR-034 | SQL injection, XSS, CSRF and OWASP | now | none | S02 | [Deployment and security](../02-architecture/deployment-and-security.md) (CSRF, CSP, escaping), [T-SEC-01](TEST-STRATEGY.md) | done: parameterized queries, origin guard on mutations, CSP and escaping in the client |
| PR-035 | File uploads scanned, size-limited, non-executable storage | now | an upload size limit message | S02 | [Deployment and security](../02-architecture/deployment-and-security.md), [SPEC-011](../03-subsystem-specs/011-extensibility-sandbox-and-permissions.md) (EXT-10), [S02-W3](../04-implementation-stages/stage-02-onboarding-and-workspaces.md), malware scanning not specified | done: size, count, path and type limits with non-executable storage (malware scanning not added) |
| PR-036 | CORS not allow-all | now | none | S02 | [Deployment and security](../02-architecture/deployment-and-security.md) (strict origin checks) | done: crates/server CORS tests reject foreign origins |
| PR-037 | Security headers | now | none | S02 | [Deployment and security](../02-architecture/deployment-and-security.md) (CSP only) | done: crates/server security header tests |
| PR-038 | Encryption at rest and in transit | now | none | S08 | [Deployment and security](../02-architecture/deployment-and-security.md) (secret store encrypted at rest), TLS and disk encryption not specified | gap |
| PR-039 | Least privilege including the AI agent with production credentials | now | none | S03 | [SPEC-011](../03-subsystem-specs/011-extensibility-sandbox-and-permissions.md), [Deployment and security](../02-architecture/deployment-and-security.md), [ADR-020](../02-architecture/maintainer-decisions-2026-10-24.md) | planned |
| PR-040 | Logs of who did what without passwords, tokens, card numbers | now | none | S02 | [Deployment and security](../02-architecture/deployment-and-security.md) (redact before log persistence), [SPEC-011](../03-subsystem-specs/011-extensibility-sandbox-and-permissions.md) (EXT-12) | done: crates/server log redaction test |
| PR-041 | Vulnerability disclosure process | now, as a SECURITY.md with a contact address | none | S08 | gap | gap |
| PR-042 | Stolen laptop plan | now, as disk encryption and a short key revocation list | none | S08 | gap | gap |

## Compliance and privacy

| ID | Item | Fit | Friction | Owner stage | Where specified | Status now |
|---|---|---|---|---|---|---|
| PR-043 | PCI scope and card numbers out of the DB | when needed: when paid credits launch, using Stripe-hosted checkout only so card numbers never touch our systems and only SAQ A scope applies | none | backlog | [ADR-019](../02-architecture/maintainer-decisions-2026-10-24.md) | gap |
| PR-044 | GDPR deletion reaching every table, backup and log | now, scoped to tables, artifacts and derived memory, with backups aging out and logs redacted | none | S08 | [Deployment and security](../02-architecture/deployment-and-security.md), [Data, context and state](../02-architecture/data-context-and-state.md), [ADR-021](../02-architecture/maintainer-decisions-2026-10-24.md), [RISK-25](RISKS-DECISIONS-AND-OPEN-QUESTIONS.md), [SPEC-004](../03-subsystem-specs/004-context-memory-and-runtime-learning.md) (MEM-10) | planned |

## Environments and delivery

| ID | Item | Fit | Friction | Owner stage | Where specified | Status now |
|---|---|---|---|---|---|---|
| PR-045 | Dev, staging and prod that match | now | none | S08 | [Deployment and security](../02-architecture/deployment-and-security.md), [S08-W1](../04-implementation-stages/stage-08-packaging-and-pilot.md), [T-DEPLOY-01](TEST-STRATEGY.md) | planned |
| PR-046 | Infrastructure as code | now | none | S08 | [Deployment and security](../02-architecture/deployment-and-security.md), compose.yaml in the repository root | in progress |
| PR-047 | Deploy pipeline running tests | now | none | S08 | gap | gap |
| PR-048 | One-click rollback | now | none | S08 | [S08](../04-implementation-stages/stage-08-packaging-and-pilot.md) (risk and rollback: redeploy the previous image digest) | planned |
| PR-049 | Zero-downtime deploys | when needed: when paid credits launch or restarts disturb real users, tasks already recover after restarts | a short outage at each deploy until then | backlog | gap | gap |
| PR-050 | Feature flags | now, as a kill switch and a way to hide unfinished features | none | S08 | [RISK-24](RISKS-DECISIONS-AND-OPEN-QUESTIONS.md) (kill switch) | planned |

## Infrastructure and reliability

| ID | Item | Fit | Friction | Owner stage | Where specified | Status now |
|---|---|---|---|---|---|---|
| PR-051 | CDN | when needed: when the static bundle is slow far from the host | none | backlog | gap | gap |
| PR-052 | DNS, SSL and renewals | now, as auto-renewing certificates plus an expiry alert | none | S08 | gap | gap |
| PR-053 | Background job queues | now | none | S03 | [SPEC-003](../03-subsystem-specs/003-orchestration-and-recovery.md), [S03-W2](../04-implementation-stages/stage-03-native-agent-and-durability.md) | planned |
| PR-054 | Retries with backoff | now | none | S03 | [SPEC-003](../03-subsystem-specs/003-orchestration-and-recovery.md), [Contracts](../02-architecture/contracts-and-interfaces.md) (retry classes), [RISK-23](RISKS-DECISIONS-AND-OPEN-QUESTIONS.md) | planned |
| PR-055 | Dead-letter queue | when needed: when a job class such as webhooks fails repeatedly unseen, blocked and unknown-effect states serve until then | none | backlog | [SPEC-003](../03-subsystem-specs/003-orchestration-and-recovery.md) | planned |
| PR-056 | Cron jobs that don't run twice with two servers | now, for deletion purge, stale lessons and webhook reconciliation during rolling deploys | none | S03 | [SPEC-003](../03-subsystem-specs/003-orchestration-and-recovery.md) (leases for tasks only) | gap |
| PR-057 | Caching and invalidation | now, where a stale index would mislead the agent | none | S05 | [SPEC-004](../03-subsystem-specs/004-context-memory-and-runtime-learning.md), [S05-W3](../04-implementation-stages/stage-05-context-and-recovery-hardening.md) | planned |
| PR-058 | Written and tested disaster recovery plan | now, as short restore steps and one tested restore | none | S08 | [Deployment and security](../02-architecture/deployment-and-security.md) (EXP-08), [S08-W3](../04-implementation-stages/stage-08-packaging-and-pilot.md), recovery time and point targets not set | planned |

## Email

| ID | Item | Fit | Friction | Owner stage | Where specified | Status now |
|---|---|---|---|---|---|---|
| PR-059 | SPF, DKIM, DMARC | now, so verification and reset mail reaches inboxes | none | S08 | [S02 scope](../../evidence/S02/scope.md) (production mail decided at S08) | gap |
| PR-060 | Bounce and complaint handling | when needed: when volume or bounce rate threatens sender reputation | none | backlog | gap | gap |
| PR-061 | Emails rendering in every client | now, kept simple with plain text and minimal HTML | none | S02 | gap | gap |

## Cost and storage

| ID | Item | Fit | Friction | Owner stage | Where specified | Status now |
|---|---|---|---|---|---|---|
| PR-062 | Storage rules for old temp files | now | workspace and upload size limits | S08 | [Data, context and state](../02-architecture/data-context-and-state.md), [T-PERF-03](TEST-STRATEGY.md), [RISK-24](RISKS-DECISIONS-AND-OPEN-QUESTIONS.md) | planned |
| PR-063 | Cost alerts for runaway agent loops | now | a task stops at its budget | S03 | [REQ-003](../01-product/product-requirements.md), [SPEC-002](../03-subsystem-specs/002-native-runtime-and-models.md), [RISK-24](RISKS-DECISIONS-AND-OPEN-QUESTIONS.md), [T-RUN-06](TEST-STRATEGY.md), operator alert not specified | planned |

## Observability and operations

| ID | Item | Fit | Friction | Owner stage | Where specified | Status now |
|---|---|---|---|---|---|---|
| PR-064 | Structured searchable logs | now, as JSON logs with request IDs and the host's log viewer | none | S08 | [Contracts](../02-architecture/contracts-and-interfaces.md) (request IDs) | gap |
| PR-065 | Error tracking that groups bugs | when needed: when logs alone cannot keep up with bug reports | none | backlog | gap | gap |
| PR-066 | External uptime checks | now, as one free check | none | S08 | gap | gap |
| PR-067 | Dashboards for latency, errors, throughput | when needed: when load or latency questions come up | none | backlog | gap | gap |
| PR-068 | Runbooks | now, as short restore, rollback and kill switch steps | none | S08 | [S08](../04-implementation-stages/stage-08-packaging-and-pilot.md) (backup and restore instructions) | planned |
| PR-069 | Postmortems that change something | when needed: after the first real outage or data incident, the lesson loop can record the change | none | backlog | gap | gap |
| PR-070 | Status page | when needed: when paid credits launch or outages hit many users | none | backlog | gap | gap |
| PR-071 | Admin panel for ops | now, minimal: suspend account, kill switch, pause a global lesson | none | S08 | [SPEC-011](../03-subsystem-specs/011-extensibility-sandbox-and-permissions.md) (suspended accounts), [RISK-24](RISKS-DECISIONS-AND-OPEN-QUESTIONS.md), [ADR-022](../02-architecture/maintainer-decisions-2026-10-24.md) | planned |
| PR-072 | Support tooling | when needed: when paid credits launch or support volume grows | none | backlog | gap | gap |

## Testing

| ID | Item | Fit | Friction | Owner stage | Where specified | Status now |
|---|---|---|---|---|---|---|
| PR-073 | Unit tests by someone who knows the logic | now | none | S02 | [Test strategy](TEST-STRATEGY.md), crates/workspace/tests, crates/game-ref/tests | in progress |
| PR-074 | Integration tests for every third-party API | now for GitHub, Anthropic and OpenAI, Stripe when credits launch | none | S03 | [Test strategy](TEST-STRATEGY.md) (T-GH-01 to T-GH-03, T-RUN-05), [S01 checks](../../evidence/S01/checks.json) (live probes) | in progress |
| PR-075 | End-to-end tests for signup and money flows | now for sign-up and the focus slice, money flows when credits launch | none | S04 | [T-ACCT-02](TEST-STRATEGY.md), [T-RELEASE-04](TEST-STRATEGY.md) | planned |
| PR-076 | Load testing before launch | when needed: before announcing the service publicly or expecting real traffic | none | S08 | gap | gap |
| PR-077 | Production-like test data that is not production data | now | none | S02 | [Test strategy](TEST-STRATEGY.md) (fixture design, synthetic only) | in progress |
| PR-078 | Regression tests | now | none | S02 | [ADR-023](../02-architecture/maintainer-decisions-2026-10-24.md), [T-DEV-04](TEST-STRATEGY.md), the pinned S01-L06 test in crates/agent/src/openai.rs | in progress |
| PR-079 | Testing on an old slow laptop | when needed: if users report slowness | slow pages for those users | S04 | [T-PERF-01](TEST-STRATEGY.md) (reference hardware only) | gap |

## Accessibility and UX

| ID | Item | Fit | Friction | Owner stage | Where specified | Status now |
|---|---|---|---|---|---|---|
| PR-080 | Accessibility compliance | now, as semantic HTML, labels and contrast, with no formal audit | none | S04 | [SPEC-007](../03-subsystem-specs/007-visual-state-synchronization.md) (VIS-10 keyboard only, no baseline chosen) | gap |
| PR-081 | Pages that load on hotel wifi | now, as a small bundle and cached assets | slower first load on bad networks | S04 | [T-PERF-01](TEST-STRATEGY.md) (cached preview target, no network budget) | gap |
| PR-082 | Empty, loading and error states | now | none | S04 | [REQ-010](../01-product/product-requirements.md) | gap |
| PR-083 | Actionable error messages | now | none | S02 | [Contracts](../02-architecture/contracts-and-interfaces.md) (error codes with user-facing explanation) | done: error envelope codes mapped to actionable client messages |
| PR-084 | Forms that keep input on validation failure | now | none | S02 | gap | done: web/src/test form retention test |
| PR-085 | Notifications users can turn off | now, only if non-essential notifications are sent | none | S06 | gap | gap |
| PR-086 | Undo | now | none | S04 | [REQ-024](../01-product/product-requirements.md), [SPEC-007](../03-subsystem-specs/007-visual-state-synchronization.md) | planned |
| PR-087 | Keyboard and screen reader support | now | none | S04 | [SPEC-007](../03-subsystem-specs/007-visual-state-synchronization.md) (VIS-10), [SPEC-009](../03-subsystem-specs/009-focus-and-other-editors.md), screen reader checks not specified | planned |
| PR-088 | Translations with locale formatting | now, as translatable strings and locale-aware dates, English only at launch | none | S02 | [ADR-021](../02-architecture/maintainer-decisions-2026-10-24.md), [S02 scope](../../evidence/S02/scope.md) | done: all client strings through i18n, Intl formatting |
| PR-089 | A back button that works | now | none | S04 | gap | gap |

## Billing

| ID | Item | Fit | Friction | Owner stage | Where specified | Status now |
|---|---|---|---|---|---|---|
| PR-090 | Sales tax and VAT | when needed: when paid credits launch | none | backlog | [ADR-019](../02-architecture/maintainer-decisions-2026-10-24.md), [Backlog](../04-implementation-stages/BACKLOG-AND-DEFERRED-SCOPE.md) | planned |
| PR-091 | Refunds and credits | when needed: when paid credits launch | none | backlog | [Backlog](../04-implementation-stages/BACKLOG-AND-DEFERRED-SCOPE.md) (named as an uncertainty) | gap |
| PR-092 | Usage metering matching billing to the cent | when needed: when paid credits launch, S03 metering is the base | none | S03 | [SPEC-002](../03-subsystem-specs/002-native-runtime-and-models.md), [Backlog](../04-implementation-stages/BACKLOG-AND-DEFERRED-SCOPE.md), [T-RUN-06](TEST-STRATEGY.md) | planned |
| PR-093 | Revenue reports finance can reconcile | when needed: when paid credits launch | none | backlog | gap | gap |

## API and integrations

| ID | Item | Fit | Friction | Owner stage | Where specified | Status now |
|---|---|---|---|---|---|---|
| PR-094 | Webhooks that verify signatures | now | none | S06 | [ADR-018](../02-architecture/maintainer-decisions-2026-10-24.md), [Deployment and security](../02-architecture/deployment-and-security.md), [SPEC-005](../03-subsystem-specs/005-git-collaboration-and-review.md), [T-GH-03](TEST-STRATEGY.md) | planned |
| PR-095 | Webhooks that retry with a delivery log | now, as inbound dedupe plus periodic reconciliation, no outbound webhooks are offered | none | S06 | [ADR-018](../02-architecture/maintainer-decisions-2026-10-24.md), [SPEC-005](../03-subsystem-specs/005-git-collaboration-and-review.md), delivery log not specified | planned |
| PR-096 | Versioned API | now | none | S02 | [S02 scope](../../evidence/S02/scope.md) (/api/v1), [Contracts](../02-architecture/contracts-and-interfaces.md) | done: /api/v1 routes |
| PR-097 | Customer API keys create and revoke | when needed: if a public API or MCP export ships | none | backlog | [Backlog](../04-implementation-stages/BACKLOG-AND-DEFERRED-SCOPE.md) (P3 MCP export) | gap |
| PR-098 | API rate limits | now | none in normal use | S02 | [REQ-046](../01-product/product-requirements.md), [SPEC-011](../03-subsystem-specs/011-extensibility-sandbox-and-permissions.md) | in progress: sign-in and sign-up rate limits only, upload and setup routes not yet limited |
| PR-099 | Plan for a third-party API changing format | now, light, as capability probes and a provider fault corpus | none | S03 | [SPEC-002](../03-subsystem-specs/002-native-runtime-and-models.md), [RISK-19](RISKS-DECISIONS-AND-OPEN-QUESTIONS.md) | planned |
| PR-100 | OAuth token refresh | now | none, users stay signed in with GitHub | S02 | [ADR-018](../02-architecture/maintainer-decisions-2026-10-24.md) names user-to-server tokens, refresh not specified | gap |
| PR-101 | Integration sandbox | now | none | S02 | [Test strategy](TEST-STRATEGY.md) (F-GITHUB-REPO), [S02 scope](../../evidence/S02/scope.md), [S01 checks](../../evidence/S01/checks.json) (disposable repository) | done: hoi4-factory-test-mod and the fake GitHub in crates/intake tests |

## AI and model use

| ID | Item | Fit | Friction | Owner stage | Where specified | Status now |
|---|---|---|---|---|---|---|
| PR-102 | Prompt injection protection in every field reaching the model | now | none | S03 | [SPEC-004](../03-subsystem-specs/004-context-memory-and-runtime-learning.md), [SPEC-011](../03-subsystem-specs/011-extensibility-sandbox-and-permissions.md), [T-SEC-01](TEST-STRATEGY.md), [T-MEM-03](TEST-STRATEGY.md) | planned |
| PR-103 | Customer data kept out of prompts that leave the network | now | none | S03 | [SPEC-011](../03-subsystem-specs/011-extensibility-sandbox-and-permissions.md), [Deployment and security](../02-architecture/deployment-and-security.md) | planned |
| PR-104 | Per-customer model spend cap | now | a task stops at its budget | S03 | [REQ-003](../01-product/product-requirements.md), [SPEC-002](../03-subsystem-specs/002-native-runtime-and-models.md), [RISK-24](RISKS-DECISIONS-AND-OPEN-QUESTIONS.md) | planned |
| PR-105 | Evals for model updates | now, light, as a rerun of the scenario matrix on a model change | none | S08 | [Evaluation matrix](EVALUATION-MATRIX.md), [REQ-041](../01-product/product-requirements.md), [S08-W4](../04-implementation-stages/stage-08-packaging-and-pilot.md) | planned |
| PR-106 | Fallback when the model provider is down | now, only to destinations the user already approved, otherwise a clear error | a notice when the model switches | S05 | [SPEC-002](../03-subsystem-specs/002-native-runtime-and-models.md), [S05-W5](../04-implementation-stages/stage-05-context-and-recovery-hardening.md) | planned |
| PR-107 | Migration plan for model deprecation | when needed: when a pinned model announces retirement | none | backlog | [SPEC-002](../03-subsystem-specs/002-native-runtime-and-models.md) (capability profiles only) | gap |
| PR-108 | Prompt and output logs minus what you can't keep | now | none | S03 | [Data, context and state](../02-architecture/data-context-and-state.md) (retention), [SPEC-011](../03-subsystem-specs/011-extensibility-sandbox-and-permissions.md), [Deployment and security](../02-architecture/deployment-and-security.md) | planned |
| PR-109 | Human approval for irreversible AI actions | now | approval prompts for push, pull request and merge, standing grants avoid repeats | S03 | [SPEC-011](../03-subsystem-specs/011-extensibility-sandbox-and-permissions.md) (grants), [ADR-014](../02-architecture/maintainer-decisions-2026-10-24.md), [ADR-018](../02-architecture/maintainer-decisions-2026-10-24.md) | planned |

## Code and team health

| ID | Item | Fit | Friction | Owner stage | Where specified | Status now |
|---|---|---|---|---|---|---|
| PR-110 | Readable code | now | none | S02 | gap | gap |
| PR-111 | Code review on every change | when needed: when a second contributor joins, the maintainer reviews agent output until then | none | backlog | [AGENTS.md](../../AGENTS.md) (diff review) | gap |
| PR-112 | Branching strategy | now, kept small as main plus short-lived branches | none | S02 | [AGENTS.md](../../AGENTS.md) (owned branches) | in progress |
| PR-113 | Monthly dependency upgrades | when needed: when scanning flags a release or a framework nears end of support | none | backlog | [REQ-043](../01-product/product-requirements.md), [Deployment and security](../02-architecture/deployment-and-security.md) (update policy) | planned |
| PR-114 | Framework upgrades before end of support | when needed: check end-of-support dates at each release | none | backlog | [REQ-043](../01-product/product-requirements.md) | planned |
| PR-115 | Deleting dead AI code | now, as a cheap diff review step | none | S08 | gap | gap |
| PR-116 | Splitting 2,000-line files | when needed: when a file nears 2,000 lines, the largest is 1,120 lines at b328f8a | none | backlog | gap | gap |
| PR-117 | Architecture decisions written down | now | none | S02 | [maintainer decisions](../02-architecture/maintainer-decisions-2026-10-24.md) | done: `docs/02-architecture/options-and-decisions.md` and `docs/02-architecture/maintainer-decisions-2026-10-24.md` (ADR-001 to ADR-023) |
| PR-118 | A README that gets a new engineer running in under an hour | now | none | S08 | [Environment bootstrap](../07-developer-workflow/ENVIRONMENT-BOOTSTRAP-AND-CHECKS.md), [README.md](../../README.md) still says implementation has not started | gap |
| PR-119 | Documentation for inheritors | when needed: if the project is ever handed over, the handoff template is the base | none | backlog | [Stage handoff template](../06-agent-handoffs/STAGE-HANDOFF-TEMPLATE.md) | planned |

## Business and ownership

| ID | Item | Fit | Friction | Owner stage | Where specified | Status now |
|---|---|---|---|---|---|---|
| PR-120 | Customer-readable changelog | when needed: when users ask what changed, GitHub release notes are enough | none | backlog | gap | gap |
| PR-121 | Terms and privacy policy matching the code | now | a link or one checkbox at sign-up | S08 | [Deployment and security](../02-architecture/deployment-and-security.md), [RISK-25](RISKS-DECISIONS-AND-OPEN-QUESTIONS.md), [maintainer decisions](../02-architecture/maintainer-decisions-2026-10-24.md) (open questions), [S08-W6](../04-implementation-stages/stage-08-packaging-and-pilot.md) | planned |
| PR-122 | Accounts, domains and repos in the company's name | when needed: when paid credits launch, since Stripe and tax registration need a legal owner | none | backlog | gap | gap |
| PR-123 | Bus factor above one | when needed: when real users depend on the service, a written recovery note is the cheap step | none | backlog | gap | gap |

## Worth doing now

These are the rows with Fit now and Status gap, grouped by owner stage. Everything marked when needed waits for its trigger.

- **S02**: PR-019 apostrophes in names, PR-020 emoji, accents, RTL in every field, PR-061 emails rendering in every client, PR-084 forms that keep input on validation failure, PR-100 oAuth token refresh, PR-110 readable code
- **S03**: PR-056 cron jobs that don't run twice with two servers
- **S04**: PR-080 accessibility compliance, PR-081 pages that load on hotel wifi, PR-082 empty, loading and error states, PR-089 a back button that works
- **S06**: PR-085 notifications users can turn off
- **S08**: PR-030 rotating every key an AI pasted into config, PR-032 dependency scanning, PR-038 encryption at rest and in transit, PR-041 vulnerability disclosure process, PR-042 stolen laptop plan, PR-047 deploy pipeline running tests, PR-052 DNS, SSL and renewals, PR-059 SPF, DKIM, DMARC, PR-064 structured searchable logs, PR-066 external uptime checks, PR-115 deleting dead AI code, PR-118 a README that gets a new engineer running in under an hour
