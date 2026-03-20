/// Phase 1 — File extension and filename detection.
///
/// Maps file extensions and known filenames to language IDs.
/// This is the fastest and most deterministic phase.
use std::path::Path;

use super::SUPPORTED_LANGUAGES;

/// Extension → language ID (lowercase, no dot prefix in storage; lookup trims dot).
static EXTENSION_MAP: &[(&str, &str)] = &[
    // ── Data formats ─────────────────────────────────────────
    (".json", "json"),
    (".jsonc", "json"),
    (".json5", "json"),
    (".geojson", "json"),
    (".webmanifest", "json"),
    (".har", "json"),
    (".csv", "csv"),
    (".tsv", "csv"),
    (".xml", "xml"),
    (".svg", "xml"),
    (".plist", "xml"),
    (".xsl", "xml"),
    (".xslt", "xml"),
    (".xsd", "xml"),
    (".wsdl", "xml"),
    (".rss", "xml"),
    (".atom", "xml"),
    (".xaml", "xml"),
    (".csproj", "xml"),
    (".fsproj", "xml"),
    (".vcxproj", "xml"),
    // ── Config ───────────────────────────────────────────────
    (".yaml", "yaml"),
    (".yml", "yaml"),
    (".toml", "toml"),
    (".ini", "text"),
    (".cfg", "text"),
    (".env", "text"),
    // ── Markup ───────────────────────────────────────────────
    (".html", "html"),
    (".htm", "html"),
    (".xhtml", "html"),
    (".svelte", "svelte"),
    (".vue", "vue"),
    (".md", "markdown"),
    (".markdown", "markdown"),
    (".mdx", "markdown"),
    // ── Web languages ────────────────────────────────────────
    (".js", "javascript"),
    (".mjs", "javascript"),
    (".cjs", "javascript"),
    (".jsx", "javascript"),
    (".ts", "typescript"),
    (".tsx", "typescript"),
    (".mts", "typescript"),
    (".cts", "typescript"),
    (".css", "css"),
    (".less", "css"),
    (".scss", "scss"),
    (".sass", "sass"),
    // ── Systems / compiled ───────────────────────────────────
    (".py", "python"),
    (".pyi", "python"),
    (".pyw", "python"),
    (".c", "c"),
    (".h", "c"),
    (".cpp", "cpp"),
    (".cxx", "cpp"),
    (".cc", "cpp"),
    (".hpp", "cpp"),
    (".hxx", "cpp"),
    (".hh", "cpp"),
    (".java", "java"),
    (".go", "go"),
    (".rs", "rust"),
    (".rb", "ruby"),
    (".php", "php"),
    (".php3", "php"),
    (".php4", "php"),
    (".php5", "php"),
    (".php7", "php"),
    (".phtml", "php"),
    (".swift", "swift"),
    (".kt", "kotlin"),
    (".kts", "kotlin"),
    (".cs", "csharp"),
    (".scala", "scala"),
    (".dart", "dart"),
    (".m", "objectivec"),
    (".mm", "objectivecpp"),
    (".lua", "text"),
    (".pl", "text"),
    (".pm", "text"),
    // ── Functional ───────────────────────────────────────────
    (".clj", "clojure"),
    (".cljs", "clojure"),
    (".cljc", "clojure"),
    (".edn", "clojure"),
    // ── Shell ────────────────────────────────────────────────
    (".sh", "shell"),
    (".bash", "shell"),
    (".zsh", "shell"),
    (".fish", "shell"),
    (".ksh", "shell"),
    (".ps1", "powershell"),
    (".psd1", "powershell"),
    (".psm1", "powershell"),
    (".bat", "text"),
    (".cmd", "text"),
    // ── Dockerfile (explicit extension) ──────────────────────
    (".dockerfile", "dockerfile"),
    // ── SQL ──────────────────────────────────────────────────
    (".sql", "sql"),
    // ── Template languages ──────────────────────────────────
    (".j2", "jinja"),
    (".jinja", "jinja"),
    (".jinja2", "jinja"),
];

/// Full filenames (lowercased) → language ID.
static FILENAME_MAP: &[(&str, &str)] = &[
    ("dockerfile", "dockerfile"),
    ("makefile", "shell"),
    ("gnumakefile", "shell"),
    (".bashrc", "shell"),
    (".bash_profile", "shell"),
    (".bash_aliases", "shell"),
    (".zshrc", "shell"),
    (".zprofile", "shell"),
    (".profile", "shell"),
    (".editorconfig", "yaml"),
    (".gitignore", "text"),
    (".gitattributes", "text"),
    (".env", "text"),
    (".env.local", "text"),
    ("jenkinsfile", "text"),
    ("vagrantfile", "text"),
    ("cargo.toml", "toml"),
    ("cargo.lock", "toml"),
    ("deps.edn", "clojure"),
    ("gemfile", "ruby"),
    ("rakefile", "ruby"),
];

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
    for &(name, lang) in FILENAME_MAP {
        if base == name {
            return Some(ensure_supported(lang));
        }
    }

    // Regex filename patterns: nginx*.conf
    if is_nginx_conf(base) {
        return Some(ensure_supported("nginx"));
    }

    // Extension match
    let path = Path::new(base);
    let ext_str = path.extension().and_then(|e| e.to_str())?;
    let dot_ext = format!(".{}", ext_str);
    for &(ext, lang) in EXTENSION_MAP {
        if dot_ext == ext {
            return Some(ensure_supported(lang));
        }
    }

    None
}

fn is_nginx_conf(base: &str) -> bool {
    base.starts_with("nginx") && base.ends_with(".conf")
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
