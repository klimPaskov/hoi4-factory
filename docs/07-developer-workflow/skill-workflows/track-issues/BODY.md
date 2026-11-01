# Track issues

## Trigger and inputs

Use this workflow when a session finds a problem it cannot fix itself, at every stage handoff, and when work starts on a tracked issue. GitHub issues are the project's ticket system (maintainer decisions of 2026-10-25 and 2026-11-01). File each problem in the repository it belongs to: `klimPaskov/hoi4-factory`, `klimPaskov/hoi4-agent-tools`, `klimPaskov/Agentic-HOI4-Modding` (the kit), `klimPaskov/HOI4-Mod-Setup` or `klimPaskov/hoi4-imagegen-mcp`. Problems and open gaps are tracked there, not only in plan documents, and plan documents link the issues. `<repo>` below is that repository. Inputs are the problem, its evidence, the current revision and the [instructions](../../../../CLAUDE.md).

## Decide: fix now or file an issue

Fix it in the session whenever you can. A fix with a regression test is always better than a ticket. File an issue only when the problem truly cannot be fixed in this session, for example when:

- It needs something outside the session, such as a maintainer decision, an account with balance, another machine (Windows, Linux hardware, the user's game) or an external service.
- It belongs to a later stage's scope, and fixing it now would start unauthorized work.
- It is larger than the session can finish and verify, and a partial fix would leave the code worse.

Do not file issues for things you just fixed, for routine follow-up the next stage's packet already lists, for style nits, or for guesses without evidence. One issue holds one problem.

## Procedure for filing

1. Pick the repository that owns the problem. Search it first, so nothing is duplicated: `gh issue list -R <repo> --state all --search "<words>"`. Add a comment to an existing issue instead of opening a second one.
2. Make sure the labels exist (`gh label list -R <repo>`). The set is `bug`, `enhancement`, `blocked` (needs a decision or a resource the agent cannot get), `verification` (needs a check on another platform or with a real resource), `backlog` (planned product scope with no stage yet), and in hoi4-factory one `stage:SNN` label for the stage that should pick it up. Other repositories use their own labels where they have them. Create a missing one with `gh label create -R <repo>`.
3. Write the body to a UTF-8 file in the scratchpad, never inline on the command line (Git Bash corrupts non-ASCII arguments), and create the issue with `gh issue create -R <repo> --title "<short problem>" --body-file <file> --label <labels>`. The body has these sections:
   - **Problem**: what is wrong or missing, in plain words.
   - **Evidence**: the commit, the command and its observed output, test names, and evidence files linked at that commit (`https://github.com/<repo>/blob/<sha>/<path>`). Use real observations only, and say "not reproduced" when that is the truth.
   - **Why not fixed now**: the concrete reason from the list above.
   - **Owner**: the session, worker or person who will fix it.
   - **Done when**: the observable acceptance, normally a named test or check and the evidence file it writes. The issue closes only with that proof.
   - **Context**: the stage, the files involved, related decisions and issues.
4. The command prints the issue URL. Record it in the stage handoff under "Decisions and unresolved issues", and in `evidence/maintainer-questions.md` when it needs the maintainer. If the command fails or times out, list issues again before retrying, because the issue may already exist.

## Procedure for fixing a tracked issue

1. Read the issue and its comments, then check that its evidence still matches the current tree. Reproduce the problem, or comment that it no longer reproduces and close it with the evidence.
2. Fix it with a regression test that fails without the fix. Run the checks named under **Done when**, plus the affected crate's tests, on every platform the issue names.
3. Commit with `Fixes #N` in the message (`Fixes <repo>#N` from another repository), and the attribution trailer. Pushing to `main`, or merging the pull request, closes the issue. Then comment with what changed, the tests that ran with their results, and the evidence path.
4. Mention the issue in the stage's `checks.json` or handoff when it was part of a stage.

## Outputs and evidence

Issues with evidence and acceptance on GitHub, their URLs in the handoff, and closed issues that link the fixing commit and its test results.

## Stop and error paths

If `gh` is not signed in or the repository refuses the call, write the issue text into the handoff under a "Not yet filed" heading and tell the maintainer. Never put secrets, keys, private paths or user data in an issue, because the repositories are public. In another repository, follow its own contribution rules for the fix itself, such as its branch and pull request process.
