# Code Formatters Implementation Plan

## Overview

Add code formatting support for JavaScript, TypeScript, CSS, HTML, YAML, JSON, and SQL to the Grayslate scratchpad. Uses Prettier (Web Worker) for web languages and `sqlformat` (Rust) for SQL to keep binary size minimal.

## Architecture

```
┌─────────────────┐     ┌──────────────────┐
│  Transformations│     │  Format Actions    │
│  Palette (UI)   │     │  (JS/TS/CSS/HTML/  │
│                 │     │   YAML/JSON fmt)   │
└────────┬────────┘     └────────┬──────────┘
         │                       │
         ▼                       ▼
  ┌──────────────┐      ┌───────────────┐
  │  Rust IPC     │      │  Web Worker   │
  │  (existing)   │      │  (Prettier)   │
  │               │      │  ~460 KB gz   │
  │  - SQL fmt    │      │               │
  │  - JSON minify│      │  - JS fmt     │
  │  - CSV/JSON   │      │  - TS fmt     │
  │  - text ops   │      │  - CSS fmt    │
  │  - etc.       │      │  - HTML fmt    │
  └──────────────┘      │  - YAML fmt    │
                        │  - JSON fmt    │
                        └───────────────┘
```

## Formatter Mapping

| Language | Engine | Where | Prettier Parser | Plugins Needed |
|---|---|---|---|---|
| JavaScript | Prettier | Web Worker | `babel` | `babel`, `estree` |
| TypeScript | Prettier | Web Worker | `typescript` | `babel`, `estree`, `typescript` |
| CSS | Prettier | Web Worker | `css` | `postcss` |
| HTML | Prettier | Web Worker | `html` | `html` |
| YAML | Prettier | Web Worker | `yaml` | `yaml` |
| JSON | Prettier | Web Worker | `json` | `babel`, `estree` |
| SQL | `sqlformat` | Rust IPC | — | — |

## Prettier Plugin Sizes (gzipped)

| Plugin | Size | Load Strategy |
|---|---|---|
| `prettier/standalone` (core) | ~35 KB | Eager |
| `prettier/plugins/babel` | ~80 KB | Eager |
| `prettier/plugins/estree` | ~45 KB | Eager |
| `prettier/plugins/typescript` | ~200 KB | Eager |
| `prettier/plugins/html` | ~35 KB | Eager |
| `prettier/plugins/postcss` | ~35 KB | Eager |
| `prettier/plugins/yaml` | ~28 KB | Eager |
| **Total** | **~460 KB gz** | |

## Bundle Size Impact

| Component | Delta | Notes |
|---|---|---|
| Rust binary (remove `dprint-plugin-jsonc`, add `sqlformat`) | **-0.5 MB** | Net reduction from dprint-core removal |
| Frontend JS (Prettier + plugins, gzipped) | **+460 KB** | Loaded once at worker init |
| **Net total** | **~0 MB** | Roughly size-neutral |

## Files Changed

| File | Action |
|---|---|
| `src/lib/formatter/worker.ts` | **NEW** — Prettier Web Worker |
| `src/lib/formatter/index.ts` | **NEW** — Worker manager + API |
| `src-tauri/Cargo.toml` | MODIFY — Remove `dprint-plugin-jsonc`, add `sqlformat` |
| `src-tauri/src/commands/transform.rs` | MODIFY — Remove `JsonFormat` (moves to Prettier), add `FormatSql` |
| `src/lib/transformations/actions.ts` | MODIFY — Add 6 format action definitions, new `FormatTransformationActionId` type |
| `src/lib/state/editor.svelte.ts` | MODIFY — Add `"sql"` to `FileType` union |
| `src/lib/editor/components/EditorWrapper.svelte` | MODIFY — Route format actions to Prettier worker vs Rust IPC |

## Step-by-Step Implementation

### 1. Install Prettier

```bash
pnpm add prettier
```

Plugins are subpath exports of the `prettier` package — no separate installs needed.

### 2. Create Web Worker (`src/lib/formatter/worker.ts`)

- Import `prettier/standalone` + all 6 plugins eagerly
- Accept messages: `{ text: string, parser: string, options?: PrettierOptions }`
- Return: `{ ok: true, formatted: string }` or `{ ok: false, error: string }`
- Language → parser mapping:
  - `javascript` → `babel`
  - `typescript` → `typescript`
  - `css` → `css`
  - `html` → `html`
  - `yaml` → `yaml`
  - `json` → `json`

```ts
// src/lib/formatter/worker.ts
import * as prettier from "prettier/standalone";
import * as babel from "prettier/plugins/babel";
import * as estree from "prettier/plugins/estree";
import * as typescript from "prettier/plugins/typescript";
import * as html from "prettier/plugins/html";
import * as postcss from "prettier/plugins/postcss";
import * as yaml from "prettier/plugins/yaml";

const pluginMap: Record<string, unknown[]> = {
  babel: [babel, estree],
  typescript: [babel, estree, typescript],
  json: [babel, estree],
  css: [postcss],
  html: [html],
  yaml: [yaml],
};

self.onmessage = async (e) => {
  const { text, parser, options = {} } = e.data;
  try {
    const formatted = await prettier.format(text, {
      parser,
      plugins: pluginMap[parser] ?? [],
      tabWidth: 2,
      useTabs: false,
      printWidth: 120,
      singleQuote: false,
      trailingComma: "all",
      semi: true,
      ...options,
    });
    self.postMessage({ ok: true, formatted });
  } catch (err) {
    self.postMessage({ ok: false, error: String(err) });
  }
};
```

### 3. Create Formatter Module (`src/lib/formatter/index.ts`)

- Manages Web Worker lifecycle (create on first call, reuse, clean up on teardown)
- Exposes `formatText(text: string, language: string, options?: object): Promise<string>`
- Throws on formatting error

```ts
// src/lib/formatter/index.ts
import type { FileType } from "$lib/state/editor.svelte";

type PrettierParser = "babel" | "typescript" | "css" | "html" | "yaml" | "json";

const LANGUAGE_TO_PARSER: Record<string, PrettierParser> = {
  javascript: "babel",
  typescript: "typescript",
  css: "css",
  html: "html",
  yaml: "yaml",
  json: "json",
};

let worker: Worker | null = null;

function getWorker(): Worker {
  if (!worker) {
    worker = new Worker(new URL("./worker.ts", import.meta.url), { type: "module" });
  }
  return worker;
}

export function formatText(
  text: string,
  language: string,
  options?: object,
): Promise<string> {
  const parser = LANGUAGE_TO_PARSER[language];
  if (!parser) throw new Error(`No formatter for: ${language}`);

  return new Promise((resolve, reject) => {
    const w = getWorker();
    w.onmessage = (e) => {
      if (e.data.ok) resolve(e.data.formatted);
      else reject(new Error(e.data.error));
    };
    w.postMessage({ text, parser, options });
  });
}

export function destroyFormatter() {
  worker?.terminate();
  worker = null;
}
```

### 4. Modify `src-tauri/Cargo.toml`

```diff
- dprint-plugin-jsonc = "0.7.4"
+ sqlformat = "0.5"
```

Keep `jsonc-parser` (still used by minify, validate, to-csv, to-yaml, keys-*).

### 5. Modify `src-tauri/src/commands/transform.rs`

**Remove:**
- `use dprint_plugin_jsonc::...;` imports (lines 2-4)
- `JSONC_FORMAT_CONFIG` static (lines 31-40)
- `TransformationActionId::JsonFormat` variant and its `#[serde(rename = "json.format")]` attribute
- `JsonFormat` match arm in `dispatch_transformation()` (lines 1715-1721)

**Add:**
- `use sqlformat;` import
- `FormatSql` variant: `#[serde(rename = "format.sql")]` and `FormatSql`
- SQL formatter in `dispatch_transformation()`:

```rust
TransformationActionId::FormatSql => {
    ctx.run_replace_text("Formatted SQL.", "SQL is already formatted.", |ctx| {
        Ok(sqlformat::format(
            ctx.text(),
            &sqlformat::QueryParams::None,
            &sqlformat::FormatOptions::default(),
        ))
    })
}
```

**Keep unchanged:** `jsonc-parser` imports and all other JSON operations (minify, validate, to-csv, to-yaml, keys-*).

### 6. Modify `src/lib/transformations/actions.ts`

**Add new type:**
```ts
export type FormatTransformationActionId =
    | "format.javascript"
    | "format.typescript"
    | "format.css"
    | "format.html"
    | "format.yaml"
    | "format.sql";
```

**Add to master union:**
```ts
export type TransformationActionId =
    | /* existing types */
    | FormatTransformationActionId;
```

**Add 6 action definitions:**
```ts
{
    id: "format.javascript",
    title: "Format JavaScript",
    description: "Pretty-print JavaScript with consistent indentation.",
    category: "Format",
    keywords: ["javascript", "js", "format", "pretty", "indent"],
    fileTypes: ["javascript"],
    supportsSelection: true,
},
{
    id: "format.typescript",
    title: "Format TypeScript",
    description: "Pretty-print TypeScript with consistent indentation.",
    category: "Format",
    keywords: ["typescript", "ts", "format", "pretty", "indent"],
    fileTypes: ["typescript"],
    supportsSelection: true,
},
{
    id: "format.css",
    title: "Format CSS",
    description: "Pretty-print CSS with consistent indentation.",
    category: "Format",
    keywords: ["css", "style", "format", "pretty", "indent"],
    fileTypes: ["css"],
    supportsSelection: true,
},
{
    id: "format.html",
    title: "Format HTML",
    description: "Pretty-print HTML with consistent indentation.",
    category: "Format",
    keywords: ["html", "markup", "format", "pretty", "indent"],
    fileTypes: ["html"],
    supportsSelection: true,
},
{
    id: "format.yaml",
    title: "Format YAML",
    description: "Pretty-print YAML with consistent indentation.",
    category: "Format",
    keywords: ["yaml", "yml", "format", "pretty", "indent"],
    fileTypes: ["yaml"],
    supportsSelection: true,
},
{
    id: "format.sql",
    title: "Format SQL",
    description: "Pretty-print SQL with consistent indentation.",
    category: "Format",
    keywords: ["sql", "format", "pretty", "indent"],
    fileTypes: ["sql"],
    supportsSelection: true,
},
```

**Update `json.format`** — keep the definition, but set its `category` to `"Format"` and note it routes to Prettier (not Rust).

### 7. Modify `src/lib/state/editor.svelte.ts`

Add `"sql"` to the `FileType` union:

```diff
  export type FileType =
      | "text"
      | "csv"
      | "markdown"
      | "json"
      | "javascript"
      | "typescript"
      | "python"
      | "html"
      | "css"
      | "yaml"
+     | "sql"
      | "c"
      | /* ... */
```

### 8. Modify `src/lib/editor/components/EditorWrapper.svelte`

**Import the formatter:**
```ts
import { formatText } from "$lib/formatter/index";
```

**Route format actions in the dispatch:**
- Prettier actions: `json.format`, `format.javascript`, `format.typescript`, `format.css`, `format.html`, `format.yaml`
  → Call `formatText(text, language, options)`, apply result via `dispatchManagedEditorChange`
- SQL action: `format.sql`
  → Route to Rust IPC via `invoke("execute_transformation", ...)`
- All other actions
  → Existing Rust IPC pipeline (unchanged)

**Result application:**
```ts
const SET = new Set([
  "json.format",
  "format.javascript",
  "format.typescript",
  "format.css",
  "format.html",
  "format.yaml",
]);

if (SET.has(actionId)) {
  // Prettier path
  const formatted = await formatText(sourceText, language, formatOptions);
  dispatchManagedEditorChange({
    changes: { from: selection ? from : 0, to: selection ? to : docLength, insert: formatted },
    // ...
  });
} else {
  // Existing Rust IPC path for everything else including format.sql
  await invoke("execute_transformation", { request, onEvent });
}
```

### 9. Cleanup

In the EditorWrapper teardown, call `destroyFormatter()` to terminate the Web Worker.

```ts
import { destroyFormatter } from "$lib/formatter/index";

// In onDestroy:
destroyFormatter();
```

## Caveats

### JSONC Support Lost for `json.format`

Prettier's `json` parser does **not** support JSON with comments (JSONC) or trailing commas. The existing `json.format` action used `dprint-plugin-jsonc` which preserved both.

**What still works with JSONC:**
- `json.validate` — uses `jsonc-parser` in Rust
- `json.minify` — uses `jsonc-parser` in Rust
- `json.to-csv` — uses `jsonc-parser` in Rust
- `json.to-yaml` — uses `jsonc-parser` in Rust
- `json.keys-*` — uses `jsonc-parser` in Rust

**What breaks:** Only `json.format` — it now requires valid standard JSON (no comments, no trailing commas).

**Fallback if needed:** Add `prettier-plugin-jsonc` later, or keep `dprint-plugin-json` as a Rust-side fallback for JSONC formatting.

### Formatting Options

Prettier options (`tabWidth`, `useTabs`, `printWidth`, etc.) are passable via the worker message `options` field. Defaults are in the worker code. When a user-facing settings UI is built, connect it to pass custom options through the `formatText()` call.

### `jsonc-parser` dependency

Not removed — still used by `json.validate`, `json.minify`, `json.to-csv`, `json.to-yaml`, `json.keys-*`. Only `dprint-plugin-jsonc` is removed (used exclusively by `json.format`).
