# Setup decisions

HOI4 Mod Setup: its builds, install steps and image step. New decisions for this project are added under Decisions. Older entries live in the history records linked from each row.

## In force

| ID | Since | In short | Full entry |
|---|---|---|---|
| R-2 | 2026-11-06 | Setup builds are not paid-signed. A short screenshot guide shows the Windows Run anyway and macOS Open anyway steps, linked from the README and download page. | [expansion record](../08-expansion-research/00-decision-record.md) |
| W5-3 | 2026-11-02 | Setup offers only hoi4-imagegen for images, defaulting to the ChatGPT login. A generic OpenAI-compatible backend covers other services. Projects on an unsupported service keep working. None, I make art myself stays. | [expansion record](../08-expansion-research/00-decision-record.md) |
| W5-7 | 2026-11-05 | In Setup the image step is one checkbox. Ticking it installs hoi4-imagegen and its MCP entry, and the browser sign-in then opens by itself. | [expansion record](../08-expansion-research/00-decision-record.md) |
| W5-2 (Setup part) | 2026-11-01 | Old gptimage paths are removed from Setup once hoi4-imagegen replaces them. | [expansion record](../08-expansion-research/00-decision-record.md) |
| P4 (see factory) | 2026-10-31 | Setup shares the import preservation report with Factory. See factory. | [expansion record](../08-expansion-research/00-decision-record.md) |
| D-23, D-18 (see operations) | 2026-11-06 | Setup pull requests merge once required checks are green, without asking. See operations. | [expansion record](../08-expansion-research/00-decision-record.md) |
| D-36 | 2026-11-08 | Until about 2026-11-22 tools for other modders come first. A modder installs through Setup and has a working agent with every tool within minutes, proven by fresh-install tests on Windows and Linux. | [operations](operations.md#decisions) |
| D-40 | 2026-11-08 | Feature releases at most about weekly, fixes any time. A promotion first releases the latest version and cleans the public text. | [operations](operations.md#decisions) |
| D-43 | 2026-11-08 | The next promotion is the whole modder workflow: Setup, the kit and every tool it installs. | [operations](operations.md#decisions) |
| D-73 | 2026-11-15 | The Setup session tags and publishes 0.9.5 by itself once the kit pin is in and the fresh-install test passes on Windows and Linux. Kit #127 merges after that. | [below](#decisions) |
| D-97 | 2026-11-21 | Setup's audit check may pass only advisories on a short reviewed list in its repository, as Agent Tools does (D-80). The list starts with the dev-only picomatch advisory that has no fix. | [below](#decisions) |

## Decisions

New entries go here in full, oldest first.

| ID | Date | Question | Decision |
|---|---|---|---|
| D-73 | 2026-11-15 | Who tags and publishes Setup 0.9.5? | The Setup session, without asking the maintainer again, once the kit has pinned the week's tool releases and the fresh-install test passes on Windows and on Linux. Kit pull request 127 merges after 0.9.5 is published. Maintainer decision |
| D-97 | 2026-11-21 | Setup pull request 204 fails on the picomatch advisory GHSA-fc5q-39xc-vjpf, which has no fixed version. Does Setup wait for upstream? | No. Setup gets the same reviewed exception list as Agent Tools under D-80. Each entry names the advisory, the package, the reason and a review date, and stops counting once a fixed version is published. The audit of what users install stays strict with no exceptions. The list starts with this one advisory, which is reached only through development tools. Maintainer decision |

## Replaced or expired

| ID | Status | By or until |
|---|---|---|
| D-28 (Setup parts) | Expired | Deadline 2026-11-07 passed |
| D-27 (Setup release pause) | Expired | Night of 2026-11-06 to 2026-11-07 |
