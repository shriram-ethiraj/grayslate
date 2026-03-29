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
        // No keywords — common English words cause false positives
        // (same trap as CMD's "if"/"for"/"do"/"set" problem).
        keywords: &[],
        builtins: &[],
        // ── Family-gated fields ──────────────────────────────
        content_families: &[ContentFamily::Prose],
        anchors: &[
            wp!(r"(?mi)^(Dear|Hi|Hello|Hey|Good\s+(?:morning|afternoon|evening))\s+[A-Z]", 5),
            wp!(r"(?mi)^(Hi|Hello|Hey)\s*,\s*$", 4),
            wp!(r"(?mi)^\s*(Subject|From|To|CC|BCC)\s*:", 5),
            wp!(r"(?mi)^(Best\s+regards|Kind\s+regards|Warm\s+regards|Regards|Thanks|Thank\s+you|Sincerely|Cheers|Best|Many\s+thanks)\s*[,.]?\s*$", 4),
            wp!(r"(?mi)^On\s+.+\s+wrote\s*:", 4),
        ],
        hints: &[
            wp!(r"(?mi)^\s*(Date|Sent)\s*:", 3),
            wp!(r"(?mi)^(Re|Fwd|FW|Fw)\s*:", 3),
            wp!(r"(?mi)(let\s+me\s+know|please\s+(?:advise|confirm|reply)|looking\s+forward)", 2),
            wp!(r"(?mi)^(Hi|Hello|Hey)\s+(there|all|everyone|team|folks)\b", 2),
            wp!(r"(?m)^>+\s", 2),
            wp!(r"(?m)^--\s*$", 2),
        ],
        disqualifiers: &[
            wp!(r"(?m)^\s*(import|export)\s+\w", -5),
            wp!(r"(?m)^\s*(const|let|var|function|class)\s", -4),
            wp!(r"(?m)^\s*#!\s*/", -5),
        ],
    }
}
