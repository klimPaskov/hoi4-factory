# SPEC-008: Maps, structured import and bounded editing

Status: **Proposed**. Primary requirements: REQ-029, REQ-030, REQ-031 and REQ-032. Shared requirements: REQ-006, REQ-011, REQ-025 and REQ-028. Implement the selected map surface in S07 after S05. Full world generation is deferred beyond the first release scope.

Read first: [existing assets audit](../00-research/02-existing-assets-audit.md), [visual synchronization](007-visual-state-synchronization.md), [HOI4 validation](006-hoi4-tools-and-validation.md), [Git review](005-git-collaboration-and-review.md) and [contracts](../02-architecture/contracts-and-interfaces.md).

## User value and release boundary

The user can open an existing mod, understand where map data comes from, inspect a state or province, compare proposed changes, import a supported structured map package and make a small validated edit. A reference image can guide the work without becoming authoritative game data. Visual, hybrid and autonomous workflows use the same map snapshot and proposal contract.

S07 prioritizes inspection, layer provenance, bounded import and state-level changes. Large province reshaping, automatic whole-world creation, comprehensive terrain synthesis, supply redesign and automatic campaign balancing require separate acceptance work. A failed map generator must not display inherited vanilla geography as if it generated a new world.

## Evidence and reusable boundary

Agent Tools `25f5641bbc07c65a822ef576ac6e92bd12666880` has map inspection, semantic diffs, preview, bounded geography reads and transactional edit preparation. `MapService.planRewriteWithDiff` binds the immutable scan to the plan. `previewRewriteWithDiff` builds evidence without a write transaction. The public result explicitly describes an offline representation. [SRC-A13, map service](https://github.com/klimPaskov/hoi4-agent-tools/blob/25f5641bbc07c65a822ef576ac6e92bd12666880/src/hoi4_agent_tools/map/service.ts)

Structured geography-model preview exists, but synthesis from that model is unavailable. The inspected MCP integration test expects `MAP_GEOGRAPHY_SYNTHESIS_UNAVAILABLE` and no changed files. This cannot be enabled by flipping a factory feature flag. S07 must either use a separately implemented and verified bounded importer of already structured HOI4 files, or keep that input read-only with an explicit unavailable conversion action. [SRC-A13, model integration tests](https://github.com/klimPaskov/hoi4-agent-tools/blob/25f5641bbc07c65a822ef576ac6e92bd12666880/tests/integration/mcp-map-geography.test.ts)

Modmaker supplies useful tests for base/mod state overlays and nearest-neighbor political-map lookup, but its text writers are not the factory's preservation foundation. HOIIVUtils supplies a province-generation algorithm reference, not evidence of an accepted complete world pipeline. [SRC-A23, overlay tests](https://github.com/Floxizdia/hoi4-modmaker/blob/a5c138ffacf4ed471769558f864cbe55ec5163fa/tests/test_world_states.py) [SRC-A24, province generator](https://github.com/battleskorpion/HOIIVUtils/blob/0fe06450bf17c506485caf90fb5d52cf8a2e1b33/src/main/java/com/hoi4utils/hoi4/map/province/ProvinceGeneration.java)

## Authoritative map snapshot

A proposed MapSnapshot binds workspace ID, complete input hash manifest, game/DLC/dependency profile, effective load order, descriptor replacement rules, parser versions, map dimensions and supported capabilities. Each province, state, region and displayed property retains its winning source layer and all shadowed definitions. Missing, duplicate or ambiguous records remain diagnostics rather than silently receiving arbitrary winners.

The index joins province color and ID definitions, raster pixels, state membership, strategic-region membership, histories, ownership, cores/claims and dependent supply, position or adjacency records where supported. Terrain, height, river and other layers are indexed only with their actual decoded coverage. Cache keys include all contributing file hashes, not only the visible bitmap. Game and dependency roots remain read-only.

The viewer provides selectable political, state, province and supported terrain layers, a searchable object list, zoom, region selection, provenance and diagnostics. Display downsampling must preserve categorical identities and never feed interpolated colors back into province data. A tile or partial index reports its bounds. Counts from a bounded result are not represented as full-world counts.

## Operations and contracts

| Operation | Input | Output and limits |
|---|---|---|
| Inspect map | C-01 profile, snapshot request and optional region | C-05 summary, layer provenance, completeness and missing inputs |
| Lookup | Province/state/region ID, name or coordinates with frame | Exact matches, ambiguity or missing geometry, source-linked result |
| Render | Snapshot, layer, viewport and overlays | Content-addressed image, transform and source manifest, offline evidence label |
| Import structured package | Approved archive/directory and MapImportManifest | Candidate file set, collision map, semantic diff and validation report |
| Edit state | Selected stable object, field values and projection hash | C-02 with exact history spans and reference checks |
| Edit topology | Explicit supported province operation and affected closure | C-02 with binary hashes, changed bounds, ID allocation and dependent changes, or unsupported refusal |
| Add image reference | Asset hash, rights/provenance and proposed transform | Non-game reference layer and reviewed geography interpretation |
| Apply or restore | C-02, C-08 and current input hashes | C-04 outcome, refreshed MapSnapshot and recoverable before-state |

Every write is a proposal against a known base. Canvas gestures, model plans and imported files must converge on the same operation representation. IDs do not authorize filesystem access. Mutations are scoped to the mod task root and require fresh dependency checks before publication.

## Structured import path

The first writable import accepts already structured HOI4 files with a declared manifest. It is distinct from generating a world from polygons, labels or a reference picture. MapImportManifest is a proposed data-only format with schema version, target profile, source provenance, file hashes, expected map dimensions, declared IDs and replacement policy. It contains no commands or executable hooks.

Extract an archive to bounded temporary storage with traversal, absolute path, symlink, case-collision and expansion-limit checks. Scan both archive and selected directory inputs through the same normalized manifest. Identify map files and their dependencies rather than copying every supplied file. Unknown content remains quarantined for inspection. External absolute references require a new explicit scope, not automatic traversal.

Build a candidate effective map using the actual base game, dependencies and proposed mod overlay. Show whether each incoming path creates, replaces or conflicts with a file. A partial package is acceptable only when its declared dependencies resolve against the target snapshot. A package intended as a full replacement must declare that intent and meet its larger validation closure. The tool must never synthesize missing world components and imply they were supplied.

Produce text and semantic diffs, binary before/after hashes and visual overlays for changed bounds. For destructive replacement or renumbering, present reference updates and unresolved dynamic references. If those references affect the operation's safety and cannot be resolved, block that operation and allow read-only review. Apply the accepted file set through the host's recoverable transaction boundary. Test this independent importer before advertising writable structured import. Otherwise S07 can accept only the read-only subset with a recorded scope decision.

## Reference images and inferred geography

Treat an image as a design reference with its own author, license or user-provided provenance, hash and permitted processing scope. Display it in an overlay with a coordinate frame, aspect ratio, crop, rotation and landmarks. Calibration is reviewable. A changed image or transform invalidates derived interpretations.

The model may propose coastlines, states, regions, rivers or labels, each with source regions, confidence and unresolved ambiguity. OCR output is a suggestion. Users can correct boundaries and names before those interpretations become approved intermediate geography. Store the intermediate model separately from game-ready files. Unsupported model-to-world synthesis remains unavailable, even when the preview looks plausible.

Never resize a categorical province raster through smoothing to fit a visual reference. Separate aesthetic illustration generation from the deterministic data needed to represent province identity. Optional reference-image generation capabilities can be explored in isolated experiments, with all proposed outputs marked unverified until their complete dependency closure and actual game behavior are checked.

## Bounded editing and preservation

The first editor supports a narrow set of state-history changes whose source spans can be unambiguously identified. A state ownership change, for example, must distinguish the relevant history scope from a later dated block and must not automatically add a core merely because another editor does so. Show the semantic intent and actual changed scopes.

Province split, merge or renumber operations affect a wider closure. Require stable color/ID mapping, unique allocation, membership updates, bounds, adjacency and dependent script-reference review. Unsupported topology operations remain read-only. A bitmap edit records original file hash, changed pixel bounds, resulting hash and affected province IDs. A text companion records source spans and preserved encoding. Do not atomically rename one file and call the whole multi-file map change atomic.

An external edit marks the projection stale. Rebase independent state fields only after semantic checks. Conflicting topology or ID changes require a new reviewed proposal. On crash, reconcile the host journal and tool receipt before rebuilding a current preview. Rollback restores only owned effects and preserves later external edits as conflicts. Large binary restoration uses verified retained objects rather than assuming a text Git diff can recover every asset.

## Deferred drawing and world-design contracts

These are **proposed later capabilities**, beyond the selected S07 scope. They require S07's source provenance, dependency closure and recovery evidence, followed by a separately approved authoring stage. They do not establish an existing synthesis API or enable the unavailable Agent Tools model-to-world path.

| Proposed contract | Required inputs | Reviewable output |
|---|---|---|
| MapDrawingIntent | Base MapSnapshot or design revision, coordinate frame, target layer, selected objects, stroke or boundary geometry, brush radius/category where relevant, affected bounds and IDs, protected constraints | Candidate region change, semantic and visual comparison, affected dependency closure and retained before-state |
| WorldDesignBrief | Approved lore/canon versions, cultures, nations and proposed country identities, terrain and geographic intent, references and rights, target game profile, required constraints, permitted creative choices and unresolved questions | Bounded design plan and versioned intermediate geography with provenance and explicit unresolved decisions |
| GeographyDesignRevision | Accepted brief, prior design hash, named geometry and properties, human corrections and generation provenance | Source-linked preview and a candidate native-file plan only where an independently verified converter supports the complete operation |

A drawing intent declares whether it changes a reference/design layer or a supported native map layer. Painting an illustration does not change province identity. Native terrain painting uses the selected game's supported categories and dependent records. Boundary drawing cannot silently allocate colors, renumber provinces or alter state membership. Those consequences must appear in the candidate closure. Unsupported conversion leaves the design editable as a reference and native files untouched. Cultures and lore remain design constraints unless an explicit reviewed mapping gives them a native representation.

After a human redraws a boundary or adjusts terrain, the accepted revision becomes a required input to the next C-06 context bundle. The agent sees the changed region, constraints and unresolved conflicts and continues from that hash. A stale autonomous plan cannot overwrite the correction. The same typed intents support an autonomous task with no open canvas, subject to its existing grant. Conflicts with approved lore require a separate canon decision. Cancellation retains the last accepted revision and keeps incomplete candidates outside the accepted native snapshot.

The future sequence is a reviewed design brief, intermediate geography, bounded geometry and terrain operations, complete native-file planning, source/topology checks, then a scoped game observation. Start with one synthetic-region drawing probe that independently checks affected pixels or records, inverse recovery and agent continuation after a human correction. Expand only after dependent IDs, references, large-map performance and interruption tests pass. Whole-world work additionally covers states, regions, country histories, supply, positions and other required profile inputs. A missing required output blocks native publication. A plausible preview cannot pass that gate.

Future acceptance includes culture or nation intent that conflicts with canon, a stroke racing a source edit, a topology change with unresolved consumers, cancellation during conversion and recovery from partial companion-file writes. Each preserves accepted inputs and reports the exact unresolved boundary. These are promotion gates for later work, not additional S07 exit requirements.

## Validation and acceptance

| Case | Required observation |
|---|---|
| Base map with partial mod overlay | Untouched base states remain visible, winning and shadowed sources are correct |
| Dependency order and replacement directive | Effective map follows the selected profile, with different profile producing a different snapshot |
| Duplicate ID, color or membership | Specific conflict reported, no arbitrary loss of a competing definition |
| Reference image only | Useful overlay, with no claim that it is a playable map or approved canonical geography |
| Valid geography model | Preview and report available, model synthesis still unavailable at current tool pin |
| Structured package with traversal or symlink | Rejected before writes outside extraction storage |
| Structured partial import | All required dependencies resolved or explicit blocker, with exact accepted file set |
| Bounded state edit | Only intended history span changes, raw unknown content and unrelated files preserved |
| Province operation with unresolved references | Operation blocked where correctness depends on missing references, inspection remains available |
| Crash between companion files | Applying state visible, recovery yields verified complete snapshot or explicit unresolved conflict |
| Failed generator | No inherited map mislabeled as generated output |

S07 retains input and candidate manifests, layer-resolution fixtures, semantic/binary diffs, test results and before/after renders. Static checks cover formats, dimensions, palettes, IDs, membership and supported topology. Actual game checks are separately recorded against an installed build and DLC profile. A native Windows release may use a user-observed game check with evidence rather than assume the Rust Live Tools adapter supports Windows.

S08 packages only the map operations whose acceptance passed. Later roadmap work can add province geometry tools, richer structured geography authoring, terrain and river synthesis, whole-world assembly, supply and position generation, performance at target scale and campaign evaluation. Each expansion must supply its own complete dependency and recovery tests.
