#!/usr/bin/env python3
"""
metrics.py — Per-language performance summary for Grayslate naming & detection.

Reads all CSV files produced by audit_repos.py and produces a single
metrics CSV with one row per detected language, showing:

  - total files detected as that language
  - content-detection accuracy  (content_ext_match=yes %)
  - naming success rate          (is_name_fallback=no %)

Content-detection note:
  "accuracy" here means: when the system says a file is language X, how often
  does the actual file extension agree?  Files with no extension (Dockerfile,
  Makefile, etc.) can never match, so they always count as detection mismatches.
  The column "total_with_ext" shows how many files in each group *had* an
  extension — a low ratio of total_with_ext / total suggests the language
  mostly appears in extension-less files rather than being misdetected.

Usage:
    python metrics.py                        # reads ./output/*.csv, writes ./analysis/metrics/metrics.csv
    python metrics.py --input-dir /tmp/out   # custom input directory
    python metrics.py --output metrics.csv   # custom output path

Output CSV columns:
    language              — content_detected_lang value (ground truth: what the
                            system called the file)
    total_files           — total files detected as this language
    total_with_ext        — subset that had an actual file extension
    detection_correct     — files where content_ext_match=yes
    detection_accuracy_pct — detection_correct / total_with_ext * 100
                             (n/a if total_with_ext=0)
    named                 — files where is_name_fallback=no
    name_fallback         — files where is_name_fallback=yes
    naming_success_pct    — named / total_files * 100
    repos                 — comma-separated list of repos that contributed data
"""

import argparse
import csv
import glob
import os
from collections import defaultdict
from pathlib import Path


# ── CSV columns ──────────────────────────────────────────────────────────────

_OUT_FIELDS = [
    "language",
    "total_files",
    "total_with_ext",
    "detection_correct",
    "detection_accuracy_pct",
    "named",
    "name_fallback",
    "naming_success_pct",
    "repos",
]


# ── Aggregation ───────────────────────────────────────────────────────────────

def _pct(numerator: int, denominator: int) -> str:
    """Return a percentage string or 'n/a' when denominator is zero."""
    if denominator == 0:
        return "n/a"
    return f"{100 * numerator / denominator:.1f}"


def aggregate(input_dir: Path) -> list[dict]:
    """
    Walk every *.csv in input_dir, accumulate per-language stats, and return
    a list of result dicts sorted by total_files descending.
    """
    csv_paths = sorted(
        p for p in glob.glob(str(input_dir / "*.csv"))
        if Path(p).name != "metrics.csv"
    )
    if not csv_paths:
        raise FileNotFoundError(f"No CSV files found in: {input_dir}")

    # lang → stat bucket
    stats: dict[str, dict] = defaultdict(lambda: {
        "total_files": 0,
        "total_with_ext": 0,
        "detection_correct": 0,
        "named": 0,
        "name_fallback": 0,
        "repos": set(),
    })

    for csv_path in csv_paths:
        repo_name = Path(csv_path).stem  # filename without .csv
        try:
            with open(csv_path, encoding="utf-8") as fh:
                reader = csv.DictReader(fh)
                for row in reader:
                    lang = row.get("content_detected_lang", "").strip()
                    if not lang:
                        continue

                    has_ext = bool(row.get("actual_ext", "").strip())
                    det_ok = row.get("content_ext_match", "").strip() == "yes"
                    is_fallback = row.get("is_name_fallback", "").strip() == "yes"

                    bucket = stats[lang]
                    bucket["total_files"] += 1
                    bucket["repos"].add(repo_name)
                    if has_ext:
                        bucket["total_with_ext"] += 1
                    if det_ok:
                        bucket["detection_correct"] += 1
                    if is_fallback:
                        bucket["name_fallback"] += 1
                    else:
                        bucket["named"] += 1

        except Exception as exc:
            print(f"  Warning: could not read {csv_path}: {exc}")

    rows = []
    for lang, b in stats.items():
        rows.append({
            "language": lang,
            "total_files": b["total_files"],
            "total_with_ext": b["total_with_ext"],
            "detection_correct": b["detection_correct"],
            "detection_accuracy_pct": _pct(b["detection_correct"], b["total_with_ext"]),
            "named": b["named"],
            "name_fallback": b["name_fallback"],
            "naming_success_pct": _pct(b["named"], b["total_files"]),
            "repos": ", ".join(sorted(b["repos"])),
        })

    # Sort by total_files descending so the most-seen languages are at the top.
    rows.sort(key=lambda r: r["total_files"], reverse=True)
    return rows


# ── Printing ──────────────────────────────────────────────────────────────────

def print_table(rows: list[dict]) -> None:
    """Print a compact human-readable table to stdout."""
    hdr = (
        f"{'Language':<18} {'Files':>6} {'Det%':>6} {'Name%':>6}  "
        f"{'Det✓':>5}/{'>Ext':>5}  {'Named':>5}/{'>Tot':>5}  Repos"
    )
    sep = "-" * len(hdr)
    print(sep)
    print(hdr)
    print(sep)
    for r in rows:
        det = r["detection_accuracy_pct"]
        nam = r["naming_success_pct"]
        det_display = f"{det:>5}" if det == "n/a" else f"{float(det):5.1f}"
        nam_display = f"{float(nam):5.1f}"
        print(
            f"{r['language']:<18} {r['total_files']:>6} "
            f"{det_display}% {nam_display}%  "
            f"{r['detection_correct']:>5}/{r['total_with_ext']:>5}  "
            f"{r['named']:>5}/{r['total_files']:>5}  "
            f"{r['repos']}"
        )
    print(sep)
    print(f"  {len(rows)} languages  |  {sum(r['total_files'] for r in rows)} total files")
    print()


# ── Main ──────────────────────────────────────────────────────────────────────

def main() -> None:
    script_dir = Path(__file__).parent.resolve()

    parser = argparse.ArgumentParser(
        description="Summarise per-language naming & detection metrics from audit CSVs.",
        formatter_class=argparse.RawDescriptionHelpFormatter,
        epilog=__doc__,
    )
    parser.add_argument(
        "--input-dir",
        default=str(script_dir / "output"),
        help="Directory containing audit CSV files (default: ./output/).",
    )
    parser.add_argument(
        "--output",
        default=str(script_dir / "analysis" / "metrics" / "metrics.csv"),
        help="Path for the output metrics CSV (default: ./analysis/metrics/metrics.csv).",
    )
    args = parser.parse_args()

    input_dir = Path(args.input_dir)
    output_path = Path(args.output)

    print(f"Input  : {input_dir}")
    print(f"Output : {output_path}")
    print()

    rows = aggregate(input_dir)

    # Write CSV.
    output_path.parent.mkdir(parents=True, exist_ok=True)
    with open(output_path, "w", newline="", encoding="utf-8") as fh:
        writer = csv.DictWriter(fh, fieldnames=_OUT_FIELDS)
        writer.writeheader()
        writer.writerows(rows)

    print_table(rows)
    print(f"Metrics written to: {output_path}")


if __name__ == "__main__":
    main()
