# Goal: redesign the HOI4 Factory browser client

For the maintainer: run this session after S03 is finished and pushed. Open Claude Code at the root of an up-to-date clone of https://github.com/klimPaskov/hoi4-factory and give it everything below this line as its goal. This track is authorized by ADR-025 and needs no `/start-implementation-stage`. It is interactive, so stay available to choose between previews.

---

You are redesigning the browser client of HOI4 Factory, a free, open-source hosted web app where hobbyist modders and AI agents build Hearts of Iron IV mods. The current client under `web/` works but looks temporary. Your job is to give it a polished, consistent design that the maintainer chooses page by page from live previews. Follow the `ui-design-iteration` skill for the loop.

## Before any design work

1. Read CLAUDE.md fully, then `docs/02-architecture/maintainer-decisions-2026-10-24.md` (ADR-021 on translatable strings and ADR-025 on this track), the latest stage handoff under `evidence/`, and `docs/07-developer-workflow/DEVELOPER-SETUP.md`.
2. Make sure the stack runs (`docker compose up -d --build --wait`) and the client checks pass (`web/scripts/docker-npm.sh run check`). Start the client dev server with `web/scripts/docker-npm.sh run dev`, which serves http://127.0.0.1:5173 and proxies `/api` to the server.
3. Inventory every screen and state in `web/src`: sign up, sign in, email verification, password reset, project list, project creation, upload and GitHub import, the Setup wizard steps and recovery, account settings, and the task and API key screens S03 added. Include empty, loading, error and unavailable states. Take a screenshot of each current screen with the built-in browser, so the maintainer can compare before and after.

## How the work goes

1. **Overall direction first.** Build a development-only design lab at `/design-lab`, included only when `import.meta.env.DEV` is true so it never reaches the production bundle. Show three or four distinct visual directions applied to one representative page, such as the project list. Vary layout, typography, color, density and tone. Use realistic HOI4 modding data from fixtures, not lorem ipsum. Each variant is real React code that the maintainer can click through at a phone width and a desktop width.
2. **Ask the maintainer to choose.** Use a question with one option per variant and a short description of each. The maintainer may pick one, combine parts or ask for another round. Never pick for them.
3. **Turn the choice into a design system.** Define color, spacing, type and radius tokens as CSS custom properties with light and dark themes, then the shared components in `web/src/components/ui.tsx` (buttons, fields, cards, tables, dialogs, notices, empty states, step indicators). Show them on a components page in the design lab for approval.
4. **Then page by page,** in the order the maintainer prefers, defaulting to sign-in, project list, project page and the Setup wizard. For each page, show three or four variants in the lab built on the approved system, ask, implement the chosen one in the real page, remove the losing variants, run the checks and commit. Then move to the next page.
5. Record each choice in `docs/design/DESIGN-DECISIONS.md`: the page, the variants offered, what was chosen and why, plus screenshots saved under `docs/design/screenshots/`.

## Rules

- Keep every string in the i18n files. No hard-coded text.
- Keep accessibility: semantic elements, labels, focus states, keyboard use and sufficient contrast in both themes. The jsx-a11y lint rules must pass.
- Keep behavior. Do not change API calls, routes or server code unless a design needs a small, reviewed API addition, and ask first in that case.
- Add no new runtime dependency without asking. Prefer plain CSS with custom properties.
- Keep `web/scripts/docker-npm.sh run check` green after every page, and keep or update the existing tests.
- Batch questions so the maintainer answers several at once when possible. Between answers, prepare the next round of variants.
- Commit after each approved page with the attribution trailer CLAUDE.md requires.

## Finish

When the maintainer says the design is done, remove nothing they still want from the design lab, write a short handoff in `docs/design/DESIGN-DECISIONS.md` covering the system, the pages done and any page left for later, update the README screenshots if any, run the package validator and the skill check, and push to `origin main`. Record in CLAUDE.md or the skill whatever the loop taught you. Do not start S04. The next session uses `docs/06-agent-handoffs/GOAL-S04.md`.
