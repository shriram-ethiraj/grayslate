#!/usr/bin/env python3
"""
audit_repos.py — Naming & Detection Audit Tool for Grayslate

Reads a list of GitHub URLs and/or local paths from repos.txt, fetches each
repo's source files, runs them through the Grayslate content-only detection
and naming pipelines, and writes one CSV per repo to the output/ directory.

Cloned repos are cached in the repos/ directory so repeat runs are fast.
Use --update to pull the latest commits for already-cached repos.

Use case: evaluating how well the system handles raw content with no filename
context — the primary path for paste and untitled documents in Grayslate.
The actual file extension is recorded as ground truth for accuracy measurement.

CSV columns:
  file                  Relative path within the repo (ground truth reference)
  actual_ext            Ground-truth file extension derived from the filename
  content_detected_lang Language detected from content alone (no filename hint)
  content_suggested_ext Extension the system maps to content_detected_lang
  content_ext_match     "yes" if content_suggested_ext == actual_ext  ← primary metric
  suggested_name        Stem suggested by the naming pipeline, or "" on fallback
  is_name_fallback      "yes" if naming fell back (no useful name found)

Key metrics for fine-tuning:
  - Filter content_ext_match=no  → detection failures to fix
  - Filter is_name_fallback=yes  → naming gaps
  - Group by content_detected_lang → per-language naming fallback rates

Usage:
    # Build the Rust binary first, then run the audit:
    python audit_repos.py --build

    # If the binary is already built:
    python audit_repos.py

    # Pull latest for already-cloned repos:
    python audit_repos.py --update

    # Custom output directory:
    python audit_repos.py --output-dir /tmp/audit-out

    # Custom repos file:
    python audit_repos.py --repos repos.txt

    # Limit parallelism to 4 workers (default: all logical CPU cores):
    python audit_repos.py --workers 4

Requirements:
    - Python 3.10+
    - git (for cloning remote repos)
    - The name_file Rust binary (built by this script via --build, or pre-built)
    - pathspec (pip install pathspec)  — for .gitignore-aware file filtering
"""

import argparse
import concurrent.futures
import csv
import json
import os
import re
import subprocess
import sys
from pathlib import Path

try:
    import pathspec
except ImportError:
    print(
        "Error: 'pathspec' is required.\n"
        "Install it with:  pip install pathspec",
        file=sys.stderr,
    )
    sys.exit(1)

# ── Paths ────────────────────────────────────────────────────────────────────

SCRIPT_DIR = Path(__file__).parent.resolve()
REPO_ROOT = SCRIPT_DIR.parent.parent  # tools/naming-audit → tools → repo root

# Cloned repos are cached here so repeat runs skip re-cloning.
REPOS_CACHE_DIR = SCRIPT_DIR / "repos"

# Search order: workspace root target → src-tauri target (release → debug).
# Since the crates were extracted to workspace root, cargo builds to target/.
_BINARY_STEMS = [
    REPO_ROOT / "target" / "release" / "name_file",
    REPO_ROOT / "target" / "debug" / "name_file",
    REPO_ROOT / "src-tauri" / "target" / "release" / "name_file",
    REPO_ROOT / "src-tauri" / "target" / "debug" / "name_file",
]

# ── File filtering ────────────────────────────────────────────────────────────
# SKIP_EXTENSIONS covers binary/non-text formats that are never in .gitignore
# (and that the name_file binary can't meaningfully detect anyway).
# Directory filtering is done entirely via .gitignore — see walk_repo().

SKIP_EXTENSIONS: set[str] = {
    ".png", ".jpg", ".jpeg", ".gif", ".ico", ".webp", ".bmp", ".tiff",
    ".svg",
    ".woff", ".woff2", ".ttf", ".eot", ".otf",
    ".mp4", ".mp3", ".wav", ".ogg", ".flac",
    ".zip", ".tar", ".gz", ".bz2", ".7z", ".rar", ".xz",
    ".pdf", ".doc", ".docx", ".xls", ".xlsx", ".ppt",
    ".exe", ".dll", ".so", ".dylib", ".a", ".lib", ".o",
    ".lock",          # lockfiles (pnpm-lock.yaml, Cargo.lock) are huge + not useful
    ".map",           # source maps
    ".pyc", ".pyo",
    ".class",
    ".wasm",
    ".patch", ".diff",
    ".npmrc",         # INI-like config; trivial to detect as text, adds noise
}

MAX_FILE_BYTES = 500_000  # match Grayslate's 500 KB read limit


# ── Binary helpers ────────────────────────────────────────────────────────────

def find_binary() -> Path | None:
    """Return the first name_file binary that exists (release before debug)."""
    for stem in _BINARY_STEMS:
        for candidate in [stem.with_suffix(".exe"), stem]:
            if candidate.is_file():
                return candidate
    return None


def build_binary(manifest_path: Path) -> None:
    """Compile the name_file Rust binary (release profile to match find_binary priority)."""
    print(
        "Building name_file binary "
        "(cargo build --release --bin name_file --features naming-audit-cli)…"
    )
    result = subprocess.run(
        [
            "cargo", "build",
            "--release",
            "--bin", "name_file",
            "--features", "naming-audit-cli",
            "--manifest-path", str(manifest_path),
        ],
        capture_output=True,
        text=True,
    )
    if result.returncode != 0:
        print("Build FAILED:\n", result.stderr, file=sys.stderr)
        sys.exit(1)
    print("Build complete.\n")


# ── File helpers ──────────────────────────────────────────────────────────────

def _is_gitignored(
    abs_path: Path,
    specs: list[tuple[Path, "pathspec.PathSpec"]],
) -> bool:
    """Return True if abs_path is matched by any applicable .gitignore spec."""
    for base_dir, spec in specs:
        try:
            rel = abs_path.relative_to(base_dir).as_posix()
        except ValueError:
            continue
        if spec.match_file(rel):
            return True
    return False


def _collect_gitignore_specs(
    repo_dir: Path,
) -> list[tuple[Path, "pathspec.PathSpec"]]:
    """
    Walk repo_dir top-down and load every .gitignore file found.

    Each directory's .gitignore is loaded *before* its subdirectories are
    pruned, so heavily-ignored trees (node_modules/, target/, etc.) are
    skipped immediately on the first encounter — no descending into them.

    Returns a list of (base_dir, spec) pairs where each spec's patterns are
    relative to base_dir, matching git's per-directory semantics.
    """
    specs: list[tuple[Path, pathspec.PathSpec]] = []
    for root, dirs, files in os.walk(repo_dir):
        root_path = Path(root)
        # Load the .gitignore in this directory FIRST so its patterns
        # take effect when we prune subdirectories below.
        if ".gitignore" in files:
            gi_path = root_path / ".gitignore"
            try:
                lines = gi_path.read_text(encoding="utf-8", errors="ignore").splitlines()
                spec = pathspec.PathSpec.from_lines("gitwildmatch", lines)
                specs.append((root_path, spec))
            except OSError:
                pass
        # .git is never listed in .gitignore (git treats it specially), so
        # exclude it explicitly. Everything else is covered by the specs.
        dirs[:] = sorted(
            d for d in dirs
            if d != ".git" and not _is_gitignored(root_path / d, specs)
        )
    return specs


def walk_repo(repo_dir: Path):
    """
    Yield (rel_path, abs_path) for every text file worth auditing.

    Directories and files are filtered using all .gitignore files found under
    repo_dir (including nested ones), matching git's per-directory semantics.
    Binary/non-text extensions listed in SKIP_EXTENSIONS are also excluded.
    """
    gitignore_specs = _collect_gitignore_specs(repo_dir)

    for root, dirs, files in os.walk(repo_dir):
        root_path = Path(root)
        dirs[:] = sorted(
            d for d in dirs
            if d != ".git" and not _is_gitignored(root_path / d, gitignore_specs)
        )
        for fname in sorted(files):
            abs_path = root_path / fname
            rel_path = abs_path.relative_to(repo_dir).as_posix()
            name = fname.lower()
            if "." in name:
                ext = "." + name.rsplit(".", 1)[-1]
                if ext in SKIP_EXTENSIONS:
                    continue
            if _is_gitignored(abs_path, gitignore_specs):
                continue
            try:
                st = abs_path.stat()
                if st.st_size == 0 or st.st_size > MAX_FILE_BYTES:
                    continue
            except OSError:
                continue  # path too long or inaccessible on Windows
            yield rel_path, abs_path


# ── Naming + detection via the Rust binary ───────────────────────────────────

# Fallback result when the binary fails or produces no output.
_EMPTY_RESULT = {
    "content_detected_lang": "",
    "content_suggested_ext": "",
    "suggested_name": "",
}


def query_binary(binary: Path, content: str) -> dict:
    """
    Call the name_file binary with content from stdin only — no filename hint.

    The binary performs content-only detection and naming, mirroring the
    paste/untitled document flow in Grayslate.

    Returns a dict with keys:
      content_detected_lang, content_suggested_ext, suggested_name
    On any failure, returns _EMPTY_RESULT (all empty strings).
    """
    try:
        result = subprocess.run(
            [str(binary)],
            input=content[:50000],  # mirror detection::MAX_DETECTION_BYTES (50 KB)
            capture_output=True,
            text=True,
            timeout=10,
            encoding="utf-8",
            errors="replace",
        )
        raw = result.stdout.strip()
        if not raw:
            return dict(_EMPTY_RESULT)
        return json.loads(raw)
    except (subprocess.TimeoutExpired, json.JSONDecodeError, Exception):
        return dict(_EMPTY_RESULT)


def actual_ext(rel_path: str) -> str:
    """
    Return the lowercased file extension from a relative path (e.g. "py"),
    or "" for files with no extension (e.g. "Dockerfile", "Makefile").
    """
    name = Path(rel_path).name.lower()
    if "." in name:
        return name.rsplit(".", 1)[-1]
    return ""


# ── Per-repo audit ────────────────────────────────────────────────────────────

# CSV column order — update README.md if this changes.
_FIELDNAMES = [
    "file",
    "actual_ext",
    "content_detected_lang",
    "content_suggested_ext",
    "content_ext_match",
    "suggested_name",
    "is_name_fallback",
]


def _process_file(args: tuple) -> dict | None:
    """
    Worker unit: read one file and query the binary.

    Accepts a (rel_path, abs_path, binary) tuple so it can be dispatched via
    executor.map() without needing a closure.
    Returns a result dict on success, or None if the file cannot be read.
    """
    rel_path, abs_path, binary = args
    try:
        content = abs_path.read_text(encoding="utf-8", errors="ignore")
    except Exception:
        return None

    data = query_binary(binary, content)
    ext = actual_ext(rel_path)
    is_name_fb = data["suggested_name"] == ""

    # Extension alias groups — all extensions in a group are considered equivalent
    # for detection accuracy purposes (e.g. .tsx IS TypeScript, .yml IS YAML).
    _TS_GROUP = {"ts", "tsx", "mts", "cts"}
    _JS_GROUP = {"js", "jsx", "mjs", "cjs"}
    _YAML_GROUP = {"yaml", "yml", "cff"}
    _MD_GROUP = {"md", "mdx", "markdown"}
    _CPP_GROUP = {"cpp", "cc", "cxx", "c++", "h", "hpp", "hh", "hxx"}
    _C_GROUP = {"c", "h"}
    _RB_GROUP = {"rb", "gemspec", "rake", "jbuilder"}
    _KT_GROUP = {"kt", "kts"}
    _XML_GROUP = {"xml", "plist", "csproj", "nuspec", "xcworkspacedata",
                  "storyboard", "xcscheme", "entitlements", "xcsettings"}
    _CSV_GROUP = {"csv", "tsv"}
    _SCSS_GROUP = {"scss", "sass"}
    # JSON-family: .arb and .prettierrc are JSON-formatted config files
    _JSON_GROUP = {"json", "jsonc", "json5", "geojson", "arb", "prettierrc"}
    # Text-family: ignore-files and plain-text configs that correctly detect as text
    _TEXT_GROUP = {"txt", "ini", "cfg", "eslintignore", "gitignore",
                   "gitattributes", "editorconfig"}

    _EXT_ALIASES = {}
    for _group in [_TS_GROUP, _JS_GROUP, _YAML_GROUP, _MD_GROUP,
                   _CPP_GROUP, _RB_GROUP, _KT_GROUP, _XML_GROUP,
                   _CSV_GROUP, _SCSS_GROUP, _JSON_GROUP, _TEXT_GROUP]:
        for _ext in _group:
            _EXT_ALIASES[_ext] = _group
    # C/C++ header overlap: `h` maps to both C and C++ — use the wider group
    for _ext in _C_GROUP:
        _EXT_ALIASES[_ext] = _CPP_GROUP  # h→{cpp,cc,cxx,c++,h,hpp,hh,hxx}

    # Known no-extension filenames and their expected content_suggested_ext value.
    # Used to evaluate detection accuracy for files like Dockerfile, Makefile, etc.
    _NO_EXT_EXPECTED = {
        "dockerfile": "dockerfile",
        "makefile": "sh",
        "gnumakefile": "sh",
        "rakefile": "rb",
        "gemfile": "rb",
        "jenkinsfile": "txt",
        "vagrantfile": "txt",
    }

    suggested = data["content_suggested_ext"]
    # Check if suggested extension matches actual extension (including aliases)
    if ext and suggested:
        alias_group = _EXT_ALIASES.get(suggested, {suggested})
        content_ext_ok = "yes" if ext in alias_group else "no"
    elif not ext and suggested:
        # No-extension file (e.g. Dockerfile, Makefile): compare against known map
        filename_base = Path(rel_path).name.lower()
        expected_ext = _NO_EXT_EXPECTED.get(filename_base)
        content_ext_ok = "yes" if expected_ext and suggested == expected_ext else "no"
    else:
        content_ext_ok = "no"

    return {
        "file": rel_path,
        "actual_ext": ext,
        "content_detected_lang": data["content_detected_lang"],
        "content_suggested_ext": data["content_suggested_ext"],
        "content_ext_match": content_ext_ok,
        "suggested_name": data["suggested_name"],
        "is_name_fallback": "yes" if is_name_fb else "no",
        # internal flags used only for stats accumulation
        "_is_name_fb": is_name_fb,
        "_content_ext_ok": content_ext_ok,
        "_ext": ext,
    }


def audit_repo(
    repo_dir: Path,
    repo_name: str,
    binary: Path,
    output_dir: Path,
    workers: int = 1,
) -> None:
    """Walk repo_dir, analyse every file in parallel, and write <repo_name>.csv to output_dir."""
    output_dir.mkdir(parents=True, exist_ok=True)
    csv_path = output_dir / f"{repo_name}.csv"

    # Collect all files upfront so executor.map() can distribute them evenly.
    file_list = list(walk_repo(repo_dir))
    work_items = [(rel_path, abs_path, binary) for rel_path, abs_path in file_list]

    rows: list[dict] = []
    total = name_fallbacks = detect_mismatches = 0

    # ThreadPoolExecutor is appropriate here: each task is subprocess/I/O-bound,
    # so the GIL is released during the subprocess call and threads scale well.
    with concurrent.futures.ThreadPoolExecutor(max_workers=workers) as executor:
        for result in executor.map(_process_file, work_items):
            if result is None:
                continue
            total += 1
            if result["_is_name_fb"]:
                name_fallbacks += 1
            if result["_content_ext_ok"] == "no" and result["_ext"] != "":
                detect_mismatches += 1
            # Strip internal stat keys before appending to output rows.
            rows.append({k: v for k, v in result.items() if not k.startswith("_")})

    with open(csv_path, "w", newline="", encoding="utf-8") as fh:
        writer = csv.DictWriter(fh, fieldnames=_FIELDNAMES)
        writer.writeheader()
        writer.writerows(rows)

    named_pct = round(100 * (total - name_fallbacks) / total) if total else 0
    detected_pct = round(100 * (total - detect_mismatches) / total) if total else 0
    print(
        f"  {repo_name}: {total} files  |  "
        f"content-detection: {detected_pct}% ({detect_mismatches} mismatches)  |  "
        f"naming: {named_pct}% named ({name_fallbacks} fallbacks)  "
        f"→ {csv_path.name}"
    )


# ── Repo entry parsing ────────────────────────────────────────────────────────

def parse_repos_file(path: Path) -> list[str]:
    entries: list[str] = []
    for line in path.read_text(encoding="utf-8").splitlines():
        line = line.strip()
        if line and not line.startswith("#"):
            entries.append(line)
    return entries


def repo_name_from_entry(entry: str) -> str:
    """Derive a filesystem-safe name from a GitHub URL or local path."""
    m = re.search(r"github\.com/[^/]+/([^/\s#?]+)", entry)
    if m:
        name = m.group(1)
        return re.sub(r"\.git$", "", name, flags=re.IGNORECASE)
    resolved = Path(entry).expanduser().resolve()
    return resolved.name or "repo"


# ── Clone / update helpers ────────────────────────────────────────────────────

def clone_or_update(clone_url: str, dest: Path, update: bool) -> bool:
    """
    Clone a repo into dest/ if it doesn't exist, or pull if update=True.
    Returns True on success.
    """
    if dest.is_dir():
        if update:
            print(f"  Updating cached repo at {dest.name} …")
            result = subprocess.run(
                ["git", "-C", str(dest), "pull", "--quiet", "--ff-only"],
                capture_output=True,
                text=True,
            )
            if result.returncode != 0:
                print(f"  Pull failed (using cached): {result.stderr.strip()}", file=sys.stderr)
        else:
            print(f"  Using cached clone at repos/{dest.name}")
        return True

    print(f"  Cloning {clone_url} → repos/{dest.name} …")
    result = subprocess.run(
        ["git", "clone", "--depth=1", "--quiet", clone_url, str(dest)],
        capture_output=True,
        text=True,
    )
    if result.returncode != 0:
        if dest.is_dir():
            # Partial checkout — some paths were too long for the OS (e.g. Windows
            # MAX_PATH).  Audit whatever files were successfully checked out.
            print(
                "  Checkout partially failed (long paths?). "
                "Auditing available files.",
                file=sys.stderr,
            )
            return True
        err = result.stderr.strip() or result.stdout.strip()
        print(f"  Clone failed: {err}", file=sys.stderr)
        return False
    return True


# ── Main ──────────────────────────────────────────────────────────────────────

def main() -> None:
    parser = argparse.ArgumentParser(
        description="Audit file naming across one or more repos.",
        formatter_class=argparse.RawDescriptionHelpFormatter,
        epilog=__doc__,
    )
    parser.add_argument(
        "--build",
        action="store_true",
        help="Build (or rebuild) the name_file Rust binary before running.",
    )
    parser.add_argument(
        "--workers",
        type=int,
        default=os.cpu_count() or 1,
        metavar="N",
        help=(
            "Number of parallel workers for processing files within each repo "
            "(default: number of logical CPU cores)."
        ),
    )
    parser.add_argument(
        "--update",
        action="store_true",
        help="Pull latest commits for already-cached repos before auditing.",
    )
    parser.add_argument(
        "--repos",
        default=str(SCRIPT_DIR / "repos.txt"),
        help="Path to repos.txt (default: repos.txt next to this script).",
    )
    parser.add_argument(
        "--output-dir",
        default=str(SCRIPT_DIR / "output"),
        help="Directory to write CSV files into (default: ./output/).",
    )
    args = parser.parse_args()

    repos_file = Path(args.repos)
    output_dir = Path(args.output_dir)
    manifest_path = REPO_ROOT / "src-tauri" / "Cargo.toml"

    REPOS_CACHE_DIR.mkdir(parents=True, exist_ok=True)

    # ── Build binary if needed ────────────────────────────────────────────────
    binary = find_binary()
    if args.build or binary is None:
        build_binary(manifest_path)
        binary = find_binary()

    if binary is None:
        print(
            "Error: name_file binary not found.\n"
            "Run with --build to compile it first.",
            file=sys.stderr,
        )
        sys.exit(1)

    print(f"Binary  : {binary}")
    print(f"Cache   : {REPOS_CACHE_DIR}")
    print(f"Output  : {output_dir}")
    print(f"Workers : {args.workers}\n")

    # ── Parse repos list ──────────────────────────────────────────────────────
    if not repos_file.exists():
        print(f"repos.txt not found at: {repos_file}", file=sys.stderr)
        sys.exit(1)

    entries = parse_repos_file(repos_file)
    if not entries:
        print("repos.txt is empty (no non-comment lines found).", file=sys.stderr)
        sys.exit(1)

    print(f"Repos  : {len(entries)} entries in {repos_file.name}\n")

    # ── Process each entry ────────────────────────────────────────────────
    for entry in entries:
        repo_name = repo_name_from_entry(entry)
        is_github = "github.com" in entry

        print(f">> {repo_name}")

        if is_github:
            clone_url = entry.rstrip("/")
            if not clone_url.endswith(".git"):
                clone_url += ".git"

            dest = REPOS_CACHE_DIR / repo_name
            if not clone_or_update(clone_url, dest, args.update):
                continue
            audit_repo(dest, repo_name, binary, output_dir, args.workers)
        else:
            local_path = Path(entry).expanduser().resolve()
            if not local_path.is_dir():
                print(f"  Local path not found: {local_path}", file=sys.stderr)
                continue
            audit_repo(local_path, repo_name, binary, output_dir, args.workers)

    print(f"\nDone! CSVs are in: {output_dir}")


if __name__ == "__main__":
    main()
