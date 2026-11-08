---
name: prepare-promotion-release
description: "Use when the maintainer names a tool or the whole modder workflow for promotion. Release every promoted tool and its installers at the latest version, clean their public text, draft the video for the maintainer's review and hand over a short list, never posting anything."
---

# Prepare a promotion release

## Trigger and inputs

Use this workflow when the maintainer says a tool, or the whole modder workflow, will be promoted, for example once a trailer is finished (D-40, D-43). Ordinary releases follow the weekly rhythm of D-40 and do not need it. Inputs are the promoted projects and the target date, the [decisions in force](../../../docs/decisions/README.md) for operations, Setup, the kit and each tool, each repository's own CLAUDE.md or AGENTS.md and its release guide, and the [instructions](../../../CLAUDE.md).

## Procedure

1. **Scope.** List the promoted project and everything that installs it. For the whole workflow that is HOI4 Agent Tools, hoi4-imagegen, HOI4 Live Tools where Setup installs it, the pinned Meshy and Blender MCP servers, the kit and HOI4 Mod Setup. Record the list where the coordinator or the maintainer will see it.
2. **Release the latest version.** In each tool repository merge the finished, green work, then release by that repository's own release guide. Respect a release owner where one exists, such as M-16 for Agent Tools. Pin the new versions in the kit once, run every step of the kit's Validate workflow locally, and run Setup's offline acceptance against the kit change before merging it, because Setup installs from kit `main`. Release Setup after its fresh-install test passes on Windows and Linux.
3. **Clean the public text.** Read each promoted repository as a new modder would: the README, the docs index and how-to pages, the latest release notes, the gallery or screenshots, the repository description and topics, and Setup's own text for that tool. Make it:
   - current, so versions, commands and screenshots match the release
   - accurate, claiming only what was proven and naming platform gaps
   - short, with internal detail moved to developer docs
   - clean, with no internal notes, session names, dated incident stories, open to-do lists, private paths, private material or the maintainer's own ratings.

   Check every link. Land the changes as ordinary pull requests and merge them on green checks.
4. **macOS.** When the maintainer has run the [macOS checks prompt](../../../docs/06-agent-handoffs/GOAL-MACOS-CHECKS.md) (D-47), link its results. Otherwise name the macOS limits in the docs and tell the maintainer the prompt is still waiting.
5. **Video.** When the promotion uses a video, an agent drafts it from real captures under D-49, and the visual review rules in CLAUDE.md apply before the maintainer sees it.
6. **Hand over.** Give the maintainer one short list: the released versions with links, the cleaned pages, the video draft and anything left open.

## Outputs and evidence

Release links, the merged pull requests for the text, the fresh-install results on Windows and Linux, the macOS results or the named gap, the video draft, and the hand-over list.

## Stop and error paths

- A required check or the fresh-install test fails: fix it, or tell the maintainer the promotion is not ready yet. Never skip a failing check.
- A release needs a credential, a sign-in or a paid step: record it as a blocker and report it, then carry on with the rest.
- Agents never post, upload, list the tools anywhere or contact anyone (R-12). The maintainer promotes.
