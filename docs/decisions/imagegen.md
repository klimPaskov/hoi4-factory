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
| W5-8 | 2026-11-05 | As a trial, up to three login-route requests may run at once across all sessions on the machine, with backoff. If the server signs out or rate limits, drop back to one at a time and tell the maintainer. | [expansion record](../08-expansion-research/00-decision-record.md) |
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

## Decisions

No new entries yet.

## Replaced or expired

| ID | Status | By or until |
|---|---|---|
| D-12 (opt-in only login plugin) | Replaced | D-12a |
| D-12a (high effort) | Replaced | W5-6 |
| D4 | Replaced | D4a |
| W5-5 | Replaced | W5-8 |
| D-20 (widening to eleven imagegen families) | Expired | Night of 2026-11-02 to 2026-11-03, never ran |
