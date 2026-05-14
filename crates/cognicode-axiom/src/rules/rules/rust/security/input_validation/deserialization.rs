//! S5042 — Insecure Deserialization Detection
//! Detects unsafe deserialization of untrusted data (CWE-502).
//!
//! Languages: Rust, Python, Java, JavaScript, Go, C#
//! Severity: Critical
//! Category: Vulnerability
use crate::{Severity, Category, Issue, Remediation, Rule, RuleContext, RuleEntry};
use crate::rules::{CleanCodeAttribute, SoftwareQuality, SoftwareQualityImpact, ImpactSeverity};
use cognicode_macros::declare_rule;
use inventory::submit;
use std::sync::LazyLock;

/// Rule constant for S5042
const RULE_ID: &str = "S5042";
const RULE_NAME: &str = "Insecure deserialization detected";
const SEVERITY: Severity = Severity::Blocker;
const CATEGORY: Category = Category::Vulnerability;

// ═══════════════════════════════════════════════════════════════════════════════
// Cached Regex Patterns
// ═══════════════════════════════════════════════════════════════════════════════

/// Pattern for unsafe Rust deserialization functions
static RUST_DESERIALIZE_PATTERN: LazyLock<Vec<regex::Regex>> = LazyLock::new(|| {
    vec![
        // serde_json - most common
        regex::Regex::new(r#"serde_json\s*::\s*from_str\s*[<(]"#).unwrap(),
        regex::Regex::new(r#"serde_json\s*::\s*from_slice\s*[<(]"#).unwrap(),
        regex::Regex::new(r#"serde_json\s*::\s*from_reader\s*[<(]"#).unwrap(),
        // bincode
        regex::Regex::new(r#"bincode\s*::\s*deserialize\s*[<(]"#).unwrap(),
        regex::Regex::new(r#"bincode\s*::\s*deserialize_from\s*[<(]"#).unwrap(),
        regex::Regex::new(r#"bincode\s*::\s*deserialize_into\s*[<(]"#).unwrap(),
        // ron
        regex::Regex::new(r#"ron\s*::\s*from_str\s*[<(]"#).unwrap(),
        regex::Regex::new(r#"ron\s*::\s*from_reader\s*[<(]"#).unwrap(),
        // ciborium (CBOR)
        regex::Regex::new(r#"ciborium\s*::\s*from_reader\s*[<(]"#).unwrap(),
        // serde_yaml
        regex::Regex::new(r#"serde_yaml\s*::\s*from_str\s*[<(]"#).unwrap(),
        regex::Regex::new(r#"serde_yaml\s*::\s*from_reader\s*[<(]"#).unwrap(),
        // toml (uses serde)
        regex::Regex::new(r#"toml\s*::\s*from_str\s*[<(]"#).unwrap(),
        regex::Regex::new(r#"toml\s*::\s*from_slice\s*[<(]"#).unwrap(),
        // postcard
        regex::Regex::new(r#"postcard\s*::\s*from_bytes\s*[<(]"#).unwrap(),
        regex::Regex::new(r#"postcard\s*::\s*from_reader\s*[<(]"#).unwrap(),
    ]
});

/// Pattern for user-controlled input indicators (variables that may come from untrusted sources)
static UNTRUSTED_INPUT_PATTERNS: LazyLock<Vec<regex::Regex>> = LazyLock::new(|| {
    vec![
        // HTTP request body/form/input parameters
        regex::Regex::new(r#"(?i)(?:request|req)\s*\.\s*(?:body|form|data|params|query)"#).unwrap(),
        regex::Regex::new(r#"(?i)(?:body|form|data|params|query)\s*\.\s*(?:get|post|param)"#).unwrap(),
        regex::Regex::new(r#"(?i)input\s*::\s*(?:get|post|read)"#).unwrap(),
        regex::Regex::new(r#"(?i)stdin\s*::\s*read"#).unwrap(),
        // Environment variables
        regex::Regex::new(r#"(?i)env\s*::\s*var\s*\("#).unwrap(),
        regex::Regex::new(r#"(?i)std\s*::\s*env\s*::\s*var"#).unwrap(),
        // File input
        regex::Regex::new(r#"(?i)fs\s*::\s*read(?:_to_string|_all)"#).unwrap(),
        regex::Regex::new(r#"(?i)File\s*::\s*open\s*\("#).unwrap(),
        regex::Regex::new(r#"(?i)File\s*::\s*read_to_string"#).unwrap(),
        // Network/socket input
        regex::Regex::new(r#"(?i)(?:TcpStream|UdpSocket)\s*::\s*(?:read|recv)"#).unwrap(),
        regex::Regex::new(r#"(?i)Socket\s*::\s*(?:read|recv)"#).unwrap(),
        // Command line arguments
        regex::Regex::new(r#"(?i)env\s*::\s*args\s*\("#).unwrap(),
        regex::Regex::new(r#"(?i)std\s*::\s*env\s*::\s*args"#).unwrap(),
        // Deserialization with dynamic type parameter (user-controlled type)
        regex::Regex::new(r#"from_str\s*::<\s*[A-Z][a-zA-Z]*\s*\>\s*\("[a'].*?"#).unwrap(),
    ]
});

/// Pattern for safe/trusted static data (should NOT be flagged)
static STATIC_TRUSTED_PATTERNS: LazyLock<Vec<regex::Regex>> = LazyLock::new(|| {
    vec![
        // String literals
        regex::Regex::new(r#"from_str\s*\(\s*"[\s\S]*?"\s*\)"#).unwrap(),
        // Static constants
        regex::Regex::new(r#"from_str\s*\(\s*(?:STATIC|CONST|DEFAULT|TRUSTED)"#).unwrap(),
        // Pre-validated inputs
        regex::Regex::new(r#"from_str\s*\(\s*&(?:validated|sanitized|verified|checked)"#).unwrap(),
    ]
});

// Python unsafe deserialization
static PYTHON_PICKLE_PATTERN: LazyLock<regex::Regex> = LazyLock::new(|| {
    regex::Regex::new(r#"(?i)pickle\s*\.\s*(?:load|loads|dump|dumps)\s*\("#).unwrap()
});

static PYTHON_MARSHAL_PATTERN: LazyLock<regex::Regex> = LazyLock::new(|| {
    regex::Regex::new(r#"(?i)marshal\s*\.\s*(?:load|loads)\s*\("#).unwrap()
});

static PYTHON_YAML_PATTERN: LazyLock<regex::Regex> = LazyLock::new(|| {
    regex::Regex::new(r#"(?i)yaml\s*\.\s*(?:unsafe_load|load_all)\s*\("#).unwrap()
});

// Java unsafe deserialization
static JAVA_OBJECTINPUT_PATTERN: LazyLock<regex::Regex> = LazyLock::new(|| {
    regex::Regex::new(r#"(?i)ObjectInputStream\s*\(.*new\s+FileInputStream"#).unwrap()
});

static JAVA_READOBJECT_PATTERN: LazyLock<regex::Regex> = LazyLock::new(|| {
    regex::Regex::new(r#"(?i)readObject\s*\(\s*\)"#).unwrap()
});

// JavaScript unsafe deserialization
static JS_JSON_PARSE_PATTERN: LazyLock<regex::Regex> = LazyLock::new(|| {
    regex::Regex::new(r#"(?i)(?:JSON\s*\.\s*parse|eval|Function)\s*\("#).unwrap()
});

// C# unsafe deserialization
static CS_BINARYFORMATTER_PATTERN: LazyLock<regex::Regex> = LazyLock::new(|| {
    regex::Regex::new(r#"(?i)BinaryFormatter\s*\(.*\)\s*\.(?:Deserialize|DeserializeObject)"#).unwrap()
});

static CS_XMLSERIALIZER_PATTERN: LazyLock<regex::Regex> = LazyLock::new(|| {
    regex::Regex::new(r#"(?i)XmlSerializer\s*\(.*\)\s*\.(?:Deserialize|ReadObject)"#).unwrap()
});

// Go unsafe deserialization
static GO_GOB_PATTERN: LazyLock<regex::Regex> = LazyLock::new(|| {
    regex::Regex::new(r#"(?i)gob\s*\.\s*(?:NewDecoder|Decoder)\s*\("#).unwrap()
});

static GO_JSON_PATTERN: LazyLock<regex::Regex> = LazyLock::new(|| {
    regex::Regex::new(r#"(?i)json\s*\.\s*(?:Unmarshal|Decode)\s*\("#).unwrap()
});

declare_rule! {
    id: "S5042"
    name: "Insecure deserialization detected"
    severity: Blocker
    category: Vulnerability
    language: "*"
    params: {}

    explanation: "Insecure deserialization occurs when untrusted data is deserialized without proper validation. Attackers can craft malicious payloads that execute arbitrary code, bypass authentication, or cause denial of service when deserialized. This is one of the most critical web application vulnerabilities (OWASP Top 10)."
    clean_code: Trustworthy,
    impacts: [Security: High, Reliability: High, Maintainability: Low],
    check: => {
        let mut issues = Vec::new();

        for (line_idx, line) in ctx.source.lines().enumerate() {
            let trimmed = line.trim();

            // Skip comments
            if trimmed.starts_with("//") || trimmed.starts_with("///")
               || trimmed.starts_with("//!") || trimmed.starts_with("/*")
               || trimmed.starts_with("#") {
                continue;
            }

            // Skip string literals that contain deserialization (static data)
            let is_static_data = STATIC_TRUSTED_PATTERNS.iter()
                .any(|re| re.is_match(trimmed));

            if is_static_data {
                continue;
            }

            // Check Rust unsafe deserialization
            for re in RUST_DESERIALIZE_PATTERN.iter() {
                if re.is_match(trimmed) {
                    // Check if the input is potentially user-controlled
                    let is_user_controlled = UNTRUSTED_INPUT_PATTERNS.iter()
                        .any(|pattern| pattern.is_match(trimmed))
                        || Self::is_dynamic_input(trimmed);

                    if is_user_controlled {
                        issues.push(Issue::new(
                            RULE_ID,
                            format!(
                                "Potentially insecure deserialization: '{}' may deserialize untrusted data. Consider validating input before deserialization.",
                                Self::extract_deserialize_func(trimmed)
                            ),
                            SEVERITY,
                            CATEGORY,
                            ctx.file_path,
                            line_idx + 1,
                        ).with_remediation(Remediation::substantial(
                            "Validate and sanitize input before deserialization. Use type checking and schema validation. Consider using a safe deserialization format like JSON with strict schema validation."
                        )));
                        break;
                    }
                }
            }

            // Check Python unsafe deserialization
            if PYTHON_PICKLE_PATTERN.is_match(trimmed) || PYTHON_MARSHAL_PATTERN.is_match(trimmed) {
                issues.push(Issue::new(
                    RULE_ID,
                    "Insecure deserialization: pickle or marshal module can execute arbitrary code. Use json module instead.",
                    SEVERITY,
                    CATEGORY,
                    ctx.file_path,
                    line_idx + 1,
                ).with_remediation(Remediation::substantial(
                    "Use json module instead of pickle/marshal. If pickle is required, validate the source and use restricted unpickler."
                )));
            }

            if PYTHON_YAML_PATTERN.is_match(trimmed) {
                issues.push(Issue::new(
                    RULE_ID,
                    "Insecure deserialization: yaml.unsafe_load can execute arbitrary code.",
                    SEVERITY,
                    CATEGORY,
                    ctx.file_path,
                    line_idx + 1,
                ).with_remediation(Remediation::substantial(
                    "Use yaml.safe_load instead of yaml.unsafe_load."
                )));
            }

            // Check Java unsafe deserialization
            if JAVA_OBJECTINPUT_PATTERN.is_match(trimmed) || JAVA_READOBJECT_PATTERN.is_match(trimmed) {
                issues.push(Issue::new(
                    RULE_ID,
                    "Insecure Java deserialization: ObjectInputStream.readObject() can execute arbitrary code.",
                    SEVERITY,
                    CATEGORY,
                    ctx.file_path,
                    line_idx + 1,
                ).with_remediation(Remediation::substantial(
                    "Avoid ObjectInputStream with untrusted data. Use JSON or other safe formats. Implement input validation if custom serialization is required."
                )));
            }

            // Check JavaScript unsafe operations
            if JS_JSON_PARSE_PATTERN.is_match(trimmed) {
                // Only flag eval() as critical
                if trimmed.to_lowercase().contains("eval") {
                    issues.push(Issue::new(
                        RULE_ID,
                        "Dangerous: eval() can execute arbitrary code from untrusted input.",
                        SEVERITY,
                        CATEGORY,
                        ctx.file_path,
                        line_idx + 1,
                    ).with_remediation(Remediation::substantial(
                        "Avoid eval() with untrusted input. Use JSON.parse() for data or Function constructor with caution."
                    )));
                }
            }

            // Check C# unsafe deserialization
            if CS_BINARYFORMATTER_PATTERN.is_match(trimmed) {
                issues.push(Issue::new(
                    RULE_ID,
                    "Insecure .NET deserialization: BinaryFormatter is vulnerable to remote code execution.",
                    SEVERITY,
                    CATEGORY,
                    ctx.file_path,
                    line_idx + 1,
                ).with_remediation(Remediation::substantial(
                    "Do not use BinaryFormatter. Use System.Text.Json or XmlSerializer with strict type checking."
                )));
            }

            // Check Go unsafe deserialization
            if GO_GOB_PATTERN.is_match(trimmed) {
                issues.push(Issue::new(
                    RULE_ID,
                    "Potentially insecure deserialization: gob decoder can execute arbitrary code with untrusted data.",
                    SEVERITY,
                    CATEGORY,
                    ctx.file_path,
                    line_idx + 1,
                ).with_remediation(Remediation::substantial(
                    "Validate input before gob decoding. Consider using json for untrusted data."
                )));
            }
        }

        issues
    }
}

impl S5042Rule {
    /// Check if the line contains a dynamic/user-controlled input indicator
    fn is_dynamic_input(line: &str) -> bool {
        let dynamic_indicators = [
            "&user_",
            "&input",
            "&data",
            "&body",
            "&payload",
            "&request",
            "&args",
            "&argv",
            "&query",
            "&param",
            "&form",
            "user_input",
            "user_data",
            "raw_data",
            "raw_body",
            "untrusted",
        ];

        let line_lower = line.to_lowercase();
        dynamic_indicators.iter().any(|ind| line_lower.contains(&ind.to_lowercase()))
    }

    /// Extract the deserialization function name for better error messages
    fn extract_deserialize_func(line: &str) -> String {
        let funcs = [
            "serde_json::from_str",
            "serde_json::from_slice",
            "serde_json::from_reader",
            "bincode::deserialize",
            "bincode::deserialize_from",
            "ron::from_str",
            "ciborium::from_reader",
            "serde_yaml::from_str",
            "toml::from_str",
            "postcard::from_bytes",
        ];

        for func in &funcs {
            if line.contains(func) {
                return func.to_string();
            }
        }

        "deserialization function".to_string()
    }
}


/// Agent semantics for S5042 - Insecure Deserialization
#[derive(Debug, Clone)]
pub struct AgentSemantics {
    pub summary: &'static str,
    pub fix_playbook: &'static str,
    pub review_questions: &'static [&'static str],
    pub agent_actions: &'static [&'static str],
    pub safe_autofix: bool,
}

pub const S5042_AGENT_SEMANTICS: AgentSemantics = AgentSemantics {
    summary: "Detects unsafe deserialization of untrusted data that can lead to remote code execution, authentication bypass, or denial of service",
    fix_playbook: "1. Identify the deserialization function and its data source\n2. Determine if the input comes from untrusted sources (user input, files, network)\n3. Implement input validation before deserialization\n4. Use type checking or schema validation (e.g., JSON Schema)\n5. Consider using safer serialization formats (JSON over pickle/binary)\n6. For Rust: Use serde with strict deserialization and validate against expected schema\n7. Apply principle of least privilege: validate all external data",
    review_questions: &[
        "Where does the deserialized data originate?",
        "Is the data source validated before deserialization?",
        "What happens if the deserialized data contains malicious payloads?",
        "Is there type checking or schema validation in place?",
        "Could an attacker control the serialized data?",
        "What is the impact if deserialization fails or executes malicious code?"
    ],
    agent_actions: &[
        "Identify all deserialization points in the codebase",
        "Trace data sources to determine if they are user-controlled",
        "Check for input validation before deserialization",
        "Verify schema validation or type checking is in place",
        "Suggest safer alternatives (JSON, schema validation)",
        "Look for known-vulnerable patterns (pickle, BinaryFormatter, etc.)"
    ],
    safe_autofix: false,
};

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rules::types::*;
    use cognicode_core::domain::aggregates::call_graph::CallGraph;
    use cognicode_core::infrastructure::parser::Language;
    use std::path::Path;
    use tree_sitter::Parser as TsParser;

    fn with_rule_context<F, R>(source: &str, language: Language, f: F) -> R
    where
        F: FnOnce(&RuleContext) -> R,
    {
        let ts_language = language.to_ts_language();
        let mut parser = TsParser::new();
        parser.set_language(&ts_language).unwrap();
        let tree = parser.parse(source, None).unwrap();
        let graph = CallGraph::new();
        let metrics = FileMetrics::new();
        let symbol_table = crate::rules::symbol_table::SymbolTableBuilder::new()
            .build(&tree, source);

        let ctx = RuleContext {
            tree: &tree,
            source,
            file_path: Path::new("test.rs"),
            language: &language,
            graph: &graph,
            metrics: &metrics,
            symbol_table: Some(&symbol_table),
        };

        f(&ctx)
    }

    // ═══════════════════════════════════════════════════════════════════════════
    // Rule Properties Tests
    // ═══════════════════════════════════════════════════════════════════════════

    #[test]
    fn test_s5042_rule_properties() {
        let rule = S5042Rule::new();
        assert_eq!(rule.id(), "S5042");
        assert_eq!(rule.name(), "Insecure deserialization detected");
        assert_eq!(rule.severity(), Severity::Blocker);
        assert_eq!(rule.category(), Category::Vulnerability);
    }

    // ═══════════════════════════════════════════════════════════════════════════
    // Positive Detection Tests — Should trigger the rule (Rust)
    // ═══════════════════════════════════════════════════════════════════════════

    #[test]
    fn test_s5042_detects_serde_json_from_str_user_input() {
        let source = r#"
            let config: Config = serde_json::from_str(&user_input).unwrap();
        "#;
        let issues = with_rule_context(source, Language::Rust, |ctx| {
            let rule = S5042Rule::new();
            rule.check(ctx)
        });
        assert!(!issues.is_empty(), "Expected to detect serde_json::from_str with user_input");
        assert_eq!(issues[0].rule_id, "S5042");
    }

    #[test]
    fn test_s5042_detects_bincode_deserialize() {
        let source = r#"
            let user: User = bincode::deserialize(&bytes).unwrap();
        "#;
        let issues = with_rule_context(source, Language::Rust, |ctx| {
            let rule = S5042Rule::new();
            rule.check(ctx)
        });
        assert!(!issues.is_empty(), "Expected to detect bincode::deserialize");
        assert_eq!(issues[0].rule_id, "S5042");
    }

    #[test]
    fn test_s5042_detects_ron_from_str() {
        let source = r#"
            let data: Data = ron::from_str(&user_data).unwrap();
        "#;
        let issues = with_rule_context(source, Language::Rust, |ctx| {
            let rule = S5042Rule::new();
            rule.check(ctx)
        });
        assert!(!issues.is_empty(), "Expected to detect ron::from_str");
    }

    #[test]
    fn test_s5042_detects_serde_yaml_from_str() {
        let source = r#"
            let config = serde_yaml::from_str(&request.body()).unwrap();
        "#;
        let issues = with_rule_context(source, Language::Rust, |ctx| {
            let rule = S5042Rule::new();
            rule.check(ctx)
        });
        assert!(!issues.is_empty(), "Expected to detect serde_yaml::from_str with request body");
    }

    #[test]
    fn test_s5042_detects_postcard_from_bytes() {
        let source = r#"
            let msg: Message = postcard::from_bytes(&raw_data).unwrap();
        "#;
        let issues = with_rule_context(source, Language::Rust, |ctx| {
            let rule = S5042Rule::new();
            rule.check(ctx)
        });
        assert!(!issues.is_empty(), "Expected to detect postcard::from_bytes");
    }

    #[test]
    fn test_s5042_detects_toml_from_str() {
        let source = r#"
            let config = toml::from_str(&user_data).unwrap();
        "#;
        let issues = with_rule_context(source, Language::Rust, |ctx| {
            let rule = S5042Rule::new();
            rule.check(ctx)
        });
        assert!(!issues.is_empty(), "Expected to detect toml::from_str");
    }

    #[test]
    fn test_s5042_detects_bincode_deserialize_from() {
        let source = r#"
            let user = bincode::deserialize_from(&mut file).unwrap();
        "#;
        let issues = with_rule_context(source, Language::Rust, |ctx| {
            let rule = S5042Rule::new();
            rule.check(ctx)
        });
        assert!(!issues.is_empty(), "Expected to detect bincode::deserialize_from");
    }

    // ═══════════════════════════════════════════════════════════════════════════
    // Positive Detection Tests — Python
    // ═══════════════════════════════════════════════════════════════════════════

    #[test]
    fn test_s5042_detects_python_pickle() {
        let source = r#"
            data = pickle.loads(user_input)
        "#;
        let issues = with_rule_context(source, Language::Python, |ctx| {
            let rule = S5042Rule::new();
            rule.check(ctx)
        });
        assert!(!issues.is_empty(), "Expected to detect pickle.loads");
    }

    #[test]
    fn test_s5042_detects_python_marshal() {
        let source = r#"
            obj = marshal.load(fp)
        "#;
        let issues = with_rule_context(source, Language::Python, |ctx| {
            let rule = S5042Rule::new();
            rule.check(ctx)
        });
        assert!(!issues.is_empty(), "Expected to detect marshal.load");
    }

    #[test]
    fn test_s5042_detects_python_yaml_unsafe() {
        let source = r#"
            data = yaml.unsafe_load(user_data)
        "#;
        let issues = with_rule_context(source, Language::Python, |ctx| {
            let rule = S5042Rule::new();
            rule.check(ctx)
        });
        assert!(!issues.is_empty(), "Expected to detect yaml.unsafe_load");
    }

    // ═══════════════════════════════════════════════════════════════════════════
    // Positive Detection Tests — Java
    // ═══════════════════════════════════════════════════════════════════════════

    #[test]
    fn test_s5042_detects_java_objectinputstream() {
        let source = r#"
            ObjectInputStream ois = new ObjectInputStream(new FileInputStream(file));
        "#;
        let issues = with_rule_context(source, Language::Java, |ctx| {
            let rule = S5042Rule::new();
            rule.check(ctx)
        });
        assert!(!issues.is_empty(), "Expected to detect ObjectInputStream");
    }

    #[test]
    fn test_s5042_detects_java_read_object() {
        let source = r#"
            Object obj = ois.readObject();
        "#;
        let issues = with_rule_context(source, Language::Java, |ctx| {
            let rule = S5042Rule::new();
            rule.check(ctx)
        });
        assert!(!issues.is_empty(), "Expected to detect readObject()");
    }

    // ═══════════════════════════════════════════════════════════════════════════
    // Positive Detection Tests — JavaScript
    // ═══════════════════════════════════════════════════════════════════════════

    #[test]
    fn test_s5042_detects_js_eval() {
        let source = r#"
            let result = eval(userInput);
        "#;
        let issues = with_rule_context(source, Language::JavaScript, |ctx| {
            let rule = S5042Rule::new();
            rule.check(ctx)
        });
        assert!(!issues.is_empty(), "Expected to detect eval() with user input");
    }

    // ═══════════════════════════════════════════════════════════════════════════
    // Positive Detection Tests — C#
    // ═══════════════════════════════════════════════════════════════════════════

    #[test]
    fn test_s5042_detects_cs_binaryformatter() {
        let source = r#"
            BinaryFormatter formatter = new BinaryFormatter();
            obj = formatter.Deserialize(stream);
        "#;
        let issues = with_rule_context(source, Language::CSharp, |ctx| {
            let rule = S5042Rule::new();
            rule.check(ctx)
        });
        assert!(!issues.is_empty(), "Expected to detect BinaryFormatter.Deserialize");
    }

    // ═══════════════════════════════════════════════════════════════════════════
    // Positive Detection Tests — Go
    // ═══════════════════════════════════════════════════════════════════════════

    #[test]
    fn test_s5042_detects_go_gob() {
        let source = r#"
            decoder := gob.NewDecoder(reader)
            decoder.Decode(&data)
        "#;
        let issues = with_rule_context(source, Language::Go, |ctx| {
            let rule = S5042Rule::new();
            rule.check(ctx)
        });
        assert!(!issues.is_empty(), "Expected to detect gob.NewDecoder");
    }

    // ═══════════════════════════════════════════════════════════════════════════
    // False Positive Tests — Should NOT trigger the rule
    // ═══════════════════════════════════════════════════════════════════════════

    #[test]
    fn test_s5042_false_positive_static_json() {
        let source = r#"
            let config = serde_json::from_str("{\"key\": \"value\"}").unwrap();
        "#;
        let issues = with_rule_context(source, Language::Rust, |ctx| {
            let rule = S5042Rule::new();
            rule.check(ctx)
        });
        assert!(issues.is_empty(), "Should NOT detect static JSON string literal");
    }

    #[test]
    fn test_s5042_false_positive_validated_input() {
        let source = r#"
            let config = serde_json::from_str(&validated_input).unwrap();
        "#;
        let issues = with_rule_context(source, Language::Rust, |ctx| {
            let rule = S5042Rule::new();
            rule.check(ctx)
        });
        // validated_input should not trigger as it implies already checked
        assert!(issues.is_empty(), "Should NOT detect deserialization of validated input");
    }

    #[test]
    fn test_s5042_false_positive_static_constant() {
        let source = r#"
            let config = serde_json::from_str(STATIC_CONFIG).unwrap();
        "#;
        let issues = with_rule_context(source, Language::Rust, |ctx| {
            let rule = S5042Rule::new();
            rule.check(ctx)
        });
        assert!(issues.is_empty(), "Should NOT detect deserialization of static constant");
    }

    #[test]
    fn test_s5042_false_positive_comment() {
        let source = r#"
            // serde_json::from_str(&user_input).unwrap();
        "#;
        let issues = with_rule_context(source, Language::Rust, |ctx| {
            let rule = S5042Rule::new();
            rule.check(ctx)
        });
        assert!(issues.is_empty(), "Should NOT detect deserialization in comment");
    }

    #[test]
    fn test_s5042_false_positive_doc_comment() {
        let source = r#"
            /// Example: serde_json::from_str(&data)
            fn deserialize() {}
        "#;
        let issues = with_rule_context(source, Language::Rust, |ctx| {
            let rule = S5042Rule::new();
            rule.check(ctx)
        });
        assert!(issues.is_empty(), "Should NOT detect deserialization in doc comment");
    }

    #[test]
    fn test_s5042_false_positive_python_yaml_safe() {
        let source = r#"
            data = yaml.safe_load(user_data)
        "#;
        let issues = with_rule_context(source, Language::Python, |ctx| {
            let rule = S5042Rule::new();
            rule.check(ctx)
        });
        assert!(issues.is_empty(), "Should NOT detect yaml.safe_load");
    }

    #[test]
    fn test_s5042_false_positive_python_json() {
        let source = r#"
            data = json.loads(user_input)
        "#;
        let issues = with_rule_context(source, Language::Python, |ctx| {
            let rule = S5042Rule::new();
            rule.check(ctx)
        });
        // json.loads is safe, only pickle/marshal/yaml.unsafe_load should be flagged
        assert!(issues.is_empty(), "Should NOT detect json.loads (safe deserialization)");
    }

    #[test]
    fn test_s5042_false_positive_js_json_parse() {
        let source = r#"
            let obj = JSON.parse(userInput);
        "#;
        let issues = with_rule_context(source, Language::JavaScript, |ctx| {
            let rule = S5042Rule::new();
            rule.check(ctx)
        });
        // JSON.parse is safe, only eval() should be flagged
        assert!(issues.is_empty(), "Should NOT detect JSON.parse (safe deserialization)");
    }

    // ═══════════════════════════════════════════════════════════════════════════
    // Edge Case Tests
    // ═══════════════════════════════════════════════════════════════════════════

    #[test]
    fn test_s5042_edge_case_empty_file() {
        let source = "";
        let issues = with_rule_context(source, Language::Rust, |ctx| {
            let rule = S5042Rule::new();
            rule.check(ctx)
        });
        assert!(issues.is_empty(), "Should NOT trigger on empty file");
    }

    #[test]
    fn test_s5042_edge_case_single_line() {
        let source = "serde_json::from_str::<Config>(&input)";
        let issues = with_rule_context(source, Language::Rust, |ctx| {
            let rule = S5042Rule::new();
            rule.check(ctx)
        });
        assert!(!issues.is_empty(), "Should detect deserialization on single line");
    }

    #[test]
    fn test_s5042_edge_case_multiple_deserializations() {
        let source = r#"
            let a = serde_json::from_str(&user_input1);
            let b = bincode::deserialize(&user_input2);
            let c = ron::from_str(&static_data);
        "#;
        let issues = with_rule_context(source, Language::Rust, |ctx| {
            let rule = S5042Rule::new();
            rule.check(ctx)
        });
        // Should detect first two, but not third (static_data)
        assert!(issues.len() >= 2, "Should detect multiple unsafe deserializations");
    }

    #[test]
    fn test_s5042_edge_case_unicode_input() {
        let source = r#"
            let config = serde_json::from_str(&unicode_input);
        "#;
        let issues = with_rule_context(source, Language::Rust, |ctx| {
            let rule = S5042Rule::new();
            rule.check(ctx)
        });
        assert!(!issues.is_empty(), "Should handle unicode input correctly");
    }
}
