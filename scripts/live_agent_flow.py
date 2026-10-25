"""Drive one native agent task on a running HOI4 Factory server over HTTP (S03 live check).

Sign up, verify the email through Mailpit, create a project, upload a small mod, create an edit
task limited to one folder, start an attempt (the server's worker calls the real model through
the Rust loop), wait for the attempt to end, then read its events, receipts summary and the
account's usage. With --selfhosted-url it also saves a self-hosted OpenAI-compatible key for that
address and probes it (connection and model listing only, never inference); the destination
policy is expected to refuse a private address unless the operator allowed its range.

Run with `scripts/py.sh scripts/live_agent_flow.py --base http://127.0.0.1:8080 --mailpit
http://127.0.0.1:8025 --out .work/live-agent-flow.json`. Standard library only.
"""

from __future__ import annotations

import argparse
import http.cookiejar
import io
import json
import re
import sys
import time
import urllib.error
import urllib.request
import uuid
import zipfile

PASSWORD = "correct horse battery"


def build_zip() -> bytes:
    files = [
        ("descriptor.mod", b'name="Live Agent Fixture"\r\nversion="0.1"\r\nsupported_version="1.19.*"\r\n'),
        ("common/national_focus/live_focus.txt", b"focus_tree = {\r\n\tid = live_tree\r\n\tfocus = {\r\n\t\tid = live_focus\r\n\t}\r\n}\r\n"),
        ("common/ideas/existing_ideas.txt", b"ideas = {\r\n\tcountry = {\r\n\t\tlive_existing_idea = {\r\n\t\t\tmodifier = { stability_factor = 0.05 }\r\n\t\t}\r\n\t}\r\n}\r\n"),
        ("notes.txt", b"Notes stay untouched.\n"),
    ]
    buffer = io.BytesIO()
    with zipfile.ZipFile(buffer, "w", zipfile.ZIP_DEFLATED) as archive:
        for name, data in files:
            info = zipfile.ZipInfo(name, date_time=(2026, 10, 25, 0, 0, 0))
            info.external_attr = 0o100644 << 16
            archive.writestr(info, data)
    return buffer.getvalue()


class Client:
    def __init__(self, base: str) -> None:
        self.base = base.rstrip("/")
        self.opener = urllib.request.build_opener(urllib.request.HTTPCookieProcessor(http.cookiejar.CookieJar()))

    def call(self, method: str, path: str, body: object | None = None, raw: tuple[str, bytes] | None = None):
        data = None
        headers = {"Accept": "application/json"}
        if raw is not None:
            headers["Content-Type"], data = raw
        elif body is not None:
            data = json.dumps(body, ensure_ascii=False).encode("utf-8")
            headers["Content-Type"] = "application/json"
        request = urllib.request.Request(self.base + path, data=data, method=method, headers=headers)
        try:
            with self.opener.open(request, timeout=120) as response:
                text = response.read().decode("utf-8")
                return response.status, (json.loads(text) if text else None)
        except urllib.error.HTTPError as error:
            text = error.read().decode("utf-8")
            try:
                return error.code, json.loads(text)
            except json.JSONDecodeError:
                return error.code, {"raw": text}


def multipart(fields: list[tuple[str, str]], files: list[tuple[str, str, bytes]]) -> tuple[str, bytes]:
    boundary = "----live-" + uuid.uuid4().hex
    out = io.BytesIO()
    for name, value in fields:
        out.write(f'--{boundary}\r\nContent-Disposition: form-data; name="{name}"\r\n\r\n'.encode())
        out.write(value.encode("utf-8") + b"\r\n")
    for name, filename, data in files:
        out.write(f'--{boundary}\r\nContent-Disposition: form-data; name="{name}"; filename="{filename}"\r\nContent-Type: application/octet-stream\r\n\r\n'.encode())
        out.write(data + b"\r\n")
    out.write(f"--{boundary}--\r\n".encode())
    return f"multipart/form-data; boundary={boundary}", out.getvalue()


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--base", required=True)
    parser.add_argument("--mailpit", required=True)
    parser.add_argument("--out", required=True)
    parser.add_argument("--selfhosted-url", help="probe a self-hosted OpenAI-compatible endpoint (listing only)")
    parser.add_argument("--selfhosted-only", action="store_true", help="skip the agent task")
    args = parser.parse_args()
    log: list[dict] = []

    def expect(name: str, response, ok: int = 200):
        status, body = response
        log.append({"step": name, "status": status})
        print(f"{name}: HTTP {status}", "" if status == ok else json.dumps(body, ensure_ascii=False)[:600], flush=True)
        if status != ok:
            raise SystemExit(f"{name} failed")
        return body

    client = Client(args.base)
    email = f"live-agent-{uuid.uuid4().hex[:8]}@example.org"
    expect("signup", client.call("POST", "/api/v1/auth/signup", {"email": email, "password": PASSWORD}), 201)
    token = None
    for _ in range(30):
        with urllib.request.urlopen(args.mailpit.rstrip("/") + "/api/v1/messages") as r:
            messages = json.loads(r.read().decode("utf-8"))["messages"]
        mine = [m for m in messages if any(a["Address"] == email for a in m["To"])]
        if mine:
            with urllib.request.urlopen(args.mailpit.rstrip("/") + "/api/v1/message/" + mine[0]["ID"]) as r:
                token = re.search(r"token=([A-Za-z0-9_\-]+)", json.loads(r.read().decode("utf-8"))["Text"]).group(1)
            break
        time.sleep(1)
    if token is None:
        raise SystemExit("no verification mail arrived")
    expect("verify email", client.call("POST", "/api/v1/auth/verify-email", {"token": token}))
    result: dict = {"steps": log}
    result["model_profiles"] = expect("model profiles", client.call("GET", "/api/v1/model-profiles"))

    if args.selfhosted_url:
        status, body = client.call("POST", "/api/v1/account/api-keys", {
            "provider": "openai_compatible", "label": "self-hosted", "api_key": "selfhosted-unused-key", "base_url": args.selfhosted_url,
        })
        log.append({"step": "save self-hosted key", "status": status})
        print("save self-hosted key:", status, json.dumps(body)[:400], flush=True)
        result["selfhosted"] = {"url": args.selfhosted_url, "save_status": status, "save": body}
        if status == 201:
            probe = expect("probe self-hosted (listing only)", client.call("POST", f"/api/v1/account/api-keys/{body['id']}/probe", {}))
            result["selfhosted"]["probe"] = probe
    if args.selfhosted_only:
        with open(args.out, "w", encoding="utf-8", newline="\n") as handle:
            json.dump(result, handle, ensure_ascii=False, indent=2)
            handle.write("\n")
        return 0

    project = expect("create project", client.call("POST", "/api/v1/projects", {"name": "Live agent"}), 201)["project"]["id"]
    base = f"/api/v1/projects/{project}"
    content_type, payload = multipart([("kind", "zip"), ("git_init_consent", "true")], [("files", "live.zip", build_zip())])
    expect("upload zip", client.call("POST", base + "/uploads", raw=(content_type, payload)), 201)
    task = expect("create edit task", client.call("POST", base + "/tasks", {
        "objective": (
            "Add a new file common/ideas/live_agent_ideas.txt that defines one country idea named "
            "live_agent_idea with modifier stability_factor = 0.02, in the same style as the existing "
            "ideas file. Look at the existing ideas file first. Change nothing else, then finish."
        ),
        "mode": "edit",
        "allowed_paths": ["common/ideas"],
        "budget_usd": 0.05,
    }), 201)
    started = time.time()
    start = expect("start attempt", client.call("POST", f"{base}/tasks/{task['id']}/attempts", {"expected_version": task["version"]}), 201)
    detail = None
    while time.time() - started < 300:
        detail = expect("poll", client.call("GET", f"{base}/tasks/{task['id']}"))
        state = detail["attempts"][-1]["state"]
        if state in ("succeeded", "failed", "cancelled", "unknown_effect"):
            break
        time.sleep(2)
    log[:] = [s for s in log if s["step"] != "poll"]
    events = expect("events", client.call("GET", f"{base}/tasks/{task['id']}/events?after=0&limit=500"))["events"]
    usage = expect("usage", client.call("GET", "/api/v1/account/usage"))
    attempt = detail["attempts"][-1]
    seqs = [e["seq"] for e in events]
    result.update({
        "project_id": project,
        "task_id": task["id"],
        "attempt": {k: attempt.get(k) for k in ("state", "error_code", "outcome", "bundle", "grant", "fencing_generation", "funding")},
        "task_state": detail["task"]["state"],
        "pending_effects": detail["pending_effects"],
        "seconds": round(time.time() - started, 1),
        "events": [{"seq": e["seq"], "type": e["type"], "summary": e["summary"]} for e in events],
        "events_gapless": seqs == list(range(1, len(seqs) + 1)),
        "model_turns": [e["payload"] for e in events if e["type"] == "model_turn"],
        "tools_ready": next((e["payload"] for e in events if e["type"] == "tools_ready"), None),
        "worker": [e["payload"] for e in events if e["type"] == "worker_started"],
        "usage": usage,
        "start_response_grant": start["attempt"]["grant"],
    })
    with open(args.out, "w", encoding="utf-8", newline="\n") as handle:
        json.dump(result, handle, ensure_ascii=False, indent=2)
        handle.write("\n")
    print("done:", attempt["state"], attempt.get("error_code"), result["seconds"], "s")
    return 0 if attempt["state"] == "succeeded" else 1


if __name__ == "__main__":
    sys.exit(main())
