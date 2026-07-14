# Grayslate Landing Site — Plan

## Context

Grayslate is an open-source Tauri devtool scratchpad. Domain `grayslate.app` is owned; README download badges already point there but nothing resolves yet (circular). Need a single-page, static, elegant landing — download-focused, GitHub-forward — on Cloudflare Pages. No docs/help/plugins. Design must match the app's own clean, calm look (not a black "AI" theme) and support light + dark that follows the visitor's OS.

## Decisions (confirmed)

- **Framework:** Astro (static, ships ~zero JS, first-class Cloudflare Pages support).
- **Location:** new `/website` subfolder in this monorepo; Cloudflare Pages deploys from that subdir. Not wired into the pnpm workspace / Tauri build (self-contained).
- **Downloads:** GitHub Releases, auto-latest. No releases exist yet — buttons target `.../releases/latest` (GitHub redirects to newest); OS auto-detect sets the primary CTA label + highlights the matching platform.
- **Design:** "Option 1" — app-native slate, neutral off-white CTA, chalk `</>` mark, centered hero, product screenshot as the visual. Quiet and minimal (Zed / Linear / Ghostty references). No shouty "open source · local · free" pill — "open source" is carried by the GitHub star count; privacy is one muted line under the CTA.
- **Theming:** light + dark, both reused from the app's real shadcn tokens. Default follows `prefers-color-scheme`; a nav toggle overrides and persists in `localStorage`.
- **Screenshots:** reuse `docs/hero.png`, `docs/csv.png`, `docs/json-copy.png`, `docs/transforms.png`. Logo `app-icon.png`. (No new screenshots.)

## Constants

- Repo: `https://github.com/shriram-ethiraj/grayslate`
- Domain: `grayslate.app`
- Releases latest: `https://github.com/shriram-ethiraj/grayslate/releases/latest`

## Brand + palette (from `src/routes/shadcn.css`, verbatim)

Chalk mark: mono `</>` — `<` cyan `#3fd0e0`, `/` foreground, `>` pink `#f0537f`. Cyan is the single accent, used sparingly (mark, link hovers, focus ring).

Dark (`.dark`): bg `#1b1e26` · card `#1f232c` · raised `#272c37` · accent-surface `#2f3542` · border `#3a4050` · text `#f2f5f9` · muted-text `#9aa5b8` / hint `#6b7385`. CTA = off-white `#eef1f5` with `#1b1e26` text.
Light (`:root`): bg `#f0f2f7` · card `#fafbfc` · muted-surface `#dfe4ec` · accent-surface `#d3d9e5` · border `#bec8d6` · text near-`#111318` · muted-text gray-blue. CTA = near-black `#1b1e26` with off-white text.

Define these as CSS custom properties in one global stylesheet; `:root` = light, `[data-theme="dark"]` = dark. Every component reads the vars — no per-mode hardcoding.

## Scaffold

```
website/
  package.json          # astro, @astrojs/tailwind (or vanilla CSS), tailwindcss
  astro.config.mjs      # site: 'https://grayslate.app', static output
  README.md             # Cloudflare Pages + custom-domain setup steps (manual)
  public/
    app-icon.png  favicon.png
    hero.png csv.png json-copy.png transforms.png   # copied from /docs
    _headers            # Cloudflare security headers (CSP, etc.)
  src/
    styles/theme.css            # the CSS-var palette above, both modes
    layouts/Base.astro          # <head>, OG/meta, inline anti-flash theme script (see below), loads theme.css
    pages/index.astro           # single page, composes sections
    components/
      Nav.astro                 # chalk mark + wordmark; GitHub stars badge; ThemeToggle; Download
      ThemeToggle.astro         # sun/moon; sets data-theme + localStorage
      Hero.astro                # headline, subhead, OS-detected CTA + "View source", muted platform line, hero.png in a window frame
      DownloadButtons.astro     # 3 OS buttons + auto-detect highlight (also used in a lower Download section)
      Features.astro            # 4 feature cards (from README "What it does")
      Screenshots.astro         # csv / json-copy / transforms shots + captions
      Transformations.astro     # grouped 80+ sample list
      FAQ.astro                 # reuse README FAQ Q&As
      Footer.astro              # GitHub, Issues, MIT, built-with-Tauri note
    lib/detectOS.ts             # UA-based OS detect (tiny client script)
```

## Theming behavior

- **Anti-flash inline script** in `<head>` (before body paints): read `localStorage.theme`; else use `matchMedia('(prefers-color-scheme: dark)')`; set `document.documentElement.dataset.theme` immediately. No FOUC.
- **ThemeToggle**: click cycles light/dark, writes `localStorage.theme`, updates `data-theme`. If the user never toggles, it keeps following the OS via a `matchMedia` change listener.
- All colors flow from `theme.css` vars so both modes are correct by construction. Screenshots: `docs/*.png` are dark-theme app shots — fine on dark; on light, frame them in a card with a subtle border so they still read cleanly (acceptable; regenerating light-mode shots is out of scope).

## OS auto-detect (`detectOS.ts`)

- `navigator.userAgent`/`platform` → `mac | windows | linux | unknown`.
- Set hero primary CTA label ("Download for macOS") + href to that platform's target; add accent ring to the matching button in the download row. Unknown → generic "Download" to `releases/latest`, all buttons equal.
- Progressive enhancement: with JS off, every button is a plain link to `releases/latest`.

## GitHub stars

Use the Shields.io badge image (`img.shields.io/github/stars/shriram-ethiraj/grayslate`) — no JS, no API rate limits, matches README. (Live API fetch is the rejected alternative.)

## Page sections (reuse README copy verbatim where possible)

- **Hero:** headline "A fast scratchpad for code, data, and quick thinking."; subhead = the "window you keep open next to your editor…" line; CTA (auto OS) + "View source"; muted "Universal · Windows · Linux · runs entirely on your machine"; `hero.png` in a window frame.
- **Features (4 cards):** Transform text without a website (80+, local); Open big CSVs (Rust-backed virtualized, 100k+ rows); Paste first, name it later (40+ language detection, auto-save slates); Work with JSON faster (right-click copy path/key/value).
- **Screenshots:** csv.png, json-copy.png, transforms.png + README captions.
- **Transformations:** grouped list (JSON / CSV / Encoding / Hashing / Text / Numbers & time / Formatters / Misc).
- **FAQ:** the README Q&As (drop the price-focused framing; keep "different from Boop?", "why Tauri not Electron?", "why not an online formatter?", "large files?").
- **Footer:** GitHub, Issues, MIT license, built-with-Tauri note.

## Cloudflare Pages deploy (documented in `website/README.md`; user does dashboard clicks)

- Cloudflare Pages → connect repo → root directory `website`, build `npm run build`, output `dist`.
- Add custom domain `grayslate.app` (+ `www` → apex redirect).
- `astro.config.mjs` `site: 'https://grayslate.app'` for canonical/OG URLs.

## Out of scope

Docs, help, blog, plugin pages, analytics, live GitHub API calls, new/light-mode screenshots, wiring the site into the Tauri/pnpm workspace build.

## Verification

1. `cd website && npm install && npm run dev` → open via preview tools.
2. Dark + light both render correctly (`preview_resize` colorScheme dark/light); toggle flips theme and persists (reload keeps choice); no FOUC; no console errors.
3. OS auto-detect: `preview_eval` to spoof UA / inspect which button gets the accent + CTA href → `releases/latest`.
4. All links resolve to correct GitHub URLs; star badge image loads.
5. Responsive: mobile stacks single column (`preview_resize mobile`).
6. `npm run build` → static `dist/` produced.
7. (Manual, user) Cloudflare Pages connect + custom domain per `website/README.md`.
