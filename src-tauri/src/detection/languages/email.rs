use super::{wp, LanguageDefinition};
use super::ContentFamily;

pub fn definition() -> LanguageDefinition {
    LanguageDefinition {
        name: "email",
        extensions: &[".eml", ".mbox"],
        filenames: &[],
        filename_patterns: &[],
        shebangs: &[],
        structural_priority: None,
        structural_detect: None,
        patterns: &[
            // ── RFC-style headers ────────────────────────────────────────
            // Subject/From/To/Date/CC/BCC at line start — strongest signal
            wp!(r"(?mi)^\s*(Subject|From|To|Date|CC|BCC)\s*:", 5),
            // Additional email headers
            wp!(r"(?mi)^\s*(Reply-To|In-Reply-To|Message-ID|References|Delivered-To|Return-Path|Received|MIME-Version|Content-Type|X-Mailer)\s*:", 4),
            // Sent/Received/Importance — Outlook-style metadata
            wp!(r"(?mi)^\s*(Sent|Received|Importance|Priority|Sensitivity)\s*:", 3),

            // ── Greeting + closing combo ─────────────────────────────────
            // Greeting at line start: "Hi Team", "Hello John", "Dear Sir"
            wp!(r"(?mi)^(Dear|Hi|Hello|Hey|Good\s+(?:morning|afternoon|evening))\s+[A-Z]", 4),
            // Extended greetings: "Hi there", "Hi all", "Hello everyone"
            wp!(r"(?mi)^(Hi|Hello|Hey)\s+(there|all|everyone|team|folks)\b", 3),
            // Closing line
            wp!(r"(?mi)^(Best\s+regards|Kind\s+regards|Warm\s+regards|Regards|Thanks|Thank\s+you|Sincerely|Cheers|Best|Many\s+thanks|Thanks\s+in\s+advance)\s*[,.]?\s*$", 4),

            // ── Reply / forward markers ──────────────────────────────────
            // Re:/Fwd:/FW: prefix lines
            wp!(r"(?mi)^(Re|Fwd|FW|Fw)\s*:", 3),
            // Quoted text lines — email thread replies
            wp!(r"(?m)^>+\s", 2),
            // "On <date>, <person> wrote:" attribution line
            wp!(r"(?mi)^On\s+.+\s+wrote\s*:", 4),
            // "From: ... Sent: ... To: ... Subject:" Outlook inline forward block
            wp!(r"(?mi)^-{3,}\s*(Original\s+Message|Forwarded\s+message)", 3),

            // ── Sign-off patterns ────────────────────────────────────────
            // Signature separator
            wp!(r"(?m)^--\s*$", 2),
            // "Let me know", "Please advise", "Looking forward" — common email phrases
            wp!(r"(?mi)(let\s+me\s+know|please\s+(?:advise|let\s+me\s+know|confirm|reply)|looking\s+forward|get\s+back\s+to\s+(?:you|me))", 2),
        ],
        anti_patterns: &[
            // Code signals — strongly rule out email
            wp!(r"(?m)^\s*(import|export)\s+", -4),
            wp!(r"(?m)^\s*(const|let|var)\s+\w+\s*[=:]", -4),
            wp!(r"(?m)^\s*function\s+\w*\s*\(", -4),
            wp!(r"(?m)^\s*(class|interface|type|enum)\s+\w+", -4),
            wp!(r"(?m)^\s*def\s+\w+\s*\(", -4),
            wp!(r#"(?m)^\s*#include\s*[<"]"#, -4),
            wp!(r"(?m);\s*$", -2),
            // Script signals
            wp!(r"(?mi)^\s*@echo\s+(off|on)", -5),
            wp!(r"\$\{?[A-Za-z_]\w*\}?", -3),
            wp!(r"(?m)^\s*#!\s*/", -5),
            // Structural data
            wp!(r#"(?m)^\s*\{"#, -3),
            wp!(r"(?m)^\s*<[a-zA-Z!?]", -3),
        ],
        uses_hash_comments: false,
        // No keywords — common English words cause false positives
        // (same trap as CMD's "if"/"for"/"do"/"set" problem).
        keywords: &[],
        builtins: &[],
        family: Some("prose"),
        exclusive_patterns: &[
            // Prose greetings penalize code/script languages
            wp!(r"(?mi)^(Dear|Hi|Hello|Hey)\s+[A-Z]", 3),
            // RFC headers are exclusive to email
            wp!(r"(?mi)^\s*(Subject|From|To|CC|BCC)\s*:", 3),
        ],
        // ── Family-gated fields ──────────────────────────────
        content_families: &[ContentFamily::Prose],
        anchors: &[
            // Greeting + personal name
            wp!(r"(?mi)^(Dear|Hi|Hello|Hey|Good\s+(?:morning|afternoon|evening))\s+[A-Z]", 5),
            // Greeting alone on line: "Hi," or "Hello," — very common casual email
            wp!(r"(?mi)^(Hi|Hello|Hey)\s*,\s*$", 4),
            // RFC headers
            wp!(r"(?mi)^\s*(Subject|From|To|CC|BCC)\s*:", 5),
            // Signature / closing lines
            wp!(r"(?mi)^(Best\s+regards|Kind\s+regards|Warm\s+regards|Regards|Thanks|Thank\s+you|Sincerely|Cheers|Best|Many\s+thanks)\s*[,.]?\s*$", 4),
        ],
        hints: &[
            // Date line patterns
            wp!(r"(?mi)^\s*(Date|Sent)\s*:", 3),
            // Forwarded / reply markers
            wp!(r"(?mi)^(Re|Fwd|FW|Fw)\s*:", 3),
            wp!(r"(?mi)^On\s+.+\s+wrote\s*:", 3),
            // Common email phrases
            wp!(r"(?mi)(let\s+me\s+know|please\s+(?:advise|confirm|reply)|looking\s+forward)", 2),
            // Group greetings: Hi all, Hi team, Hello everyone
            wp!(r"(?mi)^(Hi|Hello|Hey)\s+(there|all|everyone|team|folks)\b", 2),
        ],
        rivals: &["prompt"],
        differentiators: &[
            // RFC headers distinguish from prompt
            wp!(r"(?mi)^\s*(Subject|From|To|CC|BCC)\s*:", 5),
            // Greeting + closing pair
            wp!(r"(?mi)^(Dear|Hi|Hello|Hey)\s+[A-Z]", 4),
            // Personal address / sign-off
            wp!(r"(?mi)^(Best\s+regards|Kind\s+regards|Regards|Sincerely|Cheers)\s*[,.]?\s*$", 4),
        ],
        disqualifiers: &[],
    }
}
