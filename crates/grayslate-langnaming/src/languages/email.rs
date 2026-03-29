use super::{NamingDefinition, Extractor};

/// Email naming delegates to the prose extractor which already handles
/// email-specific stem extraction (subject lines, greeting-based fallback).
pub fn definition() -> NamingDefinition {
    NamingDefinition {
        name: "email",
        extension: "txt",
        extract: Extractor::Custom(|content| crate::prose::extract_prose(content)),
    }
}

#[cfg(test)]
mod tests {
    use crate::suggest_stem;

    fn name(src: &str) -> Option<String> {
        suggest_stem(src, "email")
    }

    #[test]
    fn email_subject_with_suffix() {
        let src = "Subject: Meeting Notes Q1\nFrom: alice@example.com\nTo: team@example.com\n\nHi team,\nPlease review the notes.\nBest regards,\nAlice";
        let n = name(src).unwrap();
        assert!(n.contains("meeting-notes"), "subject extracted: {n}");
        assert!(n.ends_with("-email"), "email suffix: {n}");
    }

    #[test]
    fn email_greeting_fallback_with_suffix() {
        let src = "Dear Professor,\n\nI wanted to follow up on the research proposal.\n\nThank you,\nStudent";
        let n = name(src).unwrap();
        assert!(n.ends_with("-email"), "email suffix: {n}");
    }
}
