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
