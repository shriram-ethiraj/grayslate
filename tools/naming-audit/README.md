# Naming Audit Tool

Runs every source file in a set of repos through the Grayslate naming pipeline and records the results in a CSV. Use it to measure how well the naming system performs across real-world codebases.

## How it works

1. `audit_repos.py` reads `repos.txt` for a list of GitHub URLs and/or local paths.
2. Remote repos are cloned with `--depth=1` and cached in `repos/` so re-runs are instant.
3. Each text file is piped through the `name_file` Rust binary, which auto-detects the language from the file extension and content, then runs the Grayslate naming pipeline.
4. Results are written as one CSV per repo to `output/`.

The `name_file` binary is a thin CLI wrapper around `grayslate_lib::naming`. It is the same naming logic the app uses — no approximations.

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
cargo build --bin name_file --manifest-path ../../src-tauri/Cargo.toml
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
```

## Output

Each repo produces a CSV in `output/` with three columns:

| Column | Description |
|---|---|
| `file` | Relative path of the source file within the repo |
| `suggested_name` | Stem suggested by the Grayslate naming pipeline, or empty if it fell back |
| `is_fallback` | `yes` if the pipeline produced no useful name, `no` otherwise |

The fallback rate (`is_fallback = yes`) is the primary metric. Lower is better.

## Directory layout

```
tools/naming-audit/
├── audit_repos.py      # Main audit script
├── repos.txt           # List of repos to audit (edit this)
├── repos/              # Cached git clones (gitignored, auto-created)
│   └── <repo-name>/    # One subdirectory per cloned repo
├── output/             # Generated CSVs (gitignored)
│   └── <repo-name>.csv
└── README.md
```

Cloned repos in `repos/` are gitignored. Delete a subdirectory to force a fresh clone on the next run.

## How language detection works

The audit tool passes `"auto"` as the language hint to the `name_file` binary along with the relative file path. The Rust detection pipeline resolves the language in four phases:

1. **Extension / filename** — fast, deterministic map (`.rs` → rust, `Dockerfile` → dockerfile, etc.)
2. **Shebang** — parses `#!/usr/bin/env python3` style lines
3. **Structural** — recognises JSON, YAML, TOML, HTML, CSV, Markdown, and similar formats from content shape
4. **Heuristic + tree-sitter** — weighted regex scoring across 20 language signatures; tree-sitter validates ambiguous results

No language map is maintained in Python — detection is entirely handled by Rust.
