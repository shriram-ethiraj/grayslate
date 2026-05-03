/// Family-gated candidate scoring (Phase 2b of new pipeline).
///
/// Only languages matching the classified content family enter scoring.
/// Each language is scored by its anchors + hints + keyword fingerprint,
/// not by global pattern matching. This eliminates cross-family false positives.
use regex::Regex;
use std::collections::HashSet;
use std::sync::LazyLock;

use super::family::ContentFamily;
use super::languages::{CompiledFamilyLanguage, COMPILED_FAMILY};

/// A scored language candidate.
#[derive(Debug, Clone)]
pub struct ScoredCandidate {
    pub name: &'static str,
    pub anchor_score: i32,
    pub hint_score: i32,
    pub keyword_score: i32,
    pub total_score: i32,
}

/// Minimum anchor score for a confident detection.
const ANCHOR_THRESHOLD: i32 = 4;

/// Cap on how much hint score can contribute (prevents hint-only detections).
const HINT_SCORE_CAP: i32 = 6;

/// Cap on how much keyword/builtin bonus can contribute.
const KEYWORD_SCORE_CAP: i32 = 6;

/// Minimum unique keyword+builtin hits before the bonus kicks in.
/// Prevents a single accidental match from inflating scores.
const KEYWORD_MIN_HITS: usize = 3;

/// Tokenize content into a set of unique lowercase word-like tokens.
/// Used once per scoring call; each language checks its keywords against this set.
fn tokenize(content: &str) -> HashSet<String> {
    static SPLIT: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"[a-zA-Z_][\w]*").unwrap());
    SPLIT
        .find_iter(content)
        .map(|m| m.as_str().to_lowercase())
        .collect()
}

/// Score all languages in the given families against the content.
///
/// Returns candidates sorted by total score (descending), filtered to
/// those above the minimum threshold. Empty vec = abstain.
pub fn score_candidates(
    content: &str,
    families: &[ContentFamily],
) -> Vec<ScoredCandidate> {
    let mut candidates: Vec<ScoredCandidate> = Vec::new();

    // Tokenize once for keyword fingerprinting across all candidates
    let tokens = tokenize(content);

    if cfg!(debug_assertions) {
        let fam_str: Vec<&str> = families.iter().map(|f| match f {
            ContentFamily::Prose => "Prose",
            ContentFamily::Code => "Code",
            ContentFamily::StructuredData => "Data",
            ContentFamily::Markup => "Markup",
            ContentFamily::ShellScript => "Shell",
            ContentFamily::Config => "Config",
        }).collect();
        eprintln!("[Lang Detect]   [Phase 2b] Scoring language candidates (allowed families: [{}])", fam_str.join(","));
    }

    for lang in COMPILED_FAMILY.iter() {
        // Family gate: only score languages matching the classified family
        if !lang.content_families.iter().any(|f| families.contains(f)) {
            continue;
        }

        if let Some(candidate) = score_language(content, lang, &tokens) {
            candidates.push(candidate);
        }
    }

    // Sort by total score descending
    candidates.sort_by(|a, b| b.total_score.cmp(&a.total_score));

    if cfg!(debug_assertions) {
        if candidates.is_empty() {
            eprintln!("[Lang Detect]   [Phase 2b] No candidates passed the minimum threshold — nothing to disambiguate");
        } else {
            let cand_str: Vec<String> = candidates.iter().map(|c| format!("\"{}\" (score={})", c.name, c.total_score)).collect();
            eprintln!("[Lang Detect]   [Phase 2b] {} candidate(s) passed: {}", candidates.len(), cand_str.join(", "));
        }
    }

    candidates
}

/// Score a single language against the content.
///
/// Returns None if the language is disqualified or scores too low.
fn score_language(
    content: &str,
    lang: &CompiledFamilyLanguage,
    tokens: &HashSet<String>,
) -> Option<ScoredCandidate> {
    // Check disqualifiers first — any match immediately rules out this language
    for dq in &lang.disqualifiers {
        if dq.regex.is_match(content) {
            if cfg!(debug_assertions) {
                eprintln!("[Lang Detect]   [Phase 2b] ─── \"{}\" ──────────", lang.name);
                eprintln!("[Lang Detect]   [Phase 2b]   [disqualifiers]");
                eprintln!("[Lang Detect]   [Phase 2b]     ✓ {}  (w={})  ← DISQUALIFIED", dq.regex.as_str(), dq.weight);
            }
            return None;
        }
    }

    // Score anchors (high-confidence signals)
    let anchor_score: i32 = lang
        .anchors
        .iter()
        .filter(|p| p.regex.is_match(content))
        .map(|p| p.weight)
        .sum();

    // Score hints (supporting evidence), capped
    let raw_hint_score: i32 = lang
        .hints
        .iter()
        .filter(|p| p.regex.is_match(content))
        .map(|p| p.weight)
        .sum();
    let hint_score = raw_hint_score.min(HINT_SCORE_CAP);

    // Keyword/builtin fingerprint bonus: count unique hits, gate on anchor evidence.
    // Common English words overlap with keyword lists, so require at least one
    // anchor match before awarding the bonus.
    let kw_hits = lang.keywords.iter().filter(|kw| tokens.contains(**kw)).count();
    let bi_hits = lang.builtins.iter().filter(|bi| tokens.contains(**bi)).count();
    let total_hits = kw_hits + bi_hits;
    let keyword_score = if total_hits >= KEYWORD_MIN_HITS && anchor_score > 0 {
        ((kw_hits + bi_hits) as i32).min(KEYWORD_SCORE_CAP)
    } else {
        0
    };

    let total_score = anchor_score + hint_score + keyword_score;

    if cfg!(debug_assertions) {
        // ── Header: announce this language ──────────────────────────
        eprintln!("[Lang Detect]   [Phase 2b] ─── \"{}\" ──────────", lang.name);

        // ── Disqualifiers ───────────────────────────────────────────
        eprintln!("[Lang Detect]   [Phase 2b]   [disqualifiers]");
        if lang.disqualifiers.is_empty() {
            eprintln!("[Lang Detect]   [Phase 2b]     (none)");
        } else {
            for dq in &lang.disqualifiers {
                let hit = dq.regex.is_match(content);
                let pat = dq.regex.as_str();
                if hit {
                    eprintln!("[Lang Detect]   [Phase 2b]     ✓ {}  (w={})", pat, dq.weight);
                } else {
                    eprintln!("[Lang Detect]   [Phase 2b]     ✗ {}", pat);
                }
            }
        }

        // ── Anchors ─────────────────────────────────────────────────
        eprintln!("[Lang Detect]   [Phase 2b]   [anchors] score={}", anchor_score);
        if lang.anchors.is_empty() {
            eprintln!("[Lang Detect]   [Phase 2b]     (none)");
        } else {
            for a in &lang.anchors {
                let hit = a.regex.is_match(content);
                let pat = a.regex.as_str();
                if hit {
                    eprintln!("[Lang Detect]   [Phase 2b]     ✓ {}  (w={})", pat, a.weight);
                } else {
                    eprintln!("[Lang Detect]   [Phase 2b]     ✗ {}", pat);
                }
            }
        }

        // ── Hints ───────────────────────────────────────────────────
        eprintln!(
            "[Lang Detect]   [Phase 2b]   [hints] score={} (raw={}, max_cap={})",
            hint_score, raw_hint_score, HINT_SCORE_CAP,
        );
        if lang.hints.is_empty() {
            eprintln!("[Lang Detect]   [Phase 2b]     (none)");
        } else {
            for h in &lang.hints {
                let hit = h.regex.is_match(content);
                let pat = h.regex.as_str();
                if hit {
                    eprintln!("[Lang Detect]   [Phase 2b]     ✓ {}  (w={})", pat, h.weight);
                } else {
                    eprintln!("[Lang Detect]   [Phase 2b]     ✗ {}", pat);
                }
            }
        }

        // ── Keywords ────────────────────────────────────────────────
        eprintln!(
            "[Lang Detect]   [Phase 2b]   [keywords] hits={} (kw={}, bi={}) | need≥{} & anchor>0={} → bonus={}",
            total_hits, kw_hits, bi_hits,
            KEYWORD_MIN_HITS,
            if anchor_score > 0 { "✓" } else { "✗" },
            keyword_score,
        );
        if !lang.keywords.is_empty() {
            let kw_detail: Vec<String> = lang.keywords.iter().map(|kw| {
                if tokens.contains(*kw) { format!("✓{}", kw) } else { format!("✗{}", kw) }
            }).collect();
            eprintln!("[Lang Detect]   [Phase 2b]     keywords: {}", kw_detail.join("  "));
        }
        if !lang.builtins.is_empty() {
            let bi_detail: Vec<String> = lang.builtins.iter().map(|bi| {
                if tokens.contains(*bi) { format!("✓{}", bi) } else { format!("✗{}", bi) }
            }).collect();
            eprintln!("[Lang Detect]   [Phase 2b]     builtins: {}", bi_detail.join("  "));
        }

        // ── Summary ─────────────────────────────────────────────────
        let passes = total_score >= ANCHOR_THRESHOLD;
        eprintln!(
            "[Lang Detect]   [Phase 2b]   [result] a={} + h={} + k={} = {}  ≥ {}  {}",
            anchor_score, hint_score, keyword_score, total_score, ANCHOR_THRESHOLD,
            if passes { "✓ PASSES" } else { "✗ FAILS" },
        );
    }

    // Minimum threshold: need at least some anchor evidence
    if anchor_score < ANCHOR_THRESHOLD && total_score < ANCHOR_THRESHOLD {
        return None;
    }

    Some(ScoredCandidate {
        name: lang.name,
        anchor_score,
        hint_score,
        keyword_score,
        total_score,
    })
}

// ── Tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_candidates_for_empty_content() {
        let candidates = score_candidates("", &[ContentFamily::Code]);
        assert!(candidates.is_empty());
    }

    #[test]
    fn prose_family_blocks_code_languages() {
        // Code-like keywords in prose should NOT produce code candidates
        let prose = "We need to set up monitoring and import the data from the old system. \
                     Let me know if you need help with any of this.";
        let candidates = score_candidates(prose, &[ContentFamily::Prose]);
        for c in &candidates {
            assert!(
                c.name == "email" || c.name == "prompt" || c.name == "text",
                "Prose family should not produce code candidate {:?}",
                c.name
            );
        }
    }

    #[test]
    fn code_family_finds_python() {
        let python = r#"
import os
import sys

def main():
    path = os.getcwd()
    if path == '/tmp':
        sys.exit(1)
    print(path)

if __name__ == "__main__":
    main()
"#;
        let candidates = score_candidates(python, &[ContentFamily::Code]);
        assert!(!candidates.is_empty(), "Should find Python candidate");
        assert_eq!(candidates[0].name, "python");
        assert!(candidates[0].anchor_score >= 4, "Python anchors should fire");
    }

    #[test]
    fn code_family_finds_rust() {
        let rust = r#"
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct Config {
    pub name: String,
}

pub fn process(config: &Config) -> Result<(), String> {
    let mut map = HashMap::new();
    map.insert("key", config.name.clone());
    println!("Processing: {}", config.name);
    Ok(())
}
"#;
        let candidates = score_candidates(rust, &[ContentFamily::Code]);
        assert!(!candidates.is_empty(), "Should find Rust candidate");
        assert_eq!(candidates[0].name, "rust");
    }

    #[test]
    fn code_family_finds_go() {
        let go = r#"
package main

import "fmt"

func main() {
    result, err := compute(42)
    if err != nil {
        fmt.Println("error:", err)
    }
    fmt.Println(result)
}
"#;
        let candidates = score_candidates(go, &[ContentFamily::Code]);
        assert!(!candidates.is_empty(), "Should find Go candidate");
        assert_eq!(candidates[0].name, "go");
    }

    #[test]
    fn code_family_finds_typescript() {
        let ts = r#"
interface User {
    name: string;
    age: number;
    active: boolean;
}

type Result<T> = { data: T } | { error: string };

const getUser = async (id: number): Promise<User> => {
    return { name: "Alice", age: 30, active: true };
};
"#;
        let candidates = score_candidates(ts, &[ContentFamily::Code]);
        assert!(!candidates.is_empty(), "Should find TypeScript candidate");
        assert_eq!(candidates[0].name, "typescript");
    }

    #[test]
    fn code_family_finds_java() {
        let java = r#"
import java.util.ArrayList;
import java.util.List;

public class Main {
    public static void main(String[] args) {
        List<String> items = new ArrayList<>();
        items.add("hello");
        System.out.println(items);
    }
}
"#;
        let candidates = score_candidates(java, &[ContentFamily::Code]);
        assert!(!candidates.is_empty(), "Should find Java candidate");
        assert_eq!(candidates[0].name, "java");
    }

    #[test]
    fn code_family_finds_kotlin() {
        let kotlin = r#"
data class User(val name: String, val age: Int)

fun main() {
    val users = listOf(
        User("Alice", 30),
        User("Bob", 25),
    )
    users.filter { it.age > 18 }
        .forEach { println(it.name) }
}
"#;
        let candidates = score_candidates(kotlin, &[ContentFamily::Code]);
        assert!(!candidates.is_empty(), "Should find Kotlin candidate");
        assert_eq!(candidates[0].name, "kotlin");
    }

    #[test]
    fn code_family_finds_cpp() {
        let cpp = r#"
#include <iostream>
#include <vector>
#include <string>

using namespace std;

template<typename T>
void printVec(const vector<T>& vec) {
    for (const auto& item : vec) {
        cout << item << endl;
    }
}

int main() {
    auto ptr = make_unique<string>("hello");
    cout << *ptr << endl;
    return 0;
}
"#;
        let candidates = score_candidates(cpp, &[ContentFamily::Code]);
        assert!(!candidates.is_empty(), "Should find C++ candidate");
        assert_eq!(candidates[0].name, "cpp");
    }

    #[test]
    fn shell_family_finds_shell() {
        let shell = r#"#!/bin/bash
set -e
export PATH="/usr/local/bin:$PATH"

for f in $(find . -name "*.txt"); do
    echo "Processing $f"
    cat "$f" | grep -v "^#" | sort > "${f}.sorted"
done
"#;
        let candidates = score_candidates(shell, &[ContentFamily::ShellScript]);
        assert!(!candidates.is_empty(), "Should find shell candidate");
        assert_eq!(candidates[0].name, "shell");
    }

    #[test]
    fn shell_family_finds_powershell() {
        let ps = r#"
[CmdletBinding()]
param(
    [string]$Name
)

$items = Get-ChildItem -Path $PSScriptRoot -Filter "*.ps1"
foreach ($item in $items) {
    Write-Host "Found: $($item.Name)"
}
"#;
        let candidates = score_candidates(ps, &[ContentFamily::ShellScript]);
        assert!(!candidates.is_empty(), "Should find PowerShell candidate");
        assert_eq!(candidates[0].name, "powershell");
    }

    #[test]
    fn markup_family_finds_html() {
        let html = r#"
<div class="container">
    <h1>Hello World</h1>
    <p>This is a <strong>test</strong> page.</p>
    <ul>
        <li>Item 1</li>
        <li>Item 2</li>
    </ul>
</div>
"#;
        let candidates = score_candidates(html, &[ContentFamily::Markup]);
        assert!(!candidates.is_empty(), "Should find HTML candidate");
        assert_eq!(candidates[0].name, "html");
    }

    #[test]
    fn keyword_bonus_boosts_typescript() {
        // TS code with type annotations (anchors) + multiple keywords
        let ts = r#"
interface Config {
    readonly name: string;
    type: string;
}

declare const globalConfig: Config;

enum Status { Active, Inactive }

const result = globalConfig as Config;
"#;
        let candidates = score_candidates(ts, &[ContentFamily::Code]);
        assert!(!candidates.is_empty(), "Should find TS candidate");
        let ts_candidate = candidates.iter().find(|c| c.name == "typescript").unwrap();
        assert!(ts_candidate.keyword_score > 0, "TS keyword bonus should fire: kw={}", ts_candidate.keyword_score);
    }

    #[test]
    fn keyword_bonus_requires_anchor_evidence() {
        // Ambiguous content with keywords but no clear anchors
        let ambiguous = "const x = 1;\nlet y = 2;\nvar z = 3;\n";
        let candidates = score_candidates(ambiguous, &[ContentFamily::Code]);
        for c in &candidates {
            assert_eq!(c.keyword_score, 0, "{} should not get keyword bonus without anchors", c.name);
        }
    }

    #[test]
    fn js_vs_ts_pure_js_wins() {
        // Pure JS code (no type annotations) — JS should score higher or TS should not match
        let js = r#"
const express = require('express');
const app = express();

app.get('/api', (req, res) => {
    console.log('Request received');
    res.json({ ok: true });
});

module.exports = app;
"#;
        let candidates = score_candidates(js, &[ContentFamily::Code]);
        assert!(!candidates.is_empty());
        let js_candidate = candidates.iter().find(|c| c.name == "javascript");
        let ts_candidate = candidates.iter().find(|c| c.name == "typescript");
        assert!(js_candidate.is_some(), "JS should be a candidate");
        if let Some(ts) = ts_candidate {
            let js = js_candidate.unwrap();
            assert!(js.total_score > ts.total_score, "JS ({}) should outscore TS ({})", js.total_score, ts.total_score);
        }
    }
}
