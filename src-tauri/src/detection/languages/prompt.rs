use super::{wp, LanguageDefinition};
use super::ContentFamily;
use regex::Regex;
use std::sync::LazyLock;

/// Structural detector for AI prompts.
///
/// Runs before YAML (priority 115 < 120) because prompt "Section:" headers
/// look like YAML `key: value` pairs. This detector fires only on strong
/// prompt-specific signals that YAML would never have.
fn is_likely_prompt(trimmed: &str, _was_sliced: bool) -> bool {
    // Quick reject: very short text can't be a prompt
    if trimmed.len() < 40 {
        return false;
    }

    // Quick reject: starts with structural markers of other formats
    let first = trimmed.as_bytes().first().copied().unwrap_or(0);
    if first == b'{' || first == b'[' || first == b'<' {
        return false;
    }

    let mut score = 0i32;

    // ── Role assignment (strongest signal) ───────────────────────────
    static ROLE_START: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"(?mi)^(You\s+are\s+(a|an)\s+|Act\s+as\s+(a|an)?\s*|You'?\s*re\s+(a|an)\s+|Pretend\s+you\s+are|Imagine\s+you\s+are|I\s+want\s+you\s+to\s+(act\s+as|be|become))").unwrap()
    });
    if ROLE_START.is_match(trimmed) {
        score += 5;
    }

    // ── Chat role labels ─────────────────────────────────────────────
    static CHAT_LABELS: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"(?mi)^\s*(System|User|Assistant|Human|AI)\s*:").unwrap()
    });
    let label_count = CHAT_LABELS.find_iter(trimmed).take(4).count();
    if label_count >= 2 {
        score += 4;
    } else if label_count == 1 {
        score += 2;
    }

    // ── ChatML delimiters ────────────────────────────────────────────
    static CHATML: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"<\|(system|user|assistant|im_start|im_end)\|>").unwrap()
    });
    if CHATML.is_match(trimmed) {
        score += 5;
    }

    // ── Template variables ───────────────────────────────────────────
    static TEMPLATE_VARS: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"\{\{[a-zA-Z_]\w*\}\}").unwrap()
    });
    let var_count = TEMPLATE_VARS.find_iter(trimmed).take(3).count();
    if var_count >= 2 {
        score += 3;
    } else if var_count == 1 {
        score += 1;
    }

    // ── Prompt section headers ───────────────────────────────────────
    // These look like YAML but are prompt sections. Only count if 2+.
    static SECTION_HEADERS: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"(?mi)^(Context|Instructions|Rules|Constraints|Guidelines|Requirements|Examples?|Background|Objective|Goal|Task|Persona|Tone|Style|Format)\s*:").unwrap()
    });
    let section_count = SECTION_HEADERS.find_iter(trimmed).take(4).count();
    if section_count >= 2 {
        score += 3;
    } else if section_count == 1 {
        score += 1;
    }

    // ── Output format instructions ───────────────────────────────────
    static OUTPUT_FORMAT: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"(?mi)(Respond\s+in|Format\s+(as|the\s+output)|Output\s+(as|in|format)|Return\s+(as|in)|in\s+JSON\b|in\s+(?:markdown|yaml|XML|CSV|plain\s+text)\b|as\s+(?:a\s+)?(?:JSON|markdown|yaml|XML|CSV|plain\s+text|numbered\s+list|bullet)\b)").unwrap()
    });
    if OUTPUT_FORMAT.is_match(trimmed) {
        score += 2;
    }

    // ── Numbered list items ──────────────────────────────────────────
    // Numbered instructions (1. Do X, 2. Do Y) are common in prompts,
    // rare in YAML. Especially strong when combined with section headers.
    static NUMBERED_LIST: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"(?m)^\s*\d+\.\s+\S").unwrap()
    });
    let num_list_count = NUMBERED_LIST.find_iter(trimmed).take(5).count();
    if num_list_count >= 3 {
        score += 2;
    } else if num_list_count >= 1 && section_count >= 1 {
        // Numbered items + section headers = strong prompt signal
        score += 2;
    }

    // ── Meta instruction phrases ─────────────────────────────────────
    static META_INSTRUCTIONS: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"(?mi)(Do\s+not|Don'?\s*t|Never|Always|Make\s+sure|Ensure|You\s+must|You\s+should|Be\s+sure\s+to|Remember\s+to)").unwrap()
    });
    if META_INSTRUCTIONS.is_match(trimmed) {
        score += 1;
    }

    // ── Few-shot example markers ─────────────────────────────────────
    static EXAMPLE_MARKERS: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"(?mi)^(Example|Sample)\s*\d*\s*:").unwrap()
    });
    static IO_MARKERS: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"(?mi)^(Input|Output|Expected\s+(?:output|result))\s*:").unwrap()
    });
    if EXAMPLE_MARKERS.is_match(trimmed) {
        score += 2;
    }
    let io_count = IO_MARKERS.find_iter(trimmed).take(3).count();
    if io_count >= 2 {
        score += 2;
    }

    // ── Instruction verbs early in text ──────────────────────────────
    let prefix = if trimmed.len() > 300 {
        &trimmed[..trimmed.floor_char_boundary(300)]
    } else {
        trimmed
    };
    static INSTRUCTION_VERB: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"(?mi)^(Write|Generate|Create|Summarize|Translate|Explain|Analyze|Review|Design|Implement|Rewrite|Describe|List|Provide|Convert|Extract|Classify|Evaluate|Compose|Draft|Outline)\s+(a|an|the|\w+)").unwrap()
    });
    if INSTRUCTION_VERB.is_match(prefix) {
        score += 2;
    }

    // ── Anti-signals: YAML-specific syntax ───────────────────────────
    // Indented key-value blocks with 2-space nesting are YAML, not prompts
    static YAML_INDENT: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"(?m)^  [a-z_]\w*:\s+\S").unwrap()
    });
    let yaml_indent_count = YAML_INDENT.find_iter(trimmed).take(4).count();
    if yaml_indent_count >= 3 {
        score -= 3;
    }

    // YAML list items with consistent `- key: value` structure
    static YAML_LIST: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"(?m)^\s+- [a-z_]\w*:\s+").unwrap()
    });
    if YAML_LIST.find_iter(trimmed).take(3).count() >= 2 {
        score -= 3;
    }

    score >= 4
}

pub fn definition() -> LanguageDefinition {
    LanguageDefinition {
        name: "prompt",
        extensions: &[],
        filenames: &[],
        filename_patterns: &[],
        shebangs: &[],
        // Before YAML (120) — prompts with "Section:" headers look like YAML
        structural_priority: Some(115),
        structural_detect: Some(is_likely_prompt),
        patterns: &[
            // ── Role assignment (strongest signal) ───────────────────────
            // "You are a ..." / "Act as a ..."
            wp!(r"(?mi)^You\s+are\s+(a|an)\s+", 5),
            wp!(r"(?mi)^Act\s+as\s+(a|an)?\s*", 5),
            // "You're a ..."
            wp!(r"(?mi)^You'?\s*re\s+(a|an)\s+", 4),
            // "Pretend you are" / "Imagine you are" / "I want you to act as"
            wp!(r"(?mi)(Pretend|Imagine|Assume)\s+you\s+are", 4),
            wp!(r"(?mi)I\s+want\s+you\s+to\s+(act\s+as|be|become)", 4),

            // ── Chat role labels ─────────────────────────────────────────
            // System:/User:/Assistant:/Human:/AI: at line start
            wp!(r"(?mi)^\s*(System|User|Assistant|Human|AI)\s*:", 4),
            // ### System / ### User / ### Assistant (markdown-style role headers)
            wp!(r"(?mi)^#{1,3}\s*(System|User|Assistant|Human)\s*$", 3),
            // [SYSTEM] / [USER] / [ASSISTANT] bracketed labels
            wp!(r"(?mi)^\[(SYSTEM|USER|ASSISTANT|HUMAN|AI)\]", 3),
            // <|system|> / <|user|> / <|assistant|> — ChatML delimiters
            wp!(r"<\|(system|user|assistant|im_start|im_end)\|>", 4),

            // ── Instruction verbs ────────────────────────────────────────
            // Task verbs at sentence start (early in text)
            wp!(r"(?mi)^(Write|Generate|Create|Summarize|Translate|Compare|Explain|Analyze|Analyse|Review|Design|Build|Implement|Rewrite|Refactor|Optimize|Describe|List|Provide|Convert|Extract|Transform|Classify|Evaluate|Compose|Draft|Outline|Paraphrase)\s+(a|an|the|\w+)", 3),
            // "Please write/generate/create ..."
            wp!(r"(?mi)Please\s+(write|generate|create|summarize|translate|explain|analyze|review|help|provide|list|describe)", 3),

            // ── Template variables ───────────────────────────────────────
            // {{variable}} — Mustache / Handlebars / LLM template style
            wp!(r"\{\{[a-zA-Z_]\w*\}\}", 3),
            // {variable} — single-brace placeholder
            wp!(r"\{[A-Z_][A-Z_0-9]*\}", 2),
            // <variable> / [variable] — XML/bracket placeholders in prompts
            wp!(r"<[A-Z_][A-Z_0-9]*>", 2),

            // ── Output format instructions ───────────────────────────────
            wp!(r"(?mi)(Respond\s+in|Format\s+(as|the\s+output)|Output\s+(as|in|format)|Return\s+(as|in)|in\s+JSON\b|in\s+(?:markdown|yaml|XML|CSV|plain\s+text)\b)", 3),
            // "Respond with a JSON object" / "Return a list"
            wp!(r"(?mi)(Respond|Reply|Answer|Return)\s+with\s+(a|an|the)", 2),

            // ── Section headers common in prompts ────────────────────────
            // "Context:", "Instructions:", "Rules:", "Constraints:", etc.
            wp!(r"(?mi)^(Context|Instructions|Rules|Constraints|Guidelines|Requirements|Examples?|Input|Output|Background|Objective|Goal|Task|Persona|Tone|Style|Format)\s*:", 3),

            // ── Meta instructions ────────────────────────────────────────
            // "Do not" / "Don't" / "Never" / "Always" / "Make sure" / "Ensure"
            wp!(r"(?mi)(Do\s+not|Don'?\s*t|Never|Always|Make\s+sure|Ensure|You\s+must|You\s+should|Be\s+sure\s+to|Remember\s+to|Keep\s+in\s+mind)", 2),
            // "Step 1:" / "Step 2:" numbered instruction steps
            wp!(r"(?mi)^Step\s+\d+\s*:", 2),
            // "Here is" / "Here are" / "Below is" — preamble phrases
            wp!(r"(?mi)^(Here\s+(?:is|are)|Below\s+(?:is|are)|The\s+following)", 1),

            // ── Few-shot example markers ─────────────────────────────────
            // "Example:", "Example 1:", "Input:", "Output:" pairs
            wp!(r"(?mi)^(Example|Sample)\s*\d*\s*:", 2),
            wp!(r"(?mi)^(Input|Output|Expected\s+(?:output|result))\s*:", 2),

            // ── Delimiter patterns ───────────────────────────────────────
            // "---" / "===" / "***" section separators with role labels nearby
            wp!(r"(?m)^(---+|===+|\*\*\*+)\s*$", 1),
        ],
        anti_patterns: &[
            // Code signals — strongly rule out prompt
            wp!(r"(?m)^\s*(import|export)\s+", -4),
            wp!(r"(?m)^\s*(const|let|var)\s+\w+\s*[=:]", -4),
            wp!(r"(?m)^\s*function\s+\w*\s*\(", -4),
            wp!(r"(?m)^\s*(class|interface|type|enum)\s+\w+", -4),
            wp!(r"(?m)^\s*def\s+\w+\s*\(", -4),
            wp!(r#"(?m)^\s*#include\s*[<"]"#, -4),
            wp!(r"(?m);\s*$", -2),
            // Script signals
            wp!(r"(?mi)^\s*@echo\s+(off|on)", -5),
            wp!(r"(?m)^\s*#!\s*/", -5),
            // Structural data (JSON/XML/HTML as primary content)
            wp!(r#"(?m)^\s*\{"#, -3),
            // Email signals — if it has RFC headers, it's email not prompt
            wp!(r"(?mi)^\s*(Subject|From|To|CC|BCC)\s*:\s+\S", -3),
        ],
        uses_hash_comments: false,
        // No keywords — all common English words
        keywords: &[],
        builtins: &[],
        family: Some("prose"),
        exclusive_patterns: &[
            // Role assignments are exclusive to prompts
            wp!(r"(?mi)^You\s+are\s+(a|an)\s+", 3),
            wp!(r"(?mi)^Act\s+as\s+(a|an)?\s*", 3),
            // Chat role labels
            wp!(r"(?mi)^\s*(System|User|Assistant)\s*:", 2),
        ],
        // ── Family-gated fields ──────────────────────────────
        content_families: &[ContentFamily::Prose],
        anchors: &[
            // Role markers
            wp!(r"(?mi)^You\s+are\s+(a|an)\s+", 5),
            wp!(r"(?mi)^Act\s+as\s+(a|an)?\s*", 5),
            // Instruction verbs at start
            wp!(r"(?mi)^(Given|Explain|Analyze|Summarize|Translate|Generate|Write|Create)\s+(a|an|the|\w+)", 4),
            // Output format directives
            wp!(r"(?mi)(Respond\s+in|Format\s+as|Output\s+as|Return\s+as)", 4),
        ],
        hints: &[
            // Context / section markers
            wp!(r"(?mi)^(Context|Instructions|Rules|Constraints|Guidelines)\s*:", 3),
            // System/User/Assistant labels
            wp!(r"(?mi)^\s*(System|User|Assistant|Human|AI)\s*:", 3),
            // Constraint phrases
            wp!(r"(?mi)(Do\s+not|Don'?\s*t|Avoid|Never|Always|You\s+must)", 2),
        ],
        rivals: &["email"],
        differentiators: &[
            // Role markers distinguish from email
            wp!(r"(?mi)^You\s+are\s+(a|an)\s+", 5),
            wp!(r"(?mi)^Act\s+as\s+(a|an)?\s*", 5),
            // Instruction verbs
            wp!(r"(?mi)^(Given|Explain|Analyze|Summarize|Translate|Generate|Write|Create)\s+(a|an|the|\w+)", 4),
            // Output format directives
            wp!(r"(?mi)(Respond\s+in|Format\s+as|Output\s+as|Return\s+as)", 4),
        ],
        disqualifiers: &[],
    }
}
