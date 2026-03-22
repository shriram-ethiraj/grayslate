use super::NamingDefinition;
use crate::naming::code::{extract_with_tree_sitter, field_text, Symbol};

pub fn definition() -> NamingDefinition {
    NamingDefinition {
        name: "java",
        extension: "java",
        extract: extract_java,
    }
}

fn extract_java(content: &str) -> Option<String> {
    extract_with_tree_sitter(content, tree_sitter_java::LANGUAGE, collect_java)
}

fn collect_java(root: &tree_sitter::Node, src: &[u8], symbols: &mut Vec<Symbol>) {
    let mut cursor = root.walk();
    for child in root.children(&mut cursor) {
        match child.kind() {
            // package com.example.payment.processor → "payment-processor"
            "package_declaration" => {
                let mut inner = child.walk();
                for gc in child.children(&mut inner) {
                    if gc.kind() == "scoped_identifier" || gc.kind() == "identifier" {
                        if let Ok(text) = gc.utf8_text(src) {
                            let segments: Vec<&str> = text.split('.').collect();
                            let pkg = if segments.len() >= 2 {
                                format!(
                                    "{}-{}",
                                    segments[segments.len() - 2],
                                    segments[segments.len() - 1]
                                )
                            } else {
                                segments.last().unwrap_or(&text).trim().to_string()
                            };
                            if !pkg.is_empty() {
                                symbols.push(Symbol { name: pkg, priority: 5 });
                            }
                        }
                    }
                }
            }
            "class_declaration" | "interface_declaration" | "enum_declaration"
            | "annotation_type_declaration" | "record_declaration" => {
                if let Some(name) = field_text(&child, "name", src) {
                    symbols.push(Symbol { name: name.to_string(), priority: 9 });
                }
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::naming::shared::slugify;

    fn name(src: &str) -> Option<String> {
        extract_java(src).and_then(|s| slugify(&s))
    }

    #[test]
    fn class_leads_over_package() {
        let code = "package com.example.payment;\n\npublic class PaymentProcessor {\n    public PaymentResult process(Order order) { return null; }\n    private void validate(Order order) {}\n}";
        let result = name(code).unwrap();
        assert!(result.contains("payment-processor"), "class leads: {result}");
        // Methods like "validate" should not appear in the stem
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
}
