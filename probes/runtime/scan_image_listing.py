"""Scan a `docker export | tar -t` listing for coding CLIs, interpreter
runtimes, credentials and game files. Usage: python -I scan_image_listing.py <listing>"""
import json
import re
import sys

PATTERNS = {
    "coding_cli": r"(^|/)(claude|claude-code|codex|aider|opencode|gemini|cursor|copilot|goose|continue|cline|amp|pi)(\.exe)?$|(^|/)@anthropic-ai/|(^|/)@openai/codex",
    "node_runtime": r"(^|/)(node|nodejs|npm|npx|pnpm|yarn|bun|deno)(\.exe)?$|(^|/)node_modules/",
    "python_runtime": r"(^|/)python[0-9.]*$|(^|/)pip[0-9.]*$|(^|/)lib/python[0-9.]+/",
    "credential_file": r"(^|/)(\.env(\..*)?|\.netrc|\.git-credentials|\.npmrc|\.pypirc|credentials(\.json)?|id_rsa|id_ed25519|.*\.pem|.*\.key|auth\.json|\.claude\.json)$",
    "game_file": r"(?i)(hearts of iron|hoi4(\.exe)?$|(^|/)common/national_focus/|(^|/)interface/.*\.gfx$|\.dds$|(^|/)map/provinces\.bmp$|(^|/)history/countries/)",
    "recordings_or_evidence": r"(^|/)(recordings|evidence)/|\.sse$",
}
# The CA bundle legitimately contains .pem certificates (public keys, not credentials).
ALLOW = [r"^etc/ssl/certs/", r"^usr/share/ca-certificates/", r"^usr/local/share/ca-certificates/"]
# Reviewed false positives (see evidence/S01/exp04/runs/review.out).
REVIEWED = {
    "usr/lib/ssl/cert.pem": "symlink to /etc/ssl/certs/ca-certificates.crt, the public CA bundle",
    "usr/share/gcc/python": "libstdc++6 gdb pretty-printer scripts, no Python interpreter is installed",
}


def main():
    lines = [l.strip().rstrip("/") for l in open(sys.argv[1], encoding="utf-8") if l.strip()]
    hits = {k: [] for k in PATTERNS}
    allowed = []
    reviewed = []
    for l in lines:
        if l in REVIEWED:
            reviewed.append({"path": l, "reason": REVIEWED[l]})
            continue
        for k, p in PATTERNS.items():
            if re.search(p, l):
                if k == "credential_file" and any(re.search(a, l) for a in ALLOW):
                    allowed.append(l)
                else:
                    hits[k].append(l)
    interpreters = [l for l in lines if re.search(r"(^|/)(usr/)?bin/(perl|sh|bash|dash)$", l)]
    print(json.dumps({
        "entries": len(lines),
        "hits": hits,
        "clean": all(not v for v in hits.values()),
        "allowed_ca_bundle_pem_count": len(allowed),
        "reviewed_false_positives": reviewed,
        "base_os_interpreters_present": interpreters,
        "agent_binary_present": "usr/local/bin/runtime-probe" in lines,
        "patterns": PATTERNS,
    }, indent=1))


if __name__ == "__main__":
    main()
