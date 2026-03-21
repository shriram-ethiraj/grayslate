use super::{wp, LanguageDefinition};
use regex::Regex;
use std::sync::LazyLock;

pub(crate) fn is_likely_svelte(trimmed: &str, _was_sliced: bool) -> bool {
    let starts_with_tag = trimmed.starts_with('<');
    let has_block_tag = trimmed.contains("{#");

    if !starts_with_tag && !has_block_tag {
        return false;
    }

    if !starts_with_tag {
        static JSTS_CODE: LazyLock<Regex> = LazyLock::new(|| {
            Regex::new(
                r"^(const|let|var|type|interface|function|class|export|import|async\s+function)\b",
            )
            .unwrap()
        });
        let first_lines: Vec<&str> = trimmed
            .lines()
            .map(|l| l.trim())
            .filter(|l| {
                !l.is_empty()
                    && !l.starts_with('*')
                    && !l.starts_with("//")
                    && !l.starts_with("/*")
            })
            .take(5)
            .collect();
        let code_count = first_lines.iter().filter(|l| JSTS_CODE.is_match(l)).count();
        if code_count >= 2 {
            return false;
        }
    }

    static SVELTE_SIGNALS: LazyLock<Vec<(Regex, i32)>> = LazyLock::new(|| vec![
        (Regex::new(r"\{#(if|each|await|snippet|key)[}\s]").unwrap(), 3),
        (Regex::new(r"\{:(else|then|catch)[}\s]").unwrap(), 3),
        (Regex::new(r"\{/(if|each|await|snippet|key)\}").unwrap(), 3),
        (Regex::new(r#"<script\s+(context="module"|lang="ts")[^>]*>"#).unwrap(), 3),
        (Regex::new(r"\b(bind:|on:|use:|transition:|animate:|let:|class:)[a-zA-Z\-]+=").unwrap(), 2),
        (Regex::new(r"\$(state|derived|effect|props)\(").unwrap(), 4),
        (Regex::new(r"(?m)^\s*\$:\s+").unwrap(), 4),
        (Regex::new(r"<slot[\s>]").unwrap(), 2),
        (Regex::new(r"\{@(html|render|debug|const)\s+").unwrap(), 2),
    ]);

    let mut score = 0i32;
    for (re, weight) in SVELTE_SIGNALS.iter() {
        if re.is_match(trimmed) {
            score += weight;
        }
    }
    score >= 2
}

pub fn definition() -> LanguageDefinition {
    LanguageDefinition {
        name: "svelte",
        extensions: &[".svelte"],
        filenames: &[],
        filename_patterns: &[],
        shebangs: &[],
        structural_priority: Some(20),
        structural_detect: Some(is_likely_svelte),
        patterns: &[
            wp!(r"\{#if\s", 5),
            wp!(r"\{#each\s", 5),
            wp!(r"\{#await\s", 5),
            wp!(r"\{:else", 4),
            wp!(r"\{:then", 4),
            wp!(r"\{:catch", 4),
            wp!(r"\{/if\}", 4),
            wp!(r"\{/each\}", 4),
            wp!(r"\bon:click\b", 3),
            wp!(r"\bon:\w+=", 3),
            wp!(r"\bbind:\w+", 3),
            wp!(r"\btransition:\w+", 3),
            wp!(r"\buse:\w+", 3),
            wp!(r#"<script\b.*\blang=["']ts["']"#, 2),
            wp!(r#"<style\b.*\blang=["']scss["']"#, 2),
        ],
        anti_patterns: &[
            wp!(r"\bv-if\b", -4),
            wp!(r"\bv-for\b", -4),
            wp!(r"@click\b", -3),
        ],
        uses_hash_comments: false,
        keywords: &[
            "#if", "#each", "#await", ":else", ":then", ":catch",
            "/if", "/each", "/await",
        ],
        builtins: &[
            "onMount", "onDestroy", "beforeUpdate", "afterUpdate",
            "createEventDispatcher", "tick", "setContext", "getContext",
            "writable", "readable", "derived",
        ],
        illegal: None,
        extends: None,
    }
}
