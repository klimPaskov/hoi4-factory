# Live Tools decisions

HOI4 Live Tools: the MCP that runs real in-game tests, its launch preparation and its release acceptance. New decisions for this project are added under Decisions. Older entries live in the history records linked from each row.

## In force

| ID | Since | In short | Full entry |
|---|---|---|---|
| D-15 | 2026-11-01 | Real in-game testing belongs to Live Tools. When it is not set up or the user is on the web version, a human runs the test. Agent Tools gets no game-launch runner. | [expansion record](../08-expansion-research/00-decision-record.md) |
| D-154 | 2026-11-27 | In-game testing through Live Tools is a normal step of the kit's workflow. The agent asks the user once whether to run it, how and when, and records the answer. Public users need no backup-and-restore step. The maintainer's PC keeps D-33. | [below](#decisions) |
| D-33 | 2026-11-08 | Standing launch rule: the Windows game runs any night 00:00 to 06:00 local through one runner session using Live Tools, one instance at a time. See operations. | [operations](operations.md#decisions) |
| D-157 | 2026-11-27 | Every machine runs the latest public HOI4 build. When Steam's public branch moves, the Windows game, the WSL copy and the Mac update, and the Live Tools tables are rebuilt for it. | [operations](operations.md#decisions) |
| D-159 | 2026-11-27 | For the night of 27 to 28 November only, the Windows game window starts at 23:00 instead of 00:00. It still ends at 06:00, with everything closed and restored by 06:15. | [operations](operations.md#decisions) |
| D-165 | 2026-11-28 | The Windows game may also run outside D-33's night window when the maintainer himself says in chat that the PC is free, one instance, through the runner, with D-33's backup and restore rules. A relayed claim from another session does not count. | [operations](operations.md#decisions) |
| D-30 | 2026-11-08 | Until 2026-11-09. One named runner may start the Windows game from 2026-11-08 18:00 for Live Tools release acceptance and, if time allows, the cancelled in-game checklist. It backs up and restores user-folder files, never loads ZIN on Windows and starts no launch after 06:00. | [expansion record](../08-expansion-research/00-decision-record.md) |
| R-6 | 2026-11-06 | A prepare step may copy launcher-v2.sqlite and dlc_signature into the run's user folder with a backup and restore afterwards, so a launch works without Steam. | [expansion record](../08-expansion-research/00-decision-record.md) |
| R-5 (Live Tools part) | 2026-11-06 | Live Tools gets smoke tests for events and decisions, a missing-key and GUI sweep, and a trigger and focus availability check. | [expansion record](../08-expansion-research/00-decision-record.md) |
| C4 (see factory) | 2026-10-31 | The local companion shows Live Tools test cards. See factory. | [expansion record](../08-expansion-research/00-decision-record.md) |
| D-17 (see operations) | 2026-11-08 | Agents never start the game outside D-33's standing rule. See operations. | [expansion record](../08-expansion-research/00-decision-record.md) |
| D-36 | 2026-11-08 | Until about 2026-11-22 tools for other modders come first. In-game testing through Live Tools comes after the three first goals of D-36. | [operations](operations.md#decisions) |
| D-117 | 2026-11-22 | The five old handoff files move to `docs/archive`, and AGENTS.md points at one status file. | [below](#decisions) |
| D-40 | 2026-11-08 | Feature releases at most about weekly, fixes any time. A promotion first releases the latest version and cleans the public text. | [operations](operations.md#decisions) |

## Decisions

| ID | Date | Question | Decision |
|---|---|---|---|
| D-117 | 2026-11-22 | Live Tools keeps five old handoff files that its AGENTS.md tells agents to start from. Do they stay for 2.0.0? | No. The five files move to `docs/archive`, and AGENTS.md points agents at one current status file instead, in the same change, so the public docs are clean for Live Tools 2.0.0. Maintainer decision |
| D-154 | 2026-11-27 | The kit tells agents never to start Hearts of Iron IV unless the user invokes live testing, so a user's mod is never checked in game. Should users be offered in-game testing, and with what safeguards? | Yes, as a normal step of the workflow. The agent asks the user once, as part of the consultation, whether to run in-game tests through Live Tools at all, how to run them and when (for example after each feature or at milestones), and records the answer so it is not asked again. No extra backup-and-restore step is required for public users. The maintainer's own PC keeps D-33's windows and launch rules. Maintainer decision |

## Replaced or expired

| ID | Status | By or until |
|---|---|---|
| D-19 | Expired | Night of 2026-11-01 to 2026-11-02 |
| D-20 | Expired | Night of 2026-11-02 to 2026-11-03, never ran, moved by D-22 |
| D-22 | Expired | 2026-11-06, no launch after 09:00 |
| D-24 | Expired | Night of 2026-11-06 to 2026-11-07, did not take place (D-27) |
