use super::{wp, LanguageDefinition};
use super::ContentFamily;
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
        keywords: &[
            "#if", "#each", "#await", "#snippet", "#key",
            ":else", ":then", ":catch",
            "/if", "/each", "/await", "/snippet", "/key",
        ],
        builtins: &[
            // Svelte 5 runes
            "$state", "$derived", "$effect", "$props", "$bindable",
            "$inspect", "$host",
            // Svelte 4 lifecycle (still valid)
            "onMount", "onDestroy", "beforeUpdate", "afterUpdate",
            "createEventDispatcher", "tick", "setContext", "getContext",
            // Svelte stores
            "writable", "readable", "derived",
        ],
        // ── Family-gated fields ──────────────────────────────
        content_families: &[ContentFamily::Markup],
        anchors: &[
            wp!(r"\{#if\s", 5),
            wp!(r"\{#each\s", 5),
            wp!(r"\{#await\s", 5),
            wp!(r"\{:else", 4),
            wp!(r"\{/if\}", 4),
            wp!(r"\{/each\}", 4),
            // Svelte 5 runes — extremely distinctive
            wp!(r"\$state\(", 5),
            wp!(r"\$derived\(", 5),
            wp!(r"\$effect\(", 5),
            wp!(r"\$props\(", 5),
            // Special tags: {@html}, {@render}, {@debug}, {@const}
            wp!(r"\{@(html|render|debug|const)\s", 4),
            // {#snippet} / {#key} — Svelte-specific blocks
            wp!(r"\{#(snippet|key)\s", 4),
        ],
        hints: &[
            wp!(r"\bon:click\b", 3),
            wp!(r"\bon:\w+=", 3),
            wp!(r"\bbind:\w+", 3),
            wp!(r"\btransition:\w+", 3),
            wp!(r"\buse:\w+", 3),
            // <slot> — Svelte slot element
            wp!(r"<slot[\s>]", 2),
            // animate: directive
            wp!(r"\banimate:\w+", 2),
            // $: reactive statement (Svelte 4)
            wp!(r"(?m)^\s*\$:\s+", 3),
            // <svelte:component>, <svelte:self>, <svelte:window>, etc.
            wp!(r"<svelte:\w+", 3),
            // class: directive
            wp!(r"\bclass:\w+=", 2),
        ],
        disqualifiers: &[
            // Vue directives — means Vue, not Svelte
            wp!(r"\bv-(if|for|model)=", -4),
        ],
    }
}
