# Naming & Detection Audit Tool

Runs every source file in a set of repos through the Grayslate **content-only** detection and naming pipelines, and records the results in a CSV. Use it to measure how well both systems perform on raw content — the primary path for paste and untitled documents.

## Use Case

Grayslate is a scratchpad. Users paste content without a filename. This tool answers: *"Given only the file content, does the system correctly identify the language and produce a useful name?"*

The actual file extension is recorded as ground truth — we never feed it to the pipeline.

## How it works

1. `audit_repos.py` reads `repos.txt` for a list of GitHub URLs and/or local paths.
2. Remote repos are cloned with `--depth=1` and cached in `repos/` so re-runs are instant.
3. Each text file's content is piped through the `name_file` Rust binary **without any filename hint**. Files are processed in parallel across all CPU cores (override with `--workers N`).
4. Results are written as one CSV per repo to `output/`.

The `name_file` binary is a thin CLI wrapper around `grayslate_lib::detection` and `grayslate_lib::naming`. It is the same detection and naming logic the app uses — no approximations.

## Requirements

- **Python 3.10+**
- **Rust toolchain** (`cargo`) — only needed to build the binary
- **git** — only needed for cloning remote repos

## Setup

### 1. Build the Rust binary

The easiest way — let the audit script handle it:

```sh
python audit_repos.py --build
```

Or build manually from the `tools/naming-audit/` directory:

```sh
cargo build --bin name_file --features naming-audit-cli --manifest-path ../../src-tauri/Cargo.toml
```

### 2. Configure `repos.txt`

Add one entry per line — either a GitHub URL or a local path:

```
# GitHub repo
https://github.com/sveltejs/kit

# Local path (relative from tools/naming-audit/, or absolute)
../../
/Users/me/projects/some-repo
```

Lines starting with `#` and blank lines are ignored. The current `repos.txt` already has a set of public repos for benchmarking.

## Running

```sh
cd tools/naming-audit

# Build the binary and run the full audit
python audit_repos.py --build

# Run again (binary already built, repos already cached)
python audit_repos.py

# Pull latest commits for all cached repos, then re-audit
python audit_repos.py --update

# Use a different repos file
python audit_repos.py --repos my-repos.txt

# Write CSVs to a custom directory
python audit_repos.py --output-dir /tmp/audit-out

# Limit parallelism (default: all logical CPU cores)
python audit_repos.py --workers 4
```

## Output

Each repo produces a CSV in `output/` with the following columns:

| Column | Description |
|---|---|
| `file` | Relative path of the source file (ground truth reference only) |
| `actual_ext` | File extension derived from the filename (e.g. `py`, `ts`) |
| `content_detected_lang` | Language detected from **content alone** — no filename hint |
| `content_suggested_ext` | Extension the system maps to `content_detected_lang` |
| `content_ext_match` | `yes` if `content_suggested_ext == actual_ext` — **primary detection metric** |
| `suggested_name` | Stem suggested by the naming pipeline, or `""` on fallback |
| `is_name_fallback` | `yes` if naming produced no useful name |

### How to use the CSV for fine-tuning

**Detection failures** — rows where the system misidentified the language:
```
content_ext_match = no   AND   actual_ext != ""
```

**Naming gaps** — rows where naming fell back:
```
is_name_fallback = yes
```

**Per-language naming quality** — group by `content_detected_lang`, count `is_name_fallback = yes`.

**Console summary** (printed per repo):
```
  fastapi: 500 files  |  content-detection: 94% (30 mismatches)  |  naming: 88% named (60 fallbacks)  → fastapi.csv
```

## Directory layout

```
tools/naming-audit/
├── audit_repos.py          # Main audit script — produces per-repo CSVs
├── metrics.py              # Aggregate per-language accuracy summary
├── filter_failures.py      # Extract failure rows into analysis subdirs
├── compare_runs.py         # Compare two audit runs (before vs after)
├── repos.txt               # List of repos to audit (edit this)
├── repos/                  # Cached git clones (gitignored, auto-created)
│   └── <repo-name>/
├── output/                 # Raw per-repo CSVs (gitignored)
│   └── <repo-name>.csv
├── output-with-treesitter/ # Backed-up baseline from tree-sitter era (gitignored)
│   └── <repo-name>.csv
└── analysis/               # Generated analysis outputs (gitignored)
    ├── metrics/
    │   └── metrics.csv             # Per-language accuracy summary
    ├── comparison.csv              # Before vs after per-language delta
    ├── diff-files.csv              # Files where detection/naming changed
    ├── content_match_negatives/    # Rows where content_ext_match = no
    │   └── <repo-name>.csv
    └── name_fallback_positives/    # Rows where is_name_fallback = yes
        └── <repo-name>.csv
```

Cloned repos in `repos/` are gitignored. Delete a subdirectory to force a fresh clone on the next run.

## Analysis workflow

After running `audit_repos.py`, generate all analysis outputs:

```sh
# 1. Aggregate per-language accuracy metrics
python metrics.py
# → analysis/metrics/metrics.csv

# 2. Extract failure records for targeted investigation
python filter_failures.py
# → analysis/content_match_negatives/<repo>.csv  (detection mismatches)
# → analysis/name_fallback_positives/<repo>.csv  (naming fallbacks)

# 3. Compare against a previous baseline (e.g. output-with-treesitter)
python compare_runs.py --before output-with-treesitter --after output
# → analysis/comparison.csv  (per-language detection & naming deltas)
# → analysis/diff-files.csv  (individual files that changed)
```

The `content_match_negatives/` files are the primary input for improving detection accuracy — each row shows exactly what the system guessed vs. what the actual extension was.

## How detection and naming work

The `name_file` binary receives content on stdin with **no filename argument**. It runs two steps:

1. **Content-only detection** — the full pipeline without any extension hint:
   - Phase 0: Deterministic anchors (extension, shebang, strong structural)
   - Phase 1: Content family classification
   - Phase 2: Family-gated candidate scoring
   - Phase 3: Neighbor disambiguation
   - Phase 4: Confidence gate

2. **Naming** — runs the language-appropriate extractor (code symbols, headings, keys, etc.) on the detected language.

This is the exact code path triggered when a user pastes content into an untitled Grayslate slate.
