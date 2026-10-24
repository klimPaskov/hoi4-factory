# EXP-04 report: native request from a container with no coding CLI (S01-W5)

Recorded 2026-10-24. Base revision `290af7c48ce160d4f7461b90aedc92528295330b` with uncommitted S01 changes. Machine-readable results are in [results.json](results.json). Raw outputs are in `runs/`. The GitHub App part of S01 gate 4 is outside this packet and is not covered here.

## Summary

The probe image holds one Rust binary and the Debian CA bundle. One live native request from the running container to Anthropic Messages returned HTTP 200 with a complete `tool_use` turn from `claude-haiku-5-5`. A filesystem listing of the image shows no coding CLI, no Node or Python runtime, no credential and no game file. The key was passed only at run time by variable name. OpenAI was not attempted from the container because both OpenAI keys return 429 `credit_balance_exhausted`.

## Image

| Field | Value |
|---|---|
| Tag | `hoi4-factory-agent-probe:s01` |
| Image ID (local, not pushed) | `sha256:d31ea492a4418930952b7d68b54e2328d9ba9ff12c57d253d2940c05172b4e27` |
| Build base | `rust:1.97.1-slim-bookworm@sha256:2775a09d208ff0d7c1f50490c45b62db929e87ba1dcbc3f2132ac71a704bcdd3` |
| Runtime base | `debian:bookworm-slim@sha256:7ff6ba31527b9e336162fa061746c366c4b2032f4d2cff2078bea43f78efe3f8` |
| Added package | `ca-certificates 20250419~deb12u2` |
| Platform | linux/amd64, 37,817,617 bytes |
| User | `factory` (uid 10001) |
| Entrypoint | `/usr/local/bin/runtime-probe`, default command `live-once` |
| Environment | `PATH` only |

The Dockerfile is `docker/agent-probe.Dockerfile`. It copies only the workspace manifests, `crates/` and the probe crate source into the build stage, and it builds with `--locked`. The build took 153 s on Docker Desktop 28.5.2. The apt package version is not pinned by digest.

## Filesystem proof

`docker export $(docker create IMAGE) | tar -t` produced 4,747 entries ([runs/filesystem-listing.txt](runs/filesystem-listing.txt)). `probes/runtime/scan_image_listing.py` searched them for coding CLIs (claude, codex, aider, opencode, gemini, cursor, copilot and others), Node and package managers, Python, credential file names, HOI4 game paths and asset types, and probe recordings or evidence.

| Category | Hits |
|---|---|
| Coding CLI | 0 |
| Node runtime | 0 |
| Python runtime | 0 after review |
| Credential file | 0 after review |
| Game file | 0 |
| Recordings or evidence | 0 |

The scan matched two names that review showed are false positives ([runs/review.out](runs/review.out)). `usr/lib/ssl/cert.pem` is a symlink to the public CA bundle. `usr/share/gcc/python` holds libstdc++6 gdb pretty-printer scripts, and no Python interpreter is installed. Inside the image, `command -v` found none of python, python3, node, npm, npx, claude, codex or pip. The base OS keeps `sh`, `bash`, `dash` and `perl` from Debian essential packages. None of them is a coding CLI.

## Credential proof

The exported image bytes were compared in memory against both real key values without printing them ([runs/secret-scan.json](runs/secret-scan.json)). Neither key is present. A prefix pattern scan matched only `sk-ant-s01-invalid-probe-key`, the deliberately invalid key literal the auth refusal probe compiles into the binary. `docker image inspect` shows no key in the configuration, and `docker history` shows none in any layer.

## Runs

| Check | Command | Result |
|---|---|---|
| No key | `env -u OPENAI_API_KEY -u ANTHROPIC_API_KEY docker run --rm hoi4-factory-agent-probe:s01 live-once` | Refused before sending, `request_sent: false` |
| Live request | `docker run --rm -e ANTHROPIC_API_KEY hoi4-factory-agent-probe:s01 live-once` | HTTP 200, `finish_reason: tool_use`, one `write_file` call with valid arguments, 508 input and 84 output tokens, about 0.000093 USD estimated, 1.8 s |

The live request used `anthropic-version: 2026-10-21` from Linux x86_64 inside the container. The key was read per command from the Windows user environment and passed to Docker by name. It was never written to a file, an image layer or a log.

## Results

| ID | Kind | Verdict |
|---|---|---|
| EXP04-IMAGE-BUILD | static | pass |
| EXP04-FS-NO-CODING-CLI-OR-RUNTIME | static | pass |
| EXP04-NO-CREDENTIAL-IN-IMAGE | static | pass |
| EXP04-NO-GAME-FILES | static | pass |
| EXP04-NO-KEY-REFUSES-BEFORE-SEND | static | pass |
| EXP04-LIVE-NATIVE-REQUEST | live | pass |

## Limitations

- The digest is a local image ID. No registry push was made, so no registry digest exists.
- The scan matches file names. It does not scan binary contents for embedded interpreters, except for the key check.
- No game copy exists on this host to compare against, so the game-file check is pattern based.
- OpenAI from the container is unavailable because both OpenAI keys return 429 `credit_balance_exhausted`.
- The container runs under plain Docker, which is acceptable for development only under ADR-020. gVisor containment belongs to EXP-07.

## Reproduction

```
ANTHROPIC_API_KEY="$(powershell.exe -NoProfile -Command "[Environment]::GetEnvironmentVariable('ANTHROPIC_API_KEY','User')" | tr -d '\r')" RUNTIME_PROBE_LIVE=1 bash probes/runtime/run-exp04.sh
python -I probes/runtime/collect_results.py
```

Without `RUNTIME_PROBE_LIVE=1`, the script builds, inspects and scans the image and makes no provider call.
