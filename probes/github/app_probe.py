"""S01 GitHub App probe, step 2: exercise the installed App on the disposable repository.

Reads the App credentials from .work/github-app/app.json (git-ignored), mints a
short-lived installation token limited to the probe repository, and runs the
operations HOI4 Factory needs (ADR-014, ADR-018): read, branch, commit, Git push
through a per-operation credential header, pull request with lost-reply
reconciliation, review comment, merge, issues, labels and check reads. It also
checks that the token cannot reach any other repository.

No token, key or secret is printed or written to evidence. Results go to
evidence/S01/exp04-github/results.json.

Usage: py -3.13 -I probes/github/app_probe.py <repo root> [--installation-only]

The App must be installed with "Only select repositories" on hoi4-factory-test-mod alone.
"""
import base64
import hashlib
import json
import os
import subprocess
import sys
import tempfile
import time
import urllib.error
import urllib.request
from pathlib import Path

root = Path(sys.argv[1])
only_installation = "--installation-only" in sys.argv
# REPO is the canonical test repository, the only one the App is installed on. OTHER is any private
# repository outside the installation, used only for refusal checks. Never point a write test at a real project.
OWNER, REPO, OTHER = "klimPaskov", "hoi4-factory-test-mod", "hoi4-factory-s01-probe-b"
app = json.loads((root / ".work/github-app/app.json").read_text(encoding="utf-8"))
results = []


def b64url(data: bytes) -> str:
    return base64.urlsafe_b64encode(data).rstrip(b"=").decode()


def app_jwt() -> str:
    now = int(time.time())
    header = b64url(json.dumps({"alg": "RS256", "typ": "JWT"}).encode())
    payload = b64url(json.dumps({"iat": now - 60, "exp": now + 540, "iss": str(app["id"])}).encode())
    signing_input = f"{header}.{payload}".encode()
    with tempfile.TemporaryDirectory(dir=root / ".work") as tmp:
        key = Path(tmp) / "key.pem"
        key.write_text(app["pem"], encoding="utf-8")
        sig = subprocess.run(["openssl", "dgst", "-sha256", "-sign", str(key)],
                             input=signing_input, capture_output=True, check=True).stdout
    return f"{header}.{payload}.{b64url(sig)}"


def api(method, path, token, body=None, scheme="Bearer"):
    req = urllib.request.Request(
        "https://api.github.com" + path, method=method,
        data=None if body is None else json.dumps(body).encode(),
        headers={"Accept": "application/vnd.github+json", "X-GitHub-Api-Version": "2022-11-28",
                 "Authorization": f"{scheme} {token}", "User-Agent": "hoi4-factory-s01-probe"})
    try:
        with urllib.request.urlopen(req, timeout=30) as r:
            raw = r.read()
            return r.status, (json.loads(raw) if raw else None)
    except urllib.error.HTTPError as e:
        raw = e.read()
        try:
            return e.code, json.loads(raw)
        except ValueError:
            return e.code, {"raw": raw[:200].decode(errors="replace")}


def record(check, ok, **detail):
    results.append({"check": check, "passed": ok, **detail})
    print(("PASS " if ok else "FAIL ") + check + " " + json.dumps(detail)[:300], flush=True)


jwt = app_jwt()
status, installs = api("GET", "/app/installations", jwt)
mine = [i for i in (installs or []) if i.get("account", {}).get("login") == OWNER] if status == 200 else []
record("app_installation_present", bool(mine), http=status, installations=len(mine))
if not mine:
    sys.exit(2)
inst = mine[0]
# S01 finding: an all-repositories installation let a repository-scoped token write to a public repository.
record("installation_is_selected_repositories_only", inst.get("repository_selection") == "selected",
       repository_selection=inst.get("repository_selection"))
if inst.get("repository_selection") != "selected":
    sys.exit(4)
status, repos_view = api("POST", f"/app/installations/{inst['id']}/access_tokens", jwt,
                         {"repositories": [REPO]})
token = repos_view.get("token", "") if status == 201 else ""
record("installation_token_minted", status == 201, http=status,
       token_prefix=token[:4], expires_at=repos_view.get("expires_at"),
       granted_permissions=repos_view.get("permissions"),
       repositories=[r["full_name"] for r in repos_view.get("repositories", [])],
       installation_repository_selection=inst.get("repository_selection"))
if only_installation or not token:
    sys.exit(0 if token else 3)

status, inst_repos = api("GET", "/installation/repositories", token)
names = [r["full_name"] for r in inst_repos.get("repositories", [])] if status == 200 else []
record("installation_scope_is_probe_repo_only", names == [f"{OWNER}/{REPO}"], http=status, repositories=names)

status, other = api("GET", f"/repos/{OWNER}/{OTHER}", token)
status_w, _ = api("POST", f"/repos/{OWNER}/{OTHER}/issues", token, {"title": "scoped token cross-repo test, expected refusal"})
record("token_cannot_reach_other_private_repository", status in (403, 404) and status_w in (403, 404), read_http=status, write_http=status_w)

status, readme = api("GET", f"/repos/{OWNER}/{REPO}/contents/README.md", token)
record("read_contents", status == 200, http=status, sha=readme.get("sha"))

status, ref = api("GET", f"/repos/{OWNER}/{REPO}/git/ref/heads/main", token)
base_sha = ref["object"]["sha"]
branch = f"s01-probe-{int(time.time())}"

# Git push through a per-operation credential header, never stored in Git config.
work = Path(tempfile.mkdtemp(dir=root / ".work", prefix="gh-push-"))
basic = base64.b64encode(f"x-access-token:{token}".encode()).decode()
# core.autocrlf is forced off: Git for Windows ships core.autocrlf=true in its system config,
# which rewrote CRLF to LF on commit in the first run of this probe.
env = {**os.environ, "GIT_TERMINAL_PROMPT": "0",
       "GIT_CONFIG_COUNT": "2", "GIT_CONFIG_KEY_0": "http.https://github.com/.extraheader",
       "GIT_CONFIG_VALUE_0": f"AUTHORIZATION: basic {basic}",
       "GIT_CONFIG_KEY_1": "core.autocrlf", "GIT_CONFIG_VALUE_1": "false"}
url = f"https://github.com/{OWNER}/{REPO}.git"
subprocess.run(["git", "clone", "-q", "--depth", "1", url, str(work / "r")], env=env, check=True)
fixture = "focus_branch = { id = s01_probe_focus cost = 10 }\r\n"
(work / "r" / "s01-probe.txt").write_bytes(fixture.encode())
subprocess.run(["git", "-C", str(work / "r"), "checkout", "-q", "-b", branch], env=env, check=True)
subprocess.run(["git", "-C", str(work / "r"), "add", "s01-probe.txt"], env=env, check=True)
subprocess.run(["git", "-C", str(work / "r"), "-c", "user.name=HOI4 Factory S01 probe",
                "-c", "user.email=s01-probe@users.noreply.github.com", "commit", "-q", "-m", "S01 probe commit"], env=env, check=True)
local_head = subprocess.run(["git", "-C", str(work / "r"), "rev-parse", "HEAD"], capture_output=True, text=True).stdout.strip()
push = subprocess.run(["git", "-C", str(work / "r"), "push", "-q", "origin", branch], env=env, capture_output=True, text=True)
cfg = subprocess.run(["git", "-C", str(work / "r"), "config", "--list", "--local"], capture_output=True, text=True).stdout
status, remote_ref = api("GET", f"/repos/{OWNER}/{REPO}/git/ref/heads/{branch}", token)
record("push_owned_branch_and_reconcile_ref", push.returncode == 0 and remote_ref["object"]["sha"] == local_head,
       push_exit=push.returncode, remote_matches_local=remote_ref["object"]["sha"] == local_head,
       token_absent_from_git_config=token not in cfg and basic not in cfg)
status, blob = api("GET", f"/repos/{OWNER}/{REPO}/contents/s01-probe.txt?ref={branch}", token)
remote_bytes = base64.b64decode(blob["content"]) if status == 200 else b""
record("pushed_bytes_preserved_crlf", remote_bytes == fixture.encode(),
       sha256_local=hashlib.sha256(fixture.encode()).hexdigest(), sha256_remote=hashlib.sha256(remote_bytes).hexdigest())

# Pull request with a simulated lost reply: create, discard the response, then reconcile.
body = {"title": "S01 probe pull request", "head": branch, "base": "main",
        "body": "Disposable S01 probe. Proposal hash and evidence summary would appear here."}
api("POST", f"/repos/{OWNER}/{REPO}/pulls", token, body)  # response deliberately discarded
status, open_prs = api("GET", f"/repos/{OWNER}/{REPO}/pulls?head={OWNER}:{branch}&state=open", token)
status_dup, dup = api("POST", f"/repos/{OWNER}/{REPO}/pulls", token, body)
record("pull_request_lost_reply_reconciled_without_duplicate", len(open_prs) == 1 and status_dup == 422,
       found_after_lost_reply=len(open_prs), duplicate_attempt_http=status_dup,
       duplicate_message=(dup.get("errors") or [{}])[0].get("message"))
pr = open_prs[0]

status, files = api("GET", f"/repos/{OWNER}/{REPO}/pulls/{pr['number']}/files", token)
record("pull_request_diff_readable", status == 200 and files[0]["filename"] == "s01-probe.txt", http=status)
status, review = api("POST", f"/repos/{OWNER}/{REPO}/pulls/{pr['number']}/reviews", token, {
    "event": "COMMENT", "body": "S01 probe review",
    "comments": [{"path": "s01-probe.txt", "line": 1, "side": "RIGHT", "body": "S01 probe line comment"}]})
record("pull_request_review_with_line_comment", status == 200, http=status,
       review_author_type=(review.get("user") or {}).get("type"))
status, checks = api("GET", f"/repos/{OWNER}/{REPO}/commits/{local_head}/check-runs", token)
record("check_runs_readable", status == 200, http=status, total=checks.get("total_count"))
status, merged = api("PUT", f"/repos/{OWNER}/{REPO}/pulls/{pr['number']}/merge", token,
                     {"merge_method": "squash", "sha": local_head})
status_again, again = api("PUT", f"/repos/{OWNER}/{REPO}/pulls/{pr['number']}/merge", token,
                          {"merge_method": "squash", "sha": local_head})
status, pr_after = api("GET", f"/repos/{OWNER}/{REPO}/pulls/{pr['number']}", token)
status, main_commits = api("GET", f"/repos/{OWNER}/{REPO}/commits?per_page=100", token)
squashes = [c for c in main_commits if c["commit"]["message"].startswith(f"S01 probe commit (#{pr['number']})")]
# A repeated merge call is reconciled by reading state: GitHub answers 200 again and must not create a second commit.
record("merge_with_expected_head_and_idempotent_reconcile", pr_after.get("merged") is True and len(squashes) == 1,
       merged=pr_after.get("merged"), second_merge_http=status_again, squash_commits_for_pr=len(squashes))

status, issue = api("POST", f"/repos/{OWNER}/{REPO}/issues", token, {"title": "S01 probe issue", "body": "Disposable."})
n = issue.get("number")
s_c, _ = api("POST", f"/repos/{OWNER}/{REPO}/issues/{n}/comments", token, {"body": "S01 probe comment"})
s_l, _ = api("POST", f"/repos/{OWNER}/{REPO}/issues/{n}/labels", token, {"labels": ["s01-probe"]})
s_x, closed = api("PATCH", f"/repos/{OWNER}/{REPO}/issues/{n}", token, {"state": "closed"})
s_r, _ = api("PATCH", f"/repos/{OWNER}/{REPO}/issues/{n}", token, {"state": "open"})
record("issue_create_comment_label_close_reopen", (status, s_c, s_l, s_x, s_r) == (201, 201, 200, 200, 200),
       http=[status, s_c, s_l, s_x, s_r], author_type=(issue.get("user") or {}).get("type"),
       author_login=(issue.get("user") or {}).get("login"))

out = root / "evidence/S01/exp04-github"
out.mkdir(parents=True, exist_ok=True)
(out / "results.json").write_text(json.dumps({
    "probe": "S01 GitHub App on disposable repository",
    "app": {k: app[k] for k in ("id", "slug", "permissions", "events", "created_at")},
    "repository": f"{OWNER}/{REPO}",
    "evidence_kind": "live GitHub API and Git transport",
    "secrets_recorded": False,
    "results": results}, indent=2), encoding="utf-8")
print("failed:", [r["check"] for r in results if not r["passed"]])
