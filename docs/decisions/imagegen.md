# Imagegen decisions

hoi4-imagegen-mcp: the public image generation MCP for HOI4 graphical assets, its providers, the ChatGPT sign-in route, models and gallery. New decisions for this project are added under Decisions. Older entries live in the history records linked from each row.

## In force

| ID | Since | In short | Full entry |
|---|---|---|---|
| **Project and design** | | | |
| FR-2 | 2026-10-31 | A derivative of Connected-Mate/gptimage for HOI4 assets with category profiles, reference-guided generation, validation, naming, provenance and approval. Documented, authorized image APIs only. | [expansion record](../08-expansion-research/00-decision-record.md) |
| D-09 | 2026-10-31 | The repository is public, klimPaskov/hoi4-imagegen-mcp, MIT like the upstream, with upstream notices kept. | [expansion record](../08-expansion-research/00-decision-record.md) |
| W5-2 | 2026-11-01 | Six family-bound generation tools plus a catalogue and an audit tool, all processing in Rust. Existing cross-family reuse is a warning and new reuse an error. Old gptimage paths leave the kit and Setup once replaced. | [expansion record](../08-expansion-research/00-decision-record.md) |
| R-8 | 2026-11-06 | Certain fixture files in hoi4-imagegen-mcp are removed from main only. History is left as it is, with no force push. | [expansion record](../08-expansion-research/00-decision-record.md) |
| **Providers and the sign-in route** | | | |
| D-12, D-12a | 2026-11-01 | Documented APIs are the main providers. On a local install the ChatGPT-login route is first-class in the normal build and the default once signed in. It is never part of hosted Factory. D-12a replaces D-12's opt-in-only rule. | [expansion record](../08-expansion-research/00-decision-record.md) |
| D-12b, W5-6 | 2026-11-05 | The login route uses gpt-6-luna and never another model, failing clearly instead of falling back. Effort is the highest the backend accepts: high, medium, low, then none, recorded per image. This fixes the outer chat model. The image model inside the backend is its own choice (D-12c), which is what W5-10 means. | [expansion record](../08-expansion-research/00-decision-record.md) |
| D-12c | 2026-11-01 | Only the outer model and its effort are forced and verified on the login route. The backend's image tool picks its own image model, quality and size, and each candidate records them. | [expansion record](../08-expansion-research/00-decision-record.md) |
| W5-7 | 2026-11-05 | Sign-in takes no typed confirmation. The login opens the browser sign-in directly, shows the risk note as information and counts completing sign-in as accepting it. | [expansion record](../08-expansion-research/00-decision-record.md) |
| W5-4 | 2026-11-02 | Every service Setup listed gets a backend except AWS Bedrock, Adobe Firefly and Microsoft Foundry. Backends without a key ship from documented shapes and are marked not live-verified. | [expansion record](../08-expansion-research/00-decision-record.md) |
| W5-3 | 2026-11-02 | Setup offers only hoi4-imagegen, and services are chosen and keyed inside it. See setup. | [expansion record](../08-expansion-research/00-decision-record.md) |
| D3 | 2026-10-31 | Agents may call a ComfyUI endpoint the maintainer configured, for any asset including portraits of real people. Chaos Redux keeps its own asset policy as an overlay. | [expansion record](../08-expansion-research/00-decision-record.md) |
| **Models and references** | | | |
| W5-10 | 2026-11-08 | On the OpenAI API route, GPT-Image-2.6 Flare generates and Sunburst edits. Sunburst also handles larger images, portraits, animations and 3D references, while Flare keeps icons and small sprites. The sign-in route cannot choose the image model, so it counts as ChatGPT's latest. Its outer model stays fixed by D-12b. | [expansion record](../08-expansion-research/00-decision-record.md) |
| D4a | 2026-11-01 | Vanilla Paradox art may always be sent to image providers as style references when it helps, with no per-project allowance. | [expansion record](../08-expansion-research/00-decision-record.md) |
| W5-9 | 2026-11-05 | The public gallery may show a few original vanilla images and, since 2026-11-08, a few TNO and TFR reference images, credited to their owners, beside generated ones. | [expansion record](../08-expansion-research/00-decision-record.md) |
| **Usage and cost** | | | |
| W5-1 | 2026-11-01 | Login-route generations have no fixed cap because they use plan quota. Routes using paid API keys never spend without asking first. | [expansion record](../08-expansion-research/00-decision-record.md) |
| D-11 (see factory) | 2026-10-31 | In hosted Factory the operator allowance may also pay for images. See factory. | [expansion record](../08-expansion-research/00-decision-record.md) |
| D-36 | 2026-11-08 | Until about 2026-11-22 tools for other modders come first. hoi4-imagegen makes better images: family fixes, model routing, palette variety and the TFR style packs. | [operations](operations.md#decisions) |
| D-40 | 2026-11-08 | Feature releases at most about weekly, fixes any time. A promotion first releases the latest version and cleans the public text. | [operations](operations.md#decisions) |
| D-78 | 2026-11-16 | Focus icons return to their earlier lively, varied look: the frame and muted vanilla-look changes are undone, the no-national-symbols rule stays, and the fix ships as 0.4.1 after the maintainer approves a side-by-side sheet. | [below](#decisions) |
| D-91 | 2026-11-21 | Agents use hoi4-imagegen freely, with several requests at a time within the tool's own pacing. They complete a sign-in themselves when it needs no password or code from the maintainer, and never type his credentials. | [below](#decisions) |
| D-141 | 2026-11-23 | The login route has no fixed ceiling. It sends up to 10 at once by default, and a higher number needs the user's confirmation. After a sign-out it carries on at the set number and steps down by one on each repeat. It ships at once as 0.4.7. | [below](#decisions) |

## Decisions

New entries go here in full, oldest first.

| ID | Date | Question | Decision |
|---|---|---|---|
| D-78 | 2026-11-16 | The focus icons in the 0.4.0 gallery look too dull and too alike. What happens? | Focus icons go back to the look they had before the frame and vanilla-look changes, which the maintainer liked better: livelier and clearly different from one icon to the next. The frames, the muted vanilla treatment and the simplification are undone for focus icons. The previous gallery's focus icons are the reference. The rule from the Irish feedback stays: no flags or national emblems unless a focus is directly national. After the maintainer approves a side-by-side sheet (old, current, restored), the change ships at once as 0.4.1, as a fix to a wrong result. The gallery's focus families are regenerated and the kit is re-pinned. Maintainer decision |
| D-91 | 2026-11-21 | How freely may agents use hoi4-imagegen, and who signs in? | Freely. Sessions no longer hold themselves to one request at a time. Several requests may run at once, across sessions too, and the tool's own pacing across its processes decides how many go out (W5-8). The general gptimage tool still never runs alongside it for HOI4 work. When the sign-in has lapsed, an agent starts the sign-in and completes it itself if that needs nothing from the maintainer, for example when the browser is already signed in. An agent never types a password, a code or any other credential of his. If the sign-in asks for one, the agent stops and tells him. A sign-in error is still reported at once. Maintainer decision |
| D-141 | 2026-11-23 | The sign-in has held at six requests at once since 21 November. May the route go higher, and what should happen after a sign-out? | The route has no fixed upper limit. With no setting it sends up to 10 requests at once across the machine. A project may set any other number, but a number above 10 takes effect only after the user confirms it explicitly, for example with a confirmation field beside the number or a one-time confirmation in the tool's login or status flow. Without that confirmation the route uses 10, and image_status says why. The 20 second gap between requests of one slot stays, and so does the back-off after a rate limit (HTTP 429). After a sign-out the route no longer drops to one request at a time for hours. Once the user signs in again, it carries on at the set number. If sign-outs repeat, the route lowers the number in use by one for each repeat, down to one. A sign-out counts as a repeat when it comes within six hours of the previous one, and the lowered number stays until the user resets it (both technical defaults). The route keeps the lowered number in its shared state so every process uses it, and image_status shows the number in use and the reason. The revocation record stays, and a sign-out is still reported to the maintainer at once (D-91). This replaces the trial limits of W5-8. The maintainer asked for it through ZIN's lead, first for ten or more and then in its final form (ZIN owner answers 205 and 206). So it ships at once as 0.4.7 and does not wait for the weekly feature release of D-40. ZIN's lead asks him before ZIN downloads it. Maintainer decision |

## Replaced or expired

| ID | Status | By or until |
|---|---|---|
| D-12 (opt-in only login plugin) | Replaced | D-12a |
| D-12a (high effort) | Replaced | W5-6 |
| D4 | Replaced | D4a |
| W5-5 | Replaced | W5-8 |
| W5-8 (a trial of three at once, one at a time after a sign-out) | Replaced | D-141 |
| D-20 (widening to eleven imagegen families) | Expired | Night of 2026-11-02 to 2026-11-03, never ran |
