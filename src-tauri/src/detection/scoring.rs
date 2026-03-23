/// Family-gated candidate scoring (Phase 2 of new pipeline).
///
/// Only languages matching the classified content family enter scoring.
/// Each language is scored by its anchors + hints, not by global pattern
/// matching. This eliminates cross-family false positives.
use super::family::ContentFamily;
use super::languages::{CompiledFamilyLanguage, COMPILED_FAMILY};

/// A scored language candidate.
#[derive(Debug, Clone)]
pub struct ScoredCandidate {
    pub name: &'static str,
    pub anchor_score: i32,
    pub hint_score: i32,
    pub total_score: i32,
}

/// Minimum anchor score for a confident detection.
const ANCHOR_THRESHOLD: i32 = 4;

/// Cap on how much hint score can contribute (prevents hint-only detections).
const HINT_SCORE_CAP: i32 = 6;

/// Score all languages in the given families against the content.
///
/// Returns candidates sorted by total score (descending), filtered to
/// those above the minimum threshold. Empty vec = abstain.
pub fn score_candidates(
    content: &str,
    families: &[ContentFamily],
) -> Vec<ScoredCandidate> {
    let mut candidates: Vec<ScoredCandidate> = Vec::new();

    for lang in COMPILED_FAMILY.iter() {
        // Family gate: only score languages matching the classified family
        if !lang.content_families.iter().any(|f| families.contains(f)) {
            continue;
        }

        if let Some(candidate) = score_language(content, lang) {
            candidates.push(candidate);
        }
    }

    // Sort by total score descending
    candidates.sort_by(|a, b| b.total_score.cmp(&a.total_score));
    candidates
}

/// Score a single language against the content.
///
/// Returns None if the language is disqualified or scores too low.
fn score_language(
    content: &str,
    lang: &CompiledFamilyLanguage,
) -> Option<ScoredCandidate> {
    // Check disqualifiers first — any match immediately rules out this language
    for dq in &lang.disqualifiers {
        if dq.regex.is_match(content) {
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

    let total_score = anchor_score + hint_score;

    // Minimum threshold: need at least some anchor evidence
    if anchor_score < ANCHOR_THRESHOLD && total_score < ANCHOR_THRESHOLD {
        return None;
    }

    Some(ScoredCandidate {
        name: lang.name,
        anchor_score,
        hint_score,
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
}
