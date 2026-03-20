#!/usr/bin/env python3
"""
audit_repos.py — Naming Audit Tool for Grayslate

Reads a list of GitHub URLs and/or local paths from repos.txt, fetches each
repo's source files, runs them through the Grayslate naming system, and writes
one CSV per repo to the output/ directory.

Cloned repos are cached in the repos/ directory so repeat runs are fast.
Use --update to pull the latest commits for already-cached repos.

CSV columns:  file, language, suggested_name, is_fallback

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

Requirements:
    - Python 3.10+
    - git (for cloning remote repos)
    - The name_file Rust binary (built by this script via --build, or pre-built)
"""

import argparse
import csv
import os
import re
import subprocess
import sys
from pathlib import Path

# ── Paths ────────────────────────────────────────────────────────────────────

SCRIPT_DIR = Path(__file__).parent.resolve()
REPO_ROOT = SCRIPT_DIR.parent.parent  # tools/naming-audit → tools → repo root

# Cloned repos are cached here so repeat runs skip re-cloning.
REPOS_CACHE_DIR = SCRIPT_DIR / "repos"

# Search order: release → debug (Windows adds .exe)
_BINARY_STEMS = [
    REPO_ROOT / "src-tauri" / "target" / "release" / "name_file",
    REPO_ROOT / "src-tauri" / "target" / "debug" / "name_file",
]

# ── File filtering ────────────────────────────────────────────────────────────
# Binary and generated files to skip entirely.
# The name_file binary handles language detection — no EXT_TO_LANG map needed.

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
}

# Directory names to prune during traversal
SKIP_DIRS: set[str] = {
    "node_modules", "target", ".git", ".svelte-kit",
    "dist", "build", ".next", ".nuxt", "__pycache__", ".cache",
    "vendor", ".pnpm", "coverage", ".turbo", ".vercel",
    "out", ".idea", ".vscode", ".vs",
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
    """Compile the name_file Rust binary (debug profile for speed)."""
    print("Building name_file binary (cargo build --bin name_file)…")
    result = subprocess.run(
        [
            "cargo", "build",
            "--bin", "name_file",
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

def should_skip_path(rel_path: str) -> bool:
    """True if any path segment is in SKIP_DIRS or the extension is binary/generated."""
    parts = rel_path.replace("\\", "/").split("/")
    for part in parts[:-1]:  # directory components only
        if part in SKIP_DIRS or part.startswith("."):
            return True
    name = parts[-1].lower()
    if "." in name:
        ext = "." + name.rsplit(".", 1)[-1]
        if ext in SKIP_EXTENSIONS:
            return True
    return False


def walk_repo(repo_dir: Path):
    """
    Yield (rel_path, abs_path) for every text file worth auditing.
    Uses os.walk so it works on both cloned repos and the local working tree.
    """
    for root, dirs, files in os.walk(repo_dir):
        # Prune in-place so os.walk doesn't descend into skipped dirs
        dirs[:] = sorted(
            d for d in dirs
            if d not in SKIP_DIRS and not d.startswith(".")
        )
        for fname in sorted(files):
            abs_path = Path(root) / fname
            rel_path = abs_path.relative_to(repo_dir).as_posix()
            if should_skip_path(rel_path):
                continue
            if abs_path.stat().st_size > MAX_FILE_BYTES:
                continue
            yield rel_path, abs_path


# ── Naming via the Rust binary ────────────────────────────────────────────────

def suggest_name(binary: Path, content: str, rel_path: str) -> tuple[str, bool]:
    """
    Call the name_file binary with auto-detection.

    Passes "auto" as the language hint and the relative file path so the Rust
    detection pipeline can use the file extension as a Phase 1 hint.

    Returns (suggested_name, is_fallback).
    is_fallback is True when the binary prints nothing (naming failed).
    """
    try:
        result = subprocess.run(
            [str(binary), "auto", rel_path],
            input=content[:5000],   # mirror naming::bound() 5 000-byte cap
            capture_output=True,
            text=True,
            timeout=10,
            encoding="utf-8",
            errors="replace",
        )
        name = result.stdout.strip()
        return (name, name == "")
    except subprocess.TimeoutExpired:
        return ("", True)
    except Exception:
        return ("", True)


# ── Per-repo audit ────────────────────────────────────────────────────────────

def audit_repo(
    repo_dir: Path,
    repo_name: str,
    binary: Path,
    output_dir: Path,
) -> None:
    """Walk repo_dir, name every file, and write <repo_name>.csv to output_dir."""
    output_dir.mkdir(parents=True, exist_ok=True)
    csv_path = output_dir / f"{repo_name}.csv"

    rows: list[dict] = []
    total = fallbacks = 0

    for rel_path, abs_path in walk_repo(repo_dir):
        try:
            content = abs_path.read_text(encoding="utf-8", errors="ignore")
        except Exception:
            continue

        name, is_fb = suggest_name(binary, content, rel_path)
        total += 1
        if is_fb:
            fallbacks += 1

        rows.append({
            "file": rel_path,
            "suggested_name": name,
            "is_fallback": "yes" if is_fb else "no",
        })

    with open(csv_path, "w", newline="", encoding="utf-8") as fh:
        writer = csv.DictWriter(
            fh,
            fieldnames=["file", "suggested_name", "is_fallback"],
        )
        writer.writeheader()
        writer.writerows(rows)

    pct = round(100 * (total - fallbacks) / total) if total else 0
    print(
        f"  {repo_name}: {total} files audited, "
        f"{fallbacks} fallbacks ({pct}% named) → {csv_path.name}"
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

    print(f"Binary : {binary}")
    print(f"Cache  : {REPOS_CACHE_DIR}")
    print(f"Output : {output_dir}\n")

    # ── Parse repos list ──────────────────────────────────────────────────────
    if not repos_file.exists():
        print(f"repos.txt not found at: {repos_file}", file=sys.stderr)
        sys.exit(1)

    entries = parse_repos_file(repos_file)
    if not entries:
        print("repos.txt is empty (no non-comment lines found).", file=sys.stderr)
        sys.exit(1)

    print(f"Repos  : {len(entries)} entries in {repos_file.name}\n")

    # ── Process each entry ────────────────────────────────────────────────────
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
            audit_repo(dest, repo_name, binary, output_dir)
        else:
            local_path = Path(entry).expanduser().resolve()
            if not local_path.is_dir():
                print(f"  Local path not found: {local_path}", file=sys.stderr)
                continue
            audit_repo(local_path, repo_name, binary, output_dir)

    print(f"\nDone! CSVs are in: {output_dir}")


if __name__ == "__main__":
    main()

