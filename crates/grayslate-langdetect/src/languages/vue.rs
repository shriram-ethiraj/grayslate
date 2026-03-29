use super::{wp, LanguageDefinition};
use super::ContentFamily;
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
        // ── Family-gated fields ──────────────────────────────
        content_families: &[ContentFamily::Markup],
        anchors: &[
            wp!(r"\bv-if=", 5),
            wp!(r"\bv-for=", 5),
            wp!(r"\bv-model=", 5),
            wp!(r"\bv-bind:", 4),
            wp!(r"\bv-on:", 4),
            wp!(r"<script\s+setup", 4),
            wp!(r"\bdefineProps\b", 4),
            wp!(r"\bdefineEmits\b", 4),
            // v-show — Vue visibility directive
            wp!(r"\bv-show=", 4),
            // v-slot — Vue scoped slots
            wp!(r"\bv-slot", 4),
        ],
        hints: &[
            wp!(r"<template\b", 3),
            wp!(r"@click=", 3),
            wp!(r":class=", 3),
            // More Vue shorthand event bindings
            wp!(r"@(submit|input|change|keyup|keydown)=", 2),
            // Dynamic binding shorthand
            wp!(r":(style|value|disabled|key)=", 2),
            // Vue 3 built-in components
            wp!(r"<(Teleport|Transition|KeepAlive|Suspense)\b", 3),
            // Composition API
            wp!(r"\b(ref|reactive|computed|watch|onMounted)\s*\(", 2),
            // defineExpose — Vue 3 script setup
            wp!(r"\bdefineExpose\s*\(", 2),
        ],
        disqualifiers: &[
            // Svelte block syntax — means Svelte, not Vue
            wp!(r"\{#(if|each|await)\s", -4),
        ],
    }
}
