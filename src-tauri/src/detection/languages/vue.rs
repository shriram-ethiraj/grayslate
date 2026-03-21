use super::{wp, LanguageDefinition};
use regex::Regex;
use std::sync::LazyLock;

pub(crate) fn is_likely_vue(trimmed: &str, _was_sliced: bool) -> bool {
    if !trimmed.starts_with('<') {
        return false;
    }

    static VUE_SIGNALS: LazyLock<Vec<(Regex, i32)>> = LazyLock::new(|| vec![
        (Regex::new(r"<template[\s>]").unwrap(), 4),
        (Regex::new(r"\b(v-if|v-else-if|v-else|v-show|v-for|v-on:|v-bind:|v-model|v-slot)[=>\s]").unwrap(), 2),
        (Regex::new(r"@(click|submit|input|change|keyup|keydown)=").unwrap(), 2),
        (Regex::new(r":(class|style|value|disabled|key)=").unwrap(), 2),
        (Regex::new(r"<script\s+setup[^>]*>").unwrap(), 3),
        (Regex::new(r"\b(defineProps|defineEmits|defineExpose)\s*\(").unwrap(), 2),
        (Regex::new(r"\b(ref|reactive|computed|watch|onMounted)\s*\(").unwrap(), 2),
    ]);

    let mut score = 0i32;
    for (re, weight) in VUE_SIGNALS.iter() {
        if re.is_match(trimmed) {
            score += weight;
        }
    }
    score >= 4
}

pub fn definition() -> LanguageDefinition {
    LanguageDefinition {
        name: "vue",
        extensions: &[".vue"],
        filenames: &[],
        filename_patterns: &[],
        shebangs: &[],
        structural_priority: Some(30),
        structural_detect: Some(is_likely_vue),
        patterns: &[
            wp!(r"\bv-if=", 5),
            wp!(r"\bv-for=", 5),
            wp!(r"\bv-model=", 5),
            wp!(r"\bv-bind:", 4),
            wp!(r"\bv-on:", 4),
            wp!(r"\bv-show=", 4),
            wp!(r"\bv-slot", 3),
            wp!(r"@click=", 3),
            wp!(r"@\w+=", 2),
            wp!(r":class=", 3),
            wp!(r":style=", 2),
            wp!(r"<template\b", 3),
            wp!(r"<script\s+setup", 4),
            wp!(r"\bdefineProps\b", 4),
            wp!(r"\bdefineEmits\b", 4),
        ],
        anti_patterns: &[
            wp!(r"\{#if\b", -4),
            wp!(r"\{#each\b", -4),
            wp!(r"\bon:click\b", -3),
        ],
        uses_hash_comments: false,
        keywords: &[
            "v-if", "v-else", "v-else-if", "v-for", "v-model", "v-show",
            "v-bind", "v-on", "v-slot", "v-pre", "v-cloak", "v-once", "v-memo",
        ],
        builtins: &[
            "ref", "reactive", "computed", "watch", "watchEffect",
            "onMounted", "onUnmounted", "onBeforeMount", "onBeforeUnmount",
            "defineProps", "defineEmits", "defineExpose", "withDefaults",
            "nextTick", "provide", "inject",
        ],
        illegal: None,
        extends: None,
    }
}
