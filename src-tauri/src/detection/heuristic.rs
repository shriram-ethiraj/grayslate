/// Phase 4 — Heuristic scoring for programming languages.
///
/// Weighted pattern matching against 20+ language signatures.
/// Each language has positive signals (distinctive patterns) and
/// anti-signals (patterns that rule it out).
use regex::Regex;
use std::sync::LazyLock;

/// Minimum total score for a confident detection.
const HEURISTIC_SCORE_THRESHOLD: i32 = 3;

/// Minimum score for best-guess fallback when no language clears the threshold.
const PARTIAL_SCORE_THRESHOLD: i32 = 2;

struct WeightedPattern {
    pattern: &'static str,
    weight: i32,
}

struct LanguageSignature {
    language: &'static str,
    patterns: &'static [WeightedPattern],
}

macro_rules! wp {
    ($pat:expr, $w:expr) => {
        WeightedPattern {
            pattern: $pat,
            weight: $w,
        }
    };
}

// ── Language Signatures ──────────────────────────────────────────────────

static PYTHON: &[WeightedPattern] = &[
    wp!(r"(?m)^\s*def\s+\w+\s*\(", 3),
    wp!(r"(?m)^\s*class\s+\w+[^:\n]{0,80}:", 3),
    wp!(r"(?m)^\s*from\s+\w+\s+import\s", 3),
    wp!(r"(?m)^\s*import\s+\w+", 2),
    wp!(r"(?m)^\s*elif\s+", 5),
    wp!(r#"if\s+__name__\s*==\s*['"]__main__['"]"#, 5),
    wp!(r"\bself\.\w+", 3),
    wp!(r"(?m)^\s*@\w+(\.\w+)*(\(.*\))?\s*$", 2),
    wp!(r"(?m)^\s*(try|except|finally)\s*:", 2),
    wp!(r"\b(None|True|False)\b", 1),
    wp!(r"(?m)^\s*with\s+\w+[^:\n]{0,80}\s+as\s+", 3),
    wp!(r"(?m)^\s*raise\s+\w+", 2),
    wp!(r"(?m)^\s*yield\s+", 2),
    wp!(r"\bprint\s*\(", 1),
    wp!(r"\blen\s*\(", 1),
    wp!(r"(?m)^\s*async\s+def\s+\w+", 4),
    wp!(r"(?m);\s*$", -2),
    wp!(r"\{[\s]*$", -2),
    wp!(r"(?m)^\s*\}\s*$", -1),
];

static JAVASCRIPT: &[WeightedPattern] = &[
    wp!(r"(?m)\b(const|let|var)\s+\w+\s*=", 2),
    wp!(r"(?m)\bfunction\s+\w*\s*\(", 2),
    wp!(r"=>\s*[\{(\n]", 3),
    wp!(r#"\brequire\s*\(['"`]"#, 4),
    wp!(r"\bmodule\.exports\b", 4),
    wp!(r"\bconsole\.\w+\s*\(", 2),
    wp!(r"===|!==", 2),
    wp!(r"\bdocument\.\w+", 2),
    wp!(r"\bwindow\.\w+", 1),
    wp!(r"\bPromise\.(all|resolve|reject)\b", 2),
    wp!(r"\.then\s*\(", 1),
    wp!(r"\.catch\s*\(", 1),
    wp!(r"(?m)\basync\s+(function|\w+\s*=>|\w+\s*\()", 2),
    wp!(r"\bawait\s+", 1),
    wp!(r#"(?m)^\s*import\s+[\w\{*].*\s+from\s+['"`]"#, 3),
    wp!(r"(?m)^\s*export\s+(const|let|var|function|class|default)\s", 3),
    wp!(r":\s*(string|number|boolean|void)\b", -3),
];

static TYPESCRIPT: &[WeightedPattern] = &[
    wp!(r"(?m)\binterface\s+\w+", 4),
    wp!(r"(?m)\btype\s+\w+\s*=\s*", 4),
    wp!(r":\s*(string|number|boolean|void|any|never|unknown|undefined)\b", 3),
    wp!(r"(?m)\benum\s+\w+\s*\{", 4),
    wp!(r"(?m)\bnamespace\s+\w+", 3),
    wp!(r"(?m)\bdeclare\s+(const|function|class|module|type|interface)", 4),
    wp!(r"\b(Readonly|Partial|Record|Pick|Omit|Required)<", 4),
    wp!(r"\bas\s+(string|number|any|unknown|\w+)\b", 3),
    wp!(r"(?m)^///\s*<reference\s", 5),
    wp!(r#"(?m)^\s*import\s+[\w\{*].*\s+from\s+['"`]"#, 2),
    wp!(r"(?m)^\s*export\s+(const|let|var|function|class|default|type|interface|enum)\s", 2),
    wp!(r"<\w+(\s+extends\s+\w+)?>", 2),
    wp!(r"(?m)\b(const|let|var)\s+\w+\s*=", 1),
    wp!(r"=>\s*[\{(\n]", 1),
    wp!(r"===|!==", 1),
];

static CSS: &[WeightedPattern] = &[
    wp!(r"(?m)[.#][\w\-]+\s*\{", 3),
    wp!(r"(?m)@media\s*[\s(]", 4),
    wp!(r"@keyframes\s+\w+", 4),
    wp!(r"@import\s+", 2),
    wp!(r"!important\s*;", 3),
    wp!(r":hover|:focus|:active|::before|::after", 3),
    wp!(r"\bvar\s*\(--[\w\-]+\)", 3),
    wp!(r"(?m)\b(color|margin|padding|display|font-size|background|border|width|height)\s*:", 2),
    wp!(r"\b(flex|grid|block|inline|none)\s*;", 1),
    wp!(r"@tailwind|@apply", 3),
    wp!(r"(?m)^\s*(function|const|let|var)\s", -5),
];

static SHELL: &[WeightedPattern] = &[
    wp!(r#"(?m)^\s*echo\s+["$']"#, 2),
    wp!(r"(?m)^\s*if\s+\[\[?\s", 3),
    wp!(r"(?m)^\s*fi\s*$", 5),
    wp!(r"(?m)^\s*done\s*$", 4),
    wp!(r"(?m)^\s*esac\s*$", 5),
    wp!(r"(?m)^\s*export\s+\w+=", 3),
    wp!(r"\$\{[\w?!#@*+\-]+", 2),
    wp!(r"\$\(.*\)", 2),
    wp!(r"(?m)^\s*case\s+.*\s+in\s*$", 3),
    wp!(r"(?m)^\s*(alias|source|chmod|mkdir|rm\s|cp\s|mv\s|cd\s|grep|sed|awk)\s", 2),
    wp!(r#"<<-?\s*['"]?\w+['"]?"#, 3),
    wp!(r"\bconsole\.\w+\s*\(", -5),
];

static JAVA: &[WeightedPattern] = &[
    wp!(r"(?m)\bpublic\s+class\s+\w+", 4),
    wp!(r"\bpublic\s+static\s+void\s+main", 5),
    wp!(r"\bSystem\.out\.print(ln)?\s*\(", 5),
    wp!(r"(?m)\bimport\s+java\.\w+", 5),
    wp!(r"(?m)\bimport\s+javax\.\w+", 5),
    wp!(r"(?m)\bimport\s+org\.\w+", 4),
    wp!(r"@Override\b", 3),
    wp!(r"(?m)\bthrows\s+\w+", 2),
    wp!(r"\bextends\s+\w+", 1),
    wp!(r"\bimplements\s+\w+", 2),
    wp!(r"(?m)\bprivate\s+(final\s+)?\w+\s+\w+", 2),
    wp!(r"=>\s*[\{(\n]", -3),
];

static GO: &[WeightedPattern] = &[
    wp!(r"(?m)^package\s+\w+\s*$", 5),
    wp!(r"(?m)^\s*func\s+\w+\s*\(", 3),
    wp!(r"(?m)^\s*func\s+\(\w+\s+\*?\w+\)\s+\w+", 5),
    wp!(r"\bfmt\.\w+", 4),
    wp!(r"(?m)\bimport\s+\(", 3),
    wp!(r"\bgo\s+func\b", 4),
    wp!(r"\bchan\s+\w+", 4),
    wp!(r":=\s", 2),
    wp!(r"\bif\s+err\s*!=\s*nil\b", 4),
    wp!(r"(?m)\bdefer\s+\w+", 3),
    wp!(r"\bpackage\s+main\b", 4),
    wp!(r"(?m)\bclass\s+\w+", -5),
    wp!(r"(?m)^\s*import\s+\w+\s*$", -2),
];

static C_LANG: &[WeightedPattern] = &[
    wp!(r#"(?m)#include\s*[<"]"#, 3),
    wp!(r"(?m)\bint\s+main\s*\(", 4),
    wp!(r"\bprintf\s*\(", 3),
    wp!(r"\b(malloc|calloc|realloc|free)\s*\(", 4),
    wp!(r"(?m)#define\s+\w+", 2),
    wp!(r"\btypedef\s+", 2),
    wp!(r"(?m)\bstruct\s+\w+\s*\{", 2),
    wp!(r"\bsizeof\s*\(", 2),
    wp!(r"\bNULL\b", 2),
    wp!(r"->\w+", 1),
    wp!(r"(?m)\bvoid\s+\w+\s*\(", 1),
    wp!(r"\bstd::\w+", -5),
];

static CPP: &[WeightedPattern] = &[
    wp!(r"\bstd::\w+", 5),
    wp!(r"\bcout\s*<<", 5),
    wp!(r"\bcin\s*>>", 5),
    wp!(r"(?m)#include\s*<(iostream|string|vector|map|set|algorithm|memory|functional)>", 5),
    wp!(r"\busing\s+namespace\s+std\b", 5),
    wp!(r"\bnullptr\b", 4),
    wp!(r"\b(unique_ptr|shared_ptr|weak_ptr)<", 4),
    wp!(r"\bconstexpr\b", 3),
    wp!(r"(?m)\btemplate\s*<", 3),
    wp!(r"(?m)\bauto\s+\w+\s*=", 2),
    wp!(r"(?m)\bclass\s+\w+\s*[:\{]", 2),
    wp!(r"\bvirtual\s+", 2),
    wp!(r#"(?m)#include\s*[<"]"#, 2),
    wp!(r"->\w+", 1),
];

static RUST: &[WeightedPattern] = &[
    wp!(r"(?m)^\s*fn\s+\w+\s*[<(]", 3),
    wp!(r"(?m)^\s*pub\s+(fn|struct|enum|mod|trait|impl)\s", 4),
    wp!(r"(?m)^\s*let\s+mut\s+\w+", 4),
    wp!(r"(?m)^\s*impl\s+\w+", 4),
    wp!(r"(?m)^\s*use\s+\w+(::\w+)+", 3),
    wp!(r"(?m)^\s*match\s+\w+\s*\{", 3),
    wp!(r"\b(Vec|Option|Result|Box|Rc|Arc|String)<\w", 4),
    wp!(r"\bprintln!\s*\(", 5),
    wp!(r"\b\w+\.unwrap\(\)", 3),
    wp!(r"(?m)^\s*#\[derive\(", 5),
    wp!(r"(?m)^\s*mod\s+\w+\s*[;\{]", 2),
    wp!(r"&mut\s+\w+", 3),
    wp!(r#"(?m)^\s*extern\s+"C""#, 3),
    wp!(r"(?m)^\s*trait\s+\w+", 3),
    wp!(r"\bself\.\w+", -1),
    wp!(r"(?m)class\s+\w+", -5),
];

static CLOJURE: &[WeightedPattern] = &[
    wp!(r"(?m)^\s*\(ns\s+[\w.\-]+", 5),
    wp!(r"\(defn\s+\w+", 5),
    wp!(r"\(def\s+\w+", 3),
    wp!(r"\(let\s+\[", 3),
    wp!(r"\(if\s+", 1),
    wp!(r"\(cond\s", 3),
    wp!(r"\(map\s+", 1),
    wp!(r"\(reduce\s+", 2),
    wp!(r#"\(require\s+'"#, 4),
    wp!(r#"\(import\s+'"#, 3),
    wp!(r"#\(", 2),
    wp!(r":\w[\w\-]*\b", 2),
    wp!(r"\(assoc\s", 3),
    wp!(r"\(-> ", 3),
    wp!(r"\(->> ", 3),
    wp!(r"(?m)class\s+\w+", -5),
];

static SQL: &[WeightedPattern] = &[
    wp!(r"(?mi)^\s*SELECT\s+", 3),
    wp!(r"(?i)\bFROM\s+\w+", 2),
    wp!(r"(?i)\bWHERE\s+\w+", 2),
    wp!(r"(?i)\b(INNER|LEFT|RIGHT|FULL|CROSS)\s+JOIN\b", 5),
    wp!(r"(?i)\bINSERT\s+INTO\s+\w+", 4),
    wp!(r"(?i)\bCREATE\s+(TABLE|INDEX|VIEW|DATABASE|PROCEDURE|FUNCTION)\b", 5),
    wp!(r"(?i)\bALTER\s+TABLE\s+\w+", 5),
    wp!(r"(?i)\bDROP\s+(TABLE|INDEX|VIEW|DATABASE)\b", 4),
    wp!(r"(?i)\bGROUP\s+BY\b", 3),
    wp!(r"(?i)\bORDER\s+BY\b", 2),
    wp!(r"(?i)\bHAVING\s+", 3),
    wp!(r"(?i)\bUNION\s+(ALL\s+)?SELECT\b", 4),
    wp!(r"(?i)\bPRIMARY\s+KEY\b", 3),
    wp!(r"(?i)\b(VARCHAR|INTEGER|TEXT|BOOLEAN|TIMESTAMP|BIGINT|DECIMAL)\b", 3),
    wp!(r"(?i)\bNOT\s+NULL\b", 2),
    wp!(r"(?i)\bDEFAULT\s+", 1),
    wp!(r"(?m)\bclass\s+\w+", -5),
];

static PHP_HEUR: &[WeightedPattern] = &[
    wp!(r"<\?php\b", 5),
    wp!(r"\$\w+\s*=\s*", 2),
    wp!(r"\$this->\w+", 4),
    wp!(r"(?m)\bfunction\s+\w+\s*\(", 2),
    wp!(r#"\becho\s+['"\$]"#, 3),
    wp!(r"(?m)\b(public|private|protected)\s+function\b", 4),
    wp!(r"(?m)\bnamespace\s+\w+(\\\w+)*", 3),
    wp!(r"(?m)\buse\s+\w+(\\\w+)+\s*;", 3),
    wp!(r"\bnew\s+\w+\s*\(", 1),
    wp!(r"->\w+\s*\(", 2),
    wp!(r"\b(array|isset|unset|empty|die|exit)\s*\(", 3),
    wp!(r"\$_?(GET|POST|REQUEST|SESSION|SERVER|COOKIE)\b", 5),
    wp!(r"(?m)\bclass\s+\w+\s*(extends|implements)\b", 2),
    wp!(r"=>\s*[\{(\n]", -2),
];

static RUBY: &[WeightedPattern] = &[
    wp!(r"(?m)^\s*def\s+\w+", 3),
    wp!(r"(?m)^\s*end\s*$", 3),
    wp!(r"(?m)^\s*class\s+\w+(\s*<\s*\w+)?", 2),
    wp!(r"(?m)^\s*module\s+\w+", 3),
    wp!(r#"\bputs\s+['"\w]"#, 3),
    wp!(r#"\brequire\s+['""]"#, 3),
    wp!(r#"\brequire_relative\s+['""]"#, 5),
    wp!(r"\battr_(accessor|reader|writer)\s+:", 5),
    wp!(r"\bdo\s*\|[\w,\s]+\|", 4),
    wp!(r"\.(each|map|select|reject|inject|collect)\s*(\{|\bdo\b)", 3),
    wp!(r"\b(nil|true|false)\b", 1),
    wp!(r"@\w+\s*=", 2),
    wp!(r"(?m)^\s*if\s+.*\s*$", 1),
    wp!(r"(?m)^\s*unless\s+", 4),
    wp!(r"\bself\.\w+", 1),
    wp!(r"(?m);\s*$", -2),
];

static SWIFT: &[WeightedPattern] = &[
    wp!(r"(?m)^\s*func\s+\w+\s*\(", 3),
    wp!(r"(?m)^\s*import\s+(Foundation|UIKit|SwiftUI|Combine)\b", 5),
    wp!(r"\bguard\s+let\s+\w+", 5),
    wp!(r"\bguard\s+\w+", 3),
    wp!(r"(?m)\b(struct|class|enum|protocol)\s+\w+\s*[:\{]", 2),
    wp!(r"\bweak\s+var\s+", 4),
    wp!(r"\blet\s+\w+\s*:\s*\w+", 2),
    wp!(r"\bvar\s+\w+\s*:\s*\w+", 2),
    wp!(r"\bif\s+let\s+\w+\s*=", 4),
    wp!(r"\bswitch\s+\w+\s*\{", 1),
    wp!(r#"\bprint\s*\(""#, 1),
    wp!(r"\b@(IBOutlet|IBAction|objc|escaping|Published|State|Binding)\b", 5),
    wp!(r"(?m)\bextension\s+\w+", 3),
    wp!(r"\boptional\s+func\b", 3),
    wp!(r"\b(String|Int|Double|Bool|Array|Dictionary)<?\b", 1),
    wp!(r"\bprintln!\s*\(", -5),
];

static KOTLIN: &[WeightedPattern] = &[
    wp!(r"(?m)^\s*fun\s+\w+\s*[<(]", 4),
    wp!(r"(?m)^\s*val\s+\w+\s*[=:]", 2),
    wp!(r"(?m)^\s*var\s+\w+\s*[=:]", 1),
    wp!(r"(?m)^\s*import\s+\w+\.\w+", 1),
    wp!(r"(?m)^\s*package\s+\w+\.\w+", 2),
    wp!(r"\bcompanion\s+object\b", 5),
    wp!(r"\bdata\s+class\s+\w+", 5),
    wp!(r"\bsealed\s+class\s+\w+", 5),
    wp!(r"(?m)\bobject\s+\w+\s*[:\{]", 3),
    wp!(r"\bwhen\s*\(\w+\)\s*\{", 3),
    wp!(r"\b(listOf|mapOf|setOf|mutableListOf)\s*\(", 4),
    wp!(r"\bprintln\s*\(", 2),
    wp!(r"(?m)\b(suspend|coroutineScope|launch|async)\s", 4),
    wp!(r"\b(String|Int|Double|Boolean|Long|Float)\b", 1),
    wp!(r"\bstd::\w+", -5),
];

static CSHARP: &[WeightedPattern] = &[
    wp!(r"(?m)^\s*using\s+System(\.\w+)*\s*;", 5),
    wp!(r"(?m)^\s*namespace\s+\w+(\.\w+)*", 3),
    wp!(r"(?m)\bpublic\s+(class|struct|interface|enum)\s+\w+", 2),
    wp!(r"\bstatic\s+void\s+Main\s*\(", 5),
    wp!(r"\bConsole\.(Write|WriteLine|ReadLine)\s*\(", 5),
    wp!(r"\bvar\s+\w+\s*=\s*new\s+", 2),
    wp!(r"\basync\s+Task\b", 4),
    wp!(r"\bawait\s+\w+", 1),
    wp!(r"\bstring\.\w+", 2),
    wp!(r"(?m)\b(get|set)\s*[;\{]", 2),
    wp!(r"\bLINQ|\.Select\(|\.Where\(|\.OrderBy\(", 4),
    wp!(r"(?m)^\s*\[[\w.]+(\(.*)?\]\s*$", 2),
    wp!(r"\b(IEnumerable|IList|IDictionary|IQueryable)<", 3),
    wp!(r"\bprintln!\s*\(", -5),
    wp!(r":=\s", -3),
];

static SCALA: &[WeightedPattern] = &[
    wp!(r"(?m)^\s*def\s+\w+\s*[(\[]", 2),
    wp!(r"(?m)^\s*val\s+\w+\s*[=:]", 2),
    wp!(r"(?m)^\s*var\s+\w+\s*[=:]", 1),
    wp!(r"(?m)^\s*object\s+\w+\s*(extends|\{)", 4),
    wp!(r"(?m)^\s*trait\s+\w+", 3),
    wp!(r"(?m)^\s*case\s+class\s+\w+", 5),
    wp!(r"(?m)^\s*sealed\s+trait\b", 5),
    wp!(r"(?m)^\s*import\s+\w+\.(\w+\.)*\{", 3),
    wp!(r"(?m)^\s*package\s+\w+\.\w+", 2),
    wp!(r"\b(List|Map|Set|Option|Either|Future|Seq)\b", 2),
    wp!(r"\bmatch\s*\{", 2),
    wp!(r"(?m)=>\s*$", 1),
    wp!(r"\bprintln\s*\(", 1),
    wp!(r"(?m)\bimplicit\s+(val|def|class)\b", 5),
    wp!(r"\bfor\s*\{", 2),
];

static DART: &[WeightedPattern] = &[
    wp!(r#"(?m)^\s*import\s+['"]package:"#, 5),
    wp!(r"(?m)^\s*void\s+main\s*\(\)\s*(async\s*)?\{", 3),
    wp!(r"\bWidget\s+build\s*\(", 5),
    wp!(r"\b(StatelessWidget|StatefulWidget|State<\w+>)\b", 5),
    wp!(r"\bfinal\s+\w+\s*=", 2),
    wp!(r"\bvar\s+\w+\s*=", 1),
    wp!(r"\blate\s+(final\s+)?\w+\s+\w+", 4),
    wp!(r"\b@override\b", 2),
    wp!(r"\brequired\s+this\.\w+", 4),
    wp!(r"(?m)\bclass\s+\w+\s*extends\s+\w+", 1),
    wp!(r"\bFuture<\w+>", 3),
    wp!(r"\basync\s*\*", 2),
    wp!(r"\bprint\s*\(", 1),
    wp!(r"\b(List|Map|Set|String|int|double|bool|dynamic)\b", 1),
    wp!(r"\bprintln!\s*\(", -5),
];

static POWERSHELL: &[WeightedPattern] = &[
    wp!(r"(?m)^\s*function\s+\w+-\w+", 5),
    wp!(r"\$PSVersionTable\b", 5),
    wp!(r"\$_\b", 3),
    wp!(r"\$\w+\s*=", 2),
    wp!(r"\bGet-\w+", 4),
    wp!(r"\bSet-\w+", 4),
    wp!(r"\bNew-\w+", 4),
    wp!(r"\bInvoke-\w+", 4),
    wp!(r"\bWrite-(Host|Output|Error|Verbose|Warning)\b", 5),
    wp!(r"\|\s*(Where-Object|ForEach-Object|Select-Object|Sort-Object)\b", 5),
    wp!(r"(?m)\bparam\s*\(", 3),
    wp!(r"\[CmdletBinding\(\)\]", 5),
    wp!(r"\[Parameter\s*\(", 4),
    wp!(r"\b-eq\b|-ne\b|-gt\b|-lt\b|-ge\b|-le\b", 3),
    wp!(r"\bconsole\.\w+\s*\(", -5),
];

static SIGNATURES: &[LanguageSignature] = &[
    LanguageSignature { language: "python", patterns: PYTHON },
    LanguageSignature { language: "javascript", patterns: JAVASCRIPT },
    LanguageSignature { language: "typescript", patterns: TYPESCRIPT },
    LanguageSignature { language: "css", patterns: CSS },
    LanguageSignature { language: "shell", patterns: SHELL },
    LanguageSignature { language: "java", patterns: JAVA },
    LanguageSignature { language: "go", patterns: GO },
    LanguageSignature { language: "c", patterns: C_LANG },
    LanguageSignature { language: "cpp", patterns: CPP },
    LanguageSignature { language: "rust", patterns: RUST },
    LanguageSignature { language: "clojure", patterns: CLOJURE },
    LanguageSignature { language: "sql", patterns: SQL },
    LanguageSignature { language: "php", patterns: PHP_HEUR },
    LanguageSignature { language: "ruby", patterns: RUBY },
    LanguageSignature { language: "swift", patterns: SWIFT },
    LanguageSignature { language: "kotlin", patterns: KOTLIN },
    LanguageSignature { language: "csharp", patterns: CSHARP },
    LanguageSignature { language: "scala", patterns: SCALA },
    LanguageSignature { language: "dart", patterns: DART },
    LanguageSignature { language: "powershell", patterns: POWERSHELL },
];

struct CompiledPattern {
    regex: Regex,
    weight: i32,
}

struct CompiledSignature {
    language: &'static str,
    patterns: Vec<CompiledPattern>,
}

static COMPILED: LazyLock<Vec<CompiledSignature>> = LazyLock::new(|| {
    SIGNATURES
        .iter()
        .map(|sig| CompiledSignature {
            language: sig.language,
            patterns: sig
                .patterns
                .iter()
                .map(|wp| CompiledPattern {
                    regex: Regex::new(wp.pattern).unwrap(),
                    weight: wp.weight,
                })
                .collect(),
        })
        .collect()
});

/// Detect language by heuristic pattern scoring.
///
/// Returns the highest-scoring language above the threshold, with superset
/// tie-breaking (TypeScript beats JavaScript, C++ beats C) and density
/// bonuses for repeated matches.
pub fn detect_by_scoring(content: &str) -> Option<&'static str> {
    use std::collections::HashMap;

    let mut scores: HashMap<&str, i32> = HashMap::new();
    let mut partial_best: Option<&str> = None;
    let mut partial_best_score = 0i32;

    // Pre-compute ES module signals
    static ES_IMPORT: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r#"(?m)^\s*import\s+[\w\{*].*\s+from\s+['"`]"#).unwrap());
    static ES_EXPORT: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"(?m)^\s*export\s+(const|let|var|function|class|default|type|interface|enum)\s")
            .unwrap()
    });
    let has_es_module = ES_IMPORT.is_match(content) || ES_EXPORT.is_match(content);

    for sig in COMPILED.iter() {
        // ES module guard: file is definitively JS/TS — skip others
        if has_es_module && sig.language != "javascript" && sig.language != "typescript" {
            continue;
        }

        let mut score = 0i32;
        for pat in &sig.patterns {
            if pat.weight > 0 {
                let match_count = pat.regex.find_iter(content).count();
                if match_count > 0 {
                    // Base weight + diminishing returns for repeats
                    score += pat.weight + (match_count as i32 - 1).min(3);
                }
            } else {
                // Anti-signals: just test presence
                if pat.regex.is_match(content) {
                    score += pat.weight;
                }
            }
        }

        if score < 0 {
            continue;
        }

        if score >= HEURISTIC_SCORE_THRESHOLD {
            scores.insert(sig.language, score);
        } else if score > partial_best_score {
            partial_best = Some(sig.language);
            partial_best_score = score;
        }
    }

    // Confident matches — pick the best
    if !scores.is_empty() {
        // Superset tie-breaking: TypeScript ⊃ JavaScript, C++ ⊃ C
        resolve_superset(&mut scores, "typescript", "javascript");
        resolve_superset(&mut scores, "cpp", "c");

        let best = scores
            .iter()
            .max_by_key(|(_, &score)| score)
            .map(|(&lang, _)| lang);

        return best;
    }

    // Best-guess fallback
    if partial_best_score >= PARTIAL_SCORE_THRESHOLD {
        return partial_best;
    }

    None
}

/// If both a superset language and its base language scored above threshold,
/// and the superset's score is ≥ 60% of the base, the base is removed.
fn resolve_superset(scores: &mut std::collections::HashMap<&str, i32>, superset: &str, base: &str) {
    let super_score = scores.get(superset).copied();
    let base_score = scores.get(base).copied();
    if let (Some(ss), Some(bs)) = (super_score, base_score) {
        if ss as f64 >= bs as f64 * 0.6 {
            scores.remove(base);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn python_def_class() {
        let content = r#"
import os

class MyApp:
    def __init__(self):
        self.name = "test"

    def run(self):
        print("running")
"#;
        assert_eq!(detect_by_scoring(content), Some("python"));
    }

    #[test]
    fn javascript_commonjs() {
        let content = r#"
const express = require('express');
const app = express();

app.get('/', (req, res) => {
    res.send('Hello');
});

module.exports = app;
"#;
        assert_eq!(detect_by_scoring(content), Some("javascript"));
    }

    #[test]
    fn typescript_interface() {
        let content = r#"
interface User {
    name: string;
    age: number;
    active: boolean;
}

type Result<T> = { data: T } | { error: string };

const getUser = async (id: number): Promise<User> => {
    return { name: "Alice", age: 30, active: true };
};
"#;
        assert_eq!(detect_by_scoring(content), Some("typescript"));
    }

    #[test]
    fn rust_derive_fn() {
        let content = r#"
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct Config {
    pub name: String,
    pub values: HashMap<String, String>,
}

pub fn process(config: &Config) -> Result<(), String> {
    println!("Processing: {}", config.name);
    Ok(())
}
"#;
        assert_eq!(detect_by_scoring(content), Some("rust"));
    }

    #[test]
    fn go_package_func() {
        let content = r#"
package main

import "fmt"

func main() {
    result, err := compute(42)
    if err != nil {
        fmt.Println("error:", err)
    }
    fmt.Println(result)
}
"#;
        assert_eq!(detect_by_scoring(content), Some("go"));
    }

    #[test]
    fn sql_create_select() {
        let content = r#"
CREATE TABLE users (
    id INTEGER PRIMARY KEY,
    name VARCHAR(255) NOT NULL,
    email TEXT
);

SELECT u.name, COUNT(o.id) as order_count
FROM users u
LEFT JOIN orders o ON u.id = o.user_id
GROUP BY u.name
ORDER BY order_count DESC;
"#;
        assert_eq!(detect_by_scoring(content), Some("sql"));
    }

    #[test]
    fn shell_script() {
        let content = r#"
#!/bin/bash
export PATH="/usr/local/bin:$PATH"

if [[ -z "$1" ]]; then
    echo "Usage: $0 <dir>"
    exit 1
fi

for f in "$1"/*.txt; do
    echo "Processing $f"
done
"#;
        assert_eq!(detect_by_scoring(content), Some("shell"));
    }

    #[test]
    fn cpp_with_std() {
        let content = r#"
#include <iostream>
#include <vector>

int main() {
    std::vector<int> nums = {1, 2, 3};
    for (auto& n : nums) {
        std::cout << n << std::endl;
    }
    return 0;
}
"#;
        assert_eq!(detect_by_scoring(content), Some("cpp"));
    }

    #[test]
    fn java_public_class() {
        let content = r#"
import java.util.ArrayList;
import java.util.List;

public class Main {
    public static void main(String[] args) {
        List<String> names = new ArrayList<>();
        names.add("Alice");
        System.out.println(names);
    }
}
"#;
        assert_eq!(detect_by_scoring(content), Some("java"));
    }
}