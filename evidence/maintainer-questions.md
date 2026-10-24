# Questions and requests for the maintainer

Collected while working autonomously from 2026-10-24. Nothing here blocked work unless marked **blocker**. Each item says what was done in the meantime.

## GitHub App and repositories

1. **Email permission.** The HOI4 Factory Dev App still does not request "Email addresses: read-only" (Account permissions). Without it, GitHub sign-ups whose email is private get `github_email_unavailable`. Meanwhile: GitHub sign-in works for public emails, and email sign-in is unaffected.
2. **Installation scope.** The App installation covers four repositories, including the real `klimPaskov/hoi4-factory` and both `hoi4-factory-s01-probe` repositories. CLAUDE.md says it should cover `hoi4-factory-test-mod` only. Meanwhile: the factory's per-project allowlist keeps every write away from repositories the user did not select.
3. **Old probe resources.** You can delete the first probe App (`hoi4-factory-s01-probe-6cbe22`) and the two `hoi4-factory-s01-probe` repositories when convenient. Issue #1 in `hoi4-factory` is closed.

## Upstream pull requests

4. **hoi4-agent-tools #42** (source-preservation guards) is open. You authorized merging the Setup PRs. Merging this one was not asked for, so it is left for you.
