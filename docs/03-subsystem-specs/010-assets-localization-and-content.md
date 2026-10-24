# SPEC-010: Assets, localization and content packages

Status: **Proposed**. Primary requirement: REQ-028. Shared requirements: REQ-006, REQ-011, REQ-023, REQ-025, REQ-034 and REQ-044. S04 uses a minimal focus package and existing or clearly marked placeholder assets. S06 adds the selected content workflows, with distribution checks in S08.

Read first: [focus editors](009-focus-and-other-editors.md), [HOI4 validation](006-hoi4-tools-and-validation.md), [permissions](011-extensibility-sandbox-and-permissions.md), [context and canon](004-context-memory-and-runtime-learning.md), [configuration migration](013-config-reuse-and-developer-environment.md) and [contracts](../02-architecture/contracts-and-interfaces.md).

## Purpose and scope

A generated feature is useful only when its source, localization, graphics references and associated files agree. The factory should assemble that package transparently, support existing user assets and optional generators, and preserve the user's ability to edit every output. Generation is one source of candidate content. It does not bypass review, rights provenance or technical validation.

The first content families are focus labels/descriptions and icons, event text and images, country and character text, portraits and flags where the selected template supports them. More specialized UI, audio and 3D workflows remain separately negotiated capabilities. A model that can write text is not assumed to generate a correctly formatted texture, audio track or game-ready model.

## Reusable evidence and limits

Mod Setup already distinguishes image-provider selection, optional local portrait discovery, workflow installation, MCP health and 3D health. Those are concrete onboarding seams. They do not establish that any generated output is aesthetically approved or works in HOI4. The factory should preserve the selected source manifest and credential references while replacing assumptions tied to a particular developer machine. [SRC-A02 and SRC-A05, typed setup commands](https://github.com/klimPaskov/HOI4-Mod-Setup/blob/4488a418cad56a158273cb66c4ce7794696b7ed1/src/lib/tauri.ts)

Agent Tools renders against source-linked assets and provides package checks for definitions, registrations, localization and asset paths. It returns reports with present, absent, shadowed and unresolved links. Required-case declaration is not the same as executing that case. Its original Apache-2.0 source license coexists with separate font and OCR notices. User-installed game art is not licensed for redistribution merely because a renderer can load it. [SRC-A14, package checks](https://github.com/klimPaskov/hoi4-agent-tools/blob/25f5641bbc07c65a822ef576ac6e92bd12666880/docs/mechanics.md) [SRC-A16, dependency notices](https://github.com/klimPaskov/hoi4-agent-tools/blob/25f5641bbc07c65a822ef576ac6e92bd12666880/docs/third-party-notices.md)

## ContentPackage and provenance

A proposed ContentPackage binds a package ID, workspace, source snapshot, approved canon references, content purpose, output manifest, generator or author provenance, localization key map, asset references, validation requirements and C-02 proposal. It declares which outputs are required and which can remain placeholders. Each output has a content hash, intended path, role, technical profile and origin.

Asset provenance records imported versus generated origin, source URL or user attachment reference where available, author or provider, declared license/usage rights, transformation chain and review status. Retain generator version, workflow hash, seed when supported and the exact final generation-request hash. Do not put secrets, private provider responses or hidden reasoning in the package. A user-supplied asset with unknown distribution rights can remain available for local inspection while release packaging reports the unresolved scope.

Technical profiles define accepted formats, dimensions, color mode, alpha behavior, compression, naming and registration requirements for a particular asset role and game profile. They are versioned data. Do not scatter remembered HOI4 sizes through prompts or assume one dimension works for every portrait, focus, flag or interface role. S01/S06 verify profiles against supported local templates and actual output consumers.

## Operations

| Operation | Input | Output or failure |
|---|---|---|
| Import asset | Approved file, role, provenance and target profile | Quarantined source object, decoded metadata and candidate conversion plan |
| Request generation | Content brief, canon references, style reference, provider capability and budget | Durable generation request and C-04 effect identity |
| Reconcile generation | Existing provider job and request binding | Completed artifact, pending, failed or unknown outcome without blind resubmission |
| Transform | Exact source hash and deterministic conversion profile | Candidate bytes and transform receipt, original retained |
| Register | Candidate asset and intended source consumer | Proposed path and source registration edits with collision checks |
| Localize | Stable key map, source-language text, glossary and target languages | Proposed language files, preserved placeholders and coverage report |
| Validate package | Full candidate manifest and target snapshot | C-05 technical, reference, localization and visual evidence |
| Apply or restore | Final C-02, grant and current hashes | Recoverable multi-file publication or explicit conflict |

## Asset lifecycle and generation

The lifecycle is requested, producing, received, technically validated, creatively reviewed, proposed, applied and superseded. A generation failure or rejected creative result remains a recorded candidate. Retrying with a changed prompt creates a new request identity and preserves the previous result for comparison. A connection failure after a paid request has an uncertain outcome and requires provider-job reconciliation before another charge-producing submission.

The user can upload an existing asset, use a game asset from the server reference for preview only, generate a candidate with an authorized provider, or retain a named placeholder. A game asset used for preview is never copied into the mod or any export. These options let source development continue when an optional service is unavailable. The release policy decides whether an unresolved placeholder is acceptable for that package. The UI must not misrepresent a placeholder as approved finished art.

For generated images, separate creative intent from technical conversion. The brief describes subject, composition, style, framing, transparency and forbidden accidental text where relevant. A deterministic post-processing step then validates and converts accepted output to the target profile. The original output and transformation receipt remain available. Converting to DDS or another required format is not proof that its content or visual fit is correct.

ComfyUI-style workflows on an endpoint the server can reach and cloud image providers use a capability adapter with input/output schemas, workflow identity, resource limits, cancellation semantics and credential references. Custom endpoint URLs follow the outbound request restrictions in SPEC-002. Generation spending follows the same per-account budgets and funding sources as model requests. Workflow installation is an explicit setup action. A health check may establish a reachable endpoint or registered tools. It does not automatically authorize an expensive generation or prove output quality.

Audio and 3D providers use the same provenance and receipt model but require additional role-specific conversion and runtime checks. A generated mesh must meet a separately specified model, material, animation and export contract before a game-ready claim. S06 can expose an unavailable or experimental state rather than bundling an untested pipeline. Their absence does not block ordinary focus or event text work.

## Localization pipeline

Localization keys are stable identifiers allocated against the effective source layers. Generation proposes text for those keys without silently renaming existing keys. Source-language meaning, approved canon, terminology and glossary references accompany translation. Names, numeric parameters, formatting tokens, scope expressions and embedded references are protected spans unless a requested edit explicitly changes them.

The language-file writer preserves supported encoding, BOM, newline style, comments, order and existing unrelated entries. It checks the target language header and syntax required by the current game profile. Duplicate keys, shadowed definitions, missing required keys, invalid escapes and unintended token changes are separate diagnostics. A generic YAML serializer is not automatically a compatible HOI4 localization writer.

Translations carry review status and source-text revision. If the source language changes, dependent translations become stale rather than remain silently approved. Fallback language behavior is explicit. Missing optional languages can be reported as incomplete without blocking a source-language prototype. A release targeting those languages must satisfy its own completeness gate.

Visual review samples the actual candidate strings in the relevant renderer or game scenario. It checks truncation, line breaks, font support and the context in which a description appears. A linguistically correct sentence can still be unsuitable in the available space. When the renderer lacks a faithful font or layout, the evidence states that limitation and a user-observed game check can supply the remaining result.

## Registration, package application and conflicts

An asset path, graphics registration, localization entry and referring script belong to one package manifest. Stage them together, check references against the candidate overlay and publish through the common recoverable change boundary. A filename collision with a user portrait is a conflict, not permission to overwrite. Choosing a new name updates all owned references and invalidates prior validation.

Generated binary files use before/after hashes and retained original objects. Text files use source spans and preservation rules. The proposal must show all output paths and identify inherited game/dependency files that remain read-only. A preview renderer may use those inherited resources locally, but packaging copies only artifacts whose inclusion and rights are explicitly established.

If a user edits a localized description while an agent prepares a portrait package, independent changes can be rebased into a fresh proposal. A changed localization key, renamed asset or replaced binary requires renewed reference analysis. Rollback applies only to the operation's owned effects and preserves intervening user changes as conflicts. A partially completed generator task must not be confused with a partially applied workspace transaction.

## Validation and review evidence

Technical validation checks decoded format, profile dimensions, alpha/color expectations, bounded file size, path containment, deterministic conversion and registration references. Content validation checks canon, key consistency, required outputs and declared language coverage. Creative review evaluates the image, audio or model itself. Actual-game observation is a fourth, separate evidence kind.

No automatic visual check is treated as universal artistic approval. For a simple focus icon, the reviewer should see the actual image on the candidate focus preview and know whether it is a placeholder. For an event image, the review includes relevant framing and text context. For generated variants, retain the selected variant hash so later file replacement cannot silently change the approved content.

| Test | Observable acceptance |
|---|---|
| Existing user asset collision | Original bytes preserved until a specific replacement decision |
| Generation disconnect after submission | Job reconciled without duplicate billable request |
| Provider absent or quota exhausted | Useful failure, preserved draft and optional placeholder route |
| Mis-sized or malformed image | Technical validation fails before registration or source publication |
| Localization BOM/CRLF fixture | Unrelated bytes preserved and required language syntax retained |
| Translation changes a protected token | Explicit diagnostic and no silent acceptance |
| Source text changes after translation | Dependent translations marked stale |
| Missing asset or graphics reference | Package check identifies absent or shadowed link with path evidence |
| Crash between script and asset publication | Recovery yields complete accepted package or explicit unresolved state |
| Rights unresolved for a distributed asset | Release manifest records exclusion or unresolved approval, without inventing clearance |
| Approved variant replaced in the workspace | Hash mismatch invalidates visual approval and preview evidence |

S04 proves one focus package using deterministic existing fixtures. S06 proves a complete localized content package, an imported asset and one selected generation adapter if available. S08 verifies redistribution inventory and the actual tool/dependency closure in the deployed images. Keep unselected providers, untested 3D and audio pipelines, and game assets outside release claims.
