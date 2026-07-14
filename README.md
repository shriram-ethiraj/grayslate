<div align="center">
  <img src="app-icon.png" alt="Grayslate Logo" width="128" />
  <h1>Grayslate</h1>
  <p><strong>A fast scratchpad for code, data, and quick thinking.</strong></p>

  <p>
    <a href="https://grayslate.app">
      <img src="https://img.shields.io/badge/Download-macOS-000000?style=flat-square&logo=apple&logoColor=white" alt="Download for macOS" />
    </a>
    <a href="https://grayslate.app">
      <img src="https://img.shields.io/badge/Download-Windows-0078D6?style=flat-square&logo=windows&logoColor=white" alt="Download for Windows" />
    </a>
    <a href="https://grayslate.app">
      <img src="https://img.shields.io/badge/Download-Linux-FCC624?style=flat-square&logo=linux&logoColor=black" alt="Download for Linux" />
    </a>
  </p>

  <p>
    <a href="https://github.com/shriram-ethiraj/grayslate/blob/main/LICENSE">
      <img src="https://img.shields.io/github/license/shriram-ethiraj/grayslate?style=flat-square" alt="License" />
    </a>
    <a href="https://github.com/shriram-ethiraj/grayslate/issues">
      <img src="https://img.shields.io/github/issues/shriram-ethiraj/grayslate?style=flat-square" alt="Issues" />
    </a>
    <a href="https://github.com/shriram-ethiraj/grayslate/stargazers">
      <img src="https://img.shields.io/github/stars/shriram-ethiraj/grayslate?style=flat-square" alt="Stars" />
    </a>
  </p>

  <img src="docs/hero.png" alt="Grayslate in action" width="820" />
</div>

---

Grayslate is the window you keep open next to your main editor. Paste an API response and format it. Drop in a giant CSV and actually scroll through it. Jot notes that save themselves. It starts quickly, does the small jobs well, and stays out of your way — no projects to configure.

## What it does

**Transform text without a website.** The thing most people open a browser tab for — formatting JSON, decoding Base64, converting CSV to JSON, hashing a string — is a keystroke away here, and it all runs on your machine. There are 80+ built-in transformations; a sample is [below](#transformations).

**Open big CSVs.** The table view is backed by Rust and virtualized, so files with hundreds of thousands of rows open and scroll without the app grinding to a halt.

**Paste first, name it later.** Grayslate recognizes 40+ languages from the extension, a shebang, or the content itself. It picks a useful filename and extension, then saves the note (a *slate*) as you type. Rename it whenever you like, or find it again from the sidebar.

**Work with JSON faster.** Right-click a key or value to copy its path, key, or value, much like in Chrome DevTools.

There is also a live Markdown preview and multiline find and replace, including matches across line breaks.

Your files and transformations stay on your machine. No account, no cloud sync, no telemetry.

## Screenshots

<div align="center">
  <img src="docs/csv.png" alt="CSV table view" width="760" />
  <br /><em>Large CSVs in a virtualized table</em>
  <br /><br />
  <img src="docs/json-copy.png" alt="JSON copy path, key, and value actions" width="760" />
  <br /><em>Copy a JSON path, key, or value from the context menu</em>
  <br /><br />
  <img src="docs/transforms.png" alt="Transformations menu" width="760" />
  <br /><em>Built-in transformations, one keystroke away</em>
</div>

## Download

Grab a build for your platform from [grayslate.app](https://grayslate.app), or from the [Releases page](https://github.com/shriram-ethiraj/grayslate/releases).

## Transformations

More than 80 built-in actions. A sample of what's there:

- **JSON** — format, minify, validate (comments & trailing commas allowed), sort keys, JSON ↔ YAML, JSON → CSV, JSON → TypeScript, array ↔ JSON-lines, query string ↔ JSON, rename keys to camel/snake/kebab/Title case.
- **CSV** — CSV → JSON (delimiter auto-detected), and JSON → CSV from the JSON side.
- **Encoding** — Base64 & Base64URL encode/decode, gzip ↔ Base64, hex ↔ ASCII, URL encode/decode, HTML entity encode/decode, decode a JWT (unverified).
- **Hashing** — SHA-256, SHA-512, SHA-1, MD5, CRC32.
- **Text & lines** — upper/lower/Title/camel/snake/kebab/sPoNgE case, sort lines, reverse lines, reverse string, remove duplicates, trim whitespace, collapse blank lines, ROT13, count words/lines/characters.
- **Numbers & time** — binary/decimal/hex conversions, Unix time ↔ RFC 3339.
- **Formatters** — JavaScript, TypeScript, CSS, HTML, Svelte, YAML, TOML, Markdown, SQL, XML.
- **Misc** — insert UUID v4/v7, add/remove slashes, defang/refang URLs.

## Tech stack

- **Frontend** — [SvelteKit](https://kit.svelte.dev/) + [Svelte 5](https://svelte.dev/), TypeScript
- **Editor** — [CodeMirror 6](https://codemirror.net/)
- **Styling** — [Tailwind CSS v4](https://tailwindcss.com/) + shadcn-svelte
- **Backend** — [Tauri 2](https://tauri.app/) (Rust)

Tauri means Grayslate uses your OS's built-in webview instead of bundling a whole browser, so the download is small and it's light on memory.

## Building from source

You'll need [Node.js](https://nodejs.org/) (v24+), [Rust](https://www.rust-lang.org/), and [pnpm](https://pnpm.io/).

```bash
git clone https://github.com/shriram-ethiraj/grayslate.git
cd grayslate
pnpm install
pnpm tauri dev      # run in development
pnpm tauri build    # produce an optimized build
```

## FAQ

**How is this different from Boop or Notepad++?**
Like Boop, it's a developer scratchpad for quick text jobs — but it also handles real file editing and large CSVs, and it runs on macOS, Windows, and Linux. Think of it as text transformations, a data viewer, and a notepad in one small window.

**Why Tauri and not Electron?**
Electron ships an entire Chromium and Node runtime with every app. Tauri reuses the system webview and pairs it with a Rust backend, so bundles are far smaller and memory use is lower.

**Why not just use an online formatter?**
Because your data leaves your machine when you do. Proprietary code, API keys, customer CSVs — none of it should have to travel to a stranger's server just to get pretty-printed. Grayslate does it all locally.

**Can it handle very large files?**
Grayslate can open files up to 200 MB, whether they are CSV or regular text files. The CSV table is virtualized, so files with hundreds of thousands of rows remain practical to browse.

**Is it free?**
Yes. Free and open source.

## Roadmap

- **Git sync** — automatically version and back up your notes to a Git repo.
- **Custom transformations** — write your own and add them to the menu.

## Contributing

Issues and pull requests are welcome. If you hit a bug or have an idea, open an issue.

## License

MIT.
