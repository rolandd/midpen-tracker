# Spec: Frontend major-version upgrades (web/)

Status: stages 0-2 complete; stage 3 (cleanup) tracked below
Last updated: 2026-02-06
Owner: web

## Goal

Adopt the major frontend upgrades that were deliberately deferred during the
2026-02 dependency audit, without regressions in the shipped static site:

| Package                          | From        | To          | Note                         |
| -------------------------------- | ----------- | ----------- | ---------------------------- |
| `@sveltejs/kit`                  | 2.70.3      | 3.1.x       | breaking, coupled cluster    |
| `@sveltejs/vite-plugin-svelte`   | 6.2.4       | 7.3.x       | required by kit 3            |
| `vite`                           | 7.3.7       | 8.3.x       | rolldown-based               |
| `@sveltejs/adapter-static`       | 3.0.10      | 4.0.0       | requires kit 3               |
| `typescript`                     | 5.9.3       | 6.0.x       | **not 7** — see blockers     |
| `eslint` / `@eslint/js`          | 9.39.x      | 10.13.x     | independent                  |
| `prettier-plugin-svelte`         | 3.5.2       | 4.1.x       | independent                  |
| `@types/node`                    | 25.9.9      | 24.x        | **pin to runtime**, see note |
| `@cloudflare/workers-types`      | 4.x         | 5.x         | independent                  |

Non-goals: Svelte runtime upgrades (already on 5.57), Tailwind, changes to the
Cloudflare Pages middleware behaviour, backend changes.

## Compatibility matrix (verified from registry metadata)

- `@sveltejs/kit@3.1.0` peers: `vite ^8.0.12`, `svelte ^5.57.1`,
  `typescript ^6.0.0`, `@sveltejs/vite-plugin-svelte ^7.0.0`,
  `@opentelemetry/api ^1.0.0`; engines `node >=22.17`.
- `@sveltejs/vite-plugin-svelte@7.3.1` peers: `vite ^8`, `svelte ^5.46.4`.
- `@sveltejs/adapter-static@4.0.0` peer: `@sveltejs/kit ^3.0.0-next.0`.
- `vite@8.3.5` engines `^20.19 || >=22.12`; peer `@types/node ^20.19 || >=22.12`.
- `typescript-eslint@8.72.0` peer: `typescript >=4.8.4 <6.1.0`.
- `svelte-check@4.7.7` peer: `typescript ^5.0.0 || ^6.0.0`.
- `eslint-plugin-svelte@3.23.1` peers: `eslint ^8.57.1 || ^9 || ^10`.
- `eslint@10.13.0` engines `^20.19 || ^22.13 || >=24`.
- `prettier-plugin-svelte@4.1.1` peers: `prettier ^3`, `svelte ^5`.

Local/CI runtime is Node 24 (`.nvmrc`), which satisfies every engine range above.

## Blockers and decisions

1. **TypeScript 7 cannot be adopted yet.** Kit 3 requires `^6`, while
   `typescript-eslint` 8.72 refuses `>=6.1.0` and `svelte-check` 4.7 supports
   only `^5 || ^6`. Target **TypeScript 6.0.x**; revisit TS 7 when
   `typescript-eslint` and `svelte-check` widen their peer ranges.
2. **Kit 3 removes `svelte.config.js`.** Configuration moves into the
   `sveltekit()` plugin options in `vite.config.ts`, with the `kit.` prefix
   dropped (`sveltekit({ adapter, extensions, preprocess, prerender })`).
3. **Kit 3 removes `$app/stores`.** `src/routes/+layout.svelte` must switch to
   `import { page } from '$app/state'` (`page.url.pathname`, no `$` prefix).
4. **Kit 3 removes the `$lib` alias.** Two options:
   - new idiomatic form: `package.json#imports` with `#lib/*` (Node import map);
   - escape hatch: `alias: { $lib: 'src/lib' }` in the kit config.
   Decision: use `#lib` (upstream direction). Constraint discovered while
   migrating: TypeScript does **not** infer extensions for package-import
   patterns, so every subpath import needs an explicit extension
   (`#lib/api.ts`, `#lib/state.svelte.ts`, `#lib/components/index.ts`) plus
   `allowImportingTsExtensions`. `rewriteRelativeImportExtensions` had to be
   dropped from `tsconfig.json`: it rejects non-relative `.ts` specifiers.
   Vitest/Mock-free unit tests kept relative imports, so they were untouched.
5. **`@types/node`**: pin to `^24` (the runtime and CI Node version) instead of
   `26`, so type-level APIs cannot drift ahead of what production runs.
6. **Latent bug found during recon (pre-existing, not caused by the upgrade):**
   `svelte.config.js` lists `prerender.entries`, but no root `+layout.ts` sets
   `export const prerender = true`, so **nothing is actually prerendered** —
   `pnpm build` emits a single SPA fallback `build/index.html`. The configured
   entry list is dead config. Fixing prerendering changes deploy output and is
   therefore tracked separately (see "Open questions"); the upgrade must not
   silently change it. The build verifier asserts the *current* contract
   (fallback shell + assets + Cloudflare files) and will be revisited if
   prerendering is enabled.

7. **Kit 3 replaces `$env/static/public`.** Env access is now the explicit
   `$app/env/*` API backed by `src/env.ts` + `defineEnvVars`. This app injects
   its public variables at build time through Vite `define`
   (`import.meta.env.PUBLIC_*`), so `+layout.svelte` was switched to
   `import.meta.env.PUBLIC_BASE_URL` (now also defined in `vite.config.ts`) and
   the `$env` import was dropped rather than introducing a second env mechanism.
8. **Vite 8 + bundled postcss-import + SSR.** Vite 8 externalizes packages
   during the SSR build, so its built-in postcss-import received the bare
   specifier `tailwindcss`, resolved it as `./tailwindcss`, and failed with
   `ENOENT`. Mitigated with `ssr: { noExternal: ['tailwindcss'] }` and a note
   in `vite.config.ts`. A follow-up could move to `@tailwindcss/vite` (Tailwind's
   recommended Vite integration) instead of PostCSS.
9. **`cookie` override removed.** `cookie@<0.7.0 -> ^0.7.0` existed only for
   kit 2's `cookie@^0.6` pin (GHSA-pxg6-pf52-xh8x). Kit 3 depends on cookie 2
   and imports ESM named exports, so the override produced a CJS/ESM mismatch;
   it is deleted. A stale hoisted `node_modules/cookie` 0.6.0 directory had to
   be removed manually.
10. **tsconfig shape changed by kit 3.** `tsconfig.json` now extends the virtual
    `$app/tsconfig` and must list `$app/types` in `types` (kit validates this);
    `functions/` keeps its own `tsconfig.functions.json`.

## Test strategy (the safety net, added before any major bump)

Layered, each layer catching a distinct regression class:

| Layer                 | Command               | Catches                                        |
| --------------------- | --------------------- | ---------------------------------------------- |
| Types                 | `pnpm check`          | kit/TS API changes, generated bindings drift    |
| Lint                  | `pnpm lint`           | eslint 10 migration issues                     |
| Format                | `pnpm check-format`   | prettier-plugin-svelte 4 output changes        |
| Unit (vitest)         | `pnpm test:unit`      | `api.ts` contract, demo mode, cache/mock logic |
| Build contract        | `pnpm test:build`     | adapter/kit output layout, precompression,      |
|                       |                       | Cloudflare `_headers`/`_routes.json` emission   |
| Browser smoke (e2e)   | `pnpm test:e2e`       | hydration errors, `$app/state` migration,      |
|                       |                       | demo-mode dashboard render, deep links, CSP-    |
|                       |                       | independent client routing                      |

E2E runs against a production build with `PUBLIC_DEMO_MODE=true`, served by
`scripts/serve-build.mjs`, which mirrors Cloudflare Pages' `_routes.json`
behaviour (unknown paths fall back to `index.html`). The Cloudflare `_middleware`
CSP path is **not** covered (needs the Pages runtime); a future
`wrangler pages dev` job could close that gap.

The gates were green on the old kit 2 / vite 7 tree before any bump, and were
re-run after each stage. Stage 2 was the moment this paid off: it caught the
`$env/static/public` removal, the `#lib` extension requirement, the cookie
override conflict and the Vite 8 SSR/postcss-import failure before deploy.

## Stages

Each stage is one revertable commit; `just check-all` (backend) is unaffected,
frontend gates are the six layers above.

- **Stage 0 — safety net** (done, `test(web): add frontend test layers…`).
  vitest + unit tests, build verifier, Playwright smoke suite, static file
  server, CI jobs, doc updates. No dependency bumps.
- **Stage 1 — independent majors** (done,
  `build(web): upgrade eslint 10, prettier-plugin-svelte 4, Node/Workers
  types`): `eslint`/`@eslint/js` 10, `prettier-plugin-svelte` 4,
  `@types/node` 24, `@cloudflare/workers-types` 5, plus the new
  `functions/` type-check that had no gate before.
- **Stage 2 — coupled cluster** (done, this commit). `typescript` 6 → `vite` 8 →
  `vite-plugin-svelte` 7 → `kit` 3 → `adapter-static` 4, plus config move into
  `vite.config.ts`, `$app/stores` → `$app/state`, `$lib` → `#lib`.
- **Stage 3 — cleanup.** `svelte.config.js` and the `svelte.config.js` doc
  references are already gone; remaining: optional `@tailwindcss/vite`
  follow-up, and resolving the prerender question below.

## Risks

- kit 3 is a large breaking release (cookies, `Page` type moves, `handle`
  signature, `preloadStrategy`, svelte config loading). Mitigation: the app is
  small (2.5k LOC, 5 routes), the safety net exercises every route, and the
  cluster lands in a single commit that can be reverted independently.
- `vite@8` replaces Rollup/esbuild with Rolldown; third-party plugin
  incompatibilities are the main risk. Our config is minimal (`plugins`,
  `define`), so exposure is low.
- Playwright adds a browser download to CI (~2 min). It lives in its own job so
  a browser flake cannot block check/lint/build.

## Open questions

1. Should prerendering be enabled (fixing the dead `prerender.entries`)? If yes,
   `/dashboard` etc. must be prerenderable and the deploy output changes;
   verify against Cloudflare Pages routing first.
2. ~~Migrate `$lib` → `#lib` in Stage 2 or keep `alias`?~~ Resolved: `#lib` with
   explicit extensions (see decision 4).
3. Should the Tailwind integration move from `@tailwindcss/postcss` to
   `@tailwindcss/vite`? This would delete `postcss.config.js`,
   `@tailwindcss/postcss`, `postcss` and `autoprefixer`, and may remove the
   need for the `ssr.noExternal` workaround.
