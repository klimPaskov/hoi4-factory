# Evaluation matrix

Status: **Proposed evaluation protocol. No product model trials or paid API calls were run during planning.** Revised on 2026-10-24 for the [maintainer decisions](../02-architecture/maintainer-decisions-2026-10-24.md). All profiles run through the native Rust agent loop in the hosted server. A source-level capability is a reason to test a profile, not a model performance result. Exact models and provider terms are resolved and pinned when an authorized experiment runs.

## Three distinct provider configurations

| Profile | Proposed configuration | Prerequisites and controls | What comparison establishes |
|---|---|---|---|
| M-API-A | One supported Anthropic API model through the Rust runtime's adapter | User-authorized API account, exact model ID and endpoint, recorded tool/schema/context behavior, current price snapshot and destination grant | Baseline capable API workflow and its observed limitations |
| M-API-B | One supported OpenAI API model through its own adapter | Separate authorized account/API access, exact model ID, current endpoint and price snapshot, same product permissions and task budget | A genuinely different provider, including handoff and schema differences |
| M-LOCAL-C | One self-hosted instruction model behind an OpenAI-compatible endpoint, such as Ollama or llama.cpp, reachable from the server | Exact server version, model and tokenizer identifiers, weight hash, license, quantization, context limit, recorded hardware and an operator-approved destination | Self-hosted or lower marginal-cost operation, capability degradation and data-destination boundary |

These are test configurations to instantiate, not preselected model recommendations. Do not invent a current model identifier, assume all compatible endpoints implement the same features, or reuse a consumer subscription as API entitlement. Resolve the permitted models from the actual account and current provider documentation. Record why the selected API models are comparable enough for the task and why the self-hosted model fits its endpoint hardware.

For the self-hosted test, record CPU, RAM, accelerator/VRAM, driver and quantization of the endpoint host. A user's own localhost is not reachable from the hosted server and is not a supported M-LOCAL-C configuration. If no usable reachable endpoint is available, mark M-LOCAL-C unavailable and preserve its unexecuted cells. Do not silently substitute a third API provider and call it self-hosted. A narrow release can declare its actual supported profiles after review. Full planned three-profile acceptance requires real results.

Each profile has a capability receipt covering tool calling, strict or approximate schema support, streaming, image input, context limit, cancellation semantics, usage reporting and retry policy. An unsupported feature is a legitimate outcome. The product must degrade explicitly while preserving task and authority invariants.

## Required scenario matrix

Run each scenario against the same fixture revision and approved acceptance rules. Use three independent repetitions per profile for an initial exploratory pilot. This yields 9 scenarios by 3 profiles by 3 repetitions, or 81 episodes. The sample is sufficient to expose obvious failure modes, not to make broad statistical claims about model superiority.

| Scenario | Input and bounded task | Required result | Primary checks | Earliest meaningful stage |
|---|---|---|---|---|
| E00N, new mod | Create a new synthetic mod through reused Setup, then request a small focus branch | Valid workspace handoff, native change, source preview, owned branch result and an authorized pull request on a disposable GitHub repository | T-SETUP-01, T-RUN-01, T-VIS-02, T-GH-02 | S04 |
| E00I, import | Import a fixture from a disposable GitHub repository with custom guidance, dirty Git and modified generated files | Preserve all unapproved bytes and Git state, surface conflicts, rerun safely | T-SETUP-02, T-SETUP-03, T-GIT-01, T-GH-01 | S04 |
| E01, lore contribution | Approved political history plus one unresolved branch decision | Reviewable design, unresolved question remains explicit, implementation follows accepted canon and links to source | T-CTX-01, T-VIS-02, T-CTX-04 | S05 |
| E02, collaboration | Invited artist changes an icon, owner changes focus logic, agent edits localization, then introduce a same-focus conflict | Attribution and dependency closure survive, conflict blocks silent integration, both contributions remain recoverable | T-GIT-02, T-CONTENT-01, T-CONTENT-02, T-DAG-01 | S06 |
| E03, map intent | Import complete structured map files and a separate reference sketch, propose one supported edit | Source-linked preview, bounded edit and topology evidence, reference remains design input, unavailable world synthesis is explicit | T-MAP-01, T-MAP-02, T-MAP-03, T-HOI-02 | S07 |
| E03B, focus modes | Equivalent three-to-five-node intent through visual, hybrid and no-canvas paths | All paths produce accepted native semantics and source-preserving edits, live projections update correctly | T-VIS-01, T-VIS-02, T-VIS-03 | S04 |
| E03C, delegated work | Approve a bounded task, close the browser, pause, edit a supported property and resume | Agent progresses independently, revocation stops new effects, stale work is reconciled, final review is complete | T-RUN-03, T-RUN-04, T-RUN-06, T-VIS-03 | S05, controlled multi-worker variant in S06 |
| E04, swap and recovery | Interrupt after an effect may have occurred, switch provider to a different capability profile | Same task/receipt identity, relevant context retained, unsupported features explicit, unsafe effects not replayed | T-CTX-03, T-RUN-04, T-RUN-05 | S05 |
| E05, broken result | Introduce valid-looking source with a broken reference, then a controlled simulated or observed game failure | Completion blocked, bounded correction records evidence, and the resulting lesson activates only after the SPEC-004 verification gate shows it flags or fixes the original failure without regressing past successes | T-HOI-01, T-HOI-02, T-CTX-04, T-DEV-04 | S05, game variant when authorized |

E03B contains three subpaths. One episode includes all three against isolated copies of the same starting fixture. The budget applies to the whole episode. If that is infeasible at the current provider price or model speed, revise the budget prospectively or narrow the experiment before execution. Do not discard expensive unsuccessful episodes.

E04 uses A to B for all three episodes starting under M-API-A, B to C for all three starting under M-API-B, and C to A for all three starting under M-LOCAL-C. These nine episodes retain their starting-profile row while reporting both profiles' usage. If a destination lacks a required capability, correct narrowing or refusal is a safety success and a feature limitation. Report it separately from successful task completion.

E03 tests supported structured operations. It does not assert that the full fictional-world generation aspiration is implemented. S06 is required for the first public release, so E02 must run before that release. A release that defers S07 reports E03 out of supported scope with explicit release exclusions. Full S07 acceptance requires its supported operations to run.

## Measurements and denominators

| Metric | Definition | Interpretation |
|---|---|---|
| Accepted-change rate | Episodes whose bounded objective is accepted by the reviewer divided by all started episodes | Includes failed, timed-out and budget-stopped episodes in the denominator |
| First-pass acceptance | Accepted without a correction attempt or manual source repair | Separates generation quality from eventual recovery |
| Human rework | Active minutes and changed source units required after the initial proposed result | Record review time separately from repair time |
| Regression count | Previously passing named assertions that fail after the accepted change | Zero integrity regressions is a gate, not an average |
| Unauthorized effects | Disallowed writes, requests, disclosures or publications | Any confirmed instance blocks the affected release scope |
| Preservation failures | Unexpected changed bytes or lost user work outside approved mutations | Any confirmed instance blocks the affected write capability |
| Time to usable artifact | Wall time from accepted task start to reviewable accepted source with required evidence | Break out model time, tool time, queueing and human waiting |
| Model cost | Actual billed/usage-derived amount when available, with pricing timestamp and payment source (user key or operator allowance) | Unknown cost stays unknown and is reported alongside known totals |
| Self-hosted resource cost | Wall time, energy estimate only if measured, peak RAM/VRAM and token throughput when exposed | No invented dollar equivalence with API cost |
| Tool reliability | Failed, timed-out, denied and unknown-effect calls divided by all dispatched calls | Distinguish intended denials from infrastructure failures |
| Recovery success | Injected incidents restored to a valid known state without duplicate effects divided by injected incidents | Remaining blocked uncertainty is safer than false success, report both |
| Conflict detection | Correct required collisions detected, missed collisions and false positives | Include semantic and binary conflicts beyond textual merge |
| Canon fidelity | Approved constraints satisfied, invented canon claims and omitted unresolved questions | Reviewer uses a fixed answer key and cites source |
| Context retention | Required prior facts available at resume with correct provenance | Stale facts or inaccessible original chat do not count |
| Usability | Task completion, assistance, wrong turns and evidence-label comprehension | Small pilot observations, not population statistics |

An episode is accepted only against the predeclared task and evidence scope. A model saying “done,” a new commit or a green parser does not establish acceptance. The reviewer sees source changes, evidence and unresolved checks. Record reviewer identity and resolve disagreements through the approved criteria.

## Cost and stopping rules

The initial proposed exploratory cap is 3 USD for all API usage within an episode and 20 minutes of total model/tool runtime per episode, excluding explicitly recorded human waiting. Including the E04 handoffs, 57 episodes may incur API usage, giving a maximum planned model budget of 171 USD. Thirty episodes may contain self-hosted endpoint compute, bounded above by 10 hours of endpoint runtime. Mixed episodes share the same 20-minute total cap, so these are conservative ceilings rather than additive expected durations. These are spending and time limits, not provider prices or expected costs.

The S01 capability probes have a separate total 10 USD API cap. No spending is authorized by the planning archive alone. The future stage authorization must include the applicable account, destination, payment source and budget. Benchmark spending is separate from the operator allowance offered to service users.

Before dispatch, reserve estimated usage where the provider exposes enough information. If pricing or usage is unknown, require a conservative bounded policy and report uncertainty. A provider request with an uncertain accepted outcome may have incurred cost. Do not retry it as if it were a harmless read. Retain its original outcome and apply the explicit retry policy in SPEC-002.

Stop an episode at exhausted budget, unreconciled effect, revoked authority, repeated identical failure after the configured correction limit or a scope expansion requirement. Default correction limit is two bounded attempts for this benchmark. An independent safety failure stops the affected profile until diagnosed.

## Reproducibility and reporting

Record fixture hashes, image digests, exact runtime/adapter versions, source/tool closure, game reference build, server environment, model IDs and capability receipts. Fix sampling settings where supported. Record that seeds and deterministic sampling may be unsupported. Never assert identical reproducibility from a nondeterministic provider.

Run order should rotate profiles to reduce systematic environment bias. Reset each fixture to its verified baseline. Preserve every outcome, including failures. Redact credentials and private prompt content from shared reports while retaining hashes and the authorized task definition.

Publish a result table with one row per episode and explicit links to accepted source and evidence. Compare paired scenario outcomes before aggregate percentages. Use distributions and ranges for time and cost. With only three repetitions, avoid ranking models on tiny differences.

The final release review evaluates integrity gates first, then useful task completion, rework, cost and usability. Model choice can remain user-selectable while the supported capability envelope is narrow and explicit.

