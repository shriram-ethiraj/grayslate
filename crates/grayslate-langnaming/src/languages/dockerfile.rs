use super::{NamingDefinition, Extractor};
use crate::model::MAX_TOKENS;

pub fn definition() -> NamingDefinition {
    NamingDefinition {
        name: "dockerfile",
        extension: "dockerfile",
        extract: Extractor::Custom(extract_dockerfile),
    }
}

/// Dockerfile naming extraction.
///
/// Priority order:
///   1. `LABEL` metadata (maintainer, description, name) — P10
///   2. `FROM` base image name — P8
///   3. `CMD` / `ENTRYPOINT` command name — P6
///   4. `WORKDIR` / `ENV` app name — P5
///   5. `EXPOSE` ports — P4
fn extract_dockerfile(content: &str) -> Option<String> {
    use regex::Regex;
    use std::sync::LazyLock;

    static FROM_RE: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"(?im)^FROM\s+(?:--platform=\S+\s+)?([\w./-]+?)(?::[\w.-]+)?(?:\s+AS\s+(\S+))?$").unwrap());
    static LABEL_RE: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r#"(?im)^LABEL\s+.*?(?:description|name|title)\s*=\s*"([^"]{1,60})""#).unwrap());
    static EXPOSE_RE: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"(?im)^EXPOSE\s+(\d+)").unwrap());
    static CMD_RE: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r#"(?im)^(?:CMD|ENTRYPOINT)\s+(?:\[")?([a-zA-Z_][\w.-]*)"#).unwrap());
    // WORKDIR /app/myservice or /opt/myapp
    static WORKDIR_RE: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"(?im)^WORKDIR\s+/(?:app|opt|srv|home/\w+)/([a-zA-Z][\w.-]+)").unwrap());
    // ENV APP_NAME=myservice or ENV SERVICE_NAME=foo
    static ENV_NAME_RE: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r#"(?im)^ENV\s+(?:APP_NAME|SERVICE_NAME|PROJECT_NAME)\s*=\s*"?([a-zA-Z][\w.-]+)"?"#).unwrap());

    const NOISE_IMAGES: &[&str] = &[
        "scratch", "latest", "alpine", "ubuntu", "debian", "centos", "busybox",
    ];

    let mut tokens: Vec<String> = Vec::new();

    // LABEL description/name (P10)
    if let Some(cap) = LABEL_RE.captures(content) {
        tokens.push(cap[1].to_string());
    }

    // FROM base image (P8) — prefer stage alias
    for cap in FROM_RE.captures_iter(content).take(2) {
        if let Some(alias) = cap.get(2) {
            let a = alias.as_str();
            if !a.is_empty() && a != "base" && a != "builder" {
                tokens.push(a.to_string());
                continue;
            }
        }
        let image = &cap[1];
        // Strip registry prefix: docker.io/library/node → node
        let short = image.rsplit('/').next().unwrap_or(image);
        if !short.is_empty() && !NOISE_IMAGES.contains(&short) {
            tokens.push(short.to_string());
        }
    }

    // CMD/ENTRYPOINT (P6)
    if let Some(cap) = CMD_RE.captures(content) {
        let cmd = cap[1].to_string();
        if cmd != "sh" && cmd != "bash" && cmd != "cmd" {
            tokens.push(cmd);
        }
    }

    // WORKDIR / ENV app name (P5)
    if let Some(cap) = ENV_NAME_RE.captures(content) {
        tokens.push(cap[1].to_string());
    } else if let Some(cap) = WORKDIR_RE.captures(content) {
        tokens.push(cap[1].to_string());
    }

    // EXPOSE (P4)
    if let Some(cap) = EXPOSE_RE.captures(content) {
        tokens.push(format!("port-{}", &cap[1]));
    }

    tokens.truncate(MAX_TOKENS);
    if tokens.is_empty() { None } else { Some(tokens.join("-")) }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::shared::slugify;

    fn name(src: &str) -> Option<String> {
        extract_dockerfile(src).and_then(|s| slugify(&s))
    }

    #[test]
    fn from_image() {
        let src = "FROM node:18-alpine\nWORKDIR /app\nCOPY . .\n";
        let n = name(src).unwrap();
        assert!(n.contains("node"), "got: {n}");
    }

    #[test]
    fn multistage_with_alias() {
        let src = "FROM golang:1.21 AS compiler\nRUN go build\nFROM alpine\nCOPY --from=compiler /app .\n";
        let n = name(src).unwrap();
        assert!(n.contains("compiler"), "got: {n}");
    }

    #[test]
    fn label_description() {
        let src = "FROM python:3.12\nLABEL description=\"FastAPI microservice\"\nEXPOSE 8000\n";
        let n = name(src).unwrap();
        assert!(n.contains("fast-api-microservice"), "got: {n}");
    }

    #[test]
    fn env_app_name() {
        let src = "FROM node:18\nENV APP_NAME=payment-service\nWORKDIR /app\nCOPY . .\n";
        let n = name(src).unwrap();
        assert!(n.contains("node"), "got: {n}");
    }

    #[test]
    fn workdir_app_name() {
        let src = "FROM python:3.12\nWORKDIR /app/analytics-engine\nCOPY . .\nCMD [\"python\", \"main.py\"]\n";
        let n = name(src).unwrap();
        assert!(n.contains("python"), "got: {n}");
    }

    #[test]
    fn cmd_extraction() {
        let src = "FROM scratch\nCOPY myservice /myservice\nCMD [\"myservice\"]\nEXPOSE 8080\n";
        let n = name(src).unwrap();
        assert!(n.contains("myservice"), "CMD: {n}");
    }

    #[test]
    fn expose_port_only() {
        let src = "FROM scratch\nCOPY app /app\nEXPOSE 3000\n";
        let n = name(src).unwrap();
        assert!(n.contains("port-3000"), "expose fallback: {n}");
    }
}

