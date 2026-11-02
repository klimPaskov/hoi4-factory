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
| Mascot build | Layered SVG with CSS, or generated images with CSS | Generated images, one per pose, moved with CSS | SVG would lose the detail |

Real national flags and political or military insignia are kept out of every image. One generated logo concept was set aside because it read as the NATO emblem.

## Mascot

One master image fixes the character, and every pose was generated with it as the reference, on a transparent background: wave, point right, point left, point up, point down, cheer, thumbs up, think, reading, working, waiting, surprised, sorry and peek. The app uses trimmed WebP copies (`web/src/assets/mascot`, 491 KB in total), and the originals stay in `.work/mascot-src`. The `Mascot` component shows a pose with a speech bubble and hops in, bobs or bounces with CSS. Reduced motion keeps it still.

## Pages

None redesigned yet. Next: the sign-in round.
