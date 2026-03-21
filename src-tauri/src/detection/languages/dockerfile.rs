use super::{wp, LanguageDefinition};
use regex::Regex;
use std::sync::LazyLock;

pub(crate) fn is_likely_dockerfile(trimmed: &str, _was_sliced: bool) -> bool {
    // `# syntax=docker/...` is a Dockerfile-only directive — definitive signal.
    static SYNTAX_DIRECTIVE: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"(?m)^#\s*syntax\s*=\s*docker/").unwrap());
    if SYNTAX_DIRECTIVE.is_match(trimmed) {
        return true;
    }

    // Strip comment lines and blanks for instruction-level checks.
    let lines: Vec<&str> = trimmed
        .lines()
        .map(|l| l.trim())
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .collect();

    if lines.is_empty() {
        return false;
    }

    static FIRST_LINE: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"^(?i)(FROM|ARG)\s").unwrap());
    if !FIRST_LINE.is_match(lines[0]) {
        return false;
    }

    if lines[0].to_ascii_uppercase().starts_with("FROM") {
        // Case-insensitive; allow hyphenated stage names (e.g. `as dev-envs`).
        static DOCKER_FROM: LazyLock<Regex> = LazyLock::new(|| {
            Regex::new(
                r"(?i)^FROM\s+(--[\w-]+=\S+\s+)?[\w.\-/]+(:\S+)?(\s+AS\s+[\w-]+)?$",
            )
            .unwrap()
        });
        if !DOCKER_FROM.is_match(lines[0]) {
            return false;
        }
    }

    static INSTRUCTION: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(
            r"(?i)^(FROM|RUN|CMD|LABEL|MAINTAINER|EXPOSE|ENV|ADD|COPY|ENTRYPOINT|VOLUME|USER|WORKDIR|ARG|ONBUILD|STOPSIGNAL|HEALTHCHECK|SHELL)\s",
        )
        .unwrap()
    });
    let match_count = lines.iter().filter(|l| INSTRUCTION.is_match(l)).count();
    match_count >= 2
}

pub fn definition() -> LanguageDefinition {
    LanguageDefinition {
        name: "dockerfile",
        extensions: &[".dockerfile"],
        filenames: &["dockerfile"],
        filename_patterns: &[r"(?i)^dockerfile(\.[a-z0-9_-]+)?$"],
        shebangs: &[],
        structural_priority: Some(60),
        structural_detect: Some(is_likely_dockerfile),
        patterns: &[
            // Instruction lines — uppercase by Docker convention.
            wp!(r"(?m)^FROM\s+\S", 4),
            wp!(
                r"(?m)^(RUN|CMD|ENTRYPOINT|COPY|ADD|WORKDIR|EXPOSE|ENV|ARG|LABEL|VOLUME|USER|SHELL|HEALTHCHECK|STOPSIGNAL|ONBUILD|MAINTAINER)\s",
                3
            ),
            // `# syntax=` / `# escape=` directives are Dockerfile-only.
            wp!(r"(?m)^#\s*(syntax|escape)\s*=", 5),
        ],
        anti_patterns: &[
            // HTML close tags are illegal in Dockerfiles (from highlight.js).
            wp!(r"</", -5),
        ],
        uses_hash_comments: true,
        // All Dockerfile instructions are effectively keywords.
        keywords: &[
            "from", "run", "cmd", "label", "maintainer", "expose", "env",
            "add", "copy", "entrypoint", "volume", "user", "workdir", "arg",
            "onbuild", "stopsignal", "healthcheck", "shell",
        ],
        builtins: &[],
        illegal: Some(r"</"),
        extends: None,
    }
}
