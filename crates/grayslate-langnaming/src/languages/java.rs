use super::{NamingDefinition, Extractor};
use crate::code::{symbols_to_stem, Symbol};

pub fn definition() -> NamingDefinition {
    NamingDefinition {
        name: "java",
        extension: "java",
        extract: Extractor::Custom(extract_java),
    }
}

fn extract_java(content: &str) -> Option<String> {
    extract_java_regex(content)
}

/// Regex-based Java naming: package, class/interface/enum/record/annotation, methods.
fn extract_java_regex(content: &str) -> Option<String> {
    use regex::Regex;
    use std::sync::LazyLock;

    // package com.example.payment.gateway;
    static PACKAGE_RE: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"(?m)^package\s+([\w.]+)\s*;").unwrap()
    });
    // public class PaymentProcessor / interface X / enum X / record X / @interface X
    // Also matches annotation-decorated types: @Entity\n public class Foo
    static TYPE_RE: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"(?m)^(?:@\w+(?:\([^)]*\))?\s+)*(?:public\s+)?(?:abstract\s+|final\s+|sealed\s+|static\s+)?(?:class|interface|enum|record|@interface)\s+([A-Za-z_]\w*)").unwrap()
    });
    // public/protected method: public void processOrder(...)
    static METHOD_RE: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"(?m)^\s+(?:public|protected)\s+(?:static\s+)?(?:final\s+)?(?:synchronized\s+)?(?:<[^>]+>\s+)?(?:\w[\w<>,?\[\]\s]*)\s+([a-zA-Z_]\w*)\s*\(").unwrap()
    });
    // import static org.junit.Assert.assertEquals;
    static STATIC_IMPORT_RE: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"(?m)^import\s+static\s+[\w.]+\.([A-Z]\w*)\.").unwrap()
    });

    let mut symbols: Vec<Symbol> = Vec::new();

    // Package → take last 2 segments
    if let Some(cap) = PACKAGE_RE.captures(content) {
        let pkg = &cap[1];
        let segments: Vec<&str> = pkg.split('.').collect();
        let short = if segments.len() >= 2 {
            format!("{}-{}", segments[segments.len() - 2], segments[segments.len() - 1])
        } else {
            segments.last().unwrap_or(&pkg).to_string()
        };
        if !short.is_empty() {
            symbols.push(Symbol { name: short, priority: 5 });
        }
    }

    // Type declarations (highest priority)
    for cap in TYPE_RE.captures_iter(content).take(3) {
        symbols.push(Symbol { name: cap[1].to_string(), priority: 9 });
    }

    // Static imports give context about framework usage
    for cap in STATIC_IMPORT_RE.captures_iter(content).take(2) {
        let name = cap[1].to_string();
        if !crate::code::is_noise_name(&name) {
            symbols.push(Symbol { name, priority: 4 });
        }
    }

    if let Some(stem) = symbols_to_stem(&mut symbols) {
        return Some(stem);
    }

    // Fallback: top public method names
    for cap in METHOD_RE.captures_iter(content).take(3) {
        let name = cap[1].to_string();
        if !crate::code::is_noise_name(&name) {
            symbols.push(Symbol { name, priority: 6 });
        }
    }

    symbols_to_stem(&mut symbols)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::shared::slugify;

    fn name(src: &str) -> Option<String> {
        extract_java(src).and_then(|s| slugify(&s))
    }

    #[test]
    fn class_leads_over_package() {
        let code = "package com.example.payment;\n\npublic class PaymentProcessor {\n    public PaymentResult process(Order order) { return null; }\n    private void validate(Order order) {}\n}";
        let result = name(code).unwrap();
        assert!(result.contains("payment-processor"), "class leads: {result}");
        assert!(!result.contains("validate"), "methods excluded: {result}");
    }

    #[test]
    fn deep_package_uses_two_segments() {
        let code = "package com.example.payment.gateway;\n\npublic interface PaymentGateway {\n    void charge(double amount);\n}";
        let result = name(code).unwrap();
        assert!(result.contains("payment-gateway"), "got: {result}");
    }

    #[test]
    fn enum_extraction() {
        let code = "package org.example;\n\npublic enum OrderStatus {\n    PENDING, PROCESSING, COMPLETED\n}";
        let result = name(code).unwrap();
        assert!(result.contains("order-status"), "got: {result}");
    }

    #[test]
    fn record_type() {
        let code = "package com.example.model;\n\npublic record UserDTO(String name, int age) {}";
        let result = name(code).unwrap();
        assert!(result.contains("user-dto"), "got: {result}");
    }

    #[test]
    fn java_interface_declaration() {
        let code = "package com.example.service;\n\npublic interface EventListener {\n    void onEvent(Event event);\n}";
        let result = name(code).unwrap();
        assert!(result.contains("event-listener"), "interface: {result}");
    }

    #[test]
    fn java_annotation_type() {
        let code = "package com.example;\n\npublic @interface Transactional {\n    boolean readOnly() default false;\n}";
        let result = name(code).unwrap();
        assert!(result.contains("transactional"), "annotation type: {result}");
    }

    #[test]
    fn java_abstract_class() {
        let code = "package com.example.validation;\n\npublic abstract class AbstractValidator<T> {\n    public abstract boolean validate(T input);\n}";
        let result = name(code).unwrap();
        assert!(result.contains("abstract-validator"), "abstract class: {result}");
    }

    #[test]
    fn java_method_fallback() {
        let code = "    public void processPayment(Payment payment) {\n        // ...\n    }\n    public Receipt generateReceipt() {\n        return null;\n    }";
        let result = name(code).unwrap();
        assert!(result.contains("process-payment") || result.contains("generate-receipt"), "method fallback: {result}");
    }
}
