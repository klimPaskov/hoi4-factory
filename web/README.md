# HOI4 Factory browser client

React, TypeScript (strict) and Vite. Few dependencies: `react`, `react-dom`, `react-router`, `i18next`, `react-i18next`. Versions are pinned exactly in `package.json`, and `package-lock.json` is committed.

All npm work runs in a digest-pinned Node image with `web/` mounted, never on the host:

```sh
web/scripts/docker-npm.sh install        # npm ci/install inside the image
web/scripts/docker-npm.sh run check      # lint, typecheck, tests, build
web/scripts/docker-npm.sh run dev        # dev server on http://localhost:5173
```

The image is `node:22-bookworm-slim@sha256:de170b39439eb0e5a3444ad7d60b015614a381e10fecae2e30ec9e735122f38d` (see the script).

## Running against the server

Start the Rust server on `127.0.0.1:8080`, then start the dev server. Vite proxies `/api` to it. Inside Docker the server is not `127.0.0.1`, so set the target, for example `FACTORY_API_TARGET=http://host.docker.internal:8080` in `web/.env.local`.

`npm run build` writes `web/dist`. The server serves that directory on the same origin as `/api/v1`, so the session cookie and the origin guard work without CORS.

## Layout

- `src/api/` typed client. `accounts.ts` calls routes that exist. `intake.ts` and `gameReference.ts` call routes that are still being built; their endpoints are listed in `PENDING_ENDPOINTS`, and a 404 shows a "not available yet" state.
- `src/errors.ts` maps the error envelope `code` (then `retry_class`) to actionable text.
- `src/i18n/locales/en.json` holds every user-visible string. Add a language by adding a file and listing it in `src/i18n/index.ts`.
- `src/test/` component tests, including a check that no literal user-facing text sits outside the translation file.

## Content Security Policy

The server sends `default-src 'self'; img-src 'self' data: blob:; connect-src 'self'; frame-ancestors 'none'; object-src 'none'`. The client therefore uses no inline scripts or styles, no external fonts, CDNs or APIs, and talks only to same-origin `/api/v1` with cookies. Keep it that way: no `style=` attributes, no `<script>` in `index.html`.
