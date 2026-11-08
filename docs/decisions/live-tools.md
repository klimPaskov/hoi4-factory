# Live Tools decisions

HOI4 Live Tools: the MCP that runs real in-game tests, its launch preparation and its release acceptance. New decisions for this project are added under Decisions. Older entries live in the history records linked from each row.

## In force

| ID | Since | In short | Full entry |
|---|---|---|---|
| D-15 | 2026-11-01 | Real in-game testing belongs to Live Tools. When it is not set up or the user is on the web version, a human runs the test. Agent Tools gets no game-launch runner. | [expansion record](../08-expansion-research/00-decision-record.md) |
| D-33 | 2026-11-08 | Standing launch rule: the Windows game runs any night 00:00 to 06:00 local through one runner session using Live Tools, one instance at a time. See operations. | [operations](operations.md#decisions) |
| D-30 | 2026-11-08 | Until 2026-11-09. One named runner may start the Windows game from 2026-11-08 18:00 for Live Tools release acceptance and, if time allows, the cancelled in-game checklist. It backs up and restores user-folder files, never loads ZIN on Windows and starts no launch after 06:00. | [expansion record](../08-expansion-research/00-decision-record.md) |
| R-6 | 2026-11-06 | A prepare step may copy launcher-v2.sqlite and dlc_signature into the run's user folder with a backup and restore afterwards, so a launch works without Steam. | [expansion record](../08-expansion-research/00-decision-record.md) |
| R-5 (Live Tools part) | 2026-11-06 | Live Tools gets smoke tests for events and decisions, a missing-key and GUI sweep, and a trigger and focus availability check. | [expansion record](../08-expansion-research/00-decision-record.md) |
| C4 (see factory) | 2026-10-31 | The local companion shows Live Tools test cards. See factory. | [expansion record](../08-expansion-research/00-decision-record.md) |
| D-17 (see operations) | 2026-11-08 | Agents never start the game outside D-33's standing rule. See operations. | [expansion record](../08-expansion-research/00-decision-record.md) |
| D-36 | 2026-11-08 | Until about 2026-11-22 tools for other modders come first. In-game testing through Live Tools comes after the three first goals of D-36. | [operations](operations.md#decisions) |
| D-40 | 2026-11-08 | Feature releases at most about weekly, fixes any time. A promotion first releases the latest version and cleans the public text. | [operations](operations.md#decisions) |

## Decisions

No new entries yet.

## Replaced or expired

| ID | Status | By or until |
|---|---|---|
| D-19 | Expired | Night of 2026-11-01 to 2026-11-02 |
| D-20 | Expired | Night of 2026-11-02 to 2026-11-03, never ran, moved by D-22 |
| D-22 | Expired | 2026-11-06, no launch after 09:00 |
| D-24 | Expired | Night of 2026-11-06 to 2026-11-07, did not take place (D-27) |
