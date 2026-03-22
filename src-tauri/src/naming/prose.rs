// ---------------------------------------------------------------------------
// Prose extractor — email → prompt → YAKE fallback
// ---------------------------------------------------------------------------

use regex::Regex;
use std::sync::LazyLock;

use super::model::{ExtractedName, StemKind};

/// Public entry point: tries email → prompt → YAKE in order.
pub(crate) fn extract_prose(content: &str) -> Option<String> {
    extract_prose_tagged(content).map(|en| en.stem)
}

/// Tagged variant that preserves whether the content was detected as
/// email, prompt, or generic prose — so the pipeline can append a suffix.
pub(crate) fn extract_prose_tagged(content: &str) -> Option<ExtractedName> {
    if content.trim().is_empty() {
        return None;
    }
    if let Some(stem) = try_extract_email(content) {
        return Some(ExtractedName { stem, kind: StemKind::Email });
    }
    if let Some(stem) = try_extract_prompt(content) {
        return Some(ExtractedName { stem, kind: StemKind::Prompt });
    }
    extract_yake(content).map(|stem| ExtractedName { stem, kind: StemKind::Generic })
}

// ===========================================================================
// Email extractor
// ===========================================================================

/// Known email header names (matched case-insensitively).
static EMAIL_HEADER_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)^(Subject|From|To|Date|CC|BCC)\s*:").unwrap());

/// Greeting at the start of an email body.
static GREETING_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)^(Dear|Hi|Hello)\s+[A-Z]").unwrap());

/// Closing line in an email.
static CLOSING_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)^(Best\b|Regards|Thanks|Sincerely|Cheers|Kind regards|Best regards|Thank you)")
        .unwrap()
});

/// Subject-line prefix noise: Re:/Fwd:/FW:/Fw:/RE:
static SUBJECT_PREFIX_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)^(Re|Fwd|FW|Fw|RE)\s*:\s*").unwrap());

/// Bracketed prefixes like [JIRA], [URGENT], [EXT].
static BRACKET_PREFIX_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^\[.*?\]\s*").unwrap());

pub(super) fn try_extract_email(content: &str) -> Option<String> {
    let lines: Vec<&str> = content.lines().collect();
    if !is_email(&lines) {
        return None;
    }

    // Priority 1: Subject line
    if let Some(topic) = extract_subject(&lines) {
        if !topic.is_empty() {
            return Some(topic);
        }
    }

    // Priority 2: first non-greeting, non-empty, non-header body line
    if let Some(topic) = first_body_line(&lines) {
        return Some(topic);
    }

    None
}

/// Determines whether the content looks like an email.
fn is_email(lines: &[&str]) -> bool {
    // Method 1: 2+ header lines in first 15 lines
    let header_count = lines
        .iter()
        .take(15)
        .filter(|l| EMAIL_HEADER_RE.is_match(l.trim()))
        .count();
    if header_count >= 2 {
        return true;
    }

    // Method 2: greeting in first 3 lines + closing anywhere
    let has_greeting = lines
        .iter()
        .take(3)
        .any(|l| GREETING_RE.is_match(l.trim()));
    let has_closing = lines.iter().any(|l| CLOSING_RE.is_match(l.trim()));
    has_greeting && has_closing
}

/// Extracts and cleans the Subject: line content.
fn extract_subject(lines: &[&str]) -> Option<String> {
    static SUBJECT_LINE_RE: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"(?i)^Subject\s*:\s*(.*)$").unwrap());

    for line in lines.iter().take(15) {
        if let Some(caps) = SUBJECT_LINE_RE.captures(line.trim()) {
            let mut subject = caps.get(1)?.as_str().trim().to_string();
            if subject.is_empty() {
                return None;
            }
            // Strip Re:/Fwd: prefixes recursively
            loop {
                let before = subject.clone();
                subject = SUBJECT_PREFIX_RE.replace(&subject, "").trim().to_string();
                if subject == before {
                    break;
                }
            }
            // Strip [bracketed] prefixes
            loop {
                let before = subject.clone();
                subject = BRACKET_PREFIX_RE.replace(&subject, "").trim().to_string();
                if subject == before {
                    break;
                }
            }
            if subject.is_empty() {
                return None;
            }
            return Some(subject);
        }
    }
    None
}

/// Returns the first non-empty, non-header, non-greeting body line.
fn first_body_line(lines: &[&str]) -> Option<String> {
    let mut past_headers = false;
    for line in lines {
        let trimmed = line.trim();
        if !past_headers {
            if trimmed.is_empty() {
                // Blank line after headers separates header block from body.
                past_headers = true;
                continue;
            }
            if EMAIL_HEADER_RE.is_match(trimmed) {
                continue;
            }
            // If there are no RFC-style headers, skip greeting lines too.
            past_headers = true;
        }
        if trimmed.is_empty() {
            continue;
        }
        if GREETING_RE.is_match(trimmed) {
            continue;
        }
        if EMAIL_HEADER_RE.is_match(trimmed) {
            continue;
        }
        // Use at most first ~80 chars as a topic
        let topic = if trimmed.len() > 80 {
            &trimmed[..trimmed.floor_char_boundary(80)]
        } else {
            trimmed
        };
        return Some(topic.to_string());
    }
    None
}

// ===========================================================================
// Prompt extractor
// ===========================================================================

/// Detects "You are" / "Act as" / "System:" / "User:" / "Assistant:" starts.
static ROLE_START_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)^(You are|Act as|System\s*:|User\s*:|Assistant\s*:)").unwrap()
});

/// Instruction verbs.
static INSTRUCTION_VERB_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?i)\b(Write|Generate|Create|Summarize|Translate|Compare|Explain|Analyze|Review|Design|Build|Implement)\b",
    )
    .unwrap()
});

/// Template variables: {var} or {{var}}.
static TEMPLATE_VAR_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\{\{?\w+\}?\}").unwrap());

/// Output formatting phrases.
static OUTPUT_FORMAT_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)(Respond in|Format as|Output as|in JSON|in markdown|in yaml)").unwrap()
});

/// Numbered or bullet list items at line start.
static LIST_ITEM_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?m)^\s*(\d+\.|[-*])\s+\S").unwrap());

/// "You are a {ROLE}" pattern — captures the role part.
static YOU_ARE_ROLE_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)^You are (?:a |an )?(.+)").unwrap());

/// "Act as a {ROLE}" pattern.
static ACT_AS_ROLE_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)^Act as (?:a |an )?(.+)").unwrap());

/// Task verbs: "Write/Create/Generate a {THING}".
static TASK_VERB_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)\b(Write|Create|Generate)\s+(?:a |an |the )?(.+)").unwrap()
});

/// Subject verbs: "Translate/Summarize/Compare {TOPIC}".
static SUBJECT_VERB_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)\b(Translate|Summarize|Compare|Explain|Analyze|Review)\s+(?:the |this |a |an )?(.+)").unwrap()
});

pub(super) fn try_extract_prompt(content: &str) -> Option<String> {
    if !is_prompt(content) {
        return None;
    }

    let first_line = content.lines().next().unwrap_or("").trim();

    // Priority 1: role extraction
    let role = extract_role(first_line);
    // Priority 2: task extraction (scan wider than first line)
    let task = extract_task(content);
    // Priority 3: subject extraction
    let subject = extract_subject_verb(content);

    match (role, task, subject) {
        (Some(r), Some(t), _) => Some(format!("{} {}", r, t)),
        (Some(r), None, _) => Some(r),
        (None, Some(t), _) => Some(t),
        (None, None, Some(s)) => Some(s),
        _ => None,
    }
}

/// Scores the content to decide if it's an AI prompt.
fn is_prompt(content: &str) -> bool {
    let mut score = 0u32;

    let first_line = content.lines().next().unwrap_or("").trim();
    if ROLE_START_RE.is_match(first_line) {
        score += 3;
    }

    // Check instruction verbs in first 200 chars
    let prefix = if content.len() > 200 {
        &content[..content.floor_char_boundary(200)]
    } else {
        content
    };
    if INSTRUCTION_VERB_RE.is_match(prefix) {
        score += 2;
    }

    if TEMPLATE_VAR_RE.is_match(content) {
        score += 2;
    }

    if OUTPUT_FORMAT_RE.is_match(content) {
        score += 1;
    }

    // 3+ list items
    let list_count = LIST_ITEM_RE.find_iter(content).count();
    if list_count >= 3 {
        score += 1;
    }

    score >= 3
}

/// Extracts a role from "You are a …" or "Act as a …" (first line).
fn extract_role(first_line: &str) -> Option<String> {
    let caps = YOU_ARE_ROLE_RE
        .captures(first_line)
        .or_else(|| ACT_AS_ROLE_RE.captures(first_line));

    if let Some(caps) = caps {
        let raw = caps.get(1)?.as_str().trim();
        let truncated = truncate_at_boundary(raw, 5);
        if !truncated.is_empty() {
            return Some(truncated);
        }
    }
    None
}

/// Extracts a task from "Write/Create/Generate a …".
fn extract_task(content: &str) -> Option<String> {
    // Scan first few lines for the task pattern
    for line in content.lines().take(10) {
        if let Some(caps) = TASK_VERB_RE.captures(line.trim()) {
            let verb = caps.get(1)?.as_str();
            let obj = caps.get(2)?.as_str().trim();
            let obj_truncated = truncate_at_boundary(obj, 5);
            if !obj_truncated.is_empty() {
                return Some(format!("{} {}", verb, obj_truncated));
            }
        }
    }
    None
}

/// Extracts a subject from "Translate/Summarize/Compare …".
fn extract_subject_verb(content: &str) -> Option<String> {
    for line in content.lines().take(10) {
        if let Some(caps) = SUBJECT_VERB_RE.captures(line.trim()) {
            let verb = caps.get(1)?.as_str();
            let topic = caps.get(2)?.as_str().trim();
            let topic_truncated = truncate_at_boundary(topic, 5);
            if !topic_truncated.is_empty() {
                return Some(format!("{} {}", verb, topic_truncated));
            }
        }
    }
    None
}

/// Truncates text to at most `max_words` words, also cutting at the first
/// period, comma, newline, or semicolon.
fn truncate_at_boundary(text: &str, max_words: usize) -> String {
    // Cut at sentence/clause boundary first
    let cut = text
        .find(|c: char| c == '.' || c == ',' || c == ';' || c == '\n')
        .unwrap_or(text.len());
    let text = &text[..cut];

    let words: Vec<&str> = text.split_whitespace().take(max_words).collect();
    words.join(" ")
}

// ===========================================================================
// YAKE fallback
// ===========================================================================

pub(super) fn extract_yake(content: &str) -> Option<String> {
    use yake_rust::{get_n_best, Config, StopWords};
    use super::model::MAX_TOKENS;

    if content.trim().is_empty() {
        return None;
    }

    // Strip copyright/license boilerplate before keyword extraction so it
    // doesn't dominate the result.
    let cleaned: String = content
        .lines()
        .filter(|l| {
            let t = l.trim().to_lowercase();
            !t.starts_with("copyright")
                && !t.starts_with("licensed under")
                && !t.starts_with("all rights reserved")
                && !t.starts_with("spdx-license")
                && !(t.starts_with('#') && t.contains("license"))
        })
        .collect::<Vec<_>>()
        .join("\n");

    if cleaned.trim().is_empty() {
        return None;
    }

    let stop_words = StopWords::predefined("en")?;
    let config = Config {
        ngrams: 2,
        ..Config::default()
    };

    let keywords = get_n_best(4, &cleaned, &stop_words, &config);
    if keywords.is_empty() {
        return None;
    }

    let stems: Vec<&str> = keywords
        .iter()
        .take(MAX_TOKENS)
        .map(|item| item.raw.as_str())
        .collect();
    if stems.is_empty() {
        None
    } else {
        Some(stems.join("-"))
    }
}

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // ── Email tests ─────────────────────────────────────────────────────

    #[test]
    fn email_basic_subject() {
        let email = "\
From: alice@example.com
To: bob@example.com
Subject: Weekly team standup notes
Date: Mon, 1 Jan 2026

Hi Bob,

Here are the notes from today's standup.

Best,
Alice";
        assert_eq!(
            try_extract_email(email).as_deref(),
            Some("Weekly team standup notes")
        );
    }

    #[test]
    fn email_strips_re_fwd_prefixes() {
        let email = "\
From: alice@example.com
To: bob@example.com
Subject: Re: Fwd: RE: Project deadline update

Body here.";
        assert_eq!(
            try_extract_email(email).as_deref(),
            Some("Project deadline update")
        );
    }

    #[test]
    fn email_strips_bracket_prefixes() {
        let email = "\
From: jira@company.com
To: team@company.com
Subject: [JIRA] [URGENT] PROJ-456 Fix login timeout

Description follows.";
        let result = try_extract_email(email).unwrap();
        assert!(
            result.contains("PROJ-456"),
            "should preserve ticket number, got: {result}"
        );
        assert!(
            !result.starts_with('['),
            "should strip bracket prefix, got: {result}"
        );
    }

    #[test]
    fn email_preserves_ticket_numbers() {
        let email = "\
From: support@example.com
To: dev@example.com
Subject: Re: #123 Payment processing error

Please fix ASAP.";
        let result = try_extract_email(email).unwrap();
        assert!(result.contains("#123"), "got: {result}");
    }

    #[test]
    fn email_greeting_closing_detection() {
        let email = "\
Dear Mr. Johnson,

I'm writing to inquire about the quarterly budget review.

Could you send the updated figures?

Sincerely,
Sarah";
        let result = try_extract_email(email);
        assert!(result.is_some(), "should detect email via greeting+closing");
        assert!(
            result
                .as_deref()
                .unwrap()
                .contains("quarterly budget review"),
            "got: {:?}",
            result
        );
    }

    #[test]
    fn email_no_subject_falls_back_to_body() {
        let email = "\
From: alice@example.com
To: bob@example.com
Date: Mon, 1 Jan 2026

The deployment pipeline needs a hotfix for the staging environment.

Thanks,
Alice";
        let result = try_extract_email(email);
        assert!(result.is_some(), "should find body line");
        assert!(
            result.as_deref().unwrap().contains("deployment pipeline"),
            "got: {:?}",
            result
        );
    }

    #[test]
    fn email_empty_subject_falls_back() {
        let email = "\
From: x@y.com
To: z@y.com
Subject:

Actual content starts here about API migration.";
        let result = try_extract_email(email);
        // Empty subject → falls to body line
        assert!(result.is_some());
        assert!(
            result.as_deref().unwrap().contains("API migration"),
            "got: {:?}",
            result
        );
    }

    #[test]
    fn email_not_detected_for_plain_text() {
        let text = "This is just a regular paragraph about cooking pasta.";
        assert!(try_extract_email(text).is_none());
    }

    #[test]
    fn email_cc_bcc_headers_count() {
        let email = "\
CC: manager@company.com
BCC: hr@company.com
Subject: INV-2026-Q3 Invoice correction

Please review the attached.";
        let result = try_extract_email(email).unwrap();
        assert!(
            result.contains("INV-2026-Q3"),
            "should preserve invoice number, got: {result}"
        );
    }

    #[test]
    fn email_subject_with_only_prefixes() {
        let email = "\
From: a@b.com
To: c@d.com
Subject: Re: Re: Fwd:

Some body text about database migration plan.";
        let result = try_extract_email(email);
        // Subject is empty after stripping → falls to body
        assert!(result.is_some());
        assert!(
            result.as_deref().unwrap().contains("database migration"),
            "got: {:?}",
            result
        );
    }

    #[test]
    fn email_hi_hello_greeting_variants() {
        let email = "\
Hi Team,

Please review the updated onboarding checklist before Friday.

Thanks,
Priya";
        let result = try_extract_email(email);
        assert!(result.is_some(), "Hi + Thanks should detect as email");
    }

    // ── Prompt tests ────────────────────────────────────────────────────

    #[test]
    fn prompt_you_are_role() {
        let prompt = "\
You are a senior Rust developer. Review the following code and suggest improvements.

1. Check for memory safety issues
2. Suggest better error handling
3. Improve naming conventions";
        let result = try_extract_prompt(prompt).unwrap();
        assert!(
            result.to_lowercase().contains("senior rust developer"),
            "got: {result}"
        );
    }

    #[test]
    fn prompt_act_as_role() {
        let prompt = "\
Act as a technical writing expert. Write clear API documentation for the following endpoints.

1. GET /users
2. POST /users
3. DELETE /users/:id";
        let result = try_extract_prompt(prompt).unwrap();
        assert!(
            result.to_lowercase().contains("technical writing expert"),
            "got: {result}"
        );
    }

    #[test]
    fn prompt_system_user_format() {
        let prompt = "\
System: You are a helpful coding assistant.
User: Write a Python function to parse CSV files.
Assistant: Sure, here is a function...";
        let result = try_extract_prompt(prompt).unwrap();
        assert!(!result.is_empty(), "should extract something from system prompt");
    }

    #[test]
    fn prompt_task_verb_extraction() {
        let prompt = "\
You are an AI assistant. Generate a comprehensive test suite for authentication middleware.

Requirements:
1. Test login flow
2. Test token refresh
3. Test unauthorized access";
        let result = try_extract_prompt(prompt).unwrap();
        // Should have role + task
        assert!(
            result.to_lowercase().contains("generate")
                || result.to_lowercase().contains("ai assistant"),
            "got: {result}"
        );
    }

    #[test]
    fn prompt_template_variables() {
        let prompt = "\
Write a {language} function that takes {{input}} and returns {{output}}.

Format as JSON. Include error handling.";
        let result = try_extract_prompt(prompt).unwrap();
        assert!(
            result.to_lowercase().contains("write"),
            "got: {result}"
        );
    }

    #[test]
    fn prompt_translate_subject() {
        let prompt = "\
Translate the following English marketing copy to French and Spanish.

1. Keep the brand tone consistent
2. Adapt idioms naturally
3. Preserve formatting";
        let result = try_extract_prompt(prompt).unwrap();
        assert!(
            result.to_lowercase().contains("translate"),
            "got: {result}"
        );
    }

    #[test]
    fn prompt_summarize_subject() {
        let prompt = "\
Summarize this research paper on quantum computing advances.

Respond in markdown with bullet points.";
        let result = try_extract_prompt(prompt).unwrap();
        assert!(
            result.to_lowercase().contains("summarize"),
            "got: {result}"
        );
    }

    #[test]
    fn prompt_not_detected_for_plain_text() {
        let text = "The quick brown fox jumps over the lazy dog. This is a normal paragraph.";
        assert!(try_extract_prompt(text).is_none());
    }

    #[test]
    fn prompt_output_format_scoring() {
        let prompt = "\
You are a data analyst. Analyze the sales data and identify trends.

Output as JSON with the following structure.";
        let result = try_extract_prompt(prompt).unwrap();
        assert!(!result.is_empty());
    }

    #[test]
    fn prompt_role_plus_task_combined() {
        let prompt = "\
You are a DevOps engineer. Create a CI/CD pipeline configuration for microservices.

1. Build stage
2. Test stage
3. Deploy stage";
        let result = try_extract_prompt(prompt).unwrap();
        let lower = result.to_lowercase();
        // Should combine role + task
        assert!(
            lower.contains("devops engineer") || lower.contains("create"),
            "got: {result}"
        );
    }

    #[test]
    fn prompt_numbered_list_scoring() {
        let prompt = "\
You are a teacher. Explain the following concepts clearly:

1. Recursion
2. Dynamic programming
3. Graph traversal
4. Greedy algorithms";
        let result = try_extract_prompt(prompt).unwrap();
        assert!(
            result.to_lowercase().contains("teacher"),
            "got: {result}"
        );
    }

    // ── YAKE fallback ───────────────────────────────────────────────────

    #[test]
    fn yake_extracts_keywords() {
        let text = "Rust is a systems programming language focused on safety and performance. \
                     The borrow checker ensures memory safety without garbage collection.";
        let result = extract_yake(text);
        assert!(result.is_some(), "YAKE should extract keywords");
    }

    #[test]
    fn yake_empty_returns_none() {
        assert!(extract_yake("").is_none());
        assert!(extract_yake("   ").is_none());
    }

    // ── Cascade integration ─────────────────────────────────────────────

    #[test]
    fn cascade_email_wins_over_yake() {
        let email = "\
From: test@example.com
To: dev@example.com
Subject: Database migration plan for Q4

Content about migration.";
        let result = extract_prose(email).unwrap();
        assert!(
            result.contains("Database migration plan"),
            "email should win, got: {result}"
        );
    }

    #[test]
    fn cascade_prompt_wins_over_yake() {
        let prompt = "\
You are a security auditor. Review the authentication module for vulnerabilities.

1. Check for SQL injection
2. Check for XSS
3. Check for CSRF";
        let result = extract_prose(prompt).unwrap();
        let lower = result.to_lowercase();
        assert!(
            lower.contains("security auditor") || lower.contains("review"),
            "prompt should win, got: {result}"
        );
    }

    #[test]
    fn cascade_falls_through_to_yake() {
        let text = "Distributed systems require careful consideration of network partitions, \
                     consistency models, and failure modes. The CAP theorem states that a \
                     distributed data store can only guarantee two of three properties.";
        let result = extract_prose(text);
        assert!(result.is_some(), "should fall through to YAKE");
    }

    #[test]
    fn cascade_empty_returns_none() {
        assert!(extract_prose("").is_none());
        assert!(extract_prose("   ").is_none());
    }

    // ── Tagged extraction ───────────────────────────────────────────────

    #[test]
    fn tagged_email_returns_email_kind() {
        let email = "\
From: test@example.com
To: dev@example.com
Subject: Database migration plan

Content about migration.";
        let en = extract_prose_tagged(email).unwrap();
        assert_eq!(en.kind, StemKind::Email);
        assert!(en.stem.contains("Database migration plan"));
    }

    #[test]
    fn tagged_prompt_returns_prompt_kind() {
        let prompt = "\
You are a security auditor. Review the authentication module for vulnerabilities.

1. Check for SQL injection
2. Check for XSS
3. Check for CSRF";
        let en = extract_prose_tagged(prompt).unwrap();
        assert_eq!(en.kind, StemKind::Prompt);
    }

    #[test]
    fn tagged_generic_returns_generic_kind() {
        let text = "Distributed systems require careful consideration of network partitions, \
                     consistency models, and failure modes. The CAP theorem states that a \
                     distributed data store can only guarantee two of three properties.";
        let en = extract_prose_tagged(text).unwrap();
        assert_eq!(en.kind, StemKind::Generic);
    }
}
