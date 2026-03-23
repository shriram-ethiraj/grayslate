use super::{wp, LanguageDefinition};
use super::ContentFamily;

pub fn definition() -> LanguageDefinition {
    LanguageDefinition {
        name: "nginx",
        extensions: &[],
        filenames: &[],
        filename_patterns: &[r"^nginx.*\.conf$"],
        shebangs: &[],
        structural_priority: None,
        structural_detect: None,
        patterns: &[
            wp!(r"(?m)^\s*server\s*\{", 5),
            wp!(r"(?m)^\s*location\s+[~/^]", 5),
            wp!(r"(?m)^\s*upstream\s+\w+", 5),
            wp!(r"(?m)^\s*proxy_pass\s+", 4),
            wp!(r"(?m)^\s*listen\s+\d", 4),
            wp!(r"(?m)^\s*server_name\s+", 4),
            wp!(r"(?m)^\s*root\s+/", 3),
            wp!(r"(?m)^\s*index\s+", 2),
            wp!(r"(?m)^\s*error_log\s+", 3),
            wp!(r"(?m)^\s*access_log\s+", 3),
            wp!(r"(?m)^\s*ssl_certificate\s+", 4),
            wp!(r"(?m)^\s*worker_processes\s+", 4),
            wp!(r"(?m)^\s*proxy_set_header\s+", 3),
        ],
        anti_patterns: &[
            wp!(r"(?m)^\s*(def|class|import)\s+", -4),
            wp!(r"(?m)^\s*\{", -2),
        ],
        uses_hash_comments: true,
        keywords: &[
            "server", "location", "upstream", "listen", "server_name", "root",
            "index", "proxy_pass", "proxy_set_header", "error_log", "access_log",
            "worker_processes", "worker_connections", "events", "http", "include",
            "ssl_certificate", "ssl_certificate_key", "fastcgi_pass", "try_files",
            "rewrite", "return", "add_header", "deny", "allow", "types",
            "default_type", "sendfile", "keepalive_timeout", "gzip",
        ],
        builtins: &[
            "$host", "$uri", "$args", "$request_uri", "$remote_addr",
            "$proxy_add_x_forwarded_for", "$http_upgrade", "$scheme",
            "$server_name", "$request_method", "$content_type", "$document_root",
        ],
        family: None,
        exclusive_patterns: &[],
        // ── Family-gated fields ──────────────────────────────
        content_families: &[ContentFamily::Config],
        anchors: &[
            wp!(r"(?m)^\s*server\s*\{", 5),
            wp!(r"(?m)^\s*location\s+[~/^]", 5),
            wp!(r"(?m)^\s*upstream\s+\w+", 5),
            wp!(r"(?m)^\s*proxy_pass\s+", 4),
        ],
        hints: &[
            wp!(r"(?m)^\s*listen\s+\d", 3),
            wp!(r"(?m)^\s*root\s+/", 3),
            wp!(r"(?m)^\s*index\s+", 2),
            wp!(r"(?m)^\s*access_log\s+", 3),
            wp!(r"(?m)^\s*error_log\s+", 3),
        ],
        rivals: &[],
        differentiators: &[],
        disqualifiers: &[],
    }
}
