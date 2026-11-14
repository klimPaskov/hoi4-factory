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
| S04 pages | Two rounds each for the focus editor and the change card | The ask bar with direct editing kept, and a change card that shows only the final result | The agent does most of the work, and the modder looks, asks and approves (D-61, D-63) |
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

### Setup wizard (2026-11-06)

- Offered: three generated layouts ([A horizontal stepper](mockups/40-setup-a-horizontal.webp), [B vertical steps](mockups/41-setup-b-vertical.webp), [C one question at a time](mockups/42-setup-c-focused.webp)).
- Provisional overnight: A, because it matched the project page's step line. Chosen by the maintainer on review: B.
- Built: the Setup card shows the six steps as a column on the left (done steps checked, the current one highlighted) and the current step and the one just finished on the right. On a phone the column turns back into a row of numbered circles. `StepIndicator` takes a `vertical` flag for this. Earlier finished steps open with "Show the earlier steps". All steps stay in one list, so none loses its state or message as the wizard moves on. One test now picks the visible "Scan again" button, because step 1 is folded away.
- After: `setup-*` in [screenshots/after](screenshots/after/).

### Task list (2026-11-06)

- Offered: three generated layouts ([table and side panel](mockups/50-tasks-a-panel.webp), [ask first](mockups/51-tasks-b-composer.webp), [grouped by state](mockups/52-tasks-c-grouped.webp)).
- Provisional overnight: grouped by state. Chosen by the maintainer on review: the table with a side panel.
- Built: one table of every task with the objective, mode, state badge, spend with a small meter, and model. New task in the header opens the form in a panel beside the table, and Cancel closes it. On narrow screens the panel moves above the table, and on phones the table rows become cards. A project with no tasks shows the form straight away. The form keeps its fields and behavior. A test covers the table, the panel and Cancel.
- Before: `tasks-*` in [screenshots/before](screenshots/before/). After: `tasks-*` in [screenshots/after](screenshots/after/).

### Task page (2026-11-06)

- Offered: three generated layouts ([two columns](mockups/60-task-a-columns.webp), [single-column story](mockups/61-task-b-story.webp), [tabs](mockups/62-task-c-tabs.webp)).
- Provisional overnight: the single-column story. Chosen by the maintainer on review: C, tabs.
- Built: the title with the state badge, the spend meter, then Activity, Attempts, Permission and Settings tabs with Start, Pause or Resume, Interrupt and Revoke grant at the end of the tab row. A new `Tabs` component follows the ARIA tab pattern (arrow keys, Home and End) and keeps every panel mounted, so the live activity stream keeps running on another tab. A task that needs review, is blocked or waits for input opens on Attempts. Permission shows the newest attempt's grant. Unknown effects stay visible above the tabs. The activity log shows each event's summary first, with its type, number and time, a live or polling status line, and the raw payload behind Details. A test covers the tabs and the arrow keys. Behavior and API calls are unchanged.
- Before: `task-*` in [screenshots/before](screenshots/before/). After: `task-*` in [screenshots/after](screenshots/after/).

### Account (2026-11-06)

- Offered: three generated layouts ([stacked cards](mockups/70-account-a-cards.webp), [settings list](mockups/71-account-b-settings.webp), [usage first](mockups/72-account-c-usage.webp)).
- Provisional overnight: stacked cards. Confirmed by the maintainer on review.
- Built: one narrow column of cards. Your details show the email and GitHub states as badges. Usage shows the allowance left as one line and a meter of what was used this month, with the unknown-cost note and the spend with your own keys. Model keys are quiet rows with Delete as a text button and the same confirmation. Sessions are unchanged. Behavior and API calls are unchanged.
- Before: `account-*` in [screenshots/before](screenshots/before/). After: `account-*` in [screenshots/after](screenshots/after/).

### Focus editor (2026-11-14)

- Offered, first round: four layouts around the live game render ([render and inspector](mockups/90-focus-a-render-inspector.webp), [three panes](mockups/91-focus-b-three-panes.webp), [floating inspector](mockups/92-focus-c-floating.webp), [table](mockups/93-focus-d-table.webp)). None chosen. All showed too much (D-61).
- Offered, second round: three minimal layouts ([just ask](mockups/100-focus-a-just-ask.webp), [ask with a focus card](mockups/101-focus-b-focus-card.webp), [change list](mockups/102-focus-c-change-list.webp)).
- Chosen by the maintainer: the ask bar over the full-page render, with direct editing kept (D-63). See the [final mockup](mockups/110-focus-editor-final.webp), the [drag](mockups/112-focus-editor-drag.webp) and the [agent change](mockups/113-focus-editor-agent-change.webp).
- Built: the route `/projects/:id/tasks/:taskId/focus/:treeId`. The game render of the task's worktree fills the page. A focus moves by dragging it or with the arrow keys, and a dashed box shows where it lands. Clicking a focus opens a small card with its title, description and cost. One bar holds the request box, the agent's own sentence about its change, and two buttons: Approve and Reject for the agent's changes, Save and Undo for the person's own edits. A check that fails or cannot finish is one sentence beside the buttons. What a focus requires, the problems and the source line sit behind the one Details toggle. There are no position numbers, no internal names and no lists.
- Server additions the design needed, approved by the maintainer on 2026-11-14: a new attempt can carry a follow-up request (`instruction` on `POST .../attempts`), which the request box sends, and the focus operation `set_text` changes a title or description by rewriting only the line of its key in the localisation file that defines it. The projection now carries each description.
- After: `focus-editor-*` in [screenshots/after](screenshots/after/). There is no before, because the page is new.

### Change card (2026-11-14)

- Offered, first round: four layouts ([side by side](mockups/95-change-a-side-by-side.webp), [toggle](mockups/96-change-b-toggle.webp), [slider](mockups/97-change-c-slider.webp), [tabs](mockups/98-change-d-tabs.webp)). Set aside with the first focus editor round (D-61).
- Offered, second round: four minimal layouts ([one picture with a switch](mockups/105-change-a-switch.webp), [one picture with a divider](mockups/106-change-b-divider.webp), [side by side](mockups/107-change-c-side-by-side.webp), [small card](mockups/108-change-d-small-card.webp)).
- Chosen by the maintainer: one full-page picture of the final result, with no before and after comparison (D-63). See the [final mockup](mockups/111-change-card-final.webp).
- Built: the route `/projects/:id/tasks/:taskId/changes/:proposalId`. The picture of the result, one sentence, Approve and Reject. Approve is bound to the proposal hash. When the check cannot finish, the main action becomes Approve anyway. When it fails or the tree changed since, the main action hands the change back to the agent. The diff, the checks, the limits and the restore point sit behind the one Details toggle. The task page links to the focus tree and to the change that waits for review.
- After: `change-card-*` and `task-focus-links-*` in [screenshots/after](screenshots/after/). There is no before, because the page is new.

All pages in the default order follow the design, and every choice is the maintainer's.

## Handoff (2026-11-06)

The maintainer declared the design done on 2026-11-06.

- **System.** The Workbench direction: system font stacks, neutral greys with one blue accent, light and dark themes through `light-dark()` tokens in `web/src/styles.css`. Shared components live in `web/src/components/ui.tsx`. They include buttons, fields, badges, cards, a stepper (horizontal or vertical), a native meter, chips, a segmented control, tabs, a native dialog and flag avatars. Icons are generated alpha masks colored by the text color. The logo is the factory roof in a rounded outline that follows the theme's text color. The brick factory mascot appears only for tips and tutorials, and on the sign-in panel.
- **Pages done.** Sign-in and the other auth pages, the project list, the project page, the Setup wizard, the task list, the task page and the account page. Each section above lists what was offered, what was chosen and its before and after screenshots.
- **Left for later.** No page in the current client is left. Pages that later stages add follow the `ui-design-iteration` skill and reuse these components. The mascot's tutorial phase (guiding a new user through the features) has poses and CSS motion ready but no tutorial flow yet, because that needs a product decision about when it starts.
- **Design lab.** It stays in the development build only, with the component gallery, the screens on sample data and the decision history. The production build contains no lab code.
- **README.** It has no screenshots, so nothing changed there.
