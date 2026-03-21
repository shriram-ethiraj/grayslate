#!/usr/bin/env python3
"""
filter_failures.py — Extract failure records from naming-audit CSVs.

Reads all per-repo CSV files produced by audit_repos.py and writes two
filtered datasets under the analysis/ directory:

  analysis/content_match_negatives/
      One CSV per source repo, containing only rows where
      content_ext_match == "no".  These are files the detection pipeline
      called the wrong language.

  analysis/name_fallback_positives/
      One CSV per source repo, containing only rows where
      is_name_fallback == "yes".  These are files the naming pipeline
      couldn't produce a meaningful stem for.

Run after audit_repos.py has finished populating output/.

Usage:
    python filter_failures.py                        # default paths
    python filter_failures.py --input-dir /tmp/out   # custom input
    python filter_failures.py --analysis-dir /tmp/analysis  # custom output root
"""

import argparse
import csv
import glob
import os
from pathlib import Path


# ── Filtering ─────────────────────────────────────────────────────────────────

def filter_repo_csv(
    csv_path: Path,
    negatives_dir: Path,
    fallbacks_dir: Path,
) -> tuple[int, int]:
    """
    Read one repo CSV and write two filtered copies.

    Returns (negatives_written, fallbacks_written).
    """
    repo_name = csv_path.name  # preserve original filename

    negatives: list[dict] = []
    fallbacks: list[dict] = []
    fieldnames: list[str] = []

    try:
        with open(csv_path, encoding="utf-8") as fh:
            reader = csv.DictReader(fh)
            fieldnames = reader.fieldnames or []
            for row in reader:
                if row.get("content_ext_match", "").strip() == "no":
                    negatives.append(row)
                if row.get("is_name_fallback", "").strip() == "yes":
                    fallbacks.append(row)
    except Exception as exc:
        print(f"  Warning: could not read {csv_path}: {exc}")
        return 0, 0

    if not fieldnames:
        return 0, 0

    def _write(out_dir: Path, rows: list[dict]) -> int:
        if not rows:
            return 0
        out_path = out_dir / repo_name
        with open(out_path, "w", newline="", encoding="utf-8") as fh:
            writer = csv.DictWriter(fh, fieldnames=fieldnames)
            writer.writeheader()
            writer.writerows(rows)
        return len(rows)

    n = _write(negatives_dir, negatives)
    f = _write(fallbacks_dir, fallbacks)
    return n, f


# ── Main ──────────────────────────────────────────────────────────────────────

def main() -> None:
    script_dir = Path(__file__).parent.resolve()

    parser = argparse.ArgumentParser(
        description="Extract failure records from naming-audit CSVs.",
        formatter_class=argparse.RawDescriptionHelpFormatter,
        epilog=__doc__,
    )
    parser.add_argument(
        "--input-dir",
        default=str(script_dir / "output"),
        help="Directory containing per-repo audit CSV files (default: ./output/).",
    )
    parser.add_argument(
        "--analysis-dir",
        default=str(script_dir / "analysis"),
        help="Root output directory for filtered datasets (default: ./analysis/).",
    )
    args = parser.parse_args()

    input_dir = Path(args.input_dir)
    analysis_dir = Path(args.analysis_dir)

    negatives_dir = analysis_dir / "content_match_negatives"
    fallbacks_dir = analysis_dir / "name_fallback_positives"

    negatives_dir.mkdir(parents=True, exist_ok=True)
    fallbacks_dir.mkdir(parents=True, exist_ok=True)

    csv_paths = sorted(
        p for p in (Path(p) for p in glob.glob(str(input_dir / "*.csv")))
        if p.name != "metrics.csv"  # skip the metrics summary if present
    )

    if not csv_paths:
        print(f"No CSV files found in: {input_dir}")
        return

    print(f"Input            : {input_dir}")
    print(f"Negatives output : {negatives_dir}")
    print(f"Fallbacks output : {fallbacks_dir}")
    print()

    total_neg = 0
    total_fall = 0

    for csv_path in csv_paths:
        n, f = filter_repo_csv(csv_path, negatives_dir, fallbacks_dir)
        total_neg += n
        total_fall += f
        neg_str = f"{n:>5} negatives" if n else "           -"
        fall_str = f"{f:>5} fallbacks" if f else "           -"
        print(f"  {csv_path.stem:<40}  {neg_str}  {fall_str}")

    print()
    print(f"content_match_negatives : {total_neg} records across {len(list(negatives_dir.glob('*.csv')))} repos")
    print(f"name_fallback_positives : {total_fall} records across {len(list(fallbacks_dir.glob('*.csv')))} repos")


if __name__ == "__main__":
    main()
