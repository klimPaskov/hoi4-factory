---
name: ui-design-iteration
description: "Use when a browser client page is designed or redesigned. Build three or four working variants in the development-only design lab, let the maintainer choose, implement the chosen one with i18n and accessibility intact, record the decision, and move to the next page."
---

# UI design iteration

## Trigger and inputs

Use this workflow whenever a page of the browser client is designed or redesigned: the UI design track (ADR-025), or a new page that a later stage adds. Inputs are the page or pages in scope, the approved design system if one exists, the current screens and the [instructions](../../../CLAUDE.md).

## Procedure

1. Inventory the page: its route, every state (empty, loading, error, unavailable, success), the data it shows and the actions it offers. Screenshot the current version with the built-in browser.
2. Build three or four variants as real React components in the development-only design lab (`/design-lab`, included only when `import.meta.env.DEV` is true). Use fixtures with realistic HOI4 modding data and show every important state. Make the variants differ in a way worth choosing between, such as layout, density or emphasis, not just color. When a design system is approved, every variant uses its tokens and components.
3. Check each variant yourself before showing it: open it at phone width (375 pixels) and desktop width, look for overflow, contrast and focus problems, and fix them.
4. Ask the maintainer to choose with one question: one option per variant, each with a one-line description, and the lab URL. Batch questions for several pages when they are independent. Never choose for the maintainer, and prepare the next round while waiting.
5. Implement the choice in the real page. Keep strings in i18n, keep behavior and API calls, and update tests. Delete the variants that lost, so the lab holds only open choices and the approved components.
6. Run `web/scripts/docker-npm.sh run check`, take an after screenshot, record the choice in `docs/design/DESIGN-DECISIONS.md`, and commit.
7. Repeat for the next page. When the maintainer asks for changes rather than choosing, make a new round from the closest variant.

## Outputs and evidence

The redesigned pages, the design system tokens and components, `docs/design/DESIGN-DECISIONS.md` with each choice and its before and after screenshots, and a passing client check. Screenshots are design evidence, not usability evidence. Usability is observed in the S08 pilot.

## Stop and error paths

If a design needs an API change, a new runtime dependency or a server change, ask the maintainer before building it. If the dev server cannot reach the API, check that the Compose stack is running and that Vite proxies `/api` to it. If the maintainer is unavailable, keep preparing variants and do not implement an unchosen design.
