# Grayslate website

Static Astro landing page for [grayslate.app](https://grayslate.app).

## Local development

From the repository root:

```sh
pnpm run dev:website      # start the dev server
pnpm run build:website    # production build to website/dist
pnpm run preview:website  # serve the built site locally
pnpm --filter @grayslate/website check  # type-check
```

`dev:website` and `build:website` run `svelte-kit sync` first. This generates the
root app's `.svelte-kit/tsconfig.json`, which the shared root `tsconfig.json`
extends; without it the Astro build (rolldown) fails resolving the root tsconfig on
a clean checkout. The production output is written to `website/dist`.

## Cloudflare Pages

- Root directory: repository root
- Build command: `pnpm run build:website`
- Build output directory: `website/dist`
- Production branch: `main`
- Environment variables: `NODE_VERSION=24.14.0`, `PNPM_VERSION=10.32.1`

Connect the Git repository in Cloudflare Pages, add `grayslate.app` as the custom domain, and configure `www.grayslate.app` to redirect to the apex domain.

## Release contract

Platform CTAs use GitHub's stable latest-release asset URLs. Each public release must include these aliases:

- `grayslate-macos-universal.dmg`
- `grayslate-windows-x86_64-setup.exe`
- `grayslate-windows-aarch64-setup.exe`
- `grayslate-linux-x86_64.AppImage`
- `grayslate-linux-x86_64.deb`
- `grayslate-linux-x86_64.rpm`

The Tauri updater manifest is the `latest.json` asset attached to the latest GitHub release. It is not deployed by the website.

## Product media

Source screenshots live in `src/assets/product`. Product screenshots deliberately use the original lossless PNGs so editor text remains sharp on high-DPI screens.

Capture every product view in both app themes before a public release. The dark image uses `name.png`; its light counterpart uses `name-light.png`. Keep each pair at identical dimensions and with identical content so the website can switch them together with its theme.
