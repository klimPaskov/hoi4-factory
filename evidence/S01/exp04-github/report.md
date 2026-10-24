# EXP-04 GitHub App probe

Date: 2026-10-24. Evidence kind: live GitHub API and Git transport. Probe code: [manifest_server.py](../../../probes/github/manifest_server.py) and [app_probe.py](../../../probes/github/app_probe.py). Machine-readable results of the final run: [results.json](results.json).

## Setup

The final run uses **HOI4 Factory Dev** (`hoi4-factory-dev`, App ID 5453675), a private GitHub App the maintainer created from a manifest by clicking a local page. It is installed with "Only select repositories" on the canonical test repository `klimPaskov/hoi4-factory-test-mod` alone. The first App, `hoi4-factory-s01-probe-6cbe22` (App ID 5453381), was uninstalled from every repository through its own credentials after the incident below, its local key was removed, and the maintainer deletes its registration. Requested and granted permissions were contents write, issues write, pull requests write, checks read, statuses read and metadata read. The webhook was created inactive and no events were subscribed. App credentials stay in the git-ignored `.work/github-app/` folder. No token, key or secret appears in this evidence.

Repositories: `klimPaskov/hoi4-factory-test-mod` is the canonical private test repository for all stages. `klimPaskov/hoi4-factory-s01-probe` and `hoi4-factory-s01-probe-b` were the first-run throwaway repositories. probe-b also serves as the private repository outside the installation for refusal checks.

## Final run result

All 14 checks passed, including the new guard that refuses to run unless the installation is limited to selected repositories.

| Check | Result |
|---|---|
| Installation selection | `selected`, verified through the App before any token is used |
| Installation token | Type `ghs_` installation token, one-hour lifetime, minted with `repositories` limited to the test repository |
| Token scope listing | `/installation/repositories` returned only `hoi4-factory-test-mod` |
| Other private repository | Read and issue creation in probe-b were refused with 404 |
| Push through a per-operation credential header | Push succeeded, the remote ref equals the local commit, and the token never entered Git config |
| Byte preservation | CRLF bytes pushed and read back are identical (sha256 `766cb9e7...`) |
| Pull request after a lost reply | Reading by head branch found exactly one pull request, and a repeated create returned 422 "already exists" |
| Diff, review and line comment | Readable and writable |
| Check runs | Readable (none configured) |
| Merge with expected head | Merged once. A repeated merge call returned 200 again and created no second commit |
| Issues | Create, comment, label, close and reopen all succeeded |

## Incident and failures in the first run

The first run had three failures. All are recorded here because they change the design.

1. **Write to a real repository.** The negative scope test was aimed at the public `klimPaskov/hoi4-factory` repository and was expected to be refused. GitHub accepted it and created issue #1, titled "must be refused", authored by the App bot. The cause is that the App had been installed on all repositories rather than only the probe repository. The token had been minted for the probe repository only and listed only that repository, yet issue creation on the public repository succeeded. The same token was refused on the private probe-b repository. The test was changed to target only the private disposable repository, and no further writes reached a real repository. At the maintainer's choice, issue #1 was closed with a comment explaining it was a probe artifact.
2. **CRLF lost on commit.** Git for Windows sets `core.autocrlf=true` in its system config (`C:/Program Files/Git/etc/gitconfig`). The probe's commit converted CRLF to LF before push. Forcing `core.autocrlf=false` for the operation fixed it.
3. **Wrong merge oracle.** The probe expected a repeated merge call to return 405 or 409. GitHub returns 200 again. The corrected oracle checks state, exactly one squash commit for the pull request.

## Design consequences

- The host must enforce its own repository allowlist per project before every GitHub call. Repository-scoped installation tokens are not a sufficient boundary, at least for public repositories covered by an all-repositories installation.
- The factory should require "Only select repositories" installations and verify `repository_selection` before use, refusing an all-repositories installation or warning loudly.
- Every server-side Git operation must set `core.autocrlf=false` and avoid eol conversion, whatever the host or image config says. A Linux image without Git for Windows avoids the observed default, but the setting must still be explicit.
- Reconciliation must read state rather than interpret status codes. Pull request creation is protected by a 422 on duplicates. Merge repeats return 200.

## Not covered

- Actions appear on GitHub as the App bot. ADR-018 asks for actions as the acting user through user-to-server tokens. That OAuth flow was not exercised in S01.
- Webhooks were not tested. The App's webhook is inactive and no public endpoint exists yet.
- Rate-limit behavior was not measured.
