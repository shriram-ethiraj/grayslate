/// Phase 1 — File extension and filename detection.
///
/// Maps file extensions and known filenames to language IDs.
/// This is the fastest and most deterministic phase.
///
/// All data is auto-derived from per-language definitions in `languages/`.
use std::path::Path;

use super::languages::{EXTENSION_MAP, FILENAME_MAP, FILENAME_PATTERNS, SUPPORTED_LANGUAGES};

/// Detect language from a filename or path.
///
/// Checks (in order):
///   1. Full filename match (e.g. "Dockerfile", ".bashrc")
///   2. Regex filename patterns (e.g. nginx*.conf)
///   3. File extension match
pub fn detect_by_filename(filename: &str) -> Option<&'static str> {
    let lower = filename.to_lowercase();

    // Extract the base filename (strip path separators)
    let base = lower
        .rsplit(|c| c == '/' || c == '\\')
        .next()
        .unwrap_or(&lower);

    // Full-filename match
    for &(name, lang) in FILENAME_MAP.iter() {
        if base == name {
            return Some(ensure_supported(lang));
        }
    }

    // Regex filename patterns
    for (re, lang) in FILENAME_PATTERNS.iter() {
        if re.is_match(base) {
            return Some(ensure_supported(lang));
        }
    }

    // Extension match
    let path = Path::new(base);
    let ext_str = path.extension().and_then(|e| e.to_str())?;
    // Skip the leading '.' in each map entry rather than allocating a new String.
    for &(ext, lang) in EXTENSION_MAP.iter() {
        if &ext[1..] == ext_str {
            return Some(ensure_supported(lang));
        }
    }

    None
}

fn ensure_supported(lang: &str) -> &str {
    if SUPPORTED_LANGUAGES.contains(&lang) {
        lang
    } else {
        "text"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extension_json() {
        assert_eq!(detect_by_filename("data.json"), Some("json"));
        assert_eq!(detect_by_filename("settings.jsonc"), Some("json"));
    }

    #[test]
    fn extension_typescript() {
        assert_eq!(detect_by_filename("app.ts"), Some("typescript"));
        assert_eq!(detect_by_filename("Component.tsx"), Some("typescript"));
    }

    #[test]
    fn filename_dockerfile() {
        assert_eq!(detect_by_filename("Dockerfile"), Some("dockerfile"));
        assert_eq!(detect_by_filename("dockerfile"), Some("dockerfile"));
    }

    #[test]
    fn filename_bashrc() {
        assert_eq!(detect_by_filename(".bashrc"), Some("shell"));
    }

    #[test]
    fn filename_cargo_toml() {
        assert_eq!(detect_by_filename("Cargo.toml"), Some("toml"));
    }

    #[test]
    fn nginx_conf_pattern() {
        assert_eq!(detect_by_filename("nginx.conf"), Some("nginx"));
        assert_eq!(detect_by_filename("nginx-site.conf"), Some("nginx"));
    }

    #[test]
    fn path_extraction() {
        assert_eq!(detect_by_filename("/home/user/test.py"), Some("python"));
        assert_eq!(detect_by_filename("C:\\Users\\app.rs"), Some("rust"));
    }

    #[test]
    fn unknown_extension() {
        assert_eq!(detect_by_filename("data.xyz"), None);
    }
}
