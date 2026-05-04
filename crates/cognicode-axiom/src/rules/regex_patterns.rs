//! Regex pattern constants for code analysis rules
//!
//! This module extracts commonly used regex patterns from catalog.rs to:
//! - Reduce duplication across 541+ Regex::new calls
//! - Improve maintainability and auditability
//! - Enable pattern reuse across rules
//!
//! # Usage
//!
//! ```rust
//! use cognicode_axiom::rules::regex_patterns::compile;
//! let pattern = compile(r"(?i)TODO:?");
//! ```

use regex::Regex;

/// Compile a regex pattern from a constant string.
///
/// # Panics
///
/// Panics if the pattern is invalid (which indicates a bug in this crate,
/// not in user code).
#[inline]
pub fn compile(pattern: &str) -> Regex {
    Regex::new(pattern).unwrap_or_else(|_| {
        panic!("Invalid regex pattern in regex_patterns module: {}", pattern)
    })
}

/// Regex patterns organized by category
pub mod patterns {
    // ══════════════════════════════════════════════════════════════════════════
    // COMMENTS
    // ══════════════════════════════════════════════════════════════════════════

    /// Matches TODO/FIXME/HACK/XXX comment markers (case-insensitive)
    pub const TODO_COMMENT: &str = r"(?i)(TODO|FIXME|HACK|XXX):?";

    /// Matches single-line comments (// ...)
    pub const SINGLE_LINE_COMMENT: &str = r"//.*";

    /// Matches multi-line comment start (/* ...)
    pub const MULTI_LINE_COMMENT_START: &str = r"/\*";

    // ══════════════════════════════════════════════════════════════════════════
    // CREDENTIALS & SECURITY
    // ══════════════════════════════════════════════════════════════════════════

    /// Password credential pattern
    pub const PASSWORD_CREDENTIAL: &str = r#"(?i)(password|passwd|pwd)\s*[=:]\s*["'][^"']{4,}["']"#;

    /// API key credential pattern
    pub const API_KEY_CREDENTIAL: &str = r#"(?i)(api[_-]?key|apikey)\s*[=:]\s*["'][^"']{4,}["']"#;

    /// Secret/token credential pattern
    pub const SECRET_CREDENTIAL: &str = r#"(?i)(secret|token)\s*[=:]\s*["'][^"']{4,}["']"#;

    /// Bearer/Basic auth token pattern
    pub const AUTH_TOKEN: &str = r"(?i)(bearer|basic)\s+[a-zA-Z0-9_\-]+";

    /// HTTP URL pattern (non-HTTPS)
    pub const PLAINTEXT_HTTP_URL: &str = r#"http://[^\s""']+"#;

    // ══════════════════════════════════════════════════════════════════════════
    // NAMING CONVENTIONS
    // ══════════════════════════════════════════════════════════════════════════

    /// Matches PascalCase or camelCase identifier in let binding
    /// Captures: (1) optional "mut ", (2) the identifier
    pub const LET_PASCAL_OR_CAMEL: &str = r"let\s+(mut\s+)?([A-Z][a-zA-Z0-9_]*|[a-z]+[A-Z])";

    /// Matches if let with type annotation
    pub const IF_LET_TYPE: &str = r"if\s+let\s+[A-Z]";

    /// Matches single-line comment with todo marker
    pub const SINGLE_LINE_COMMENT_TODO: &str = r"//\s*(TODO|FIXME|HACK|XXX):?";

    // ══════════════════════════════════════════════════════════════════════════
    // FUNCTION PATTERNS
    // ══════════════════════════════════════════════════════════════════════════

    /// Matches function named 'new'
    pub const FN_NEW: &str = r"fn\s+new\(";

    /// Matches test function (test_ prefix or _test suffix)
    pub const FN_TEST: &str = r"fn\s+(test_[a-z_]+|[a-z_]+_test)\s*\(";

    /// Matches setup/before/init function
    pub const FN_SETUP: &str = r"fn\s+(setup|before|init|new_test)\s*\(\s*\)";

    /// Matches function with generic type parameters
    pub const FN_GENERIC: &str = r"fn\s+\w+<('[a-z]+\s*,\s*)*'[a-z]+\s*>\s*\(";

    /// Matches public function declaration
    pub const FN_PUBLIC: &str = r"^pub\s+fn\s+(\w+)";

    /// Matches function with type parameters and parentheses
    pub const FN_WITH_PARAMS_TYPE: &str = r"fn\s+([A-Z][a-zA-Z0-9_]*|[a-z]+[A-Z])";

    // ══════════════════════════════════════════════════════════════════════════
    // PATTERN MATCHING
    // ══════════════════════════════════════════════════════════════════════════

    /// Matches wildcard pattern in match arm
    pub const MATCH_WILDCARD_EMPTY: &str = r"_\s*=>\s*\{\s*\}";

    /// Matches match expression with block
    pub const MATCH_EXPR: &str = r"match\s+\w+\s*\{";
    /// Matches match expression with full block
    pub const MATCH_EXPR_BLOCK: &str = r"match\s+\w+\s*\{([^}]+)\}";

    // ══════════════════════════════════════════════════════════════════════════
    // TYPE PATTERNS
    // ══════════════════════════════════════════════════════════════════════════

    /// Matches impl for trait
    pub const IMPL_FOR: &str = r"impl\s+\w+\s+for\s+";

    /// Matches impl Drop for type
    pub const IMPL_DROP: &str = r"impl\s+Drop\s+for\s+(\w+)";

    /// Matches impl Clone for type
    pub const IMPL_CLONE: &str = r"impl\s+Clone\s+for\s+(\w+)";

    /// Matches impl PartialEq for type
    pub const IMPL_PARTIALEQ: &str = r"impl\s+PartialEq\s+for\s+(\w+)";

    /// Matches impl Default for type
    pub const IMPL_DEFAULT: &str = r"impl\s+Default\s+for\s+(\w+)";

    /// Matches impl Display for type
    pub const IMPL_DISPLAY: &str = r"impl\s+.*\s+Display\s+for\s+";

    /// Matches derive attribute
    pub const DERIVE_ATTRIBUTE: &str = r"#\[derive\(([^)]+)\)\]";

    /// Matches derive with specific trait
    pub const DERIVE_WITH_TRAIT: &str = r"#\[derive\([^)]*\b(PartialEq|Hash)\b[^)]*\)";

    /// Matches struct declaration
    pub const STRUCT_DECL: &str = r"struct\s+(\w+Error)";

    /// Matches public struct declaration
    pub const STRUCT_PUBLIC: &str = r"^pub\s+struct\s+(\w+)";

    /// Matches trait declaration
    pub const TRAIT_DECL: &str = r"trait\s+([a-z][a-z0-9_]*)\s*(<|\{|;)";

    /// Matches public trait declaration
    pub const TRAIT_PUBLIC: &str = r"^pub\s+trait\s+(\w+)";

    /// Matches enum declaration
    pub const ENUM_DECL: &str = r"enum\s+\w+\s*\{";

    /// Matches public enum declaration
    pub const ENUM_PUBLIC: &str = r"^pub\s+enum\s+(\w+)";

    /// Matches module declaration
    pub const MOD_DECL: &str = r"(?:^|\s)mod\s+([A-Z][a-zA-Z0-9_]*|[a-z]+[A-Z])";

    /// Matches public module declaration
    pub const MOD_PUBLIC: &str = r"^pub\s+mod\s+(\w+)";

    /// Matches RefCell with primitive type
    pub const REFCELL_PRIMITIVE: &str = r"RefCell<\s*(i8|i16|i32|i64|i128|isize|u8|u16|u32|u64|u128|usize|f32|f64|bool|char)\s*>";

    // ══════════════════════════════════════════════════════════════════════════
    // COLLECTION & ITERATOR PATTERNS
    // ══════════════════════════════════════════════════════════════════════════

    /// Matches empty length check (.len() == 0)
    pub const EMPTY_LEN_CHECK: &str = r"\.len\(\)\s*==\s*0";

    /// Matches .to_owned().clone() anti-pattern
    pub const TO_OWNED_CLONE: &str = r"\.to_owned\(\)\s*\.clone\(\)";

    /// Matches for loop over iterator
    pub const FOR_ITERATOR: &str = r"for\s+(\w+)\s+in\s+";

    /// Matches for loop with into_iter()
    pub const FOR_INTO_ITER: &str = r"for\s+\w+\s+in\s+\w+\.into_iter\(\)";

    /// Matches for loop over range with len()
    pub const FOR_RANGE_LEN: &str = r"for\s+\w+\s+in\s+0\s*\.\.\s*\w+\.len\(\)";

    /// Matches .iter_mut()
    pub const ITER_MUT: &str = r"\.iter_mut\(\)";

    /// Matches .collect with .iter() anti-pattern
    pub const COLLECT_ITER: &str = r"\.collect::<.*>\(\)\.iter\(\)|\.collect::<Vec<.*>>\(\)\.iter\(\)";

    /// Matches .filter().map() chaining
    pub const FILTER_MAP: &str = r"\.filter_map\(\s*\|\s*\w+\s\|\s*\w+\.ok\(\)\s*\)\.flatten\(\)";

    /// Matches Box::pin
    pub const BOX_PIN: &str = r"Box::pin\s*\(\s*(\w+)\s*\)";

    /// Matches .for_each with complex closure
    pub const FOR_EACH_COMPLEX: &str = r"\.for_each\s*\(\s*\|\s*\w+\s*,\s*\|[^|]+";

    /// Matches .map with push/insert/print side effects
    pub const MAP_SIDE_EFFECT: &str = r"\.map\s*\(\s*\|\s*\w+\s*\|[^}]*\.push\(|\.map\s*\(\s*\|\s*\w+\s*\|[^}]*\.insert\(|\.map\s*\(\s*\|\s*\w+\s*\|[^}]*print|\.map\s*\(\s*\|\s*\w+\s*\|[^}]*eprint";

    /// Matches Rc/Arc with .clone() anti-pattern
    pub const RC_ARC_CLONE: &str = r"(Rc<|Arc<)[^>]+>\s*\([^)]+\)\.clone\(\)";

    // ══════════════════════════════════════════════════════════════════════════
    // ERROR HANDLING PATTERNS
    // ══════════════════════════════════════════════════════════════════════════

    /// Matches Box<dyn Error>
    pub const BOX_DYN_ERROR: &str = r"Box<dyn\s+Error>";

    /// Matches -> Box<dyn Error> return type
    pub const RETURN_BOX_DYN_ERROR: &str = r"->\s*Box<dyn\s+Error>";

    // ══════════════════════════════════════════════════════════════════════════
    // TESTING PATTERNS
    // ══════════════════════════════════════════════════════════════════════════

    /// Matches assert! macro
    pub const ASSERT_MACRO: &str = r"assert!\s*\(([^,]+)\s*\)";

    /// Matches assert_eq! with variable references
    pub const ASSERT_EQ_VAR: &str = r"assert_eq!\s*\(\s*\$(\w+)\s*,\s*\$(\w+)";

    /// Matches assert_eq! with boolean literal
    pub const ASSERT_EQ_BOOL: &str = r"assert_eq!\s*\(\s*(true|false)\s*,\s*";

    /// Matches assert_ne! with variable references
    pub const ASSERT_NE_VAR: &str = r"assert_ne!\s*\(\s*\$(\w+)\s*,\s*\$(\w+)";

    /// Matches test module attribute
    pub const TEST_MODULE: &str = r"#\[cfg\(test\)\]\s+mod\s+(\w+)";

    /// Matches static mut (unsafe)
    pub const STATIC_MUT: &str = r"static\s+(mut|ref)\s+\w+";

    /// Matches thread_local attribute
    pub const THREAD_LOCAL: &str = r"#\[thread_local\]";

    // ══════════════════════════════════════════════════════════════════════════
    // CONTROL FLOW
    // ══════════════════════════════════════════════════════════════════════════

    /// Matches loop with break
    pub const LOOP_WITH_BREAK: &str = r"loop\s*\{[^}]*break[^}]*\}";

    /// Matches triple boolean comparison (== true, != false, etc.)
    pub const BOOL_COMPARISON_TRIPLE: &str = r"(==|!=)\s*(true|false)|(true|false)\s*(==|!=)";

    /// Matches double negation
    pub const DOUBLE_NEGATION: &str = r"!!\w+";

    /// Matches if with true/false comparison
    pub const IF_BOOL_COMPARE: &str = r"(if\s+\w+\s*==\s*true|if\s+\w+\s*!=\s*false)";

    // ══════════════════════════════════════════════════════════════════════════
    // ASSIGNMENT & EXPRESSIONS
    // ══════════════════════════════════════════════════════════════════════════

    /// Matches IP address literal
    pub const IP_LITERAL: &str = r#""\d{1,3}\.\d{1,3}\.\d{1,3}\.\d{1,3}""#;

    /// Matches let with tuple destructuring
    pub const LET_TUPLE: &str = r"let\s*\(([^)]+)\)\s*=";

    /// Matches variable assignment pattern
    pub const VAR_ASSIGN: &str = r"let\s+(\w+)\s*=\s*(\w+)\s*;";

    /// Matches let mut declaration
    pub const LET_MUT: &str = r"let\s+mut\s+(\w+)\s*=";

    /// Matches let underscore variable
    pub const LET_UNDERSCORE: &str = r"let\s+_(\w+)\s*=";

    /// Matches const declaration
    pub const CONST_DECL: &str = r"const\s+([a-z][A-Za-z0-9_]*)\s*:";

    /// Matches pub field declaration
    pub const PUB_FIELD: &str = r"pub\s+([a-z])\s*:";

    /// Matches deref chain (*var.field)
    pub const DEREF_CHAIN: &str = r"\*(\w+)\s*\.\s*\w+";

    /// Matches method call chain
    pub const METHOD_CHAIN: &str = r"\w+\.\w+\s*\(\s*\w+\s+\w+\s*\)";

    /// Matches ::<generic>() call
    pub const GENERIC_CALL: &str = r"::<\w+>\(\)|::<\w+>::\w+";

    /// Matches method call with angle brackets
    pub const METHOD_CALL_ANGLE: &str = r"<\w+>\s*\(";

    // ══════════════════════════════════════════════════════════════════════════
    // STRING & FORMAT PATTERNS
    // ══════════════════════════════════════════════════════════════════════════

    /// Matches string .to_string() anti-pattern
    pub const STRING_TO_STRING: &str = r#""([^"]*)"\s*\.to_string\(\)"#;

    /// Matches format! with arguments
    pub const FORMAT_MACRO: &str = r#"format!\("([^}"]*)"\)"#;

    /// Matches format! with .to_string() in arguments
    pub const FORMAT_TOSTRING: &str = r#"format!\s*\(\s*"[^"]*"\s*,\s*[^)]*\.to_string\(\)"#;

    /// Matches .expect or .unwrap with empty string
    pub const EXPECT_UNWRAP_EMPTY: &str = r#"\.(expect|unwrap)\s*\(\s*(""\s*|\s*")\)"#;

    /// Matches Vec::with_capacity
    pub const VEC_WITH_CAPACITY: &str = r"Vec::with_capacity\((\d+)\)";

    /// Matches closure with comma in pipe
    pub const CLOSURE_COMMA_PIPE: &str = r",\s*\|";

    // ══════════════════════════════════════════════════════════════════════════
    // OPERATORS & COMPARISONS
    // ══════════════════════════════════════════════════════════════════════════

    /// Matches magic number comparison (>= 3 digits)
    pub const MAGIC_NUMBER_COMPARE: &str = r"[=<>!]\s*\d{3,}";

    /// Matches floating point comparison
    pub const FLOAT_COMPARE: &str = r"(f32|f64)\b.*\s*==\s*";

    /// Matches string/indexOf call
    pub const INDEXOF_CALL: &str = r"\.indexOf\s*\([^)]+\)";

    /// Matches boolean expression with &&
    pub const BOOL_AND_CHAIN: &str = r"\w+\s*&&\s*\w+\s*&&\s*\w+\.";

    /// Matches vec! macro with const pattern
    pub const VEC_CONST_PATTERN: &str = r"const\s+\[(\w+[^;]*)\]";

    // ══════════════════════════════════════════════════════════════════════════
    // RUST SPECIFIC
    // ══════════════════════════════════════════════════════════════════════════

    /// Matches unsafe block
    pub const UNSAFE_BLOCK: &str = r"unsafe\s*\{";

    /// Matches volatile modifier
    pub const VOLATILE: &str = r"volatile\s+\w+\s+\w+\s*;";

    /// Matches transmute call
    pub const TRANSMUTE: &str = r"transmute\s*<";

    /// Matches skip() call on iterator
    pub const ITER_SKIP: &str = r"\.skip\s*\(\s*\d+\s*\)";

    /// Matches filter() call on iterator
    pub const ITER_FILTER: &str = r"\.filter\s*\([^)]+\)";

    /// Matches equals method call
    pub const EQUALS_METHOD: &str = r"\.equals\s*\(\s*(new\s+\w+|[^)]+)\s*\)";

    /// Matches window assignment
    pub const WINDOW_ASSIGN: &str = r"window\.\w+\s*=";

    /// Matches window.open call
    pub const WINDOW_OPEN: &str = r"window\.open\s*\([^)]*\)";

    // ══════════════════════════════════════════════════════════════════════════
    // JAVA SPECIFIC
    // ══════════════════════════════════════════════════════════════════════════

    /// Matches synchronized block
    pub const SYNCHRONIZED: &str = r"synchronized\s*\(\s*this\s*\)";

    /// Matches public boolean equals
    pub const JAVA_EQUALS: &str = r"public\s+boolean\s+equals\s*\(\s*Object\s*\)";

    /// Matches constructor
    pub const JAVA_CONSTRUCTOR: &str = r"constructor\s*\([^)]*\)";

    /// Matches implements clause
    pub const JAVA_IMPLEMENTS: &str = r"implements\s+(java\.io\.)";

    // ══════════════════════════════════════════════════════════════════════════
    // JAVASCRIPT/TYPESCRIPT SPECIFIC
    // ══════════════════════════════════════════════════════════════════════════

    /// Matches require('http')
    pub const JS_REQUIRE_HTTP: &str = r#"require\s*\(\s*['"]http['"]\s*\)"#;

    /// Matches RegExp constructor
    pub const JS_NEW_REGEX: &str = r"new\s+RegExp\s*\(";

    /// Matches function declaration
    pub const JS_FUNCTION_DECL: &str = r"function\s+\w+\s*\([^)]*\)";

    /// Matches arrow function or const assignment
    pub const JS_ARROW_FUNCTION: &str = r"(?:function\s+\w+|const\s+\w+\s*=\s*(?:async\s*)?";

    /// Matches useEffect hook
    pub const JS_USE_EFFECT: &str = r"useEffect\s*\(\s*\(\s*\)\s*=>";

    /// Matches useState hook
    pub const JS_USE_STATE: &str = r"useState\s*\(\s*\w+\s*\([^)]*\)";

    /// Matches useContext hook
    pub const JS_USE_CONTEXT: &str = r"useContext\s*\(\s*(\w+)";

    /// Matches useCallback hook
    pub const JS_USE_CALLBACK: &str = r"useCallback\s*\([^,]+,\s*\[\s*\]\s*\)";

    /// Matches useLayoutEffect hook
    pub const JS_USE_LAYOUT_EFFECT: &str = r"useLayoutEffect";

    /// Matches useEffect with setState
    pub const JS_USE_EFFECT_SETSTATE: &str = r"useEffect\s*\([^}]*set\w+\s*\([^)]*\)";

    // ══════════════════════════════════════════════════════════════════════════
    // PYTHON SPECIFIC
    // ══════════════════════════════════════════════════════════════════════════

    /// Matches def function declaration
    pub const PY_DEF: &str = r"def\s+\w+\s*\(([^)]*)\)";

    /// Matches def with PascalCase
    pub const PY_DEF_PASCAL: &str = r"def\s+([A-Z][a-zA-Z0-9_]*)";

    /// Matches for loop (Python)
    pub const PY_FOR: &str = r"for\s+\w+\s+in\s+";

    /// Matches if statement (Python)
    pub const PY_IF: &str = r"if\s+\([^)]+\)";

    /// Matches except clause
    pub const PY_EXCEPT: &str = r"except\s*\([^)]*\)";

    /// Matches unittest.skip
    pub const PY_UNITTEST_SKIP: &str = r#"@unittest\.skip\s*\("""#;

    /// Matches type alias
    pub const PY_TYPE_ALIAS: &str = r"type\s+\w+\s*=\s*\{[^}]*\}\s*;";

    /// Matches waitFor (async)
    pub const PY_WAITFOR: &str = r"waitFor\s*\(\s*\(\s*\)\s*=>";

    /// Matches window access
    pub const JS_WINDOW: &str = r"window\.\w+";

    // ══════════════════════════════════════════════════════════════════════════
    // GO SPECIFIC
    // ══════════════════════════════════════════════════════════════════════════

    /// Matches go function declaration
    pub const GO_FUNC: &str = r"func\s+\w+\s*\(\s*\\";

    /// Matches go function with params
    pub const GO_FUNC_PARAMS: &str = r"func\s+\w+\s*\(\s*";

    // ══════════════════════════════════════════════════════════════════════════
    // CRYPTO & SECURITY
    // ══════════════════════════════════════════════════════════════════════════

    /// Matches weak TLS versions
    pub const WEAK_TLS: &str = r"Tlsv1_0|Sslv3|Sslv23|TLSv1\.0|SSLv3";

    /// Matches weak key algorithms (RSA/DSA/DH < 2048)
    pub const WEAK_KEY_ALGO: &str = r"(RSA|DSA|DH)\w*\s*\(\s*1024\s*\)";

    /// Matches chmod 777
    pub const CHMOD_777: &str = r"0o?777|chmod\s+777";

    // ══════════════════════════════════════════════════════════════════════════
    // JAVA SPRING SPECIFIC
    // ══════════════════════════════════════════════════════════════════════════

    /// Matches @Value annotation
    pub const JAVA_VALUE_ANNOTATION: &str = r#"@Value\s*\(\s*"\$\{[^}]+\.[^}]+\}"\s*\)"#;

    /// Matches <...Provider value => pattern
    pub const PROVIDER_VALUE: &str = r"<\w+Provider\s+value\s*=";

    /// Matches typeof undefined check
    pub const JS_TYPEOF_UNDEFINED: &str = r#"typeof\s+\w+\s*==\s*["']undefined["']|typeof\s+\w+\s*===\s*["']undefined["']"#;

    // ══════════════════════════════════════════════════════════════════════════
    // SWITCH & CONTROL
    // ══════════════════════════════════════════════════════════════════════════

    /// Matches switch statement
    pub const SWITCH_STMT: &str = r"switch\s*\([^)]+\)";

    /// Matches it/test pattern in describe/it blocks
    pub const TEST_DESCRIBE_IT: &str = r"(?:it|test";

    // ══════════════════════════════════════════════════════════════════════════
    // USE STATEMENTS
    // ══════════════════════════════════════════════════════════════════════════

    /// Matches use statement
    pub const USE_STMT: &str = r"use\s+([\w:]+)::(\w+)\s*;";

    // ══════════════════════════════════════════════════════════════════════════
    // PATTERN UTILITIES
    // ══════════════════════════════════════════════════════════════════════════

    /// Matches dot-separated method call
    pub const DOT_METHOD_CALL: &str = r"\w+\.\w+\s*\(\s*";

    /// Matches generic type
    pub const GENERIC_TYPE: &str = r"<(\w\w+[^>]*|[A-Z][a-z]+)>";

    /// Matches wait call
    pub const WAIT_CALL: &str = r"\.wait\s*\(";

    /// Matches skip/expect pattern
    pub const SKIP_OR_EXPECT: &str = r"\.skip\s*\(\s*\d+\s*\)|expect\s*\([^)]+\)";

    /// Matches simple word
    pub const SIMPLE_WORD: &str = r"^\w+$";

    /// Matches spread operator
    pub const SPREAD_OPERATOR: &str = r"\{\.\.\.\w+\}";

    /// Matches deref call
    pub const DEREF_CALL: &str = r"\.\w+\(";

    /// Matches Arc:: usage
    pub const ARC_USAGE: &str = r"Arc::";

    /// Matches var declaration (JS/TS)
    pub const VAR_DECL: &str = r"var\s+(\w+)\s*=";

    /// Matches var declaration PascalCase (JS/TS)
    pub const VAR_DECL_PASCAL: &str = r"var\s+([A-Z][a-zA-Z0-9_]*)";
}

// ══════════════════════════════════════════════════════════════════════════
// PATTERN GROUPS — Pre-compiled bundles for multi-pattern matching
// ══════════════════════════════════════════════════════════════════════════

/// Pre-compiled credential patterns for S2068
pub fn credential_patterns() -> Vec<(&'static str, &'static str)> {
    vec![
        (patterns::PASSWORD_CREDENTIAL, "password"),
        (patterns::API_KEY_CREDENTIAL, "api_key"),
        (patterns::SECRET_CREDENTIAL, "secret"),
        (patterns::AUTH_TOKEN, "bearer_token"),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compile_valid_pattern() {
        let re = compile(patterns::TODO_COMMENT);
        assert!(re.is_match("TODO: fix this"));
        assert!(re.is_match("FIXME"));
        assert!(re.is_match("hack"));
    }

    #[test]
    fn test_credential_patterns() {
        let patterns = credential_patterns();
        assert_eq!(patterns.len(), 4);
    }

    #[test]
    fn test_ip_literal_pattern() {
        let re = compile(patterns::IP_LITERAL);
        assert!(re.is_match("\"192.168.1.1\""));
        assert!(re.is_match("\"10.0.0.255\""));
    }

    #[test]
    fn test_weak_tls_pattern() {
        let re = compile(patterns::WEAK_TLS);
        assert!(re.is_match("TLSv1.0"));
        assert!(re.is_match("SSLv3"));
        assert!(re.is_match("Sslv23"));
    }

    #[test]
    fn test_derive_with_trait() {
        let re = compile(patterns::DERIVE_WITH_TRAIT);
        assert!(re.is_match("#[derive(PartialEq, Clone)]"));
        assert!(re.is_match("#[derive(Hash)]"));
    }

    #[test]
    fn test_box_dyn_error() {
        let re = compile(patterns::BOX_DYN_ERROR);
        assert!(re.is_match("Box<dyn Error>"));
    }

    #[test]
    fn test_refcell_primitive() {
        let re = compile(patterns::REFCELL_PRIMITIVE);
        assert!(re.is_match("RefCell<i32>"));
        assert!(re.is_match("RefCell<u64>"));
        assert!(re.is_match("RefCell<bool>"));
        // RefCell<String> should NOT match since String is not a primitive
        assert!(!re.is_match("RefCell<String>"));
    }
}
