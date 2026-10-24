# EXP-01 report: Setup binding and recovery

Stage S01, experiment EXP-01. Setup revision `4488a418cad56a158273cb66c4ce7794696b7ed1`. Linux image `rust@sha256:af306cfa71d987911a781c37b59d7d67d934f49684058f96cf72079c3626bfe0` (rust:1.88-bookworm). Windows host toolchain 1.88.0-x86_64-pc-windows-msvc. Factory base revision `290af7c48ce160d4f7461b90aedc92528295330b`. Every check, command, exit code and artifact hash is in `results.json`.

## 1. CI recovery failure: cause found and reproduced

**What CI showed.** Run 39281117181, job 118160257466, ran on Windows Server 2025. It stopped the app with `taskkill /F` after one non-skip operation was verified during apply. It then ran the native rollback, which reached `rolled_back`. After that, `assertOriginals` failed with "The project was not restored exactly". The two inventory digests were 7a0535da and 1641aef2 (evidence: `ci/`).

**What the failure means.** `assertOriginals` first checks every original file for presence and hash. It reaches the digest assertion only when all of those pass. So the CI failure means that extra files existed after rollback, and that no original file was missing or changed. The CI artifact holds only screenshots and digests, so the extra path in CI cannot be named from artifacts.

**Cause.** `safe_fs::write_atomic_noreplace_from` writes new bytes to a temporary file named `.hoi4ms-<random uuid>.tmp` beside the destination. It syncs that file and then renames it into place. A hard kill between creating the temporary file and the rename leaves it behind. The name is random and is never written to the journal. Neither rollback nor the quarantine sweep looks for it. Rollback restores every original byte but leaves this one extra file.

**Reproduction.** I wrote a harness inside the upstream test module of a local clone. It runs a real `run_transaction` in a child process, kills it with `Child::kill` (TerminateProcess on Windows, SIGKILL on Linux), and runs the recovery action that `recovery_action` recommends. My own independent inventory scripts then compare before and after.

| Set | Apply-kill trials | Leftover after native rollback |
|---|---|---|
| Windows, unpatched | 20 | 17, each a `.hoi4ms-<uuid>.tmp` |
| Linux, unpatched | 20 | 20 |
| Linux, unpatched, other runs | 5 + 3 | 8 of 8 |
| Linux, embedded probe recovery | 3 | 3 of 3 |

The 3 clean Windows trials are ones where the kill landed between files rather than inside the write window. In every trial, all unapproved bytes and `.git/index` were unchanged. The only difference was the extra file, which matches the CI pattern exactly.

**Confirming fix (local clone only, 99 lines added and 5 removed).** The fix gives the temporary file a fixed name tied to the transaction and operation, `.hoi4ms-placing-<txid>-<op>.tmp`. Rollback removes that file if it exists. The diff is `patch/fix-placement-temp.diff`.

With the fix, 20 of 20 apply-kill trials restored files exactly on both Windows and Linux. The tied temporary file was present right after the kill in 19 of 20 Windows trials and 20 of 20 Linux trials. Rollback removed it every time, so the fixed runs did hit the crash window.

Upstream `transaction::` and `safe_fs::` tests on the fixed clone also passed:
- Windows: 165 passed, 0 failed.
- Linux: 157 passed, 0 failed.

## 2. Upstream tests

| Run | Result |
|---|---|
| Windows host, unpatched, `--all-features`, transaction and lifecycle filters | 147 passed, 0 failed, 6 ignored |
| Linux, no system deps | Build fails: `libdbus-sys` needs dbus-1 even with the desktop feature off |
| Linux, libdbus-1-dev added in the container, transaction and safe_fs filters | 155 passed, 0 failed |
| Linux, webkit and gtk added, lifecycle filter | 3 passed, 6 ignored |
| Linux, ignored offline all-component lifecycle test, source repo 856940f2 | Could not run. `build_plan` fails because "the trusted native Node runtime is unavailable". It was not rerun with a pinned Node, to keep the work bounded |

The 6 ignored lifecycle tests are marked ignore upstream. They need an explicit package checkout or live installs.

## 3. Binding comparison and recommendation

**Direct embedding works for the scanner and transaction services.** The `hoi4-mod-setup` library crate builds with the desktop feature off on Linux as a git dependency pinned to the revision. `scanner`, `transaction` and `paths` are public and compile without Tauri. The probe build and runs show:
- No tauri, wry, tao, webkit or gtk crates in the normal dependency graph. Only the build-time crates tauri-build, tauri-utils and tauri-winres are compiled.
- The runtime links `libdbus-1.so.3` through `keyring`.
- Scan, the gate refusal, recovery inspection and rollback all ran on server paths as an unprivileged user (uid 10001).

**`commands.rs` cannot be embedded.** It is private and compiled only with the desktop feature, so it needs Tauri. It is about 10,048 lines excluding tests. Only about 92 of those lines touch Tauri APIs directly. The real blocker is design:
- Approvals, analyses and prepared plans live in process-global singletons. These include `PREPARED_PLANS`, `CODEX_ANALYSES` and `CODEX_APPROVED_EVIDENCE`, and the last one holds only the most recent scan. That is unsafe across accounts.
- It reads credentials from the OS keychain (`OsCredentialStore`) and uses `application_data_root()` in 31 places.
- The plan builder needs a trusted Node runtime.

**Gaps the factory adapter must cover:**
- **No workspace boundary.** Setup has no concept of a workspace root, so the adapter must refuse outside paths itself. The probe's gate refused 5 of 5 outside paths.
- **Root identity is not public.** The identity token that `run_transaction` requires has no public constructor.
- **Launcher discovery reads the parent folder.** It reads the project's parent directory and must not be called.
- **Missing state root means desktop paths.** `run_transaction` falls back to desktop application-data paths unless `app_data_root` is passed.
- **Ancestor folders must be readable.** `RootedDir` opens every ancestor folder for reading, so apply fails with EACCES under a 0711 account directory, while scan still works.

**Recommendation: a hybrid.** Embed the scanner and transaction services directly. Extract a stable service facade for analysis, evidence approval, plan building, conflict decisions and approval, ideally upstream in HOI4-Mod-Setup. That facade needs:
- per-session state with no globals
- an injected credential store
- an explicit state root
- a public root-identity function
- a declared Node requirement
- an optional `keyring` feature

**The gates stayed intact.** The scan reports that semantic analysis is required (status `authentication_required`). Both `validate_plan` and `run_transaction` refuse a plan with no confirmed analysis, saying "a confirmed selected-provider analysis is required before apply". No state directory was created. No model was called, and no confirmation was asserted.

## 4. Preservation fixtures

All fixtures are original synthetic fragments under `probes/setup/fixtures/`, built by `build_fixture.sh`. Inventories come from my own `inventory.py` and `compare_inventory.py`, which share no code with Setup.

**F-SETUP-IMPORT: preservation passed.** The fixture contains a CRLF focus file, a BOM localisation file, a binary asset, a 0755 script, the authors' AGENTS.md, a hand-edited skill, a staged localisation edit, an unstaged README edit and untracked files. In every scan, refused apply, interrupted trial and completed apply:
- every unapproved byte and file mode stayed identical
- the `.git/index` SHA-256 stayed identical

The completed-apply control added only the approved paths: `exp01_setup/managed`, `exp01_setup/replace-me.txt` and the lock file. A second-run idempotency check was not exercised.

**F-SETUP-PARTIAL: mixed, see the table.**

| Interruption point | Recovery offered | Result |
|---|---|---|
| Staging | `discard_staging` | Exact restore, 10 of 10 per side |
| Mid-apply | Rollback only, resume never offered | Unpatched not exact, fixed exact |
| Finalization (abort after the lock write) | Resume | Completed, 3 of 3 per side |
| Readiness block | Rollback | Exact |
| Rollback itself interrupted | Rollback retry | Exact. One unplanned Windows case where the harness was terminated by something outside the run |

**F-SETUP-NEW: not passed.** An empty owned root scanned cleanly and stayed empty after the refused apply. The generated file set was not produced, because the plan builder is desktop-only and analysis was not run.

## 5. Production apply reliance: blocked

Production apply is blocked at `4488a418`, because exact restoration fails after an interruption mid-apply. It can be cleared only on an upstream revision that has a fix like this one, tested upstream, and passes the Windows candidate-acceptance recovery run again.

Two smaller gaps also need closing:
- Rollback leaves directories the transaction created, empty, in place.
- A temporary file left by a rollback that is itself killed mid-write is not covered by the confirming fix.

## 6. Remaining unknowns

- The exact extra path in the CI run. The Kaiserreich E2E was not rerun.
- Real provider analysis through the gate.
- The offline lifecycle test, which needs Node.
- The new-project generated file set, and an idempotent second run.
- Why the scanner's Git inspection was incomplete when run as uid 10001.
- What terminated the harness externally in the Windows fixed run `none-1`.
- macOS, which was not tested.
- Trial timing: trials ran alongside builds, so kill timing varied.

## 7. Lesson candidates

These are for `capture-engineering-lessons`, which I did not run as a subagent:
- A `#[path]` containing `..` through folders that don't exist compiles on Windows but fails on Linux.
- A Docker restart can corrupt the rlibs in a cargo target volume, which shows up as error E0786.
- A wrong bind-mount source is silently created as an empty directory.
- Serialized plans holding raw bytes reached 188 MB each and nearly filled the disk.
