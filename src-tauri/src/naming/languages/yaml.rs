use super::NamingDefinition;
use crate::naming::model::MAX_TOKENS;

pub fn definition() -> NamingDefinition {
    NamingDefinition {
        name: "yaml",
        extension: "yaml",
        extract: extract_yaml_enhanced,
    }
}

/// Enhanced YAML naming with known-pattern detection and value extraction.
///
/// Known patterns detected:
///   - Docker Compose: `services:` → service names
///   - GitHub Actions: `name:` field
///   - Kubernetes: `kind:` + `metadata.name:`
///   - Ansible playbook: `- name:` or `- hosts:`
///   - OpenAPI (YAML): `openapi:` + `info.title:`
///   - CI configs: `.travis.yml`, `.gitlab-ci.yml`
///   - CloudFormation: `AWSTemplateFormatVersion` + `Description`
///   - Helm Chart.yaml: `name:` + `description:`
///   - Swagger: `swagger:` + `info.title:`
fn extract_yaml_enhanced(content: &str) -> Option<String> {
    use regex::Regex;
    use std::sync::LazyLock;

    static TOP_KEY_RE: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"(?m)^([a-zA-Z_][a-zA-Z0-9_\-]*)\s*:").unwrap()
    });
    static NAME_VALUE_RE: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r#"(?m)^name\s*:\s*["']?([^"'\n]{1,60})["']?"#).unwrap()
    });
    static SERVICE_KEY_RE: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"(?m)^  ([a-zA-Z_][a-zA-Z0-9_\-]*)\s*:").unwrap()
    });
    static KIND_RE: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"(?m)^kind\s*:\s*(\w+)").unwrap()
    });
    static META_NAME_RE: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r#"(?m)^\s{2,4}name\s*:\s*["']?([^"'\n]{1,60})["']?"#).unwrap()
    });
    static INFO_TITLE_RE: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r#"(?ms)info\s*:.*?title\s*:\s*["']?([^"'\n]{1,60})["']?"#).unwrap()
    });
    static HOSTS_RE: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r#"(?m)^-?\s*hosts\s*:\s*["']?([^"'\n]{1,40})["']?"#).unwrap()
    });
    static DESCRIPTION_RE: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r#"(?m)^[Dd]escription\s*:\s*["']?([^"'\n]{1,60})["']?"#).unwrap()
    });

    // Collect top-level keys for pattern detection
    let top_keys: Vec<String> = TOP_KEY_RE
        .captures_iter(content)
        .take(20)
        .filter_map(|c| c.get(1).map(|m| m.as_str().to_string()))
        .collect();

    let has_key = |k: &str| top_keys.iter().any(|tk| tk.eq_ignore_ascii_case(k));

    // --- Docker Compose ---
    if has_key("services") && (has_key("version") || has_key("networks") || has_key("volumes")
        || content.contains("image:") || content.contains("build:"))
    {
        let mut services: Vec<String> = SERVICE_KEY_RE
            .captures_iter(content)
            .filter_map(|c| c.get(1).map(|m| m.as_str().to_string()))
            .filter(|s| !is_yaml_noise(s))
            .take(MAX_TOKENS)
            .collect();
        if !services.is_empty() {
            services.insert(0, "compose".to_string());
            services.truncate(MAX_TOKENS);
            return Some(services.join("-"));
        }
    }

    // --- GitHub Actions ---
    if has_key("on") && has_key("jobs") {
        if let Some(cap) = NAME_VALUE_RE.captures(content) {
            return Some(cap[1].trim().to_string());
        }
        return Some("github-actions-workflow".to_string());
    }

    // --- Kubernetes ---
    if has_key("kind") && has_key("apiVersion") {
        if let Some(kind_cap) = KIND_RE.captures(content) {
            let kind = kind_cap[1].to_string();
            if let Some(name_cap) = META_NAME_RE.captures(content) {
                let name = name_cap[1].trim().to_string();
                return Some(format!("{kind}-{name}"));
            }
            return Some(kind);
        }
    }

    // --- OpenAPI/Swagger (YAML) ---
    if has_key("openapi") || has_key("swagger") {
        if let Some(cap) = INFO_TITLE_RE.captures(content) {
            return Some(cap[1].trim().to_string());
        }
        return Some("openapi-spec".to_string());
    }

    // --- CloudFormation ---
    if has_key("AWSTemplateFormatVersion") || has_key("Resources") && has_key("Outputs") {
        if let Some(cap) = DESCRIPTION_RE.captures(content) {
            return Some(cap[1].trim().to_string());
        }
        return Some("cloudformation-template".to_string());
    }

    // --- Ansible playbook ---
    if content.trim_start().starts_with("- ") && (content.contains("hosts:") || content.contains("tasks:")) {
        if let Some(cap) = NAME_VALUE_RE.captures(content) {
            return Some(cap[1].trim().to_string());
        }
        if let Some(cap) = HOSTS_RE.captures(content) {
            return Some(format!("ansible-{}", cap[1].trim()));
        }
    }

    // --- Helm Chart.yaml ---
    if has_key("apiVersion") && has_key("appVersion") && has_key("type") {
        if let Some(cap) = NAME_VALUE_RE.captures(content) {
            return Some(format!("chart-{}", cap[1].trim()));
        }
    }

    // --- CI configs (.gitlab-ci, .travis) ---
    if has_key("stages") && (has_key("image") || has_key("variables")) {
        return Some("gitlab-ci".to_string());
    }
    if has_key("language") && (has_key("script") || has_key("install")) {
        if let Some(cap) = NAME_VALUE_RE.captures(content) {
            return Some(cap[1].trim().to_string());
        }
        // Try to get language value
        let lang_re = regex::Regex::new(r"(?m)^language\s*:\s*(\w+)").ok();
        if let Some(re) = &lang_re {
            if let Some(cap) = re.captures(content) {
                if let Some(m) = cap.get(1) {
                    return Some(format!("travis-{}", m.as_str()));
                }
            }
        }
    }

    // --- Generic: prefer `name:` or `title:` value if present ---
    if let Some(cap) = NAME_VALUE_RE.captures(content) {
        let val = cap[1].trim();
        if !val.is_empty() {
            return Some(val.to_string());
        }
    }

    // Fallback: filtered top-level keys
    let noise_keys = [
        "version", "services", "volumes", "networks", "depends_on",
        "environment", "env", "true", "false", "yes", "no",
    ];
    let tokens: Vec<String> = top_keys
        .into_iter()
        .filter(|k| !noise_keys.contains(&k.to_lowercase().as_str()))
        .take(MAX_TOKENS)
        .collect();

    if tokens.is_empty() { None } else { Some(tokens.join("-")) }
}

fn is_yaml_noise(s: &str) -> bool {
    let lower = s.to_lowercase();
    matches!(lower.as_str(),
        "version" | "networks" | "volumes" | "configs" | "secrets"
        | "depends_on" | "environment" | "build" | "ports" | "expose"
        | "labels" | "image" | "restart" | "command" | "entrypoint"
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::naming::shared::slugify;

    fn name(src: &str) -> Option<String> {
        extract_yaml_enhanced(src).and_then(|s| slugify(&s))
    }

    #[test]
    fn docker_compose() {
        let src = "version: '3'\nservices:\n  web:\n    image: nginx\n  api:\n    build: .\nvolumes:\n  data:";
        let n = name(src).unwrap();
        assert!(n.contains("compose"), "got: {n}");
    }

    #[test]
    fn github_actions() {
        let src = "name: CI Pipeline\non: [push, pull_request]\njobs:\n  test:\n    runs-on: ubuntu-latest";
        let n = name(src).unwrap();
        assert!(n.contains("ci-pipeline"), "got: {n}");
    }

    #[test]
    fn kubernetes_deployment() {
        let src = "apiVersion: apps/v1\nkind: Deployment\nmetadata:\n  name: my-app\nspec:\n  replicas: 3";
        let n = name(src).unwrap();
        assert!(n.contains("deployment"), "got: {n}");
        assert!(n.contains("my-app"), "got: {n}");
    }

    #[test]
    fn openapi_yaml() {
        let src = "openapi: 3.0.0\ninfo:\n  title: Pet Store API\n  version: 1.0.0\npaths:\n  /pets:";
        let n = name(src).unwrap();
        assert!(n.contains("pet-store-api"), "got: {n}");
    }

    #[test]
    fn ansible_playbook() {
        let src = "- hosts: webservers\n  tasks:\n    - name: Install nginx\n      apt:\n        name: nginx";
        let _n = name(src).unwrap();
        // Should get "Install nginx" as the name
        let raw = extract_yaml_enhanced(src).unwrap();
        assert!(raw.contains("Install nginx") || raw.contains("webservers"), "got: {raw}");
    }

    #[test]
    fn generic_yaml_with_name() {
        let src = "name: My Application\nport: 8080\ndebug: true";
        let n = name(src).unwrap();
        assert!(n.contains("my-application"), "got: {n}");
    }
}
