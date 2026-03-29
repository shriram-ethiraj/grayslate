#!/usr/bin/env python3
"""
compare_runs.py — Compare two audit runs (before vs after).

Reads paired CSV directories from two separate audit runs and produces a
summary showing per-language detection accuracy and naming success rate
deltas, plus a list of files where results changed.

Typical workflow:
  1. Run audit_repos.py → output in output-with-treesitter/ (old baseline)
  2. Make code changes (remove tree-sitter, improve regex, etc.)
  3. Run audit_repos.py again → output in output/ (new results)
  4. python compare_runs.py --before output-with-treesitter --after output

Usage:
    python compare_runs.py
    python compare_runs.py --before output-with-treesitter --after output
    python compare_runs.py --output analysis/comparison.csv
"""

import argparse
import csv
from collections import defaultdict
from pathlib import Path


SCRIPT_DIR = Path(__file__).parent.resolve()


def _pct(n: int, d: int) -> str:
    if d == 0:
        return "n/a"
    return f"{100 * n / d:.1f}"


def _pct_float(n: int, d: int) -> float:
    if d == 0:
        return 0.0
    return 100 * n / d


def load_csvs(directory: Path) -> dict[str, list[dict]]:
    """Load all audit CSVs from a directory, keyed by repo name."""
    result = {}
    for csv_path in sorted(directory.glob("*.csv")):
        if csv_path.name in ("metrics.csv", ".gitkeep"):
            continue
        repo = csv_path.stem
        with open(csv_path, encoding="utf-8") as fh:
            result[repo] = list(csv.DictReader(fh))
    return result


def build_file_index(rows: list[dict]) -> dict[str, dict]:
    """Index rows by file path for O(1) lookup."""
    return {row["file"]: row for row in rows}


def main() -> None:
    parser = argparse.ArgumentParser(
        description="Compare two audit runs (before vs after).",
    )
    parser.add_argument(
        "--before",
        default=str(SCRIPT_DIR / "output-with-treesitter"),
        help="Directory with BEFORE CSVs (default: output-with-treesitter/).",
    )
    parser.add_argument(
        "--after",
        default=str(SCRIPT_DIR / "output"),
        help="Directory with AFTER CSVs (default: output/).",
    )
    parser.add_argument(
        "--output",
        default=str(SCRIPT_DIR / "analysis" / "comparison.csv"),
        help="Output CSV for per-language comparison.",
    )
    parser.add_argument(
        "--diff-output",
        default=str(SCRIPT_DIR / "analysis" / "diff-files.csv"),
        help="Output CSV listing files where detection or naming changed.",
    )
    args = parser.parse_args()

    before_dir = Path(args.before)
    after_dir = Path(args.after)
    output_path = Path(args.output)
    diff_path = Path(args.diff_output)

    if not before_dir.is_dir():
        print(f"Error: {before_dir} not found.")
        return
    if not after_dir.is_dir():
        print(f"Error: {after_dir} not found. Run audit_repos.py first.")
        return

    before_data = load_csvs(before_dir)
    after_data = load_csvs(after_dir)

    common_repos = sorted(set(before_data) & set(after_data))
    if not common_repos:
        print("No common repos found between the two directories.")
        return

    print(f"Comparing {len(common_repos)} repos")
    print(f"  Before: {before_dir}")
    print(f"  After:  {after_dir}\n")

    # Per-language aggregation
    lang_stats = defaultdict(lambda: {
        "before_total": 0, "before_det_ok": 0, "before_named": 0, "before_ext_files": 0,
        "after_total": 0, "after_det_ok": 0, "after_named": 0, "after_ext_files": 0,
    })

    # Files where results differ
    diff_files = []

    for repo in common_repos:
        before_index = build_file_index(before_data[repo])
        after_index = build_file_index(after_data[repo])

        common_files = sorted(set(before_index) & set(after_index))

        for filepath in common_files:
            bef = before_index[filepath]
            aft = after_index[filepath]

            bef_lang = bef.get("content_detected_lang", "")
            aft_lang = aft.get("content_detected_lang", "")
            bef_ext_match = bef.get("content_ext_match", "") == "yes"
            aft_ext_match = aft.get("content_ext_match", "") == "yes"
            bef_named = bef.get("is_name_fallback", "") != "yes"
            aft_named = aft.get("is_name_fallback", "") != "yes"
            has_ext = bool(bef.get("actual_ext", "").strip())

            # Aggregate under the BEFORE detected language
            if bef_lang:
                b = lang_stats[bef_lang]
                b["before_total"] += 1
                if has_ext:
                    b["before_ext_files"] += 1
                if bef_ext_match:
                    b["before_det_ok"] += 1
                if bef_named:
                    b["before_named"] += 1

            if aft_lang:
                b = lang_stats[aft_lang]
                b["after_total"] += 1
                if has_ext:
                    b["after_ext_files"] += 1
                if aft_ext_match:
                    b["after_det_ok"] += 1
                if aft_named:
                    b["after_named"] += 1

            # Track differences
            if bef_lang != aft_lang or bef_ext_match != aft_ext_match or bef_named != aft_named:
                diff_files.append({
                    "repo": repo,
                    "file": filepath,
                    "actual_ext": bef.get("actual_ext", ""),
                    "before_lang": bef_lang,
                    "after_lang": aft_lang,
                    "before_ext": bef.get("content_suggested_ext", ""),
                    "after_ext": aft.get("content_suggested_ext", ""),
                    "before_det_match": "yes" if bef_ext_match else "no",
                    "after_det_match": "yes" if aft_ext_match else "no",
                    "before_named": "yes" if bef_named else "no",
                    "after_named": "yes" if aft_named else "no",
                    "det_improved": "yes" if aft_ext_match and not bef_ext_match else "",
                    "det_regressed": "yes" if not aft_ext_match and bef_ext_match else "",
                    "name_improved": "yes" if aft_named and not bef_named else "",
                    "name_regressed": "yes" if not aft_named and bef_named else "",
                })

    # Compute totals
    improved = sum(1 for d in diff_files if d["det_improved"] == "yes")
    regressed = sum(1 for d in diff_files if d["det_regressed"] == "yes")
    name_improved = sum(1 for d in diff_files if d["name_improved"] == "yes")
    name_regressed = sum(1 for d in diff_files if d["name_regressed"] == "yes")
    total_diff = len(diff_files)

    # Build comparison rows
    rows = []
    for lang, b in lang_stats.items():
        bef_det_pct = _pct_float(b["before_det_ok"], b["before_ext_files"])
        aft_det_pct = _pct_float(b["after_det_ok"], b["after_ext_files"])
        bef_name_pct = _pct_float(b["before_named"], b["before_total"])
        aft_name_pct = _pct_float(b["after_named"], b["after_total"])

        rows.append({
            "language": lang,
            "before_files": b["before_total"],
            "after_files": b["after_total"],
            "before_det_pct": f"{bef_det_pct:.1f}",
            "after_det_pct": f"{aft_det_pct:.1f}",
            "det_delta": f"{aft_det_pct - bef_det_pct:+.1f}",
            "before_name_pct": f"{bef_name_pct:.1f}",
            "after_name_pct": f"{aft_name_pct:.1f}",
            "name_delta": f"{aft_name_pct - bef_name_pct:+.1f}",
        })

    rows.sort(key=lambda r: r["before_files"], reverse=True)

    # Print summary
    print(f"{'='*80}")
    print(f"  Before vs After — Impact Summary")
    print(f"{'='*80}")
    print(f"  Total changed files:     {total_diff}")
    print(f"  Detection IMPROVED:      {improved} files (wrong before → correct after)")
    print(f"  Detection REGRESSED:     {regressed} files (correct before → wrong after)")
    print(f"  Detection net:           {improved - regressed:+d} files")
    print(f"  Naming IMPROVED:         {name_improved} files (fallback before → named after)")
    print(f"  Naming REGRESSED:        {name_regressed} files (named before → fallback after)")
    print(f"  Naming net:              {name_improved - name_regressed:+d} files")
    print(f"{'='*80}\n")

    hdr = f"{'Language':<18} {'BefF':>6} {'AftF':>6} {'BDet%':>7} {'ADet%':>7} {'ΔDet':>6}  {'BName%':>7} {'AName%':>7} {'ΔName':>6}"
    print(hdr)
    print("-" * len(hdr))
    for r in rows:
        det_d = float(r["det_delta"])
        name_d = float(r["name_delta"])
        det_marker = " ▲" if det_d > 0.5 else " ▼" if det_d < -0.5 else "  "
        name_marker = " ▲" if name_d > 0.5 else " ▼" if name_d < -0.5 else "  "
        print(
            f"{r['language']:<18} {r['before_files']:>6} {r['after_files']:>6} "
            f"{r['before_det_pct']:>6}% {r['after_det_pct']:>6}%{det_marker} "
            f" {r['before_name_pct']:>6}% {r['after_name_pct']:>6}%{name_marker}"
        )
    print()

    # Write comparison CSV
    output_path.parent.mkdir(parents=True, exist_ok=True)
    comp_fields = [
        "language", "before_files", "after_files",
        "before_det_pct", "after_det_pct", "det_delta",
        "before_name_pct", "after_name_pct", "name_delta",
    ]
    with open(output_path, "w", newline="", encoding="utf-8") as fh:
        writer = csv.DictWriter(fh, fieldnames=comp_fields)
        writer.writeheader()
        writer.writerows(rows)
    print(f"Comparison CSV: {output_path}")

    # Write diff files CSV
    if diff_files:
        diff_fields = [
            "repo", "file", "actual_ext",
            "before_lang", "after_lang",
            "before_ext", "after_ext",
            "before_det_match", "after_det_match",
            "before_named", "after_named",
            "det_improved", "det_regressed",
            "name_improved", "name_regressed",
        ]
        diff_path.parent.mkdir(parents=True, exist_ok=True)
        with open(diff_path, "w", newline="", encoding="utf-8") as fh:
            writer = csv.DictWriter(fh, fieldnames=diff_fields)
            writer.writeheader()
            writer.writerows(diff_files)
        print(f"Diff files CSV: {diff_path}")

        # Show a sample of regressions if any
        regressions = [d for d in diff_files if d["det_regressed"] == "yes"]
        if regressions:
            print(f"\nSample detection REGRESSIONS (first 10):")
            for row in regressions[:10]:
                print(
                    f"  {row['repo']}/{row['file']}: "
                    f".{row['actual_ext']} → before={row['before_lang']}({row['before_ext']}) "
                    f"after={row['after_lang']}({row['after_ext']})"
                )

        improvements = [d for d in diff_files if d["det_improved"] == "yes"]
        if improvements:
            print(f"\nSample detection IMPROVEMENTS (first 10):")
            for row in improvements[:10]:
                print(
                    f"  {row['repo']}/{row['file']}: "
                    f".{row['actual_ext']} → before={row['before_lang']}({row['before_ext']}) "
                    f"after={row['after_lang']}({row['after_ext']})"
                )
    else:
        print("No differences found between before and after runs.")


if __name__ == "__main__":
    main()
