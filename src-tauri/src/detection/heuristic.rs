/// Phase 4 — Heuristic scoring for programming languages.
///
/// Weighted pattern matching against 20+ language signatures.
/// Language definitions live in `languages/` (one file per language).
/// This module owns the scoring loop, superset tie-breaking, density
/// bonus logic, keyword fingerprinting, and **cross-family penalty**.
///
/// Cross-family penalty: each language definition may declare
/// `exclusive_patterns` — patterns near-exclusive to that language family.
/// When matched, their weights automatically penalize all languages
/// that belong to a *different* family.
use regex::Regex;
use std::collections::HashSet;
use std::sync::LazyLock;

use super::languages::COMPILED;

/// Minimum total score for a confident detection.
const HEURISTIC_SCORE_THRESHOLD: i32 = 3;

/// Minimum score for best-guess fallback when no language clears the threshold.
const PARTIAL_SCORE_THRESHOLD: i32 = 2;

/// Weight per unique keyword match (reserved words).
const KEYWORD_WEIGHT: i32 = 1;

/// Weight per unique builtin match (stdlib identifiers).
const BUILTIN_WEIGHT: i32 = 1;

/// Minimum unique keyword+builtin hits before the bonus kicks in.
/// Prevents a single accidental "print" from adding score.
const KEYWORD_MIN_HITS: usize = 3;

/// Fraction of exclusive-pattern weight applied as a cross-family penalty.
/// For each exclusive pattern that matched from *another* family, the language
/// under evaluation loses `floor(matched_weight * factor)` points.
const CROSS_PENALTY_FACTOR: f64 = 0.5;

/// Tokenize content into a set of unique lowercase word-like tokens.
/// Used once per scoring call; each language checks its keywords against this set.
fn tokenize(content: &str) -> HashSet<String> {
    // Split on anything that isn't alphanumeric or underscore, then lowercase.
    // This is intentionally simple — we're matching identifiers, not parsing.
    static SPLIT: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"[a-zA-Z_][\w]*").unwrap());
    SPLIT
        .find_iter(content)
        .map(|m| m.as_str().to_lowercase())
        .collect()
}

/// Detect language by heuristic pattern scoring.
///
/// Returns the highest-scoring language above the threshold, with superset
/// tie-breaking (TypeScript beats JavaScript, C++ beats C) and density
/// bonuses for repeated matches.
pub fn detect_by_scoring(content: &str) -> Option<&'static str> {
    let (winner, _) = detect_by_scoring_with_runner_up(content);
    winner
}

/// Like `detect_by_scoring` but also returns the runner-up candidate
/// for tree-sitter validation.
pub fn detect_by_scoring_with_runner_up(content: &str) -> (Option<&'static str>, Option<&'static str>) {
    use std::collections::HashMap;

    let mut scores: HashMap<&str, i32> = HashMap::new();
    let mut partial_best: Option<&str> = None;
    let mut partial_best_score = 0i32;

    // Pre-compute ES module signals
    static ES_IMPORT: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r#"(?m)^\s*import\s+[\w\{*].*\s+from\s+['"`]"#).unwrap());
    static ES_EXPORT: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"(?m)^\s*export\s+(const|let|var|function|class|default|type|interface|enum)\s")
            .unwrap()
    });
    let has_es_module = ES_IMPORT.is_match(content) || ES_EXPORT.is_match(content);

    // Tokenize once for keyword fingerprinting
    let tokens = tokenize(content);

    // ── Pre-pass: collect exclusive-pattern hits by family ──
    // For each language that declares exclusive_patterns, check them against the
    // content and accumulate the total matched weight per family. This map is used
    // in the per-language loop to penalize unrelated families.
    let mut family_exclusive_score: HashMap<&str, i32> = HashMap::new();
    for sig in COMPILED.iter() {
        for ep in &sig.exclusive {
            let match_count = ep.regex.find_iter(content).take(5).count();
            if match_count > 0 {
                let weight = ep.weight + (match_count as i32 - 1).min(3);
                *family_exclusive_score.entry(sig.family).or_insert(0) += weight;
            }
        }
    }

    for sig in COMPILED.iter() {
        // ES module guard: file is definitively JS/TS — skip others
        if has_es_module && sig.name != "javascript" && sig.name != "typescript" {
            continue;
        }

        // ── Pattern scoring ──
        let mut score = 0i32;
        let mut pattern_score = 0i32; // track positive pattern evidence separately
        for pat in &sig.patterns {
            if pat.weight > 0 {
                let match_count = pat.regex.find_iter(content).take(5).count();
                if match_count > 0 {
                    let pts = pat.weight + (match_count as i32 - 1).min(3);
                    score += pts;
                    pattern_score += pts;
                }
            } else {
                if pat.regex.is_match(content) {
                    score += pat.weight;
                }
            }
        }

        // ── Keyword fingerprint bonus ──
        // Count unique keyword+builtin hits; only add bonus if ≥ KEYWORD_MIN_HITS.
        // Guard: require at least one positive pattern hit before applying the
        // keyword bonus. Common English words ("with", "given", "for", "if",
        // "not", "set") appear in many keyword lists and inflate scores for
        // plain prose when no actual code patterns matched.
        let kw_hits: usize = sig.keywords.iter().filter(|kw| tokens.contains(**kw)).count();
        let bi_hits: usize = sig.builtins.iter().filter(|bi| tokens.contains(**bi)).count();
        let total_hits = kw_hits + bi_hits;
        if total_hits >= KEYWORD_MIN_HITS && pattern_score > 0 {
            score += kw_hits as i32 * KEYWORD_WEIGHT + bi_hits as i32 * BUILTIN_WEIGHT;
        }

        // ── Cross-family penalty ──
        // Sum exclusive scores from families OTHER than this language's family.
        // Each point of foreign exclusive evidence is scaled by CROSS_PENALTY_FACTOR.
        let cross_penalty: i32 = family_exclusive_score
            .iter()
            .filter(|(&fam, _)| fam != sig.family)
            .map(|(_, &w)| w)
            .sum();
        if cross_penalty > 0 {
            score -= (cross_penalty as f64 * CROSS_PENALTY_FACTOR) as i32;
        }

        if score < 0 {
            continue;
        }

        if score >= HEURISTIC_SCORE_THRESHOLD {
            scores.insert(sig.name, score);
        } else if score > partial_best_score {
            partial_best = Some(sig.name);
            partial_best_score = score;
        }
    }

    // Confident matches — pick the best
    if !scores.is_empty() {
        // Superset tie-breaking: TypeScript ⊃ JavaScript, C++ ⊃ C
        resolve_superset(&mut scores, "typescript", "javascript");
        resolve_superset(&mut scores, "cpp", "c");
        resolve_superset(&mut scores, "kotlin", "java");

        let mut sorted: Vec<_> = scores.iter().collect();
        sorted.sort_by(|a, b| b.1.cmp(a.1));

        let best = sorted.first().map(|(&lang, _)| lang);
        let runner_up = sorted.get(1).map(|(&lang, _)| lang);

        return (best, runner_up);
    }

    // Best-guess fallback
    if partial_best_score >= PARTIAL_SCORE_THRESHOLD {
        return (partial_best, None);
    }

    (None, None)
}

/// If both a superset language and its base language scored above threshold,
/// and the superset's score is ≥ 60% of the base, the base is removed.
fn resolve_superset(scores: &mut std::collections::HashMap<&str, i32>, superset: &str, base: &str) {
    let super_score = scores.get(superset).copied();
    let base_score = scores.get(base).copied();
    if let (Some(ss), Some(bs)) = (super_score, base_score) {
        if ss as f64 >= bs as f64 * 0.6 {
            scores.remove(base);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn python_def_class() {
        let content = r#"
import os

class MyApp:
    def __init__(self):
        self.name = "test"

    def run(self):
        print("running")
"#;
        assert_eq!(detect_by_scoring(content), Some("python"));
    }

    #[test]
    fn javascript_commonjs() {
        let content = r#"
const express = require('express');
const app = express();

app.get('/', (req, res) => {
    res.send('Hello');
});

module.exports = app;
"#;
        assert_eq!(detect_by_scoring(content), Some("javascript"));
    }

    #[test]
    fn typescript_interface() {
        let content = r#"
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
        assert_eq!(detect_by_scoring(content), Some("typescript"));
    }

    #[test]
    fn rust_derive_fn() {
        let content = r#"
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct Config {
    pub name: String,
    pub values: HashMap<String, String>,
}

pub fn process(config: &Config) -> Result<(), String> {
    println!("Processing: {}", config.name);
    Ok(())
}
"#;
        assert_eq!(detect_by_scoring(content), Some("rust"));
    }

    #[test]
    fn go_package_func() {
        let content = r#"
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
        assert_eq!(detect_by_scoring(content), Some("go"));
    }

    #[test]
    fn sql_create_select() {
        let content = r#"
CREATE TABLE users (
    id INTEGER PRIMARY KEY,
    name VARCHAR(255) NOT NULL,
    email TEXT
);

SELECT u.name, COUNT(o.id) as order_count
FROM users u
LEFT JOIN orders o ON u.id = o.user_id
GROUP BY u.name
ORDER BY order_count DESC;
"#;
        assert_eq!(detect_by_scoring(content), Some("sql"));
    }

    #[test]
    fn shell_script() {
        let content = r#"
#!/bin/bash
export PATH="/usr/local/bin:$PATH"

if [[ -z "$1" ]]; then
    echo "Usage: $0 <dir>"
    exit 1
fi

for f in "$1"/*.txt; do
    echo "Processing $f"
done
"#;
        assert_eq!(detect_by_scoring(content), Some("shell"));
    }

    #[test]
    fn cpp_with_std() {
        let content = r#"
#include <iostream>
#include <vector>

int main() {
    std::vector<int> nums = {1, 2, 3};
    for (auto& n : nums) {
        std::cout << n << std::endl;
    }
    return 0;
}
"#;
        assert_eq!(detect_by_scoring(content), Some("cpp"));
    }

    #[test]
    fn java_public_class() {
        let content = r#"
import java.util.ArrayList;
import java.util.List;

public class Main {
    public static void main(String[] args) {
        List<String> names = new ArrayList<>();
        names.add("Alice");
        System.out.println(names);
    }
}
"#;
        assert_eq!(detect_by_scoring(content), Some("java"));
    }

    // ── Keyword fingerprinting tests ──────────────────────────────────────

    #[test]
    fn keyword_boost_python_builtins() {
        // Short snippet — patterns alone are weak but keyword hits push it over.
        // The print() call provides the minimal pattern evidence needed.
        let content = r#"
result = isinstance(x, str)
items = enumerate(data)
frozen = frozenset([1, 2])
print(result)
"#;
        assert_eq!(detect_by_scoring(content), Some("python"));
    }

    #[test]
    fn keyword_boost_rust_builtins() {
        let content = r#"
let val: Option<String> = None;
let items: Vec<i32> = vec![1, 2, 3];
let guard = mutex.lock().unwrap();
"#;
        assert_eq!(detect_by_scoring(content), Some("rust"));
    }

    #[test]
    fn keyword_boost_go_builtins() {
        let content = r#"
package main

func main() {
    ch := make(chan int)
    defer close(ch)
    go func() { ch <- 42 }()
    val := <-ch
    fmt.Println(len(append(items, val)))
}
"#;
        assert_eq!(detect_by_scoring(content), Some("go"));
    }

    #[test]
    fn keyword_no_boost_below_threshold() {
        // Only 1-2 keyword hits — should NOT inflate score
        // "print" alone shouldn't be enough to detect Python
        let content = "print something\n";
        assert_eq!(detect_by_scoring(content), None);
    }

    #[test]
    fn tokenize_extracts_identifiers() {
        let tokens = tokenize("fn main() { let mut x = Vec::new(); }");
        assert!(tokens.contains("fn"));
        assert!(tokens.contains("main"));
        assert!(tokens.contains("let"));
        assert!(tokens.contains("mut"));
        assert!(tokens.contains("vec"));
        // Doesn't contain punctuation
        assert!(!tokens.contains("("));
        assert!(!tokens.contains("::"));
    }

    // ── Cross-family penalty tests ───────────────────────────────────────

    #[test]
    fn cpp_tensorflow_style_not_typescript() {
        // TensorFlow-style C++ with custom namespaces (no std::).
        // Previously misdetected as TypeScript because of shared keywords
        // (namespace, override, public, protected) and template patterns.
        let content = r#"
#include "tensorflow/core/framework/op_kernel.h"
#include <vector>

namespace tensorflow {

class MyOp : public OpKernel {
 public:
  explicit MyOp(OpKernelConstruction* context) : OpKernel(context) {}

  void Compute(OpKernelContext* context) override {
    const Tensor& input = context->input(0);
    auto flat = input.flat<float>();
    Tensor* output = nullptr;
    OP_REQUIRES_OK(context, context->allocate_output(0, input.shape(), &output));
  }

 private:
  int batch_size_;
};

}  // namespace tensorflow
"#;
        let result = detect_by_scoring(content);
        assert!(
            result == Some("cpp") || result == Some("c"),
            "Expected C/C++ but got {:?}",
            result
        );
    }

    #[test]
    fn cpp_header_with_templates_not_typescript() {
        // C++ header with templates and preprocessor guards.
        let content = r#"
#ifndef MY_HEADER_H_
#define MY_HEADER_H_

#include <string>
#include <memory>

namespace mylib {

template<typename T>
class Container {
 public:
  void push(const T& item);
  T pop();
  size_t size() const;

 private:
  std::vector<T> items_;
};

template<typename T>
void Container<T>::push(const T& item) {
  items_.push_back(item);
}

}  // namespace mylib

#endif  // MY_HEADER_H_
"#;
        let result = detect_by_scoring(content);
        assert_eq!(result, Some("cpp"), "Expected cpp but got {:?}", result);
    }

    #[test]
    fn cross_penalty_does_not_affect_same_family() {
        // TypeScript should still beat JavaScript when TS-specific syntax is present.
        // The JS-family exclusive patterns should NOT penalize TypeScript.
        let content = r#"
interface User {
    name: string;
    age: number;
}

const greet = (user: User): string => {
    return `Hello ${user.name}`;
};

export const validate = (x: unknown): x is User => {
    return typeof x === 'object' && x !== null;
};
"#;
        assert_eq!(detect_by_scoring(content), Some("typescript"));
    }

    #[test]
    fn c_preprocessor_penalizes_unrelated_langs() {
        // Pure C with preprocessor directives. Should not be detected as
        // PHP, C#, or Perl (all of which share some syntax with C).
        let content = r#"
#include <stdio.h>
#include <stdlib.h>

#ifndef MAX_SIZE
#define MAX_SIZE 1024
#endif

#pragma once

int main(int argc, char* argv[]) {
    printf("Hello, world!\n");
    return 0;
}
"#;
        let result = detect_by_scoring(content);
        assert!(
            result == Some("c") || result == Some("cpp"),
            "Expected C/C++ but got {:?}",
            result
        );
    }
}