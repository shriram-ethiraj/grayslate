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
    // Ansible: play-level `name:` only. Allows 0-2 leading spaces and an optional `- ` prefix.
    // This matches play attributes like `  name: My Play` or `- name: My Play`,
    // but NOT deeply nested task args like `        name: nginx` (8-space indent).
    static ANSIBLE_PLAY_NAME_RE: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r#"(?m)^\s{0,2}(?:-\s+)?name\s*:\s*["']?([^"'\n]{1,60})["']?"#).unwrap()
    });
    // GitLab CI stages list: `stages: [test, build, deploy]` or multiline list
    static STAGES_INLINE_RE: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"(?m)^stages\s*:\s*\[([^\]]+)\]").unwrap()
    });
    static STAGES_ITEM_RE: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"(?m)^\s+-\s+(\w[\w\-]*)").unwrap()
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
        let services: Vec<String> = SERVICE_KEY_RE
            .captures_iter(content)
            .filter_map(|c| c.get(1).map(|m| m.as_str().to_string()))
            .filter(|s| !is_yaml_noise(s))
            .take(MAX_TOKENS)
            .collect();
        return if services.is_empty() {
            Some("compose".to_string())
        } else {
            Some(format!("{}-compose", services.join("-")))
        };
    }

    // --- GitHub Actions ---
    // Format: "{workflow-name}-workflow" so the type is always clear.
    if has_key("on") && has_key("jobs") {
        if let Some(cap) = NAME_VALUE_RE.captures(content) {
            return Some(format!("{}-workflow", cap[1].trim()));
        }
        return Some("github-actions-workflow".to_string());
    }

    // --- Kubernetes ---
    // Format: "{Kind}-{resource-name}" — kind already carries the type.
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
    // Format: "{api-title}-openapi" or "{api-title}-swagger".
    if has_key("openapi") || has_key("swagger") {
        let spec_type = if has_key("swagger") { "swagger" } else { "openapi" };
        if let Some(cap) = INFO_TITLE_RE.captures(content) {
            return Some(format!("{}-{spec_type}", cap[1].trim()));
        }
        return Some(format!("{spec_type}-spec"));
    }

    // --- CloudFormation ---
    // Format: "{description}-cfn"
    if has_key("AWSTemplateFormatVersion") || (has_key("Resources") && has_key("Outputs")) {
        if let Some(cap) = DESCRIPTION_RE.captures(content) {
            return Some(format!("{}-cfn", cap[1].trim()));
        }
        return Some("cloudformation-template".to_string());
    }

    // --- Ansible playbook ---
    // Format: "ansible-{play-name}" — prefer the first play's `name:` field,
    // then the `hosts:` value.
    if content.trim_start().starts_with("- ") && (content.contains("hosts:") || content.contains("tasks:")) {
        if let Some(cap) = ANSIBLE_PLAY_NAME_RE.captures(content) {
            return Some(format!("ansible-{}", cap[1].trim()));
        }
        if let Some(cap) = HOSTS_RE.captures(content) {
            return Some(format!("ansible-{}", cap[1].trim()));
        }
        return Some("ansible-playbook".to_string());
    }

    // --- Helm Chart.yaml ---
    // Format: "helm-{name}" — uses "helm-" prefix to avoid slug deduplication
    // when the chart name itself contains "chart" (e.g. "my-chart" → "helm-my-chart").
    if has_key("apiVersion") && has_key("appVersion") && has_key("type") {
        if let Some(cap) = NAME_VALUE_RE.captures(content) {
            return Some(format!("helm-{}", cap[1].trim()));
        }
    }

    // --- GitLab CI ---
    // Format: "{first-stage}-gitlab-ci". Tries inline list then block list.
    if has_key("stages") && (has_key("image") || has_key("variables") || has_key("include")) {
        let stage = STAGES_INLINE_RE.captures(content)
            .and_then(|c| c.get(1))
            .map(|m| m.as_str().split(',').next().unwrap_or("").trim().to_string())
            .filter(|s| !s.is_empty())
            .or_else(|| {
                // Skip lines until we're past `stages:` then grab first list item
                let after_stages = content.find("stages:").map(|i| &content[i..]);
                after_stages.and_then(|s| {
                    STAGES_ITEM_RE.captures(s)
                        .and_then(|c| c.get(1))
                        .map(|m| m.as_str().to_string())
                })
            });
        return match stage {
            Some(s) => Some(format!("{s}-gitlab-ci")),
            None => Some("gitlab-ci".to_string()),
        };
    }

    // --- Travis CI ---
    // Format: "travis-{language}"
    if has_key("language") && (has_key("script") || has_key("install")) {
        static LANG_RE: LazyLock<Regex> = LazyLock::new(|| {
            Regex::new(r"(?m)^language\s*:\s*(\w+)").unwrap()
        });
        if let Some(cap) = LANG_RE.captures(content) {
            return Some(format!("travis-{}", &cap[1]));
        }
        if let Some(cap) = NAME_VALUE_RE.captures(content) {
            return Some(format!("travis-{}", cap[1].trim()));
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
        assert!(n.contains("web"), "service name included: {n}");
    }

    #[test]
    fn github_actions() {
        let src = "name: CI Pipeline\non: [push, pull_request]\njobs:\n  test:\n    runs-on: ubuntu-latest";
        let n = name(src).unwrap();
        // Now includes "-workflow" suffix
        assert!(n.contains("ci-pipeline"), "got: {n}");
        assert!(n.contains("workflow"), "workflow suffix: {n}");
    }

    #[test]
    fn github_actions_no_name() {
        let src = "on: [push]\njobs:\n  test:\n    runs-on: ubuntu-latest";
        let n = name(src).unwrap();
        assert!(n.contains("github-actions-workflow"), "got: {n}");
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
        // Now includes "-openapi" suffix
        assert!(n.contains("pet-store-api"), "got: {n}");
        assert!(n.contains("openapi"), "openapi suffix: {n}");
    }

    #[test]
    fn swagger_yaml() {
        let src = "swagger: '2.0'\ninfo:\n  title: Legacy API\n  version: 1.0.0\n";
        let n = name(src).unwrap();
        assert!(n.contains("legacy-api"), "got: {n}");
        assert!(n.contains("swagger"), "swagger suffix: {n}");
    }

    #[test]
    fn cloudformation_template() {
        let src = "AWSTemplateFormatVersion: '2010-09-09'\nDescription: My web server stack\nResources:\n  WebServer:\n    Type: AWS::EC2::Instance\n";
        let n = name(src).unwrap();
        assert!(n.contains("web-server"), "got: {n}");
        assert!(n.contains("cfn"), "cfn suffix: {n}");
    }

    #[test]
    fn ansible_playbook() {
        let src = "- hosts: webservers\n  name: Deploy web app\n  tasks:\n    - name: Install nginx\n      apt:\n        name: nginx";
        let n = name(src).unwrap();
        // Uses play name with ansible- prefix
        assert!(n.starts_with("ansible"), "got: {n}");
    }

    #[test]
    fn ansible_playbook_hosts_fallback() {
        let src = "- hosts: webservers\n  tasks:\n    - apt:\n        name: nginx";
        let n = name(src).unwrap();
        assert!(n.contains("ansible"), "got: {n}");
        assert!(n.contains("webservers"), "hosts in name: {n}");
    }

    #[test]
    fn helm_chart() {
        let src = "apiVersion: v2\nname: my-chart\nappVersion: 1.0.0\ntype: application\n";
        let n = name(src).unwrap();
        assert!(n.contains("helm"), "got: {n}");
        assert!(n.contains("my-chart"), "got: {n}");
    }

    #[test]
    fn gitlab_ci_with_stages() {
        let src = "stages:\n  - test\n  - build\n  - deploy\nvariables:\n  FOO: bar\ntest-job:\n  stage: test\n";
        let n = name(src).unwrap();
        assert!(n.contains("gitlab-ci"), "got: {n}");
        assert!(n.contains("test"), "first stage included: {n}");
    }

    #[test]
    fn travis_ci() {
        let src = "language: python\ninstall:\n  - pip install -r requirements.txt\nscript:\n  - pytest\n";
        let n = name(src).unwrap();
        assert!(n.contains("travis"), "got: {n}");
        assert!(n.contains("python"), "language included: {n}");
    }

    #[test]
    fn generic_yaml_with_name() {
        let src = "name: My Application\nport: 8080\ndebug: true";
        let n = name(src).unwrap();
        assert!(n.contains("my-application"), "got: {n}");
    }
}
