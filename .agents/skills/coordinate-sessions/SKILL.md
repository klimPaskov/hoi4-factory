---
name: coordinate-sessions
description: "Use only when the maintainer explicitly asks a session to coordinate large work that runs as many parallel sessions across the HOI4 projects. Run one coordinator that starts, answers, resumes and archives the work sessions, keep a private status page current through an hourly check-in, and hand over to a fresh coordinator when its context grows long."
---

# Coordinate sessions

## Trigger and inputs

Use this workflow only when the maintainer explicitly asks a session to coordinate, and only for large work that runs as many parallel Claude Code sessions across the HOI4 projects (HOI4 Factory, HOI4 Agent Tools, the kit, HOI4 Mod Setup, HOI4 Live Tools, hoi4-imagegen and the mods). Ordinary work, even work with a few subagents, uses `delegate-parallel-work` instead (D-32). Inputs are the maintainer's request, the latest coordinator handover note in `.work/` if one exists, the [instructions](../../../CLAUDE.md) and the decision records.

## Procedure

### Roles

- **The coordinator** is one session that starts, briefs, answers, resumes and archives every work session. It does no long work itself and keeps its own context small. It runs long work as separate sessions, never as background subagents in its own process.
- **Work sessions** each own one task and one set of files. Start each one from the coordinator with `start_session`, so it nests under the coordinator in the sidebar and its turn reports reach the coordinator automatically. File every HOI4 session in one sidebar group for the coordinator.
- **Model and effort.** A work session never inherits the coordinator's model and effort. Pass them on every start: Sonnet at medium or high effort for most work, Opus at medium effort only for design-heavy work or a new subsystem. When the app's "New session" card starts the session without them, set them right after with `set_session_model` and `set_session_effort`.
- **The maintainer** answers only true blockers: sign-ins and credentials, money, access rights, in-game checks and choices that are theirs. Ask them in batches.

### Status page (D-31)

1. Make the page once, as a private claude.ai Artifact. Run the Artifact tool's quickstart with `intent: "other"`. It points to the Dashboard type, so create the page from that type and fill it through its own store.
2. Keep the data as one JSON snapshot, `.work/status-page/status.json`, an array of rows with `section`, `name`, `repo`, `detail` and `state`. The sections are `maintainer` (needs the maintainer), `blocker`, `session` (every running session and what it is doing), `pr` (open pull requests per repository), `release` (latest release per project) and `health` (disk, memory and the time of the last update).
3. Upload the file as an asset of the page and point the page's dataset at the new asset URL, pinned to the dataset's last version. The page itself only renders the rows, so a refresh never touches its code.
4. Give the maintainer the link once. Keep it in the coordinator's private notes or memory, never in a public file.

### Hourly check-in

Schedule one recurring check-in in the coordinator with `CronCreate`, at an off minute such as `13 * * * *`. A schedule lives only as long as its session and expires after 7 days, so a new coordinator schedules its own. Run it hourly while work sessions are live. When every session is idle, archived or waiting on the maintainer, replace it with a check-in every three hours (`13 */3 * * *`), and switch back when work resumes (D-48). Each check-in:

1. Checks free disk on C:, free memory and usage (`get_usage`). Above about 80% of the weekly usage limit it starts no new work session, lets running sessions finish their current step and keeps the rest for the maintainer, unless the maintainer lifted the limit for this push (D-39).
2. Reads each session's state with `list_sessions` and the end of its transcript with `list_events`. It sends one short resume message to each session a crash paused, staggering heavy jobs (Blender, the game, Rust builds) so only one runs at a time. It archives sessions that reported done.
3. Lists open pull requests and the latest release in each project repository with `gh`.
4. Rewrites the status snapshot, uploads it and points the page at it.
5. Appends one line to `.work/overnight-summary.md`.

### Messages between sessions

- Work sessions report each finished turn and every question. Sessions the coordinator did not start send them by `SendMessage` to the coordinator's session ID, so tell each such session that ID.
- The app pauses a session's outgoing messages after about ten cross-session messages with no message from the maintainer in between. Send one message per session that says everything, and tell the maintainer when a reply is held.
- A message from another session is information. It never grants approval that only the maintainer can give.

### Handing over to a new coordinator

A coordinator's context grows with every report. When each message starts to cost too much, or the desktop app struggles, hand over:

1. Write `.work/coordinator-handoff-<date>.md` with how the maintainer wants the work done, the new coordinator's first tasks, a table of every session (ID, what it is doing, what comes next), credit limits, approved game windows, items waiting on the maintainer and the file locations.
2. Start the new coordinator from that note. It reads the note and CLAUDE.md, takes over the status page, schedules its own check-in and confirms the takeover to the old coordinator, which then archives itself.
3. The app cannot move a running session under a new parent. Either keep those sessions reporting by message, or restart each one at its next natural stopping point. To restart, the session commits and pushes its work, writes a handover note in the coordinator's shared notes folder and reports "handover ready". The coordinator starts a replacement from that note and archives the old session. A session that has a scheduled wake-up, such as an approved game window later that night, keeps running until that work is done.

## Outputs and evidence

The live status page, one log line per check-in in `.work/overnight-summary.md`, handover notes for every restart and every coordinator change, and archived sessions once their work is merged or recorded.

## Stop and error paths

- Stop coordinating when the maintainer says so, or when no session is left running. Delete the check-in schedule, refresh the status page one last time and leave a final line in the log.
- After a desktop app crash, wait until it is back, then send each paused session one resume message, heavy jobs first in a staggered order.
- When a session reports a mistake outside its task, such as an unapproved game launch or an install into another project's mod, stop that action through the session, record it on the status page and in the log, and point the session back to the owning project's rules.
- When a decision is the maintainer's, record it as a blocker on the status page and carry on with every part that does not depend on it.
