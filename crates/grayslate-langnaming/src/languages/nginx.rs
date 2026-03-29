use super::{NamingDefinition, Extractor};
use crate::model::MAX_TOKENS;

pub fn definition() -> NamingDefinition {
    NamingDefinition {
        name: "nginx",
        extension: "conf",
        extract: Extractor::Custom(extract_nginx),
    }
}

/// Nginx config naming: server_name, listen port, upstream, location blocks.
fn extract_nginx(content: &str) -> Option<String> {
    use regex::Regex;
    use std::sync::LazyLock;

    static SERVER_NAME_RE: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"(?m)server_name\s+([\w.\-]+)").unwrap()
    });
    static UPSTREAM_RE: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"(?m)upstream\s+([\w\-]+)").unwrap()
    });
    static LISTEN_RE: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"(?m)listen\s+(\d+)").unwrap()
    });
    static LOCATION_RE: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"(?m)location\s+(?:[~=]*\s+)?(/[\w/\-]+)").unwrap()
    });

    let mut tokens: Vec<String> = Vec::new();

    // server_name (P10)
    if let Some(cap) = SERVER_NAME_RE.captures(content) {
        let name = &cap[1];
        if name != "_" && name != "localhost" {
            tokens.push(name.to_string());
        }
    }

    // upstream (P9)
    for cap in UPSTREAM_RE.captures_iter(content).take(2) {
        if tokens.len() >= MAX_TOKENS { break; }
        tokens.push(cap[1].to_string());
    }

    // listen port (P5) — add "nginx-<port>" if nothing else
    if tokens.is_empty() {
        if let Some(cap) = LISTEN_RE.captures(content) {
            tokens.push(format!("nginx-{}", &cap[1]));
        }
    }

    // location blocks (P4)
    for cap in LOCATION_RE.captures_iter(content).take(2) {
        if tokens.len() >= MAX_TOKENS { break; }
        let loc = &cap[1];
        if loc != "/" {
            tokens.push(loc.trim_matches('/').replace('/', "-"));
        }
    }

    tokens.truncate(MAX_TOKENS);
    if tokens.is_empty() { None } else { Some(tokens.join("-")) }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::shared::slugify;

    fn name(src: &str) -> Option<String> {
        extract_nginx(src).and_then(|s| slugify(&s))
    }

    #[test]
    fn server_name() {
        let src = "server {\n  listen 80;\n  server_name api.example.com;\n  location /v1 {\n    proxy_pass http://backend;\n  }\n}";
        let n = name(src).unwrap();
        // Dots in domain names become hyphens after slugify
        assert!(n.contains("api"), "got: {n}");
        assert!(n.contains("example"), "got: {n}");
    }

    #[test]
    fn upstream() {
        let src = "upstream backend_pool {\n  server 127.0.0.1:3000;\n  server 127.0.0.1:3001;\n}";
        let n = name(src).unwrap();
        assert!(n.contains("backend-pool"), "got: {n}");
    }

    #[test]
    fn listen_port_fallback() {
        let src = "server {\n  listen 8080;\n  location / {\n    root /var/www/html;\n  }\n}";
        let n = name(src).unwrap();
        assert!(n.contains("nginx-8080"), "listen fallback: {n}");
    }

    #[test]
    fn wildcard_server_name_ignored() {
        let src = "server {\n  listen 443 ssl;\n  server_name _;\n  location /health {\n    return 200;\n  }\n}";
        let n = name(src).unwrap();
        assert!(n.contains("nginx-443"), "wildcard _ filtered: {n}");
    }

    #[test]
    fn location_only() {
        let src = "location /api/v2/users {\n  proxy_pass http://backend;\n}";
        let n = name(src).unwrap();
        assert!(n.contains("api-v2-users"), "location only: {n}");
    }
}
