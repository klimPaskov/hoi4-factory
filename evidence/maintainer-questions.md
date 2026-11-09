# Questions and requests for the maintainer

Collected while working autonomously from 2026-10-24. Nothing here blocked work unless marked **blocker**. Each item says what was done in the meantime.

## Answered on 2026-10-25

1. **Email permission.** The HOI4 Factory Dev App did not request "Email addresses: read-only", so GitHub sign-ups with a private email got `github_email_unavailable`. Answer: the maintainer adds the permission.
2. **Installation scope.** The App installation covered four repositories, including the real `klimPaskov/hoi4-factory`. Answer: the maintainer narrows it to `hoi4-factory-test-mod` only.
3. **Old probe resources** (the App `hoi4-factory-s01-probe-6cbe22` and the two `hoi4-factory-s01-probe` repositories). Answer: delete them. The agent does not make permanent deletions, so the maintainer deletes them.
4. **hoi4-agent-tools #42** (source-preservation guards). Answer: the agent may merge it once its checks pass. The result is in the [S02 handoff](S02/handoff.md).

## Moving to another computer (added 2026-10-25 at the S02 handoff)

5. **Local secrets are not in Git.** On the new machine you need the game folder in `.env`, your Anthropic key in the user environment, and a GitHub App private key at `.work/github-app/private-key.pem`. Generate a fresh key at https://github.com/settings/apps/hoi4-factory-dev rather than copying the old file, and revoke keys you no longer use. The [developer setup guide](../docs/07-developer-workflow/DEVELOPER-SETUP.md) lists each item. Meanwhile: everything that does not need a secret works without one, and missing items are reported as unavailable.
6. **OpenAI balance.** The OpenAI keys on this machine returned `insufficient_quota`. S03 treats OpenAI live inference as unavailable unless you add balance. Meanwhile: the OpenAI adapter is tested against recorded responses.

## S03 (added 2026-10-25 while implementing S03 on the Mac, answered the same day)

Answers are recorded in the decision record under "S03 handoff answers", ADR-026 and ADR-027. In short: 7 server-side Agent Tools stay, fully usable, plus an optional local MCP later. 8 test with the smallest local model. 9 and 10 agreed.

None of these blocked work. Each lists the default the agent used, recorded in [S03 scope](S03/scope.md).

11. **Operator spend alerts** ([#11](https://github.com/klimPaskov/hoi4-factory/issues/11)). Which channel should alerts use (an email address or a webhook), and what thresholds (a daily operator spend cap, and a level for unknown usage)? Meanwhile: the server logs `allowance_exhausted` and `usage_unknown` warnings.
12. **Queue priority rules** ([#10](https://github.com/klimPaskov/hoi4-factory/issues/10)). How long should an account count as new, and how should priority guard against fresh throwaway accounts? Meanwhile: work is admitted in arrival order.
13. **hoi4-agent-tools #49** (dependency fixes). Answered: merge once CI passes. Merged as `fbc5144`, and the factory pin moved to 4.6.1 ([#4](https://github.com/klimPaskov/hoi4-factory/issues/4), closed).

7. **Where Agent Tools runs.** The goal says "Agent Tools over MCP stdio in its pinned container". A stdio server in a separate container would need the server to drive Docker (a Docker socket in the server, which would hand it root on the host). Meanwhile: Agent Tools `731e64f` is built from that exact commit in its own stage of the server image, with its own Node 22, and the server starts it per task as a supervised child process with an environment allowlist. The gVisor per-task container (ADR-020) stays the EXP-07 launch gate. Is that what you meant, or do you want a separate tool container reached over Agent Tools' authenticated HTTP transport instead?
8. **Self-hosted endpoint for the profile check.** Neither machine has a local model server. Meanwhile: an Ollama 0.40.0 container (pinned by digest, about 2.8 GB) runs only for the check, with no model pulled, so connection, model listing and the destination policy are verified and no inference runs. If you would rather point the check at your own GPU server, give its URL.
9. **Interrupted attempts after a restart.** An attempt that was running when the server stopped is not resumed automatically, because its last model request may have been accepted and charged. Meanwhile: recovery reconciles its tool effects, marks open model usage as unknown, ends the attempt (`failed` with `host_restarted`, or `unknown_effect` when an effect stays unresolved) and the user starts a new attempt, which receives the earlier receipts and any uncertainty in its context. A paused attempt survives a restart and resumes when the user resumes it. Should interrupted attempts resume automatically instead, at the cost of possibly paying twice for one turn?
10. **Agent edits stay uncommitted.** In S03 the agent writes only in the task's own worktree and branch and does not commit, because a local commit is a named publication class (C-08) and review belongs to S04. Meanwhile: the outcome lists changed files with hashes, and S04 decides how a proposal is committed and reviewed.

## S04 (added 2026-11-08 while implementing S04 on the Windows PC)

14. **Focus editor page design.** Which variant should the focus editor use? A: the canvas fills the page with the inspector on the right, and the focus list and problems fold into a drawer below. B: three panes, the list on the left, the canvas in the middle, the inspector and problems on the right. C: only the canvas and a thin bar, with the inspector floating over the canvas when a focus is selected. D: a table of all focuses with the cost editable in the row, the canvas beside it, and the inspector in a dialog. Screenshots: `docs/design/mockups/80-focus-a-canvas-inspector.webp` to `83-focus-d-table.webp` on branch `s04-ui-lab`. Live in the design lab at `/design-lab/round/focusEditor/a` to `/d`. Meanwhile: the server routes, the agent path and the tests go on, and no variant is built into the real page.
15. **Change card design.** Which variant should the review of one change use before commit? A: everything on one page in review order, with approve and discard in a bar at the bottom. B: two columns, what changed and its findings on the left, preview and diff in tabs on the right. C: verdict first, a card on top says ready or not ready and why, with expandable checks, then the summary, preview and diff. D: tabs for summary, changes, preview and checks under a verdict strip. Screenshots: `85-change-a-page.webp` to `88-change-d-tabs.webp` in the same folder. Live at `/design-lab/round/changeCard/a` to `/d`. In every variant an incomplete reference check shows "Incomplete is not a pass" and needs a ticked acknowledgement before approval. Meanwhile: as in 14.

Update of 2026-11-08 on items 14 and 15: the maintainer asked that both pages always show a live preview of the tree as the game draws it, never a simplified one (D-50). The variants are being rebuilt around the real game render from Agent Tools, and the question is asked again with the new screenshots (`90` to `93` and `95` to `98`).
