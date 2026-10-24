"""Drive the whole onboarding flow of a running HOI4 Factory server over HTTP.

Used for the S02 live Compose run: sign up, verify the email through Mailpit, create a project,
upload a mod as a ZIP, scan, approve the offered excerpts, analyze (the server calls the real
model), confirm the proposed values, plan, decide the conflicts, approve the exact plan, apply and
read the C-01 descriptor. Prints one line per step and writes a JSON result file.

Run with `py -3.13 scripts/live_flow.py --base http://127.0.0.1:18080 --mailpit http://127.0.0.1:18025
--out .work/live-flow.json`. Standard library only. All JSON is sent as UTF-8 bytes.
"""

from __future__ import annotations

import argparse
import hashlib
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


def fixture_files() -> list[tuple[str, bytes]]:
    bom = b"\xef\xbb\xbf"
    return [
        ("descriptor.mod", bom + b'name="Live Fixture"\r\nversion="0.3.1"\r\nsupported_version="1.19.*"\r\ntags={\r\n\t"Alternative History"\r\n}\r\n'),
        ("common/national_focus/live_focus.txt", b"focus_tree = {\r\n\tid = live_tree\r\n\tfocus = {\r\n\t\tid = live_focus\r\n\t}\r\n}\r\n"),
        ("localisation/english/live_l_english.yml", bom + b'l_english:\r\n live_focus_title:0 "Live focus"\r\n'),
        ("gfx/live_asset.png", b"\x89PNG\r\n\x1a\n" + hashlib.sha256(b"live asset").digest() * 64),
        ("AGENTS.md", b"# My own agent instructions\n\nKeep exactly these words.\n"),
        (".agents/skills/live-custom/SKILL.md", b"---\nname: live-custom\ndescription: a custom skill of the author\n---\nDo the custom thing.\n"),
        ("README.md", b"# Live Fixture\n\nMy readme.\n"),
        ("notes/idea.txt", "Ideas — éè 中文\n".encode("utf-8")),
    ]


def build_zip() -> bytes:
    buffer = io.BytesIO()
    with zipfile.ZipFile(buffer, "w", zipfile.ZIP_DEFLATED) as archive:
        for name, data in fixture_files():
            info = zipfile.ZipInfo(name, date_time=(2026, 10, 24, 0, 0, 0))
            info.external_attr = 0o100644 << 16
            info.compress_type = zipfile.ZIP_DEFLATED
            archive.writestr(info, data)
    return buffer.getvalue()


class Client:
    def __init__(self, base: str) -> None:
        self.base = base.rstrip("/")
        self.jar = http.cookiejar.CookieJar()
        self.opener = urllib.request.build_opener(urllib.request.HTTPCookieProcessor(self.jar))

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
            with self.opener.open(request, timeout=600) as response:
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
    args = parser.parse_args()

    log: list[dict] = []

    def step(name: str, started: float, status: int, detail: str = "") -> None:
        elapsed = time.time() - started
        log.append({"step": name, "status": status, "seconds": round(elapsed, 1), "detail": detail})
        print(f"{name}: HTTP {status} in {elapsed:.1f}s {detail}", flush=True)

    def expect(name: str, started: float, response, ok: int = 200):
        status, body = response
        step(name, started, status, "" if status == ok else json.dumps(body, ensure_ascii=False)[:600])
        if status != ok:
            raise SystemExit(f"{name} failed")
        return body

    client = Client(args.base)
    email = f"live-{uuid.uuid4().hex[:8]}@example.org"
    t = time.time()
    body = expect("signup", t, client.call("POST", "/api/v1/auth/signup", {"email": email, "password": PASSWORD}), 201)
    account_id = body["account"]["id"]

    token = None
    for _ in range(30):
        with urllib.request.urlopen(args.mailpit.rstrip("/") + "/api/v1/messages") as r:
            messages = json.loads(r.read().decode("utf-8"))["messages"]
        mine = [m for m in messages if any(a["Address"] == email for a in m["To"])]
        if mine:
            with urllib.request.urlopen(args.mailpit.rstrip("/") + "/api/v1/message/" + mine[0]["ID"]) as r:
                text = json.loads(r.read().decode("utf-8"))["Text"]
            token = re.search(r"token=([A-Za-z0-9_\-]+)", text).group(1)
            break
        time.sleep(1)
    if token is None:
        raise SystemExit("no verification mail arrived")
    t = time.time()
    expect("verify email", t, client.call("POST", "/api/v1/auth/verify-email", {"token": token}))

    t = time.time()
    project = expect("create project", t, client.call("POST", "/api/v1/projects", {"name": "Mød Ünïcode 中文"}), 201)["project"]["id"]
    base = f"/api/v1/projects/{project}"

    t = time.time()
    content_type, payload = multipart(
        [("kind", "zip"), ("git_init_consent", "true")], [("files", "live.zip", build_zip())]
    )
    upload = expect("upload zip", t, client.call("POST", base + "/uploads", raw=(content_type, payload)), 201)
    files = upload["workspace"]["files"]

    t = time.time()
    scan = expect("scan", t, client.call("POST", base + "/setup/scan", {"mode": "existing"}))
    candidates = scan["evidence"]["candidates"]
    t = time.time()
    expect(
        "approve excerpts", t,
        client.call("POST", base + "/setup/evidence", {"scan_id": scan["scan_id"], "references": [c["reference"] for c in candidates]}),
    )
    t = time.time()
    analysis = expect("analyze (real model)", t, client.call("POST", base + "/setup/analyze", {}))
    t = time.time()
    expect("confirm values", t, client.call("POST", base + "/setup/confirm", {"analysis_id": analysis["analysis_id"], "values": analysis["values"]}))
    t = time.time()
    plan = expect("plan", t, client.call("POST", base + "/setup/plan", {}))["plan"]
    open_conflicts = [c for c in plan["conflicts"] if c["selected"] is None]
    if open_conflicts:
        t = time.time()
        plan = expect(
            "keep the author's files", t,
            client.call("POST", base + "/setup/conflicts", {"plan_id": plan["plan_id"], "decisions": [{"path": c["path"], "choice": "keep"} for c in open_conflicts]}),
        )["plan"]
    t = time.time()
    expect("approve plan", t, client.call("POST", base + "/setup/approve", {"plan_id": plan["plan_id"], "plan_sha256": plan["plan_sha256"]}))
    t = time.time()
    applied = expect("apply", t, client.call("POST", base + "/setup/apply", {"plan_id": plan["plan_id"]}))
    t = time.time()
    workspace = expect("workspace (C-01)", t, client.call("GET", base + "/workspace"))
    descriptor = workspace["descriptor"]

    result = {
        "email_domain": "example.org",
        "account_id": account_id,
        "project_id": project,
        "uploaded_files": files,
        "evidence_offered": len(candidates),
        "model": analysis["model"],
        "analysis_attempts": analysis["attempts"],
        "source_revision": analysis["source"]["revision"],
        "plan_operations": len(plan["operations"]),
        "plan_summary": plan["summary"],
        "apply_status": applied.get("status"),
        "journal_state": applied.get("journal_state"),
        "unplanned_changes": applied.get("unplanned_changes"),
        "c01": {
            "schema_version": descriptor["schema_version"],
            "workspace_id": descriptor["workspace_id"],
            "tenant": descriptor["tenant"],
            "setup_receipt_state": descriptor["setup_receipt"]["transaction_state"],
            "generated_files": len(descriptor["setup_receipt"]["generated_files"]),
            "setup_revision": descriptor["component_closure"]["setup_revision"],
            "game_profile": descriptor.get("game_profile"),
            "capabilities": [c["name"] for c in descriptor.get("capabilities", [])],
        },
        "setup": workspace["setup"],
        "steps": log,
    }
    with open(args.out, "w", encoding="utf-8", newline="\n") as handle:
        json.dump(result, handle, ensure_ascii=False, indent=2)
        handle.write("\n")
    print("done:", result["apply_status"], result["journal_state"], result["c01"]["setup_receipt_state"])
    return 0 if result["apply_status"] == "applied" else 1


if __name__ == "__main__":
    sys.exit(main())
