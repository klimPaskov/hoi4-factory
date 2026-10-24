"""Create the HOI4 Factory Dev GitHub App from a manifest.

Serves a local page with one button that submits the manifest to GitHub. GitHub
redirects back with a one-time code, which this script exchanges for the App's
credentials. Credentials are written to .work/github-app/app.json (git-ignored)
and never printed. Only nonsecret fields are printed.

Usage: py -3.13 -I probes/github/manifest_server.py <repo root>
"""
import html
import json
import secrets
import sys
import urllib.request
from http.server import BaseHTTPRequestHandler, HTTPServer
from pathlib import Path
from urllib.parse import parse_qs, urlparse

PORT = 8765
root = Path(sys.argv[1])
out_dir = root / ".work" / "github-app"
out_dir.mkdir(parents=True, exist_ok=True)
state = secrets.token_urlsafe(16)

manifest = {
    "name": "HOI4 Factory Dev",
    "url": "https://github.com/klimPaskov/hoi4-factory",
    "description": "Development GitHub App for HOI4 Factory. Install it only on hoi4-factory-test-mod.",
    "public": False,
    "redirect_url": f"http://127.0.0.1:{PORT}/callback",
    "callback_urls": [f"http://127.0.0.1:{PORT}/oauth/callback"],
    "request_oauth_on_install": False,
    "hook_attributes": {"url": "https://example.invalid/hoi4-factory-dev", "active": False},
    "default_permissions": {
        "contents": "write",
        "pull_requests": "write",
        "issues": "write",
        "checks": "read",
        "statuses": "read",
        "metadata": "read",
    },
    "default_events": [],
}

PAGE = f"""<!doctype html><meta charset="utf-8"><title>Create HOI4 Factory Dev App</title>
<body style="font-family:system-ui;max-width:40rem;margin:3rem auto">
<h1>HOI4 Factory Dev GitHub App</h1>
<p>This creates a <b>private</b> GitHub App named <code>{html.escape(manifest["name"])}</code> on your account,
with contents, issues and pull requests write, checks and statuses read. Its webhook is inactive.
Next you will install it on <code>hoi4-factory-test-mod</code> only. If GitHub says the name is taken, edit it there and continue.</p>
<form action="https://github.com/settings/apps/new?state={state}" method="post">
<input type="hidden" name="manifest" value="{html.escape(json.dumps(manifest))}">
<button type="submit" style="font-size:1.2rem;padding:.6rem 1.2rem">Create the App on GitHub</button>
</form></body>"""


class Handler(BaseHTTPRequestHandler):
    def log_message(self, *args):
        pass

    def _send(self, body, status=200):
        self.send_response(status)
        self.send_header("Content-Type", "text/html; charset=utf-8")
        self.end_headers()
        self.wfile.write(body.encode("utf-8"))

    def do_GET(self):
        url = urlparse(self.path)
        if url.path == "/":
            return self._send(PAGE)
        if url.path != "/callback":
            return self._send("not found", 404)
        q = parse_qs(url.query)
        if q.get("state", [""])[0] != state or "code" not in q:
            return self._send("state mismatch or missing code", 400)
        req = urllib.request.Request(
            f"https://api.github.com/app-manifests/{q['code'][0]}/conversions",
            method="POST",
            headers={"Accept": "application/vnd.github+json", "X-GitHub-Api-Version": "2022-11-28"},
        )
        with urllib.request.urlopen(req, timeout=30) as resp:
            app = json.loads(resp.read())
        (out_dir / "app.json").write_text(json.dumps(app), encoding="utf-8")
        public = {k: app.get(k) for k in ("id", "slug", "name", "html_url", "permissions", "events", "created_at")}
        (out_dir / "app-public.json").write_text(json.dumps(public, indent=2), encoding="utf-8")
        print(json.dumps(public), flush=True)
        install = f"https://github.com/apps/{app['slug']}/installations/new"
        self._send(f"<body style='font-family:system-ui;max-width:40rem;margin:3rem auto'><h1>App created</h1>"
                   f"<p>Now install it on <b>hoi4-factory-test-mod only</b>:</p>"
                   f"<p><a href='{install}' style='font-size:1.2rem'>Install {html.escape(app['slug'])}</a></p>"
                   f"<p>Choose <i>Only select repositories</i> and pick <code>hoi4-factory-test-mod</code>.</p></body>")
        raise SystemExit(0)


print(f"open http://127.0.0.1:{PORT}/", flush=True)
try:
    HTTPServer(("127.0.0.1", PORT), Handler).serve_forever()
except SystemExit:
    pass
