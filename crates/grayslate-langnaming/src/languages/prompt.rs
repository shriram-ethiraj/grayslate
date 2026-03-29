use super::{NamingDefinition, Extractor};

/// Prompt naming delegates to the prose extractor which already handles
/// prompt-specific stem extraction (role + task verb parsing).
pub fn definition() -> NamingDefinition {
    NamingDefinition {
        name: "prompt",
        extension: "txt",
        extract: Extractor::Custom(|content| crate::prose::extract_prose(content)),
    }
}

#[cfg(test)]
mod tests {
    use crate::suggest_stem;

    fn name(src: &str) -> Option<String> {
        suggest_stem(src, "prompt")
    }

    #[test]
    fn prompt_role_with_suffix() {
        let src = "You are a senior code reviewer.\nReview the following pull request and provide feedback on code quality.";
        let n = name(src).unwrap();
        assert!(n.ends_with("-prompt"), "prompt suffix: {n}");
    }

    #[test]
    fn prompt_task_verb_with_suffix() {
        let src = "Summarize the following article in 3 bullet points.\n\nArticle: Machine learning has transformed...";
        let n = name(src).unwrap();
        assert!(n.ends_with("-prompt"), "prompt suffix: {n}");
    }
}
