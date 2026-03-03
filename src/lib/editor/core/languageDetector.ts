/**
 * languageDetector.ts
 *
 * Production-grade content-based language detection for Grayslate.
 *
 * Fully synchronous, deterministic pipeline — no ML model dependency.
 *
 * Detection cascade (ordered by priority & reliability):
 * ┌────────┬──────────────────────────────────────────────────┐
 * │ Phase 1│ File extension      (instant, deterministic)     │
 * │ Phase 2│ Shebang line        (instant, deterministic)     │
 * │ Phase 3│ Structural signals  (fast, high confidence)      │
 * │ Phase 4│ Heuristic scoring   (fast, medium confidence)    │
 * └────────┴──────────────────────────────────────────────────┘
 *
 * Each phase either returns a confident result or defers to the next.
 * Structural detection handles deterministic formats (JSON, XML, HTML,
 * CSV, Dockerfile, Markdown, Sass/SCSS, YAML, TOML). Heuristic scoring handles
 * programming languages via weighted pattern matching with a best-guess
 * fallback for ambiguous content.
 */

// ════════════════════════════════════════════════════════════════
// Configuration
// ════════════════════════════════════════════════════════════════

/** Max bytes analysed — keeps detection < 10 ms even for huge pastes. */
const MAX_CONTENT_LENGTH = 50_000;

/** Minimum total score for heuristic scoring to return a confident result. */
const HEURISTIC_SCORE_THRESHOLD = 3;

/**
 * Minimum score for the best-guess fallback when no language clears
 * HEURISTIC_SCORE_THRESHOLD.  Must be < HEURISTIC_SCORE_THRESHOLD.
 * Avoids returning a guess on a single weak hit.
 */
const PARTIAL_SCORE_THRESHOLD = 2;

// ════════════════════════════════════════════════════════════════
// Phase 1 — File Extension Map
// ════════════════════════════════════════════════════════════════

/**
 * Maps lowercase file extensions to internal language IDs.
 * Covers all common extensions for languages the editor supports.
 */
const EXTENSION_MAP: Record<string, string> = {
    // ── Data formats ─────────────────────────────────────────
    ".json": "json", ".jsonc": "json", ".json5": "json",
    ".geojson": "json", ".webmanifest": "json", ".har": "json",
    ".csv": "csv", ".tsv": "csv",
    ".xml": "xml", ".svg": "xml", ".plist": "xml", ".xsl": "xml",
    ".xslt": "xml", ".xsd": "xml", ".wsdl": "xml", ".rss": "xml",
    ".atom": "xml", ".xaml": "xml", ".csproj": "xml", ".fsproj": "xml",
    ".vcxproj": "xml",

    // ── Config ───────────────────────────────────────────────
    ".yaml": "yaml", ".yml": "yaml",
    ".toml": "toml", ".ini": "text", ".cfg": "text", ".env": "text",

    // ── Markup ───────────────────────────────────────────────
    ".html": "html", ".htm": "html", ".xhtml": "html",
    ".svelte": "svelte", ".vue": "vue",
    ".md": "markdown", ".markdown": "markdown", ".mdx": "markdown",

    // ── Web languages ────────────────────────────────────────
    ".js": "javascript", ".mjs": "javascript", ".cjs": "javascript",
    ".jsx": "javascript",
    ".ts": "typescript", ".tsx": "typescript", ".mts": "typescript",
    ".cts": "typescript",
    ".css": "css", ".less": "css",
    ".scss": "scss", ".sass": "sass",

    // ── Systems / compiled ───────────────────────────────────
    ".py": "python", ".pyi": "python", ".pyw": "python",
    ".c": "c", ".h": "c",
    ".cpp": "cpp", ".cxx": "cpp", ".cc": "cpp",
    ".hpp": "cpp", ".hxx": "cpp", ".hh": "cpp",
    ".java": "java",
    ".go": "go",
    ".rs": "rust",
    ".rb": "ruby",
    ".php": "php", ".php3": "php", ".php4": "php",
    ".php5": "php", ".php7": "php", ".phtml": "php",
    ".swift": "swift",
    ".kt": "kotlin", ".kts": "kotlin",
    ".cs": "csharp",
    ".scala": "scala",
    ".dart": "dart",
    ".m": "objectivec",
    ".mm": "objectivecpp",
    ".lua": "text", ".pl": "text", ".pm": "text",

    // ── Functional ───────────────────────────────────────────
    ".clj": "clojure", ".cljs": "clojure", ".cljc": "clojure", ".edn": "clojure",

    // ── Shell ────────────────────────────────────────────────
    ".sh": "shell", ".bash": "shell", ".zsh": "shell",
    ".fish": "shell", ".ksh": "shell",
    ".ps1": "powershell", ".psd1": "powershell", ".psm1": "powershell",
    ".bat": "text", ".cmd": "text",

    // ── Dockerfile (explicit extension) ──────────────────────
    ".dockerfile": "dockerfile",

    // ── SQL ──────────────────────────────────────────────────
    ".sql": "sql",

    // ── Template languages ──────────────────────────────────
    ".j2": "jinja", ".jinja": "jinja", ".jinja2": "jinja",
};

/**
 * Maps full (lowercased) filenames to language IDs.
 * Handles extensionless files like Dockerfile, Makefile, .bashrc, etc.
 */
const FILENAME_MAP: Record<string, string> = {
    "dockerfile": "dockerfile",
    "makefile": "shell",
    "gnumakefile": "shell",
    ".bashrc": "shell",
    ".bash_profile": "shell",
    ".bash_aliases": "shell",
    ".zshrc": "shell",
    ".zprofile": "shell",
    ".profile": "shell",
    ".editorconfig": "yaml",
    ".gitignore": "text",
    ".gitattributes": "text",
    ".env": "text",
    ".env.local": "text",
    "jenkinsfile": "text",
    "vagrantfile": "text",
    "cargo.toml": "toml", // Rust project manifest
    "cargo.lock": "toml",
    "deps.edn": "clojure",
    "gemfile": "ruby",
    "rakefile": "ruby",
};

/**
 * Regex-based filename patterns for files that can't be matched by
 * exact name. Checked after FILENAME_MAP.
 */
const FILENAME_PATTERNS: [RegExp, string][] = [
    [/^nginx.*\.conf$/i, "nginx"],
];

// ════════════════════════════════════════════════════════════════
// Phase 2 — Shebang Patterns
// ════════════════════════════════════════════════════════════════

/** Regex → language pairs for shebang detection. First match wins. */
const SHEBANG_PATTERNS: [RegExp, string][] = [
    [/\bpython[23w]?\b/, "python"],
    [/\bnode(js)?\b/, "javascript"],
    [/\bdeno\b/, "typescript"],
    [/\b(ba|z|k|fi)?sh\b/, "shell"],
    [/\bperl\b/, "text"],
    [/\bruby\b/, "ruby"],
    [/\bphp\b/, "php"],
];

// ════════════════════════════════════════════════════════════════
// Phase 4 — Heuristic Pattern Signatures
// ════════════════════════════════════════════════════════════════

/** A weighted pattern: [regex, score_if_matched]. */
type WeightedPattern = [RegExp, number];

interface LanguageSignature {
    language: string;
    patterns: WeightedPattern[];
}

/**
 * Weighted pattern sets for heuristic-based language scoring.
 *   • Higher weight → more distinctive / unique to that language.
 *   • Total score ≥ HEURISTIC_SCORE_THRESHOLD triggers detection.
 */
const LANGUAGE_SIGNATURES: LanguageSignature[] = [
    // ── Python ───────────────────────────────────────────────
    {
        language: "python",
        patterns: [
            [/^\s*def\s+\w+\s*\(/m, 3],
            [/^\s*class\s+\w+[^:\n]{0,80}:/m, 3],
            [/^\s*from\s+\w+\s+import\s/m, 3],
            [/^\s*import\s+\w+/m, 2],
            [/^\s*elif\s+/m, 5],  // unique to Python
            [/if\s+__name__\s*==\s*['"]__main__['"]/, 5],
            [/\bself\.\w+/, 3],
            [/^\s*@\w+(\.\w+)*(\(.*\))?\s*$/m, 2],  // decorators
            [/^\s*(try|except|finally)\s*:/m, 2],
            [/\b(None|True|False)\b/, 1],
            [/^\s*with\s+\w+[^:\n]{0,80}\s+as\s+/m, 3],
            [/^\s*raise\s+\w+/m, 2],
            [/^\s*yield\s+/m, 2],
            [/\bprint\s*\(/, 1],
            [/\blen\s*\(/, 1],
            [/^\s*async\s+def\s+\w+/m, 4],  // async def (unique to Python)
            [/;\s*$/m, -2],  // Python never uses trailing semicolons
            [/\{[\s]*$/, -2],  // Curly-brace blocks are never Python
            [/^\s*\}\s*$/m, -1],
        ],
    },
    // ── JavaScript ───────────────────────────────────────────
    {
        language: "javascript",
        patterns: [
            [/\b(const|let|var)\s+\w+\s*=/m, 2],
            [/\bfunction\s+\w*\s*\(/m, 2],
            [/=>\s*[{(\n]/m, 3],  // arrow functions
            [/\brequire\s*\(['"`]/m, 4],  // CommonJS
            [/\bmodule\.exports\b/, 4],
            [/\bconsole\.\w+\s*\(/, 2],
            [/===|!==/, 2],  // strict equality
            [/\bdocument\.\w+/, 2],
            [/\bwindow\.\w+/, 1],
            [/\bPromise\.(all|resolve|reject)\b/, 2],
            [/\.then\s*\(/, 1],
            [/\.catch\s*\(/, 1],
            [/\basync\s+(function|\w+\s*=>|\w+\s*\()/m, 2],
            [/\bawait\s+/, 1],
            [/:\s*(string|number|boolean|void)\b/, -3],  // Type annotations are TS, not JS
        ],
    },
    // ── TypeScript ───────────────────────────────────────────
    {
        language: "typescript",
        patterns: [
            [/\binterface\s+\w+/m, 4],
            [/\btype\s+\w+\s*=\s*/m, 4],
            [/:\s*(string|number|boolean|void|any|never|unknown|undefined)\b/, 3],
            [/\benum\s+\w+\s*\{/m, 4],
            [/\bnamespace\s+\w+/m, 3],
            [/\bdeclare\s+(const|function|class|module|type|interface)/m, 4],
            [/\b(Readonly|Partial|Record|Pick|Omit|Required)</, 4],
            [/\bas\s+(string|number|any|unknown|\w+)\b/, 3],
            [/<\w+(\s+extends\s+\w+)?>/, 2],  // generics
            [/\b(const|let|var)\s+\w+\s*=/m, 1],  // also JS features
            [/=>\s*[{(\n]/m, 1],
            [/===|!==/, 1],
        ],
    },
    // ── CSS ──────────────────────────────────────────────────
    {
        language: "css",
        patterns: [
            [/[.#][\w-]+\s*\{/m, 3],  // .class { or #id {
            [/@media\s*[\s(]/m, 4],
            [/@keyframes\s+\w+/m, 4],
            [/@import\s+/m, 2],
            [/!important\s*;/, 3],
            [/:hover|:focus|:active|::before|::after/, 3],
            [/\bvar\s*\(--[\w-]+\)/, 3],  // CSS custom properties
            [/\b(color|margin|padding|display|font-size|background|border|width|height)\s*:/m, 2],
            [/\b(flex|grid|block|inline|none)\s*;/, 1],
            [/@tailwind|@apply/, 3],
            [/^\s*(function|const|let|var)\s/m, -5],  // Code keywords are never CSS
        ],
    },
    // ── Shell / Bash ─────────────────────────────────────────
    {
        language: "shell",
        patterns: [
            [/^\s*echo\s+["$']/m, 2],
            [/^\s*if\s+\[\[?\s/m, 3],
            [/^\s*fi\s*$/m, 5],  // nearly unique to shell
            [/^\s*done\s*$/m, 4],
            [/^\s*esac\s*$/m, 5],  // nearly unique to shell
            [/^\s*export\s+\w+=/m, 3],
            [/\$\{[\w?!#@*+-]+/, 2],  // parameter expansion
            [/\$\(.*\)/, 2],  // command substitution
            [/^\s*case\s+.*\s+in\s*$/m, 3],
            [/^\s*(alias|source|chmod|mkdir|rm\s|cp\s|mv\s|cd\s|grep|sed|awk)\s/m, 2],
            [/<<-?\s*['"]?\w+['"]?/, 3],  // heredoc
            [/\bconsole\.\w+\s*\(/, -5],  // console.log is never shell
        ],
    },
    // ── Java ─────────────────────────────────────────────────
    {
        language: "java",
        patterns: [
            [/\bpublic\s+class\s+\w+/m, 4],
            [/\bpublic\s+static\s+void\s+main/m, 5],
            [/\bSystem\.out\.print(ln)?\s*\(/, 5],
            [/\bimport\s+java\.\w+/m, 5],
            [/\bimport\s+javax\.\w+/m, 5],
            [/\bimport\s+org\.\w+/m, 4],  // org.* imports (Spring, Apache, etc.)
            [/@Override\b/, 3],
            [/\bthrows\s+\w+/m, 2],
            [/\bextends\s+\w+/m, 1],
            [/\bimplements\s+\w+/m, 2],
            [/\bprivate\s+(final\s+)?\w+\s+\w+/m, 2],
            [/=>\s*[{(\n]/, -3],  // Arrow functions are never Java
        ],
    },
    // ── Go ───────────────────────────────────────────────────
    {
        language: "go",
        patterns: [
            [/^package\s+\w+\s*$/m, 5],
            [/^\s*func\s+\w+\s*\(/m, 3],
            [/^\s*func\s+\(\w+\s+\*?\w+\)\s+\w+/m, 5],  // method receivers
            [/\bfmt\.\w+/, 4],
            [/\bimport\s+\(/m, 3],
            [/\bgo\s+func\b/, 4],  // goroutines
            [/\bchan\s+\w+/, 4],
            [/:=\s/, 2],  // short var declaration
            [/\bif\s+err\s*!=\s*nil\b/, 4],  // idiomatic Go error handling
            [/\bdefer\s+\w+/m, 3],
            [/\bpackage\s+main\b/, 4],
            [/\bclass\s+\w+/m, -5],  // Go has no class keyword
            [/^\s*import\s+\w+\s*$/m, -2],  // Single import x is Python/JS, not Go
        ],
    },
    // ── C ────────────────────────────────────────────────────
    {
        language: "c",
        patterns: [
            [/#include\s*[<"]/m, 3],
            [/\bint\s+main\s*\(/m, 4],
            [/\bprintf\s*\(/, 3],
            [/\b(malloc|calloc|realloc|free)\s*\(/, 4],
            [/#define\s+\w+/m, 2],
            [/\btypedef\s+/m, 2],
            [/\bstruct\s+\w+\s*\{/m, 2],
            [/\bsizeof\s*\(/, 2],
            [/\bNULL\b/, 2],
            [/->\w+/, 1],
            [/\bvoid\s+\w+\s*\(/m, 1],
            [/\bstd::\w+/, -5],  // C++ namespaces are never C
        ],
    },
    // ── C++ ──────────────────────────────────────────────────
    {
        language: "cpp",
        patterns: [
            [/\bstd::\w+/, 5],
            [/\bcout\s*<</, 5],
            [/\bcin\s*>>/, 5],
            [/#include\s*<(iostream|string|vector|map|set|algorithm|memory|functional)>/m, 5],
            [/\busing\s+namespace\s+std\b/, 5],
            [/\bnullptr\b/, 4],
            [/\b(unique_ptr|shared_ptr|weak_ptr)</, 4],  // C++ smart pointers
            [/\bconstexpr\b/, 3],  // constexpr (C++11+)
            [/\btemplate\s*</m, 3],
            [/\bauto\s+\w+\s*=/m, 2],
            [/\bclass\s+\w+\s*[:{]/m, 2],
            [/\bvirtual\s+/m, 2],
            [/#include\s*[<"]/m, 2],  // shared with C
            [/->\w+/, 1],
        ],
    },
    // ── Rust ─────────────────────────────────────────────────
    {
        language: "rust",
        patterns: [
            [/^\s*fn\s+\w+\s*[<(]/m, 3],   // fn declaration
            [/^\s*pub\s+(fn|struct|enum|mod|trait|impl)\s/m, 4],   // pub visibility
            [/^\s*let\s+mut\s+\w+/m, 4],   // let mut (unique)
            [/^\s*impl\s+\w+/m, 4],   // impl blocks
            [/^\s*use\s+\w+(::\w+)+/m, 3],   // use std::io
            [/^\s*match\s+\w+\s*\{/m, 3],   // match expr
            [/\b(Vec|Option|Result|Box|Rc|Arc|String)<\w/, 4],   // Rust std types
            [/\bprintln!\s*\(/, 5],   // println! macro (near-unique)
            [/\b\w+\.unwrap\(\)/, 3],   // .unwrap()
            [/^\s*#\[derive\(/m, 5],   // #[derive(...)] (unique)
            [/^\s*mod\s+\w+\s*[;{]/m, 2],   // mod declarations
            [/&mut\s+\w+/, 3],   // mutable ref
            [/^\s*extern\s+"C"/m, 3],   // FFI
            [/^\s*trait\s+\w+/m, 3],   // trait def
            [/\bself\.\w+/, -1],   // penalize (also Python)
            [/class\s+\w+/m, -5],   // Rust has no class
        ],
    },
    // ── Clojure ──────────────────────────────────────────────
    {
        language: "clojure",
        patterns: [
            [/^\s*\(ns\s+[\w.-]+/m, 5],   // namespace declaration
            [/\(defn\s+\w+/, 5],   // function definition
            [/\(def\s+\w+/, 3],   // binding definition
            [/\(let\s+\[/, 3],   // let binding
            [/\(if\s+/, 1],   // if form
            [/\(cond\s/, 3],   // cond (Lisp-ish)
            [/\(map\s+/, 1],   // map
            [/\(reduce\s+/, 2],   // reduce
            [/\(require\s+'/, 4],   // require
            [/\(import\s+'/, 3],   // import
            [/#\(/, 2],   // anonymous fn #(...)
            [/:\w[\w-]*\b/, 2],   // keywords :foo
            [/\(assoc\s/, 3],   // assoc
            [/\(-> /, 3],   // threading macro
            [/\(->> /, 3],   // threading macro
            [/class\s+\w+/m, -5],   // Clojure has no class decl
        ],
    },
    // ── SQL ──────────────────────────────────────────────────
    {
        language: "sql",
        patterns: [
            [/^\s*SELECT\s+/im, 3],   // SELECT statement
            [/\bFROM\s+\w+/im, 2],   // FROM clause
            [/\bWHERE\s+\w+/im, 2],   // WHERE clause
            [/\b(INNER|LEFT|RIGHT|FULL|CROSS)\s+JOIN\b/im, 5],   // JOINs (near-unique)
            [/\bINSERT\s+INTO\s+\w+/im, 4],   // INSERT
            [/\bCREATE\s+(TABLE|INDEX|VIEW|DATABASE|PROCEDURE|FUNCTION)\b/im, 5],
            [/\bALTER\s+TABLE\s+\w+/im, 5],   // ALTER TABLE
            [/\bDROP\s+(TABLE|INDEX|VIEW|DATABASE)\b/im, 4],
            [/\bGROUP\s+BY\b/im, 3],
            [/\bORDER\s+BY\b/im, 2],
            [/\bHAVING\s+/im, 3],
            [/\bUNION\s+(ALL\s+)?SELECT\b/im, 4],
            [/\bPRIMARY\s+KEY\b/im, 3],
            [/\b(VARCHAR|INTEGER|TEXT|BOOLEAN|TIMESTAMP|BIGINT|DECIMAL)\b/im, 3],
            [/\bNOT\s+NULL\b/im, 2],
            [/\bDEFAULT\s+/im, 1],
            [/\bclass\s+\w+/m, -5],   // SQL has no class keyword
        ],
    },
    // ── PHP ──────────────────────────────────────────────────
    {
        language: "php",
        patterns: [
            [/<\?php\b/, 5],   // opening tag (near-unique)
            [/\$\w+\s*=\s*/, 2],   // variable assignment ($var = ...)
            [/\$this->\w+/, 4],   // $this-> (OOP PHP)
            [/\bfunction\s+\w+\s*\(/m, 2],
            [/\becho\s+['"\$]/, 3],   // echo statement
            [/\b(public|private|protected)\s+function\b/m, 4],   // visibility + function
            [/\bnamespace\s+\w+(\\\w+)*/m, 3],   // namespace App\Models
            [/\buse\s+\w+(\\\w+)+\s*;/m, 3],   // use statements
            [/\bnew\s+\w+\s*\(/m, 1],
            [/->\w+\s*\(/m, 2],   // method calls ->method()
            [/\b(array|isset|unset|empty|die|exit)\s*\(/, 3],   // PHP builtins
            [/\$_?(GET|POST|REQUEST|SESSION|SERVER|COOKIE)\b/, 5],   // superglobals
            [/\bclass\s+\w+\s*(extends|implements)\b/m, 2],
            [/=>\s*[{(\n]/m, -2],   // arrow functions are JS/TS
        ],
    },
    // ── Ruby ─────────────────────────────────────────────────
    {
        language: "ruby",
        patterns: [
            [/^\s*def\s+\w+/m, 3],   // method definition
            [/^\s*end\s*$/m, 3],   // end keyword (block closer)
            [/^\s*class\s+\w+(\s*<\s*\w+)?/m, 2],   // class with optional inheritance
            [/^\s*module\s+\w+/m, 3],   // module declaration
            [/\bputs\s+['"\w]/, 3],   // puts (common Ruby I/O)
            [/\brequire\s+['"]/, 3],   // require statements
            [/\brequire_relative\s+['"]/, 5],   // require_relative (unique)
            [/\battr_(accessor|reader|writer)\s+:/, 5],   // attr_accessor (unique)
            [/\bdo\s*\|[\w,\s]+\|/, 4],   // block with params do |x|
            [/\.(each|map|select|reject|inject|collect)\s*(\{|\bdo\b)/, 3],   // iterator methods
            [/\b(nil|true|false)\b/, 1],
            [/@\w+\s*=/, 2],   // instance variable assignment
            [/^\s*if\s+.*\s*$/m, 1],
            [/^\s*unless\s+/, 4],   // unless (fairly unique to Ruby)
            [/\bself\.\w+/, 1],
            [/;\s*$/m, -2],   // Ruby rarely uses semicolons
        ],
    },
    // ── Swift ────────────────────────────────────────────────
    {
        language: "swift",
        patterns: [
            [/^\s*func\s+\w+\s*\(/m, 3],   // function declaration
            [/^\s*import\s+(Foundation|UIKit|SwiftUI|Combine)\b/m, 5],   // framework imports
            [/\bguard\s+let\s+\w+/m, 5],   // guard let (near-unique)
            [/\bguard\s+\w+/m, 3],   // guard statement
            [/\b(struct|class|enum|protocol)\s+\w+\s*[:{]/m, 2],
            [/\bweak\s+var\s+/, 4],   // weak reference
            [/\blet\s+\w+\s*:\s*\w+/, 2],   // typed let binding
            [/\bvar\s+\w+\s*:\s*\w+/, 2],   // typed var binding
            [/\bif\s+let\s+\w+\s*=/, 4],   // optional binding (unique to Swift)
            [/\bswitch\s+\w+\s*\{/m, 1],
            [/\bprint\s*\("/, 1],   // print()
            [/\b@(IBOutlet|IBAction|objc|escaping|Published|State|Binding)\b/, 5],   // attributes
            [/\bextension\s+\w+/m, 3],   // extension keyword
            [/\boptional\s+func\b/, 3],
            [/\b(String|Int|Double|Bool|Array|Dictionary)<?\b/, 1],
            [/\bprintln!\s*\(/, -5],   // Rust macro, not Swift
        ],
    },
    // ── Kotlin ───────────────────────────────────────────────
    {
        language: "kotlin",
        patterns: [
            [/^\s*fun\s+\w+\s*[<(]/m, 4],   // fun declaration (fairly unique)
            [/^\s*val\s+\w+\s*[=:]/m, 2],   // val binding
            [/^\s*var\s+\w+\s*[=:]/m, 1],   // var binding
            [/^\s*import\s+\w+\.\w+/m, 1],
            [/^\s*package\s+\w+\.\w+/m, 2],
            [/\bcompanion\s+object\b/m, 5],   // companion object (unique)
            [/\bdata\s+class\s+\w+/m, 5],   // data class (unique)
            [/\bsealed\s+class\s+\w+/m, 5],   // sealed class (unique)
            [/\bobject\s+\w+\s*[:{]/m, 3],   // object declaration
            [/\bwhen\s*\(\w+\)\s*\{/m, 3],   // when expression
            [/\b(listOf|mapOf|setOf|mutableListOf)\s*\(/, 4],   // Kotlin stdlib
            [/\bprintln\s*\(/, 2],   // println (shared w/ several)
            [/\b(suspend|coroutineScope|launch|async)\s/m, 4],   // coroutines
            [/\b(String|Int|Double|Boolean|Long|Float)\b/, 1],
            [/\bstd::\w+/, -5],   // C++ namespaces are not Kotlin
        ],
    },
    // ── C# ───────────────────────────────────────────────────
    {
        language: "csharp",
        patterns: [
            [/^\s*using\s+System(\.\w+)*\s*;/m, 5],   // using System (unique)
            [/^\s*namespace\s+\w+(\.\w+)*/m, 3],   // namespace
            [/\bpublic\s+(class|struct|interface|enum)\s+\w+/m, 2],
            [/\bstatic\s+void\s+Main\s*\(/m, 5],   // Main entry point
            [/\bConsole\.(Write|WriteLine|ReadLine)\s*\(/, 5],   // Console I/O (unique)
            [/\bvar\s+\w+\s*=\s*new\s+/, 2],
            [/\basync\s+Task\b/, 4],   // async Task (C# async)
            [/\bawait\s+\w+/, 1],
            [/\bstring\.\w+/, 2],
            [/\b(get|set)\s*[;{]/m, 2],   // property accessors
            [/\bLINQ|\.Select\(|\.Where\(|\.OrderBy\(/, 4],   // LINQ
            [/^\s*\[[\w.]+(\(.*)?\]\s*$/m, 2],   // attributes [Attribute]
            [/\b(IEnumerable|IList|IDictionary|IQueryable)</, 3],   // .NET interfaces
            [/\bprintln!\s*\(/, -5],   // Rust macro
            [/:=\s/, -3],   // Go short declaration
        ],
    },
    // ── Scala ────────────────────────────────────────────────
    {
        language: "scala",
        patterns: [
            [/^\s*def\s+\w+\s*[[(]/m, 2],   // def method
            [/^\s*val\s+\w+\s*[=:]/m, 2],   // val
            [/^\s*var\s+\w+\s*[=:]/m, 1],   // var
            [/^\s*object\s+\w+\s*(extends|\{)/m, 4],   // object (singleton)
            [/^\s*trait\s+\w+/m, 3],   // trait
            [/^\s*case\s+class\s+\w+/m, 5],   // case class (near-unique)
            [/^\s*sealed\s+trait\b/m, 5],   // sealed trait (unique)
            [/^\s*import\s+\w+\.(\w+\.)*\{/m, 3],   // import with braces
            [/^\s*package\s+\w+\.\w+/m, 2],
            [/\b(List|Map|Set|Option|Either|Future|Seq)\b/, 2],   // Scala collections
            [/\bmatch\s*\{/m, 2],   // pattern matching
            [/=>\s*$/m, 1],   // fat arrow in match
            [/\bprintln\s*\(/, 1],
            [/\bimplicit\s+(val|def|class)\b/m, 5],   // implicit (unique)
            [/\bfor\s*\{/m, 2],   // for-comprehension
        ],
    },
    // ── Dart ─────────────────────────────────────────────────
    {
        language: "dart",
        patterns: [
            [/^\s*import\s+['"]package:/, 5],   // import 'package:...' (unique)
            [/^\s*void\s+main\s*\(\)\s*(async\s*)?\{/m, 3],   // main()
            [/\bWidget\s+build\s*\(/m, 5],   // Flutter Widget build()
            [/\b(StatelessWidget|StatefulWidget|State<\w+>)\b/, 5],   // Flutter classes
            [/\bfinal\s+\w+\s*=/, 2],
            [/\bvar\s+\w+\s*=/, 1],
            [/\blate\s+(final\s+)?\w+\s+\w+/m, 4],   // late keyword (unique)
            [/\b@override\b/, 2],
            [/\brequired\s+this\.\w+/, 4],   // named constructor params
            [/\bclass\s+\w+\s*extends\s+\w+/m, 1],
            [/\bFuture<\w+>/, 3],   // Future type
            [/\basync\s*\*/m, 2],   // async generator
            [/\bprint\s*\(/, 1],
            [/\b(List|Map|Set|String|int|double|bool|dynamic)\b/, 1],
            [/\bprintln!\s*\(/, -5],   // Rust macro
        ],
    },
    // ── PowerShell ───────────────────────────────────────────
    {
        language: "powershell",
        patterns: [
            [/^\s*function\s+\w+-\w+/m, 5],   // Verb-Noun function names (unique)
            [/\$PSVersionTable\b/, 5],   // unique variable
            [/\$_\b/, 3],   // pipeline variable
            [/\$\w+\s*=/, 2],   // variable assignment
            [/\bGet-\w+/, 4],   // Get- cmdlet
            [/\bSet-\w+/, 4],   // Set- cmdlet
            [/\bNew-\w+/, 4],   // New- cmdlet
            [/\bInvoke-\w+/, 4],   // Invoke- cmdlet
            [/\bWrite-(Host|Output|Error|Verbose|Warning)\b/, 5],   // Write- cmdlets
            [/\|\s*(Where-Object|ForEach-Object|Select-Object|Sort-Object)\b/, 5],   // pipeline cmdlets
            [/\bparam\s*\(/m, 3],   // param block
            [/\[CmdletBinding\(\)\]/, 5],   // CmdletBinding attribute (unique)
            [/\[Parameter\s*\(/, 4],   // Parameter attribute
            [/\b-eq\b|-ne\b|-gt\b|-lt\b|-ge\b|-le\b/, 3],   // comparison operators
            [/\bconsole\.\w+\s*\(/, -5],   // JS console
        ],
    },
];

// ════════════════════════════════════════════════════════════════
// Supported Language Set
// ════════════════════════════════════════════════════════════════

/** Languages the editor can handle. IDs outside this set fall back to 'text'. */
const SUPPORTED_LANGUAGES = new Set([
    "json", "javascript", "typescript", "python", "html", "css",
    "yaml", "c", "cpp", "java", "go", "xml", "csv", "markdown",
    "shell", "dockerfile", "text", "svelte", "vue", "rust", "clojure",
    "sql", "php", "sass", "scss", "jinja", "angular", "nginx",
    "powershell", "ruby", "swift", "toml", "kotlin", "objectivec",
    "objectivecpp", "csharp", "scala", "dart",
]);

// ════════════════════════════════════════════════════════════════
// LanguageDetector
// ════════════════════════════════════════════════════════════════

class LanguageDetector {
    // ──────────────────────────────────────────────────────────
    // Public API
    // ──────────────────────────────────────────────────────────

    /**
     * Synchronous detection pipeline (Phases 1–4).
     *
     * @param content  - The document text to analyse
     * @param filename - Optional filename (e.g. "Dockerfile", "config.yml")
     * @returns A language ID or `null` when uncertain
     */
    detect(content: string, filename?: string): string | null {
        // Phase 1 — file extension / filename
        if (filename) {
            const extResult = this.detectByExtension(filename);
            if (extResult) return extResult;
        }

        if (!content || content.trim().length === 0) return null;

        const { content: bounded, wasSliced } = this.boundContent(content);
        // Strip BOM if present (not removed by trim())
        const trimmed = bounded.replace(/^\uFEFF/, "").trim();
        if (!trimmed) return null;

        // Phase 2 — shebang line
        const firstLine = trimmed.split("\n", 1)[0];
        if (firstLine.startsWith("#!")) {
            const shebangResult = this.detectByShebang(firstLine);
            if (shebangResult) return shebangResult;
        }

        // Phase 3 — structural signals (data formats & markup)
        const structuralResult = this.detectStructural(trimmed, wasSliced);
        if (structuralResult) return structuralResult;

        // Phase 4 — heuristic scoring (programming languages)
        if (trimmed.length >= 5) {
            const scoringResult = this.detectByScoring(trimmed);
            if (scoringResult) return scoringResult;
        }

        return null;
    }

    // ──────────────────────────────────────────────────────────
    // Phase 1 — Extension / Filename Detection
    // ──────────────────────────────────────────────────────────

    private detectByExtension(filename: string): string | null {
        const lower = filename.toLowerCase();
        const base = lower.split("/").pop() || lower;

        // Full-filename match first (Dockerfile, .bashrc, etc.)
        if (FILENAME_MAP[base]) return this.ensureSupported(FILENAME_MAP[base]);

        // Regex-based filename patterns (nginx*.conf etc.)
        for (const [pattern, lang] of FILENAME_PATTERNS) {
            if (pattern.test(base)) return this.ensureSupported(lang);
        }

        // Extension match
        const dotIdx = base.lastIndexOf(".");
        if (dotIdx === -1) return null;
        const ext = base.slice(dotIdx);
        const mapped = EXTENSION_MAP[ext];
        return mapped ? this.ensureSupported(mapped) : null;
    }

    // ──────────────────────────────────────────────────────────
    // Phase 2 — Shebang Detection
    // ──────────────────────────────────────────────────────────

    private detectByShebang(firstLine: string): string | null {
        for (const [pattern, language] of SHEBANG_PATTERNS) {
            if (pattern.test(firstLine)) return this.ensureSupported(language);
        }
        return null;
    }

    // ──────────────────────────────────────────────────────────
    // Phase 3 — Structural Detection
    // ──────────────────────────────────────────────────────────

    /**
     * Deterministic checks for formats identifiable by their syntax skeleton.
     *
     * ORDER MATTERS — most-unambiguous formats are checked first to prevent
     * false positives (e.g. HTML before XML, XML before Markdown).
     *
     * Current order:
     *   JSON → PHP → Svelte → Vue → HTML → XML → Dockerfile → CSV
     *   → Markdown → Sass/SCSS → TOML → YAML
     *
     * Sass/SCSS must come BEFORE TOML — TOML `key = value` and YAML `key:
     * value` patterns can partially match Sass property lines. YAML must
     * come AFTER Markdown to avoid treating frontmatter blocks as YAML.
     */
    private detectStructural(trimmed: string, wasSliced: boolean): string | null {
        if (this.isLikelyJson(trimmed, wasSliced)) return "json";
        if (this.isLikelyPhp(trimmed)) return "php";
        if (this.isLikelySvelte(trimmed)) return "svelte";
        if (this.isLikelyVue(trimmed)) return "vue";
        if (this.isLikelyHtml(trimmed)) return "html";
        if (this.isLikelyXml(trimmed)) return "xml";
        if (this.isLikelyDockerfile(trimmed)) return "dockerfile";
        if (this.isLikelyCsv(trimmed)) return "csv";
        if (this.isLikelyMarkdown(trimmed)) return "markdown";
        const sassLike = this.detectSassScssStructural(trimmed);
        if (sassLike) return sassLike;
        if (this.isLikelyToml(trimmed)) return "toml";
        if (this.isLikelyYaml(trimmed)) return "yaml";  // after markdown to avoid eating frontmatter
        return null;
    }

    // ── 3a. JSON ─────────────────────────────────────────────

    /**
     * Detects JSON, JSONL (JSON-Lines), and JSONC (JSON with comments).
     *
     * Strategy:
     *   1. Content must start with `{` or `[`.
     *   2. If full content is available, try JSON.parse() — authoritative.
     *   3. If parse fails, try JSONL (each line is independent JSON).
     *   4. Fall back to structural heuristic ("key": value patterns)
     *      while ruling out JS/TS object literals.
     */
    private isLikelyJson(trimmed: string, wasSliced: boolean): boolean {
        const first = trimmed[0];
        if (first !== "{" && first !== "[") return false;

        // Authoritative parse (only when we have the complete content)
        if (!wasSliced) {
            try { JSON.parse(trimmed); return true; } catch { /* fall through */ }
        }

        // JSONL — each non-empty line is its own JSON value
        const lines = trimmed.split("\n").filter(l => l.trim().length > 0);
        if (lines.length >= 2) {
            const sample = lines.slice(0, 5);
            const allJsonLines = sample.every(line => {
                const t = line.trim();
                if (t[0] !== "{" && t[0] !== "[") return false;
                try { JSON.parse(t); return true; } catch { return false; }
            });
            if (allJsonLines) return true;
        }

        // Structural heuristic for sliced / JSONC content:
        // Requires "key": <value-start> patterns AND no programming-language signals.
        const hasJsonPairs = /"[\w$][\w\s$.-]*"\s*:\s*["{\[\dtfn-]/.test(trimmed);
        if (!hasJsonPairs) return false;

        const firstLines = trimmed.split("\n").slice(0, 10);
        const codeSignals = firstLines.filter(l =>
            /^\s*(const|let|var|function|class|import|export|module|return)\b/.test(l),
        ).length;
        return codeSignals === 0;
    }

    // ── 3.a.1 Svelte ──────────────────────────────────────────

    /**
     * Svelte-specific structural detection.
     * Looks for {#if}, {:else}, {/if}, {#each}, bind:value, on:click, or Svelte 5 runes.
     */
    private isLikelySvelte(trimmed: string): boolean {
        if (trimmed[0] !== "<" && !trimmed.includes("{#")) return false;

        let score = 0;
        const signals: [RegExp, number][] = [
            [/{#(if|each|await|snippet|key)[}\s]/, 3], // block tags
            [/{:(else|then|catch)[}\s]/, 3], // block continuations
            [/{\/(if|each|await|snippet|key)}/, 3], // block closers
            [/<script\s+(context="module"|lang="ts")[^>]*>/, 3],
            [/\b(bind:|on:|use:|transition:|animate:|let:|class:)[a-zA-Z-]+=/, 2], // directives
            [/\$(state|derived|effect|props)\(/, 4], // Svelte 5 runes
            [/^\s*\$:\s+/m, 4], // Svelte 3/4 reactive statements
            [/<slot[\s>]/, 2], // <slot> tag
            [/\{@(html|render|debug|const)\s+/, 2], // special tags
        ];

        for (const [pattern, weight] of signals) {
            if (pattern.test(trimmed)) score += weight;
        }

        return score >= 2;
    }

    // ── 3.a.2 Vue ─────────────────────────────────────────────

    /**
     * Vue-specific structural detection.
     * Looks for <template>, v-if, v-model, @click, :class, <script setup>.
     */
    private isLikelyVue(trimmed: string): boolean {
        if (trimmed[0] !== "<") return false;

        let score = 0;
        const signals: [RegExp, number][] = [
            [/<template[\s>]/, 4], // <template> tag
            [/\b(v-if|v-else-if|v-else|v-show|v-for|v-on:|v-bind:|v-model|v-slot)[=>\s]/, 2], // Vue directives
            [/\B@(click|submit|input|change|keyup|keydown)=/, 2], // shorthand for v-on
            [/\B:(class|style|value|disabled|key)=/, 2], // shorthand for v-bind
            [/<script\s+setup[^>]*>/, 3],
            [/\b(defineProps|defineEmits|defineExpose)\s*\(/, 2], // Vue 3 macros
            [/\b(ref|reactive|computed|watch|onMounted)\s*\(/, 2], // Composition API hooks
        ];

        for (const [pattern, weight] of signals) {
            if (pattern.test(trimmed)) score += weight;
        }

        return score >= 4;
    }

    // ── 3b. HTML ─────────────────────────────────────────────

    /**
     * Checks for `<!DOCTYPE html>`, `<html>`, or a combination of
     * HTML-specific tags. Must be checked BEFORE XML so that HTML
     * documents are not misclassified as generic XML.
     */
    private isLikelyHtml(trimmed: string): boolean {
        // Instant bailout: If it explicitly declares XML, let the XML detector handle it
        if (trimmed.startsWith("<?xml")) return false;

        // Quick Svelte/Vue bail-outs: if unique markers are present, defer
        if (/{#\w+/.test(trimmed) || /\$state\(|\$derived\(|\$effect\(/.test(trimmed)) return false;
        if (/\bv-if=|\bv-for=|\bv-model=/.test(trimmed)) return false;

        if (/^<!doctype\s+html/i.test(trimmed)) return true;
        if (/^<html[\s>]/i.test(trimmed)) return true;

        if (trimmed[0] !== "<") return false;

        const htmlTags = [
            "head", "body", "div", "span", "script", "style",
            "meta", "link", "form", "input", "button",
            "table", "section", "article", "nav", "footer",
            "header", "main", "aside",
        ];

        let matchCount = 0;
        for (const tag of htmlTags) {
            // Ensure the tag is followed by a space or > (prevents <head from matching <heading>)
            const regex = new RegExp(`<${tag}[\\s>]`, "i");
            if (regex.test(trimmed)) matchCount++;
        }

        return matchCount >= 2;
    }

    // ── 3c. XML ──────────────────────────────────────────────

    /**
     * Detects XML by:
     *   • `<?xml` processing instruction  (definitive)
     *   • `xmlns` attribute               (definitive)
     *   • `<!--` comment followed by tags (high confidence)
     *   • Non-HTML opening tag with matching close tags
     *
     * This runs AFTER the HTML check, so `<html>` / `<!DOCTYPE html>`
     * are already classified — avoiding the classic XML-vs-HTML clash.
     */
    private isLikelyXml(trimmed: string): boolean {
        if (trimmed.startsWith("<?xml")) return true;
        if (trimmed[0] !== "<") return false;
        if (/\bxmlns\s*=/.test(trimmed)) return true;

        // Leading XML comment — check for tags after it
        if (trimmed.startsWith("<!--")) {
            const afterComments = trimmed.replace(/<!--[\s\S]*?-->\s*/g, "").trim();
            if (afterComments.startsWith("<")) return true;
        }

        // Opening tag that is NOT a common HTML tag
        const openTagMatch = trimmed.match(/^<([a-zA-Z_][\w:.-]*)/);
        if (!openTagMatch) return false;

        const tagName = openTagMatch[1].toLowerCase();
        const htmlTopLevelTags = new Set([
            "html", "head", "body", "div", "span", "p", "a", "script",
            "style", "link", "meta", "title", "form", "input", "button",
            "table", "ul", "ol", "li", "h1", "h2", "h3", "h4", "h5", "h6",
            "img", "br", "hr", "section", "article", "nav", "footer",
            "header", "main", "aside", "template",
        ]);
        if (htmlTopLevelTags.has(tagName)) return false;

        // Namespace prefix (e.g. <ns:tag>) → XML
        if (tagName.includes(":")) return true;

        // Require both opening and closing tags to confirm structure
        const openTags = (trimmed.match(/<[a-zA-Z_][\w:.-]*/g) || []).length;
        const closeTags = (trimmed.match(/<\/[a-zA-Z_][\w:.-]*/g) || []).length;
        return openTags >= 2 && closeTags >= 1;
    }

    // ── 3d. Dockerfile ───────────────────────────────────────

    /**
     * Dockerfiles have a very distinctive structure:
     *   • First non-comment line is `FROM` (or `ARG` before `FROM`).
     *   • Subsequent lines use Docker instructions (RUN, COPY, CMD…).
     *   • At least 2 instruction lines required for confidence.
     */
    private isLikelyDockerfile(trimmed: string): boolean {
        const lines = trimmed
            .split("\n")
            .map(l => l.trim())
            .filter(l => l.length > 0 && !l.startsWith("#"));

        if (lines.length === 0) return false;
        if (!/^(FROM|ARG)\s/i.test(lines[0])) return false;

        // Exhaustive list of Dockerfile instructions
        const instruction =
            /^(FROM|RUN|CMD|LABEL|MAINTAINER|EXPOSE|ENV|ADD|COPY|ENTRYPOINT|VOLUME|USER|WORKDIR|ARG|ONBUILD|STOPSIGNAL|HEALTHCHECK|SHELL)\s/i;
        const matchCount = lines.filter(l => instruction.test(l)).length;
        return matchCount >= 2;
    }

    // ── 3e. CSV / TSV ────────────────────────────────────────

    /**
     * Checks for consistent delimiter usage across lines.
     * Handles comma, tab, semicolon, and pipe delimiters.
     *
     * Guard rails:
     *   • Must NOT start with `{`, `[`, or `<` (JSON / XML / HTML).
     *   • Requires ≥2 data lines.
     *   • Lines that look like YAML `key: value` reject CSV.
     *   • Pipe-delimited markdown tables are excluded.
     */
    private isLikelyCsv(trimmed: string): boolean {
        if (trimmed[0] === "{" || trimmed[0] === "[" || trimmed[0] === "<") return false;

        const lines = trimmed.split("\n").map(l => l.trim()).filter(l => l.length > 0);
        if (lines.length < 2) return false;

        // If most lines look like YAML key: value, skip CSV
        const yamlKvPattern = /^[a-zA-Z_][\w.-]*\s*:\s/;
        const yamlLikeCount = lines.filter(l => yamlKvPattern.test(l)).length;
        if (yamlLikeCount / lines.length > 0.5) return false;

        // If most lines look like script/source code, reject CSV.
        // Shell scripts in particular can trigger the delimiter heuristic
        // (e.g. lines separated by spaces or pipes that look like columns).
        const scriptOrCommentPattern = /^\s*(#|\/\/|echo|import|from|const|let|var|def|class|function)\b/;
        const scriptLikeCount = lines.filter(l => scriptOrCommentPattern.test(l)).length;
        if (scriptLikeCount / lines.length > 0.3) return false;

        // If lines contain curly braces, it's very likely CSS, JS, etc. and not CSV
        const braceCount = lines.filter(l => l.includes("{") || l.includes("}")).length;
        if (braceCount / lines.length > 0.3) return false;

        for (const delim of [",", "\t", ";", "|"]) {
            if (this.hasConsistentDelimiter(lines, delim)) return true;
        }
        return false;
    }

    /**
     * Returns true when ≥80 % of sampled lines share the same delimiter count
     * as the header row, with at least 1 delimiter per line.
     */
    private hasConsistentDelimiter(lines: string[], delimiter: string): boolean {
        const escaped = delimiter.replace(/[|\\{}()[\]^$+*?.]/g, "\\$&");
        const re = new RegExp(escaped, "g");

        // Strip content inside double quotes to avoid counting grammatical delimiters
        const cleanHeader = lines[0].replace(/"[^"]*"/g, "");
        const headerCount = (cleanHeader.match(re) || []).length;

        if (headerCount < 1) return false;

        // Pipe delimiter: exclude markdown tables (every line starts & ends with |)
        if (delimiter === "|") {
            const looksLikeTable = lines.every(l => l.startsWith("|") && l.endsWith("|"));
            if (looksLikeTable) return false;
        }

        const sample = lines.slice(0, Math.min(lines.length, 20));
        const matching = sample.filter(l => {
            const cleanLine = l.replace(/"[^"]*"/g, "");
            return (cleanLine.match(re) || []).length === headerCount;
        }).length;

        return matching / sample.length >= 0.8;
    }

    // ── 3f. YAML ─────────────────────────────────────────────

    /**
     * YAML detection strategy:
     *   1. Recognise `---` document separators (and distinguish from
     *      Markdown frontmatter by checking for trailng content).
     *   2. Count lines matching `key: value` and `- item` patterns.
     *   3. Reject if more lines look like code than YAML.
     *   4. Require >50 % YAML-like lines (>30 % with `---` leader).
     */
    private isLikelyYaml(trimmed: string): boolean {
        const lines = trimmed.split("\n");
        const startsWithSeparator = lines[0].trim() === "---";

        // Bail out if content looks like Sass/SCSS — `$var: value` lines are not YAML keys
        // ($ is not a valid YAML key character).
        const nonEmptyLines = lines.filter(l => l.trim().length > 0 && !l.trim().startsWith("#"));
        const sassVarLineCount = nonEmptyLines.filter(l => /^\s*\$[\w-]+\s*:/.test(l)).length;
        if (sassVarLineCount >= 1) return false;

        const kvPattern = /^\s*[a-zA-Z_][\w.-]*\s*:\s/;
        const yamlListPattern = /^\s*-\s+\S/;
        const codePatterns = [
            /^\s*(def|class|if|for|while|return|import|from|try|except|with|async|yield)\s/,
            /^\s*(function|const|let|var|if|for|while|return|import|export|switch|case)\s/,
            /^\s*(#include|int\s+main|typedef|struct)\s/,
            /^\s*(public|private|protected)\s+(class|static|void|int|String)/,
            /^\s*(func|package|type|defer|go)\s/,
        ];

        const nonEmpty = lines.filter(l => l.trim().length > 0 && !l.trim().startsWith("#"));
        if (nonEmpty.length === 0) return false;

        let yamlLines = 0;
        let codeLines = 0;
        for (const line of nonEmpty) {
            if (codePatterns.some(p => p.test(line))) codeLines++;
            else if (kvPattern.test(line) || yamlListPattern.test(line)) yamlLines++;
        }

        if (codeLines > yamlLines) return false;

        const yamlRatio = yamlLines / nonEmpty.length;
        if (startsWithSeparator && yamlRatio > 0.3) return true;
        return yamlRatio > 0.5;
    }

    // ── 3g. Markdown ─────────────────────────────────────────

    /**
     * Markdown detection uses a weighted signal approach.
     * Score ≥ 4 is required — this avoids false positives from content
     * that happens to contain a single `#` line or a stray `*bold*`.
     *
     * Also handles the special case of YAML frontmatter (`---`) at the
     * top of a markdown document.
     */
    private isLikelyMarkdown(trimmed: string): boolean {
        // Anti-signals: clearly not markdown
        if (trimmed[0] === "<" || trimmed[0] === "{" || trimmed[0] === "[") return false;

        // Anti-signal: programming language patterns.
        // If the content shows strong code signals the markdown "hits" are
        // almost certainly embedded in string literals / comments.
        const codeAntiSignals: [RegExp, number][] = [
            [/^\/\*\*?\s/m, 3],   // block-comment opener
            [/^\s*(import|export)\s+/m, 3],   // ES modules / Python
            [/^\s*(const|let|var)\s+\w+\s*[=:]/m, 2],   // variable declarations
            [/^\s*function\s+\w*\s*\(/m, 2],   // function decl
            [/^\s*(interface|type|enum)\s+\w+/m, 3],   // TypeScript specifics
            [/^\s*class\s+\w+/m, 2],   // class decl
            [/=>\s*[{(\n]/m, 2],   // arrow functions
            [/^\s*def\s+\w+\s*\(/m, 3],   // Python functions
            [/^\s*#include\s*[<"]/m, 3],   // C/C++
            [/^\s*(if|for|while)\s*\(/m, 1],   // control flow (parens)
            [/;\s*$/m, 1],   // trailing semicolons
            [/^\s*async\s+(function|\w+\s*[=(])/m, 2],   // async
            [/^\s*[a-zA-Z_][\w.-]*\s*:\s+(?!http)/m, 4],   // YAML key-value pairs
        ];

        let codeScore = 0;
        for (const [pattern, weight] of codeAntiSignals) {
            if (pattern.test(trimmed)) codeScore += weight;
        }

        // Strong code signals → not markdown (embedded examples don't count)
        if (codeScore >= 6) return false;

        // Special case: YAML frontmatter → almost certainly markdown
        if (this.hasMarkdownFrontmatter(trimmed)) return true;

        let score = 0;
        const signals: [RegExp, number][] = [
            [/^#{1,6}\s+\S/m, 3],   // ATX headings
            [/\[.+?\]\(.+?\)/, 2],   // links
            [/!\[.*?\]\(.+?\)/, 2],   // images
            [/^\s*[-*+]\s+\S/m, 1],   // unordered lists
            [/^\s*\d+\.\s+\S/m, 1],   // ordered lists
            [/^\s*>\s+/m, 1],   // blockquotes
            [/\*\*.+?\*\*/, 1],   // bold
            [/^```/m, 2],   // fenced code blocks
            [/^\|.+\|.+\|/m, 2],   // tables
            [/^---\s*$/m, 1],   // horizontal rules
        ];

        for (const [pattern, weight] of signals) {
            if (pattern.test(trimmed)) score += weight;
        }

        // Moderate code signals → require higher markdown score to override
        const threshold = codeScore >= 3 ? 8 : 4;
        return score >= threshold;
    }

    /**
     * Returns true when the content begins with YAML frontmatter
     * (`---` … `---`) AND has content afterwards — a near-definitive
     * sign of Markdown (Jekyll, Hugo, Gatsby, etc.).
     */
    private hasMarkdownFrontmatter(trimmed: string): boolean {
        if (!trimmed.startsWith("---")) return false;

        const lines = trimmed.split("\n");
        const closeIdx = lines.findIndex((l, i) => i > 0 && l.trim() === "---");
        if (closeIdx <= 0) return false;

        const afterFrontmatter = lines.slice(closeIdx + 1).join("\n").trim();
        return afterFrontmatter.length > 0;
    }

    // ── 3h. PHP ──────────────────────────────────────────────

    /**
     * PHP structural detection.
     * The `<?php` opening tag is a near-definitive signal.
     */
    private isLikelyPhp(trimmed: string): boolean {
        // Must contain the <?php open tag
        if (/^<\?php\b/m.test(trimmed)) return true;
        // Short open tag with PHP content on subsequent lines
        if (trimmed.startsWith("<?") && !trimmed.startsWith("<?xml")) {
            // Check for PHP-specific patterns after the short tag
            if (/\$\w+\s*=/.test(trimmed) || /\becho\s/.test(trimmed)) return true;
        }
        return false;
    }

    // ── 3i. Sass / SCSS ─────────────────────────────────────

    /**
     * Distinguishes Sass (indented syntax) vs SCSS (CSS-like braces/semicolons).
     *
     * Two independent anchor signals:
     *   1. `$var: value` variable declarations  — Sass/SCSS-specific.
     *   2. `@mixin`, `@include`, `@extend`, `@use`, `@forward` at-rules
     *      — SCSS/Sass-specific; plain CSS only has `@media`, `@keyframes`,
     *        `@import`, `@charset`, `@layer`, `@supports`.
     *
     * Classification:
     *   • SCSS when braces or semicolon-terminated property lines are present.
     *   • Sass when there are no braces and there are indented property lines.
     */
    private detectSassScssStructural(trimmed: string): "sass" | "scss" | null {
        if (trimmed[0] === "<" || trimmed[0] === "{" || trimmed[0] === "[") return null;

        const lines = trimmed
            .split("\n")
            .map(l => l.replace(/\r$/, ""))
            .filter(l => l.trim().length > 0 && !l.trim().startsWith("//") && !l.trim().startsWith("#"));

        if (lines.length < 2) return null;

        // Anchor 1: $variable declarations (colon-separated, not equals — rules out PHP/shell)
        const sassVarPattern = /^\s*\$[\w-]+\s*:\s*.+;?\s*$/;
        const varDeclCount = lines.filter(l => sassVarPattern.test(l)).length;

        // Anchor 2: Sass/SCSS-exclusive at-rules that plain CSS never uses
        const sassAtRulePattern = /^\s*@(mixin|include|extend|use|forward)\b/;
        const sassAtRuleCount = lines.filter(l => sassAtRulePattern.test(l)).length;

        // Require at least one anchor signal to proceed
        if (varDeclCount < 1 && sassAtRuleCount < 1) return null;

        const hasOpenBrace = lines.some(l => l.includes("{"));
        const hasCloseBrace = lines.some(l => l.includes("}"));
        const hasBraces = hasOpenBrace || hasCloseBrace;

        const semicolonLineCount = lines.filter(l => /;\s*$/.test(l)).length;
        const cssSelectorWithBrace = lines.filter(l => /([.#][\w-]+|[a-z][\w-]*)\s*\{\s*$/.test(l.trim())).length;

        const scssScore =
            (hasBraces ? 2 : 0) +
            (semicolonLineCount >= 2 ? 2 : semicolonLineCount >= 1 ? 1 : 0) +
            (cssSelectorWithBrace >= 1 ? 1 : 0) +
            // @mixin/@include with braces is near-definitive SCSS
            (sassAtRuleCount >= 1 && hasBraces ? 2 : 0);

        if (scssScore >= 2) return "scss";

        // Indented syntax (no braces) → Sass
        const indentedPropertyPattern = /^\s{2,}[a-z-]+\s*:\s*[^;{}]+\s*$/;
        const indentedPropertyCount = lines.filter(l => indentedPropertyPattern.test(l)).length;
        const sassLikeVarCount = lines.filter(l => /^\s*\$[\w-]+\s*:\s*[^;{}]+\s*$/.test(l)).length;

        if (!hasBraces && (indentedPropertyCount >= 1 || sassLikeVarCount >= 2 || sassAtRuleCount >= 1)) {
            return "sass";
        }

        return null;
    }

    // ── 3j. TOML ─────────────────────────────────────────────

    /**
     * TOML detection strategy:
     *   1. Look for `[section]` headers (not markdown links or INI-only content).
     *   2. Count lines matching `key = value` patterns (with `=`, not `:`).
     *   3. Distinguish from YAML (uses `:`) and INI (fewer typed values).
     *   4. Require ≥2 TOML-like signals for confidence.
     */
    private isLikelyToml(trimmed: string): boolean {
        // Bail out for JSON/XML/HTML
        if (trimmed[0] === "<") return false;
        // Parenthesised explicitly: bail for JSON objects `{` or JSON arrays `["..."`
        if (trimmed[0] === "{" || (trimmed[0] === "[" && trimmed[1] === '"')) return false;

        const lines = trimmed.split("\n");
        const nonEmpty = lines.filter(l => l.trim().length > 0 && !l.trim().startsWith("#"));
        if (nonEmpty.length < 2) return false;

        // Bail out if this looks like Sass/SCSS — `$var: value` lines use `:`, not `=`,
        // and the $ prefix is never valid in TOML keys.
        const sassVarLineCount = nonEmpty.filter(l => /^\s*\$[\w-]+\s*:/.test(l)).length;
        if (sassVarLineCount >= 1) return false;

        let score = 0;

        // [section] or [[array-of-tables]] headers
        const sectionPattern = /^\s*\[\[?[\w.-]+\]\]?\s*$/;
        const sectionCount = nonEmpty.filter(l => sectionPattern.test(l)).length;
        if (sectionCount >= 1) score += 2;

        // key = value with TOML-typed values (strings, numbers, booleans, arrays, datetimes)
        const kvPattern = /^\s*[\w.-]+\s*=\s*(.+)$/;
        let kvCount = 0;
        for (const line of nonEmpty) {
            const m = line.match(kvPattern);
            if (m) {
                kvCount++;
                const val = m[1].trim();
                // Triple-quoted strings, inline arrays/tables, typed values
                if (/^"""/.test(val) || /^'''/.test(val)) score += 1;
                if (/^\[/.test(val) || /^\{/.test(val)) score += 1;
                if (/^(true|false)$/.test(val)) score += 1;
                if (/^\d{4}-\d{2}-\d{2}/.test(val)) score += 2; // datetime
            }
        }

        if (kvCount >= 2) score += 1;

        // YAML uses `:` as separator — if most lines use `:` not `=`, it's likely YAML
        const colonKvCount = nonEmpty.filter(l => /^\s*[\w.-]+\s*:\s/.test(l)).length;
        if (colonKvCount > kvCount) return false;

        return score >= 3;
    }

    // ──────────────────────────────────────────────────────────
    // Phase 4 — Heuristic Scoring (Programming Languages)
    // ──────────────────────────────────────────────────────────

    /**
     * Scores the content against every language signature.
     *
     * Returns the highest-scoring language above `HEURISTIC_SCORE_THRESHOLD`.
     * If no language clears the threshold, returns the top scorer as a
     * best-guess fallback (provided at least one pattern matched).
     *
     * Superset tie-breaking ensures TypeScript beats JavaScript
     * and C++ beats C when both score well.
     */
    private detectByScoring(content: string): string | null {
        const scores = new Map<string, number>();

        // Track the best partial scorer (below threshold) for fallback
        let partialBest: string | null = null;
        let partialBestScore = 0;

        for (const sig of LANGUAGE_SIGNATURES) {
            let score = 0;
            for (const [pattern, weight] of sig.patterns) {
                if (weight > 0) {
                    // For positive-weight patterns, count matches and apply density bonus
                    const flags = pattern.flags.includes("g") ? pattern.flags : pattern.flags + "g";
                    const globalPattern = new RegExp(pattern.source, flags);
                    const matches = content.match(globalPattern);
                    if (matches) {
                        // Base weight for first match + diminishing returns for repeats
                        score += weight + Math.min(matches.length - 1, 3);
                    }
                } else {
                    // Anti-signals: just test presence
                    if (pattern.test(content)) score += weight;
                }
            }

            // Negative final score → this language is definitively ruled out;
            // skip both confident and partial-fallback paths entirely.
            if (score < 0) continue;

            if (score >= HEURISTIC_SCORE_THRESHOLD) {
                scores.set(sig.language, score);
            } else if (score > partialBestScore) {
                partialBest = sig.language;
                partialBestScore = score;
            }
        }

        // Confident matches — pick the best
        if (scores.size > 0) {
            // Superset tie-breaking: TypeScript ⊃ JavaScript, C++ ⊃ C
            this.resolveSuperset(scores, "typescript", "javascript");
            this.resolveSuperset(scores, "cpp", "c");

            let best: string | null = null;
            let bestScore = 0;
            for (const [lang, score] of scores) {
                if (score > bestScore) {
                    best = lang;
                    bestScore = score;
                }
            }

            return best ? this.ensureSupported(best) : null;
        }

        // Best-guess fallback: return the top partial scorer only when
        // enough weighted points matched (avoids single weak hits)
        if (partialBest && partialBestScore >= PARTIAL_SCORE_THRESHOLD) {
            return this.ensureSupported(partialBest);
        }

        return null;
    }

    /**
     * If both a superset language and its base language scored above
     * threshold, and the superset's score is ≥ 60 % of the base, the
     * base is removed so the superset wins.
     */
    private resolveSuperset(
        scores: Map<string, number>,
        superset: string,
        base: string,
    ): void {
        const superScore = scores.get(superset);
        const baseScore = scores.get(base);
        if (superScore && baseScore && superScore >= baseScore * 0.6) {
            scores.delete(base);
        }
    }

    // ──────────────────────────────────────────────────────────
    // Utilities
    // ──────────────────────────────────────────────────────────

    /** Slice content to MAX_CONTENT_LENGTH for safe analysis. */
    private boundContent(content: string): { content: string; wasSliced: boolean } {
        if (content.length <= MAX_CONTENT_LENGTH) {
            return { content, wasSliced: false };
        }
        return { content: content.slice(0, MAX_CONTENT_LENGTH), wasSliced: true };
    }

    /** Returns the language ID if supported, otherwise 'text'. */
    private ensureSupported(lang: string): string {
        return SUPPORTED_LANGUAGES.has(lang) ? lang : "text";
    }
}

// ════════════════════════════════════════════════════════════════
// Singleton Export
// ════════════════════════════════════════════════════════════════

export const languageDetector = new LanguageDetector();
