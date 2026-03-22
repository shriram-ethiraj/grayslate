use super::LanguageDefinition;
use regex::Regex;
use std::sync::LazyLock;

pub(crate) fn is_likely_yaml(trimmed: &str, _was_sliced: bool) -> bool {
    let lines: Vec<&str> = trimmed.lines().collect();
    let starts_with_sep = lines.first().map_or(false, |l| l.trim() == "---");

    // Bail out if content looks like Sass/SCSS
    let non_empty: Vec<&str> = lines
        .iter()
        .copied()
        .map(|l| l.trim())
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .collect();

    static SASS_VAR: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"^\s*\$[\w\-]+\s*:").unwrap());
    if non_empty.iter().any(|l| SASS_VAR.is_match(l)) {
        return false;
    }

    if non_empty.is_empty() {
        return false;
    }

    static YAML_KV: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"^\s*[a-zA-Z_][\w./\-]*\s*:(\s|$)").unwrap());
    static YAML_LIST: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^\s*\-\s+\S").unwrap());
    static CODE_PATTERNS: LazyLock<Vec<Regex>> = LazyLock::new(|| vec![
        Regex::new(r"^\s*(def|class|if|for|while|return|import|from|try|except|with|async|yield)\s").unwrap(),
        Regex::new(r"^\s*(function|const|let|var|if|for|while|return|import|export|switch|case)\s").unwrap(),
        Regex::new(r"^\s*(#include|int\s+main|typedef|struct)\s").unwrap(),
        Regex::new(r"^\s*(public|private|protected)\s+(class|static|void|int|String)").unwrap(),
        Regex::new(r"^\s*(func|package|type|defer|go)\s").unwrap(),
    ]);

    let mut yaml_lines = 0usize;
    let mut code_lines = 0usize;
    for line in &non_empty {
        if CODE_PATTERNS.iter().any(|p| p.is_match(line)) {
            code_lines += 1;
        } else if YAML_KV.is_match(line) || YAML_LIST.is_match(line) {
            yaml_lines += 1;
        }
    }

    if code_lines > yaml_lines {
        return false;
    }

    // Anti-signal: trailing commas — JS/TS object literal
    static COMMA_TRAILING: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r",\s*(//.*)?$").unwrap());
    let comma_count = non_empty
        .iter()
        .filter(|l| COMMA_TRAILING.is_match(l.trim()))
        .count();
    if comma_count as f64 / non_empty.len() as f64 > 0.3 {
        return false;
    }

    // Anti-signal: shell script patterns in YAML context
    static SHELL_GUARD: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"(?m)^\s*(echo\s|fi$|done$|esac$|export\s+\w+=)").unwrap()
    });
    let shell_count = non_empty.iter().filter(|l| SHELL_GUARD.is_match(l)).count();
    if shell_count > yaml_lines && shell_count >= 2 {
        return false;
    }

    // Strong YAML-specific positive signals (bonus points)
    let mut bonus = 0i32;

    static MULTILINE: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r":\s*[|>][-+]?\s*$").unwrap());
    if non_empty.iter().any(|l| MULTILINE.is_match(l)) {
        bonus += 2;
    }

    static ANCHORS: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"[&*]\w+").unwrap());
    if non_empty.iter().any(|l| ANCHORS.is_match(l)) {
        bonus += 1;
    }

    static TAGS: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"!!\w+").unwrap());
    if non_empty.iter().any(|l| TAGS.is_match(l)) {
        bonus += 2;
    }

    let yaml_ratio = yaml_lines as f64 / non_empty.len() as f64;
    if starts_with_sep && yaml_ratio > 0.3 {
        return true;
    }
    if bonus >= 2 && yaml_ratio > 0.3 {
        return true;
    }
    yaml_ratio > 0.5
}

pub fn definition() -> LanguageDefinition {
    LanguageDefinition {
        name: "yaml",
        extensions: &[".yaml", ".yml"],
        filenames: &[".editorconfig"],
        filename_patterns: &[],
        shebangs: &[],
        structural_priority: Some(120),
        structural_detect: Some(is_likely_yaml),
        patterns: &[],
        anti_patterns: &[],
        uses_hash_comments: true,
        keywords: &[],
        builtins: &[],
        family: None,
        exclusive_patterns: &[],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kubernetes_deployment_detected() {
        let src = "\
apiVersion: apps/v1
kind: Deployment
metadata:
  name: my-app
  labels:
    app.kubernetes.io/name: my-app
spec:
  replicas: 3
  selector:
    matchLabels:
      app: my-app
  template:
    spec:
      containers:
        - name: app-container
          image: my-app:latest
          ports:
            - containerPort: 8080
";
        assert!(is_likely_yaml(src, false), "k8s deployment should be YAML");
    }

    #[test]
    fn block_mapping_keys_counted() {
        // Keys with no inline value (colon at end-of-line) must be counted.
        let src = "spec:\n  template:\n    metadata:\n      labels:\n        app: web\n";
        assert!(is_likely_yaml(src, false), "block keys should be YAML");
    }

    #[test]
    fn kubernetes_label_slash_in_key() {
        // Keys with '/' (Kubernetes labels) must be matched.
        let src = "metadata:\n  labels:\n    app.kubernetes.io/name: my-app\n    app.kubernetes.io/version: \"1.0\"\n";
        assert!(is_likely_yaml(src, false), "slash in key should be YAML");
    }

    #[test]
    fn sass_not_detected_as_yaml() {
        let src = "$primary: #333;\n$font-size: 16px;\nbody {\n  color: $primary;\n}";
        assert!(!is_likely_yaml(src, false), "Sass should NOT be YAML");
    }

    #[test]
    fn simple_config_detected() {
        let src = "name: my-service\nport: 8080\ndebug: true\n";
        assert!(is_likely_yaml(src, false), "simple config should be YAML");
    }
}
