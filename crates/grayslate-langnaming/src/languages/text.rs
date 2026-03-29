use super::{NamingDefinition, Extractor};

/// Catch-all definition for unrecognised languages.
/// Falls back to the prose extractor (email → prompt → YAKE keywords).
pub fn definition() -> NamingDefinition {
    NamingDefinition {
        name: "text",
        extension: "txt",
        extract: Extractor::Custom(|content| crate::prose::extract_prose(content)),
    }
}

#[cfg(test)]
mod tests {
    use crate::suggest_stem;

    fn name(src: &str) -> Option<String> {
        suggest_stem(src, "text")
    }

    #[test]
    fn text_email_auto_detected() {
        let src = "Subject: Budget Review\nFrom: cfo@corp.com\n\nHi team,\nPlease review the Q2 budget.\nThanks,\nCFO";
        let n = name(src).unwrap();
        assert!(n.ends_with("-email"), "text auto-detects email: {n}");
    }

    #[test]
    fn text_plain_prose_no_suffix() {
        let src = "The quick brown fox jumps over the lazy dog. This sentence contains every letter of the alphabet and is commonly used for typography testing.";
        let n = name(src);
        // Generic prose — no -email or -prompt suffix
        if let Some(ref s) = n {
            assert!(!s.ends_with("-email"), "not email: {s}");
            assert!(!s.ends_with("-prompt"), "not prompt: {s}");
        }
    }
}
