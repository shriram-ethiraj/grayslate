use std::collections::HashSet;

use super::{NamingDefinition, Extractor};
use crate::model::MAX_TOKENS;

pub fn definition() -> NamingDefinition {
    NamingDefinition {
        name: "kotlin",
        extension: "kt",
        extract: Extractor::Custom(extract_kotlin),
    }
}

/// Kotlin-specific regex extraction.
///
/// Priority order (file-local symbols outrank package context):
///   1. `@file:JvmName("...")` file annotation — P10
///   2. `object` / `data class` / `sealed class` / `class` / `interface` / `enum class` — P9
///      (includes `expect`/`actual` and `value` modifiers)
///   3. `@Composable fun` — P8 (UI component)
///   4. `typealias` declarations — P8
///   5. Extension functions `fun Receiver.name(...)` — P7
///   6. Top-level `fun` declarations — P7
///   7. `package` declaration (last segment) — P5 (fallback context)
///
/// Gradle .kts script-aware path:
///   - `description = "..."` — P10
///   - `rootProject.name = "..."` — P10
///   - Plugin IDs `id("...")` — P8
///   - `project("...")` references — P7
///   - `tasks.register(...)` / `tasks.named(...)` — P7
fn extract_kotlin(content: &str) -> Option<String> {
    use regex::Regex;
    use std::sync::LazyLock;

    // Detect Gradle/script content for the script-aware path.
    let is_gradle = content.contains("plugins {")
        || content.contains("plugins{")
        || content.contains("dependencies {")
        || content.contains("dependencies{")
        || content.contains("rootProject.name")
        || content.contains("pluginManagement")
        || content.contains("buildscript {")
        || content.contains("buildscript{")
        || content.contains("tasks.register")
        || content.contains("kotlin {")
        || content.contains("kotlin{")
        || content.contains("sourceSets {")
        || content.contains("sourceSets{");

    if is_gradle {
        if let Some(stem) = extract_kotlin_script(content) {
            return Some(stem);
        }
    }

    // --- Source (.kt) path ---

    static PACKAGE_RE: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"(?m)^package\s+([\w.]+)").unwrap());
    // @file:JvmName("FooBar")
    static FILE_ANNOTATION_RE: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r#"(?m)^@file:\s*JvmName\(\s*"([^"]+)""#).unwrap());
    // Class/object/interface with optional expect/actual/value modifiers
    static CLASS_RE: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(
            r"(?m)^(?:(?:public|private|internal|protected|abstract|open|sealed|data|inner|value|expect|actual)\s+)*(?:class|interface|object|enum\s+class)\s+([A-Z][a-zA-Z0-9_]*)",
        )
        .unwrap()
    });
    static COMPOSABLE_FUN_RE: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"(?m)@Composable\s+(?:(?:public|private|internal)\s+)?fun\s+([A-Z][a-zA-Z0-9_]*)").unwrap()
    });
    // typealias Foo = Bar
    static TYPEALIAS_RE: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"(?m)^(?:(?:public|internal|private)\s+)?typealias\s+([A-Z][a-zA-Z0-9_]*)").unwrap()
    });
    // Extension function: fun Receiver.name(...)
    static EXT_FUN_RE: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"(?m)^(?:(?:public|private|internal|protected|override|suspend|inline|operator|expect|actual)\s+)*fun\s+(?:[A-Z][a-zA-Z0-9_<>?., ]*)\.\s*([a-zA-Z_][a-zA-Z0-9_]*)").unwrap()
    });
    // Regular function (no receiver)
    static FUN_RE: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"(?m)^(?:(?:public|private|internal|protected|override|suspend|inline|operator|expect|actual)\s+)*fun\s+([a-zA-Z_][a-zA-Z0-9_]*)\s*[\(<]").unwrap()
    });

    const NOISE: &[&str] = &[
        "main", "init", "setup", "run", "start", "new", "default", "handle",
        "index", "app", "mod", "test", "self", "this", "invoke", "apply",
        "onCreate", "onStart", "onResume", "onPause", "onStop", "onDestroy",
        "toString", "hashCode", "equals", "copy", "component1",
    ];

    struct Symbol {
        name: String,
        priority: u8,
    }

    let mut symbols: Vec<Symbol> = Vec::new();

    // @file:JvmName annotation — strongest signal (P10)
    if let Some(cap) = FILE_ANNOTATION_RE.captures(content) {
        symbols.push(Symbol { name: cap[1].to_string(), priority: 10 });
    }

    // Classes/objects/interfaces (P9)
    for cap in CLASS_RE.captures_iter(content).take(4) {
        symbols.push(Symbol { name: cap[1].to_string(), priority: 9 });
    }

    // Composable functions (P8)
    for cap in COMPOSABLE_FUN_RE.captures_iter(content).take(4) {
        symbols.push(Symbol { name: cap[1].to_string(), priority: 8 });
    }

    // typealias (P8)
    for cap in TYPEALIAS_RE.captures_iter(content).take(3) {
        symbols.push(Symbol { name: cap[1].to_string(), priority: 8 });
    }

    // Extension functions (P7)
    for cap in EXT_FUN_RE.captures_iter(content).take(4) {
        symbols.push(Symbol { name: cap[1].to_string(), priority: 7 });
    }

    // Top-level functions (P7)
    for cap in FUN_RE.captures_iter(content).take(4) {
        symbols.push(Symbol { name: cap[1].to_string(), priority: 7 });
    }

    // Package (last segment) — fallback context (P5)
    if let Some(cap) = PACKAGE_RE.captures(content) {
        if let Some(pkg) = cap[1].rsplit('.').next() {
            if !pkg.is_empty() {
                symbols.push(Symbol { name: pkg.to_string(), priority: 5 });
            }
        }
    }

    // Sort by priority descending, deduplicate, filter noise.
    symbols.sort_by(|a, b| b.priority.cmp(&a.priority));

    let mut seen = HashSet::new();
    let mut tokens: Vec<String> = Vec::new();
    for sym in &symbols {
        if tokens.len() >= MAX_TOKENS {
            break;
        }
        if !NOISE.contains(&sym.name.as_str()) && seen.insert(sym.name.clone()) {
            tokens.push(sym.name.clone());
        }
    }

    if tokens.is_empty() { None } else { Some(tokens.join("-")) }
}

/// Script-aware extraction for Gradle `.kts` files and Kotlin scripts.
fn extract_kotlin_script(content: &str) -> Option<String> {
    use regex::Regex;
    use std::sync::LazyLock;

    // description = "Ktor client auth plugin"
    static DESC_RE: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r#"(?m)^\s*description\s*=\s*"([^"]{3,80})""#).unwrap()
    });
    // rootProject.name = "ktor"
    static ROOT_NAME_RE: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r#"(?m)rootProject\.name\s*=\s*"([^"]+)""#).unwrap()
    });
    // Plugin IDs: id("org.jetbrains.kotlin.jvm")
    static PLUGIN_ID_RE: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r#"(?m)\bid\s*\(\s*"([^"]+)""#).unwrap()
    });
    // project(":ktor-client:ktor-client-core")
    static PROJECT_REF_RE: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r#"(?m)project\s*\(\s*"[:]?([^"]+)""#).unwrap()
    });
    // tasks.register<Jar>("...") or tasks.named("...")
    static TASK_RE: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r#"(?m)tasks\.(?:register|named)\s*(?:<[^>]+>)?\s*\(\s*"([^"]+)""#).unwrap()
    });
    // Top-level fun in script
    static SCRIPT_FUN_RE: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"(?m)^fun\s+([a-zA-Z_][a-zA-Z0-9_]*)\s*\(").unwrap()
    });

    const NOISE_PLUGINS: &[&str] = &[
        "java", "application", "maven-publish", "signing",
    ];
    const NOISE_TASKS: &[&str] = &[
        "clean", "build", "test", "jar", "check", "assemble",
    ];

    struct Symbol {
        name: String,
        priority: u8,
    }
    let mut symbols: Vec<Symbol> = Vec::new();

    // description (P10) — most meaningful single-line signal
    if let Some(cap) = DESC_RE.captures(content) {
        return Some(cap[1].to_string());
    }

    // rootProject.name (P10)
    if let Some(cap) = ROOT_NAME_RE.captures(content) {
        symbols.push(Symbol { name: cap[1].to_string(), priority: 10 });
    }

    // Plugin IDs (P8) — take last segment of dotted ID
    for cap in PLUGIN_ID_RE.captures_iter(content).take(4) {
        let id = &cap[1];
        let short = id.rsplit('.').next().unwrap_or(id);
        if !NOISE_PLUGINS.contains(&short) && short.len() >= 2 {
            symbols.push(Symbol { name: short.to_string(), priority: 8 });
        }
    }

    // project() references (P7) — take last colon-segment
    for cap in PROJECT_REF_RE.captures_iter(content).take(4) {
        let path = &cap[1];
        let short = path.rsplit(':').next().unwrap_or(path);
        if !short.is_empty() {
            symbols.push(Symbol { name: short.to_string(), priority: 7 });
        }
    }

    // Task registrations (P7)
    for cap in TASK_RE.captures_iter(content).take(4) {
        let name = &cap[1];
        if !NOISE_TASKS.contains(&name) {
            symbols.push(Symbol { name: name.to_string(), priority: 7 });
        }
    }

    // Script functions (P6)
    for cap in SCRIPT_FUN_RE.captures_iter(content).take(3) {
        let name = cap[1].to_string();
        if name != "main" {
            symbols.push(Symbol { name, priority: 6 });
        }
    }

    symbols.sort_by(|a, b| b.priority.cmp(&a.priority));

    let mut seen = HashSet::new();
    let mut tokens: Vec<String> = Vec::new();
    for sym in &symbols {
        if tokens.len() >= MAX_TOKENS { break; }
        if seen.insert(sym.name.clone()) {
            tokens.push(sym.name.clone());
        }
    }

    if tokens.is_empty() { None } else { Some(tokens.join("-")) }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::shared::slugify;

    fn name(src: &str) -> Option<String> {
        extract_kotlin(src).and_then(|s| slugify(&s))
    }

    // --- Priority rebalance: class outranks package ---
    #[test]
    fn class_leads_over_package() {
        let src = "package com.example.model\n\ndata class User(val name: String, val age: Int)";
        let n = name(src).unwrap();
        assert!(n.contains("user"), "class wins over package: {n}");
    }

    #[test]
    fn object_declaration() {
        let src = "object DatabaseHelper {\n    fun getConnection(): Connection { }\n}";
        let n = name(src).unwrap();
        assert!(n.contains("database-helper"), "got: {n}");
    }

    #[test]
    fn sealed_class() {
        let src = "sealed class Result<out T> {\n    data class Success<T>(val data: T) : Result<T>()\n}";
        let n = name(src).unwrap();
        assert!(n.contains("result"), "got: {n}");
    }

    #[test]
    fn composable_function() {
        let src = "package com.example.ui\n\n@Composable\nfun UserProfile(user: User) { }";
        let n = name(src).unwrap();
        assert!(n.contains("user-profile"), "composable wins over package: {n}");
    }

    #[test]
    fn top_level_fun() {
        let src = "fun calculateTotal(items: List<Item>): Double { return 0.0 }";
        let n = name(src).unwrap();
        assert!(n.contains("calculate-total"), "got: {n}");
    }

    // --- New: expect/actual modifiers ---
    #[test]
    fn expect_class() {
        let src = "package io.ktor.test\n\nexpect class TestResult";
        let n = name(src).unwrap();
        assert!(n.contains("test-result"), "expect class extracted: {n}");
    }

    #[test]
    fn actual_class() {
        let src = "package io.ktor.test\n\nactual class TestResult {\n    actual fun andThen(block: () -> Unit) {}\n}";
        let n = name(src).unwrap();
        assert!(n.contains("test-result"), "actual class extracted: {n}");
    }

    // --- New: extension functions ---
    #[test]
    fn extension_function() {
        let src = "package okhttp3.okio\n\nfun ForwardingFileSystem.logAllOperations() {}";
        let n = name(src).unwrap();
        assert!(n.contains("log-all-operations"), "extension fun extracted: {n}");
    }

    // --- New: typealias ---
    #[test]
    fn typealias_declaration() {
        let src = "package com.example\n\ntypealias NetworkResult = Result<Response>";
        let n = name(src).unwrap();
        assert!(n.contains("network-result"), "typealias extracted: {n}");
    }

    // --- New: @file:JvmName ---
    #[test]
    fn file_jvm_name_annotation() {
        let src = r#"@file:JvmName("Collections")
package ktorbuild.dsl

fun <T> NamedDomainObjectContainer<T>.maybeRegister(name: String) {}"#;
        let n = name(src).unwrap();
        assert!(n.contains("collections"), "@file:JvmName extracted: {n}");
    }

    // --- New: package-only fallback ---
    #[test]
    fn package_only_when_no_symbols() {
        let src = "package com.example.util\n\nimport java.io.*\n";
        let n = name(src).unwrap();
        assert!(n.contains("util"), "package fallback: {n}");
    }

    // --- Audit regression: class wins over generic package ---
    #[test]
    fn logging_filesystem_class_wins() {
        let src = "package okhttp3.okio\n\nclass LoggingFilesystem(\n    delegate: FileSystem\n) : ForwardingFileSystem(delegate) {}";
        let n = name(src).unwrap();
        assert!(n.contains("logging-filesystem"), "class beats package: {n}");
    }

    // --- Gradle script (.kts) tests ---
    #[test]
    fn gradle_description() {
        let src = r#"plugins { id("org.jetbrains.kotlin.jvm") }
description = "Ktor HTTP client auth plugin""#;
        let n = name(src).unwrap();
        assert!(n.contains("ktor-http-client-auth-plugin"), "description extracted: {n}");
    }

    #[test]
    fn gradle_root_project_name() {
        let src = r#"pluginManagement { repositories { mavenCentral() } }
rootProject.name = "ktor""#;
        let n = name(src).unwrap();
        assert!(n.contains("ktor"), "rootProject.name extracted: {n}");
    }

    #[test]
    fn gradle_plugin_id() {
        let src = r#"plugins {
    id("org.jetbrains.kotlin.multiplatform")
    id("maven-publish")
}"#;
        let n = name(src).unwrap();
        assert!(n.contains("multiplatform"), "plugin ID extracted: {n}");
    }

    #[test]
    fn gradle_task_register() {
        let src = r#"plugins { id("java") }
tasks.register<Jar>("sourcesJar") {
    archiveClassifier.set("sources")
}"#;
        let n = name(src).unwrap();
        assert!(n.contains("sources-jar"), "task registration extracted: {n}");
    }

    #[test]
    fn gradle_script_function() {
        let src = r#"plugins { id("java") }
fun wirePackageJsonAggregationTasks() {
    // aggregation logic
}"#;
        let n = name(src).unwrap();
        assert!(n.contains("wire-package-json-aggregation-tasks"), "script fun extracted: {n}");
    }

    #[test]
    fn gradle_project_ref() {
        let src = r#"dependencies {
    implementation(project(":ktor-client:ktor-client-core"))
}"#;
        let n = name(src).unwrap();
        assert!(n.contains("ktor-client-core"), "project ref extracted: {n}");
    }

    #[test]
    fn kotlin_interface() {
        let src = "package com.example\n\ninterface Repository<T> {\n    fun findById(id: Long): T?\n}";
        let n = name(src).unwrap();
        assert!(n.contains("repository"), "interface: {n}");
    }

    #[test]
    fn kotlin_enum_class() {
        let src = "enum class Direction {\n    NORTH, SOUTH, EAST, WEST\n}";
        let n = name(src).unwrap();
        assert!(n.contains("direction"), "enum class: {n}");
    }
}

