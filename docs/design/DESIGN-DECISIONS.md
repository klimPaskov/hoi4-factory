# Design decisions

The record of the UI design track (ADR-025), started by the maintainer on 2026-11-02. Each entry names what was offered, what the maintainer chose and why. The maintainer's decisions are also summarised in [the decision record](../02-architecture/maintainer-decisions-2026-10-24.md). The workflow is the `ui-design-iteration` skill.

## How the lab works

- `/design-lab` exists only in development builds (`import.meta.env.DEV`). The production bundle carries none of it, checked by searching `web/dist` after a build.
- Fixture mode serves the real pages on sample API answers (`web/src/design-lab/scenarios.ts`), so every state can be seen without seeding a database or running a model. `/design-lab/screens` lists them.
- `node web/scripts/lab-screenshots.mjs <dir>` captures every scenario as a full-page PNG with a local Chrome or Edge, at desktop (1280) and phone (390) width.
- Reference mockups, the icon sheet and logo rounds made with the image model are in `mockups/` and on `/design-lab/mockups`.

## Before

Every current screen was captured on sample data before any design change, in light at desktop and phone width: [screenshots/before](screenshots/before/). Dark captures stay in `.work/shots/before`.

## Decisions so far

| Topic | Offered | Chosen | Why |
|---|---|---|---|
| Fonts | System fonts only, or bundled open-license fonts | System fonts only | No extra files, works under the Content Security Policy |
| Page order | Default, projects first, tasks first | Default: auth pages, project list, project page, Setup wizard, task list, task page, account | Follows the path a new user takes |
| Overall direction | A Workbench, B Dossier, C War room, D Studio, each applied to the project list with working states | A Workbench | A quiet, modern tool with sidebar navigation, neutral greys and one blue accent |
| Image model first | Not offered. The maintainer asked for it | Every visual starts as an image: page mockups, icons, logo and mascot. Code matches the chosen image | Better ideas and a clear target before code |
| Design system | Tokens (light-dark pairs), shared components in `ui.tsx`, the sidebar shell, and parts from the mockups (circle stepper, meters, segmented control, chips, timeline, stat cards, flag-style avatars) | Approved, with the [mockups](mockups/) as the target look for every page | Matches the mockups. All text pairs pass WCAG AA in both themes, form outlines reach 3:1 |
| Minimal screens | Not offered. The maintainer asked for it | As little on screen as possible | Hobbyist users, low friction |
| API additions | Project list extras, recent activity, changed file counts | None | Minimal screens. Designs use the data the API returns today |
| Icons | A generated icon sheet | Used, sliced into 24 alpha masks that take the text color (`web/src/assets/icons`) | Consistent stroke icons from one image |
| Logo | Two generated sheets, four standalone concepts, four fresh concepts, six simple factory icons | The original sawtooth factory roof in a rounded-square outline, no fill, drawn in the theme's text color. The favicon follows the browser theme | Calm and readable at 16 pixels on both themes |
| Mascot | A brass robot, a courier pigeon, a brick factory and a balloon cartographer, then three Hearts of Iron versions of the brick factory | The original brick factory with a scarf and toolbelt, with real eyes instead of window eyes | Friendly guide for a tutorial that points at features |
| Mascot use | Not offered. The maintainer set it | Only for tips, tutorials and when called up. Never as page decoration | Keeps screens minimal |
| Mascot build | Layered SVG with CSS, or generated images with CSS | Generated images, one per pose, moved with CSS | SVG would lose the detail |

Real national flags and political or military insignia are kept out of every image. One generated logo concept was set aside because it read as the NATO emblem.

## Mascot

One master image fixes the character, and every pose was generated with it as the reference, on a transparent background: wave, point right, point left, point up, point down, cheer, thumbs up, think, reading, working, waiting, surprised, sorry and peek. The app uses trimmed WebP copies (`web/src/assets/mascot`, 491 KB in total), and the originals stay in `.work/mascot-src`. The `Mascot` component shows a pose with a speech bubble and hops in, bobs or bounces with CSS. Reduced motion keeps it still.

## Pages

### Sign in, create account, verify email, reset password (2026-11-02)

- Offered: four generated mockups ([A minimal card](mockups/10-signin-a-minimal.webp), [B mascot welcome](mockups/11-signin-b-mascot.webp), [C split panel](mockups/12-signin-c-split.webp), [D one step at a time](mockups/13-signin-d-onestep-dark.webp)).
- Chosen: C. The maintainer then asked for the cinematic blue panel of the first sign-in mockup and for the mascot to lean in from a real edge, not float. A [revised mockup](mockups/14-signin-c-cinematic.webp) was generated and approved. The peek pose was regenerated because the first one had odd anatomy. The panel artwork is a generated image (`web/src/assets/auth-panel.webp`, 24 KB) under a blue wash.
- Built: the auth pages leave the sidebar shell for their own `AuthLayout`. The panel holds the mark, the name and one line. The mascot slides in from the left edge and stands on the bottom edge, and stays still with reduced motion. The form is larger and centered. On phones the panel becomes a slim bar with the mark. Behavior, routes and API calls are unchanged.
- Before: [screenshots/before](screenshots/before/) (`signin-*`, `signup-*`, `forgot-password-*`, `reset-password-*`, `verify-*`). After: [screenshots/after](screenshots/after/).

### Project list (2026-11-02)

- Offered: three generated layouts ([A minimal table](mockups/20-projects-a-table.webp), [B cards](mockups/21-projects-b-cards.webp), [C quiet list](mockups/22-projects-c-list.webp)) and an [empty state with the mascot](mockups/23-projects-empty-mascot.webp).
- Chosen: A, and the one-field empty state. The maintainer then set a rule: the mascot appears only for tips, tutorials and when called up, never as page decoration. The empty state keeps the single field and one line of help without the mascot. The sign-in panel keeps its mascot, which the maintainer asked for.
- Built: a page header with the count and a New project button that opens the form under it, a filter field, a table with a flag-style avatar, visibility badge, role, created date and quiet Rename and Delete per row, with the rename or delete panel in a row of its own. Waiting projects sit in a collapsed section that opens when a project is deleted. On phones the table stacks into cards. Rename and delete moved into a `useProjectActions` hook that the project page shares. Data and API calls are unchanged.
- Tests: the empty state test now checks the single field, and a new test covers the header form and the filter.
- Before: `projects-*` in [screenshots/before](screenshots/before/). After: `projects-*` in [screenshots/after](screenshots/after/).

### Project page (2026-11-05)

- Offered: three generated layouts ([A overview cards](mockups/30-project-a-overview.webp), [B tabs](mockups/31-project-b-tabs.webp), [C guided next step](mockups/32-project-c-guided.webp)).
- Chosen by the maintainer: C.
- Built: the header holds the title with its visibility badge and quiet Open tasks, Rename and Delete. Under it a three-step line (bring in the mod, set it up, give the agent tasks) and only the current step as a card. Finished steps stay reachable in collapsed Mod source and Setup sections. A refresh of the workspace after a Setup step keeps the wizard mounted, so its messages stay. Data and API calls are unchanged.
- Before: `project-*` and `setup-*` in [screenshots/before](screenshots/before/). After: the same names in [screenshots/after](screenshots/after/).

### Setup wizard (2026-11-06, provisional)

- Offered: three generated layouts ([A horizontal stepper](mockups/40-setup-a-horizontal.webp), [B vertical steps](mockups/41-setup-b-vertical.webp), [C one question at a time](mockups/42-setup-c-focused.webp)).
- Provisional: A. The maintainer was asleep and asked that work continue without waiting for approval. A matches the project page's step line and the approved target mockup. The design lab's front page shows the options with A marked, so the maintainer can switch it.
- Built: the Setup card shows a numbered stepper, then the current step and the one just finished. Earlier finished steps open with "Show the earlier steps". All steps stay in one list, so none loses its state or message as the wizard moves on. One test now picks the visible "Scan again" button, because step 1 is folded away.
- After: `setup-*` in [screenshots/after](screenshots/after/).

Next: the task list round.
