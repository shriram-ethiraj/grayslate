# Grayslate website

Static Astro landing page for [grayslate.app](https://grayslate.app).

## Local development

From the repository root:

```sh
pnpm --filter @grayslate/website dev
pnpm --filter @grayslate/website check
pnpm --filter @grayslate/website build
```

The production output is written to `website/dist`.

## Cloudflare Pages

- Root directory: repository root
- Build command: `pnpm --filter @grayslate/website build`
- Build output directory: `website/dist`
- Production branch: `main`

Connect the Git repository in Cloudflare Pages, add `grayslate.app` as the custom domain, and configure `www.grayslate.app` to redirect to the apex domain.

## Release contract

Platform CTAs use GitHub's stable latest-release asset URLs. Each public release must include these aliases:

- `grayslate-macos-universal.dmg`
- `grayslate-windows-x86_64-setup.exe`
- `grayslate-linux-x86_64.AppImage`

The Tauri updater owns `/latest.json`. The release process must place a valid signed updater manifest into the deployed site without allowing a normal website deploy to replace it with stale metadata.

## Product media

Source screenshots live in `src/assets/product`. Product screenshots deliberately use the original lossless PNGs so editor text remains sharp on high-DPI screens.

Capture every product view in both app themes before a public release. The dark image uses `name.png`; its light counterpart uses `name-light.png`. Keep each pair at identical dimensions and with identical content so the website can switch them together with its theme.
