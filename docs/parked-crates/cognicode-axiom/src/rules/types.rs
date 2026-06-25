//! Rule engine core types
//!
//! Provides the foundational types for the rule engine including severity levels,
//! categories, issue reporting, and the rule trait.

#![allow(non_local_definitions)] // inventory::collect! generates non-local impl

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::RwLock;

use rayon::prelude::*;
use cognicode_core::domain::aggregates::call_graph::{CallGraph, SymbolId};
use cognicode_core::infrastructure::parser::Language;
use streaming_iterator::StreamingIterator;

use crate::rules::symbol_table::SymbolTable;

/// Severity level for issues detected by rules
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize, serde::Deserialize)]
pub enum Severity {
    Info = 1,
    Minor = 2,
    Major = 3,
    Critical = 4,
    Blocker = 5,
}

impl Severity {
    /// Returns a human-readable label for the severity
    pub fn label(self) -> &'static str {
        match self {
            Severity::Info => "Info",
            Severity::Minor => "Minor",
            Severity::Major => "Major",
            Severity::Critical => "Critical",
            Severity::Blocker => "Blocker",
        }
    }
}

/// Category of issues detected by rules
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum Category {
    Bug,
    Vulnerability,
    CodeSmell,
    SecurityHotspot,
}

impl Category {
    /// Returns a human-readable label for the category
    pub fn label(self) -> &'static str {
        match self {
            Category::Bug => "Bug",
            Category::Vulnerability => "Vulnerability",
            Category::CodeSmell => "Code Smell",
            Category::SecurityHotspot => "Security Hotspot",
        }
    }
}

/// Remediation information for fixing an issue
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Remediation {
    /// Estimated effort to fix in minutes
    pub effort_minutes: u32,
    /// Description of how to fix the issue
    pub description: String,
}

impl Remediation {
    /// Create a new remediation
    pub fn new(effort_minutes: u32, description: impl Into<String>) -> Self {
        Self {
            effort_minutes,
            description: description.into(),
        }
    }

    /// Quick fix - 5 minutes
    pub fn quick(description: impl Into<String>) -> Self {
        Self::new(5, description)
    }

    /// Moderate fix - 15 minutes
    pub fn moderate(description: impl Into<String>) -> Self {
        Self::new(15, description)
    }

    /// Substantial fix - 60 minutes
    pub fn substantial(description: impl Into<String>) -> Self {
        Self::new(60, description)
    }
}

/// A detected issue from a rule check
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Issue {
    /// Unique identifier of the rule that generated this issue
    pub rule_id: String,
    /// Human-readable message describing the issue
    pub message: String,
    /// Severity level of the issue
    pub severity: Severity,
    /// Category of the issue
    pub category: Category,
    /// File path where the issue was found
    pub file: PathBuf,
    /// Line number (1-indexed)
    pub line: usize,
    /// Optional column number (1-indexed)
    pub column: Option<usize>,
    /// Optional end line number for multi-line ranges
    pub end_line: Option<usize>,
    /// Optional remediation guidance
    pub remediation: Option<Remediation>,
    /// Entity type detected (auto-extracted from AST node)
    #[serde(default)]
    pub entity_type: EntityType,
    /// Scope where the issue was found (auto-detected)
    #[serde(default)]
    pub scope: Scope,
    /// The actual code fragment that triggered the issue
    #[serde(default)]
    pub code_snippet: Option<String>,
    /// Name of the variable/function/class involved
    #[serde(default)]
    pub variable_name: Option<String>,
    /// Explanation of WHY this is a problem (for dashboard display)
    #[serde(default)]
    pub explanation: Option<String>,
    /// Example of BAD code that triggers this issue
    #[serde(default)]
    pub bad_example: Option<String>,
    /// Example of GOOD code that fixes the issue
    #[serde(default)]
    pub good_example: Option<String>,
}

impl Issue {
    /// Create a new issue
    pub fn new(
        rule_id: impl Into<String>,
        message: impl Into<String>,
        severity: Severity,
        category: Category,
        file: impl Into<PathBuf>,
        line: usize,
    ) -> Self {
        Self {
            rule_id: rule_id.into(),
            message: message.into(),
            severity,
            category,
            file: file.into(),
            line,
            column: None,
            end_line: None,
            remediation: None,
            entity_type: EntityType::Unknown,
            scope: Scope::Unknown,
            code_snippet: None,
            variable_name: None,
            explanation: None,
            bad_example: None,
            good_example: None,
        }
    }

    /// Create an issue from a tree-sitter node with auto-enrichment
    pub fn from_node(
        rule_id: impl Into<String>,
        message: impl Into<String>,
        severity: Severity,
        category: Category,
        file: impl Into<PathBuf>,
        line: usize,
        ctx: &RuleContext,
        node: tree_sitter::Node,
    ) -> Self {
        let source_bytes = ctx.source.as_bytes();
        let lang = ctx.language.name();
        let entity_type = EntityType::from_node_kind(node.kind(), lang);
        let scope = Scope::detect(node, lang);
        let code_snippet = node.utf8_text(source_bytes).ok().map(|s| s.to_string());
        let variable_name = Self::extract_name(node, source_bytes);

        Self {
            rule_id: rule_id.into(),
            message: message.into(),
            severity,
            category,
            file: file.into(),
            line,
            column: None,
            end_line: None,
            remediation: None,
            entity_type,
            scope,
            code_snippet,
            variable_name,
            explanation: None,
            bad_example: None,
            good_example: None,
        }
    }

    /// Extract identifier name from node or its children
    fn extract_name(node: tree_sitter::Node, source: &[u8]) -> Option<String> {
        if node.kind().contains("identifier") || node.kind().contains("variable") {
            return node.utf8_text(source).ok().map(|s| s.to_string());
        }
        for i in 0..node.named_child_count() {
            if let Some(child) = node.named_child(i) {
                let kind = child.kind();
                if kind.contains("identifier") || kind.contains("variable") || kind == "name" || kind == "property_identifier" {
                    return child.utf8_text(source).ok().map(|s| s.to_string());
                }
            }
        }
        None
    }

    /// Set the column number
    pub fn with_column(mut self, column: usize) -> Self {
        self.column = Some(column);
        self
    }

    /// Set the end line number
    pub fn with_end_line(mut self, end_line: usize) -> Self {
        self.end_line = Some(end_line);
        self
    }

    /// Set the remediation guidance
    pub fn with_remediation(mut self, remediation: Remediation) -> Self {
        self.remediation = Some(remediation);
        self
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Clean Code Attributes (SonarQube standard)
// ─────────────────────────────────────────────────────────────────────────────

/// Clean Code Attribute — what aspect of "clean code" a rule evaluates
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum CleanCodeAttribute {
    // Consistency
    Formatted,
    Conventional,
    Identifiable,
    // Intentionality
    Clear,
    Logical,
    Complete,
    Efficient,
    // Adaptability
    Focused,
    Distinct,
    Modular,
    // Responsibility
    Lawful,
    Trustworthy,
    Respectful,
}

impl Default for CleanCodeAttribute {
    fn default() -> Self { Self::Clear }
}

/// Software quality that a rule impacts
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum SoftwareQuality {
    Security,
    Reliability,
    Maintainability,
}

/// Severity of an impact on a software quality
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum ImpactSeverity {
    Blocker,
    High,
    Medium,
    Low,
    Info,
}

/// A single software quality impact for a rule
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SoftwareQualityImpact {
    pub quality: SoftwareQuality,
    pub severity: ImpactSeverity,
}

impl Default for SoftwareQuality {
    fn default() -> Self { Self::Maintainability }
}

impl Default for ImpactSeverity {
    fn default() -> Self { Self::Medium }
}

// ─────────────────────────────────────────────────────────────────────────────
// Entity Type & Scope — auto-extracted from AST
// ─────────────────────────────────────────────────────────────────────────────

/// Entity type detected by a rule (auto-extracted from tree-sitter node)
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum EntityType {
    FunctionDef,
    ClassDef,
    MethodCall,
    Assignment,
    Variable,
    Import,
    Conditional,
    Loop,
    StringLiteral,
    Return,
    Expression,
    Pattern,
    TraitImpl,
    Unknown,
}

impl Default for EntityType {
    fn default() -> Self { Self::Unknown }
}

impl EntityType {
    pub fn from_node_kind(kind: &str, language: &str) -> Self {
        match (language, kind) {
            // Rust
            ("Rust", "function_item") => Self::FunctionDef,
            ("Rust", "function_signature_item") => Self::FunctionDef,
            ("Rust", "struct_item") => Self::ClassDef,
            ("Rust", "enum_item") => Self::ClassDef,
            ("Rust", "trait_item") => Self::ClassDef,
            ("Rust", "impl_item") => Self::TraitImpl,
            ("Rust", "call_expression") => Self::MethodCall,
            ("Rust", "let_declaration") => Self::Assignment,
            ("Rust", "use_declaration") => Self::Import,
            ("Rust", "if_expression") => Self::Conditional,
            ("Rust", "match_expression") => Self::Conditional,
            ("Rust", "for_expression") => Self::Loop,
            ("Rust", "while_expression") => Self::Loop,
            ("Rust", "loop_expression") => Self::Loop,
            ("Rust", "string_literal") => Self::StringLiteral,
            ("Rust", "raw_string_literal") => Self::StringLiteral,
            ("Rust", "return_expression") => Self::Return,
            ("Rust", "identifier") => Self::Variable,
            ("Rust", "binary_expression") => Self::Expression,
            // Python
            ("Python", "function_definition") => Self::FunctionDef,
            ("Python", "class_definition") => Self::ClassDef,
            ("Python", "call") => Self::MethodCall,
            ("Python", "assignment") => Self::Assignment,
            ("Python", "augmented_assignment") => Self::Assignment,
            ("Python", "import_statement") => Self::Import,
            ("Python", "import_from_statement") => Self::Import,
            ("Python", "if_statement") => Self::Conditional,
            ("Python", "match_statement") => Self::Conditional,
            ("Python", "for_statement") => Self::Loop,
            ("Python", "while_statement") => Self::Loop,
            ("Python", "string") => Self::StringLiteral,
            ("Python", "return_statement") => Self::Return,
            ("Python", "identifier") => Self::Variable,
            ("Python", "binary_operator") => Self::Expression,
            // JavaScript / TypeScript
            ("JavaScript", k) | ("TypeScript", k) => match k {
                "function_declaration" | "arrow_function" | "function_expression" => Self::FunctionDef,
                "class_declaration" => Self::ClassDef,
                "call_expression" => Self::MethodCall,
                "assignment_expression" | "variable_declarator" => Self::Assignment,
                "import_statement" => Self::Import,
                "if_statement" | "switch_statement" => Self::Conditional,
                "for_statement" | "while_statement" => Self::Loop,
                "string" | "template_string" => Self::StringLiteral,
                "return_statement" => Self::Return,
                "identifier" => Self::Variable,
                "binary_expression" | "unary_expression" => Self::Expression,
                _ => Self::Unknown,
            },
            // Java
            ("Java", "method_declaration") => Self::FunctionDef,
            ("Java", "class_declaration") => Self::ClassDef,
            ("Java", "method_invocation") => Self::MethodCall,
            ("Java", "assignment_expression") => Self::Assignment,
            ("Java", "import_declaration") => Self::Import,
            ("Java", "if_statement") => Self::Conditional,
            ("Java", "switch_expression") => Self::Conditional,
            ("Java", "for_statement") => Self::Loop,
            ("Java", "while_statement") => Self::Loop,
            ("Java", "string_literal") => Self::StringLiteral,
            ("Java", "return_statement") => Self::Return,
            ("Java", "identifier") => Self::Variable,
            // Go
            ("Go", "function_declaration") => Self::FunctionDef,
            ("Go", "type_declaration") => Self::ClassDef,
            ("Go", "call_expression") => Self::MethodCall,
            ("Go", "short_var_declaration") => Self::Assignment,
            ("Go", "assignment_statement") => Self::Assignment,
            ("Go", "import_declaration") => Self::Import,
            ("Go", "if_statement") => Self::Conditional,
            ("Go", "for_statement") => Self::Loop,
            ("Go", "interpreted_string_literal") => Self::StringLiteral,
            ("Go", "raw_string_literal") => Self::StringLiteral,
            ("Go", "return_statement") => Self::Return,
            ("Go", "identifier") => Self::Variable,
            // Generic fallback
            _ => {
                if kind.contains("function") || kind.contains("method") { Self::FunctionDef }
                else if kind.contains("class") || kind.contains("struct") || kind.contains("enum") || kind.contains("interface") { Self::ClassDef }
                else if kind.contains("call") || kind.contains("invocation") { Self::MethodCall }
                else if kind.contains("assignment") || kind.contains("declaration") || kind.contains("let_") || kind.contains("var_") { Self::Assignment }
                else if kind.contains("import") || kind.contains("use_") { Self::Import }
                else if kind.contains("if_") || kind.contains("match") || kind.contains("switch") { Self::Conditional }
                else if kind.contains("for_") || kind.contains("while") || kind.contains("loop_") { Self::Loop }
                else if kind.contains("string") || kind.contains("literal") { Self::StringLiteral }
                else if kind.contains("return") { Self::Return }
                else if kind.contains("identifier") || kind.contains("variable") { Self::Variable }
                else if kind.contains("binary") || kind.contains("expression") { Self::Expression }
                else { Self::Unknown }
            }
        }
    }
}

/// Scope where an issue was detected (auto-detected from AST ancestors)
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum Scope {
    Local,
    Function,
    Method,
    Class,
    Module,
    Global,
    Unknown,
}

impl Default for Scope {
    fn default() -> Self { Self::Unknown }
}

impl Scope {
    pub fn detect(node: tree_sitter::Node, language: &str) -> Self {
        let mut current = node;
        while let Some(parent) = current.parent() {
            let kind = parent.kind();
            let (is_function, is_class) = match (language, kind) {
                // Function-level
                ("Rust", "function_item" | "closure_expression") => (true, false),
                ("Python", "function_definition" | "lambda") => (true, false),
                ("JavaScript", "function_declaration" | "arrow_function" | "function_expression") => (true, false),
                ("TypeScript", "function_declaration" | "arrow_function" | "method_definition") => (true, false),
                ("Java", "method_declaration" | "constructor_declaration") => (true, false),
                ("Go", "function_declaration") => (true, false),
                // Class-level
                ("Rust", "struct_item" | "enum_item" | "trait_item" | "impl_item") => (false, true),
                ("Python", "class_definition") => (false, true),
                ("JavaScript", "class_declaration") => (false, true),
                ("TypeScript", "class_declaration" | "interface_declaration") => (false, true),
                ("Java", "class_declaration" | "interface_declaration" | "enum_declaration") => (false, true),
                ("Go", "type_declaration") => (false, true),
                _ => (false, false),
            };
            if is_function { return Self::Function; }
            if is_class { return Self::Class; }
            current = parent;
        }
        Self::Global
    }
}

/// Cache for parsed files to avoid re-parsing
pub struct ParseCache {
    cache: RwLock<HashMap<PathBuf, (tree_sitter::Tree, String)>>,
}

impl ParseCache {
    /// Create a new parse cache
    pub fn new() -> Self {
        Self { cache: RwLock::new(HashMap::new()) }
    }

    /// Get or parse a file. Returns (Tree, source) tuple.
    pub fn get_or_parse(&self, path: &Path) -> Result<(tree_sitter::Tree, String), String> {
        // Check cache
        if let Some(cached) = self.cache.read().map_err(|e| e.to_string())?.get(path) {
            return Ok((cached.0.clone(), cached.1.clone()));
        }

        // Parse
        let source = std::fs::read_to_string(path)
            .map_err(|e| format!("Cannot read {}: {}", path.display(), e))?;
        let ext = path.extension();
        let language = Language::from_extension(ext)
            .ok_or_else(|| "Unknown language".to_string())?;
        let mut parser = tree_sitter::Parser::new();
        parser.set_language(&language.to_ts_language())
            .map_err(|e| format!("{}", e))?;
        let tree = parser.parse(&source, None)
            .ok_or_else(|| "Parse failed".to_string())?;

        // Store in cache
        if let Ok(mut cache) = self.cache.write() {
            cache.insert(path.to_path_buf(), (tree.clone(), source.clone()));
        }

        Ok((tree, source))
    }

    /// Invalidate cache entry for a path
    pub fn invalidate(&self, path: &Path) {
        if let Ok(mut cache) = self.cache.write() {
            cache.remove(path);
        }
    }

    /// Get number of cached entries
    pub fn len(&self) -> usize {
        self.cache.read().map(|c| c.len()).unwrap_or(0)
    }
}

impl Default for ParseCache {
    fn default() -> Self { Self::new() }
}

/// File-level metrics for rule analysis
#[derive(Debug, Clone, Default)]
pub struct FileMetrics {
    /// Number of lines of code
    pub lines_of_code: usize,
    /// Number of functions
    pub function_count: usize,
    /// Number of classes/structs
    pub struct_count: usize,
    /// Cyclomatic complexity sum
    pub total_complexity: u32,
    /// Number of comments
    pub comment_lines: usize,
}

impl FileMetrics {
    /// Create new empty file metrics
    pub fn new() -> Self {
        Self::default()
    }

    /// Create with basic counts
    pub fn with_counts(
        lines_of_code: usize,
        function_count: usize,
        struct_count: usize,
    ) -> Self {
        Self {
            lines_of_code,
            function_count,
            struct_count,
            total_complexity: 0,
            comment_lines: 0,
        }
    }
}

/// Context passed to rule checks
pub struct RuleContext<'a> {
    /// The parsed syntax tree
    pub tree: &'a tree_sitter::Tree,
    /// The source code being analyzed
    pub source: &'a str,
    /// Path to the file being analyzed
    pub file_path: &'a Path,
    /// Programming language of the file
    pub language: &'a Language,
    /// Call graph for the project
    pub graph: &'a CallGraph,
    /// File-level metrics
    pub metrics: &'a FileMetrics,
    /// Optional per-file symbol table (LCPG MVP)
    /// Built during analysis for semantic rules
    pub symbol_table: Option<&'a SymbolTable>,
}

/// The Rule trait that all code smell rules must implement
pub trait Rule: Send + Sync {
    /// Returns the unique identifier for this rule
    fn id(&self) -> &str;

    /// Returns the human-readable name of this rule
    fn name(&self) -> &str;

    /// Returns the severity level of issues this rule produces
    fn severity(&self) -> Severity;

    /// Returns the category of issues this rule produces
    fn category(&self) -> Category;

    /// Returns the language this rule applies to
    fn language(&self) -> &str;

    /// Analyze the context and return any issues found
    fn check(&self, ctx: &RuleContext) -> Vec<Issue>;

    // ─────────────────────────────────────────────────────────────────────────
    // UI Metadata — Dashboard integration (all have safe defaults)
    // ─────────────────────────────────────────────────────────────────────────

    /// Returns the UI category for this rule (e.g., "Error Handling", "Code Structure")
    fn ui_category(&self) -> Option<&str> { None }

    /// Returns the dashboard group for this rule (e.g., "Reliability", "Security", "Maintainability")
    fn dashboard_group(&self) -> Option<&str> { None }

    /// Returns the display icon identifier for this rule (e.g., "warning-triangle", "function", "lock")
    fn display_icon(&self) -> Option<&str> { None }

    /// Returns tags associated with this rule for filtering and search
    fn tags(&self) -> Vec<&str> { vec![] }

    /// Returns the effort category for fixing issues of this rule (e.g., "quick_fix", "moderate", "complex")
    fn effort_category(&self) -> Option<&str> { None }

    /// Returns the explanation of why this rule exists and what problem it detects
    fn explanation(&self) -> Option<&str> { None }

    /// Returns an example of bad code that triggers this rule
    fn bad_example(&self) -> Option<&str> { None }

    /// Returns an example of good code that satisfies this rule
    fn good_example(&self) -> Option<&str> { None }

    /// Returns the type of entity this rule affects
    fn affected_entity(&self) -> Option<&str> { None }

    /// Returns the default scope for issues from this rule
    fn default_scope(&self) -> Option<&str> { None }

    /// Returns the clean code attribute for this rule
    fn clean_code_attribute(&self) -> Option<CleanCodeAttribute> { None }

    /// Returns the software quality impacts for this rule
    fn software_qualities(&self) -> Vec<SoftwareQualityImpact> { vec![] }

    // ─────────────────────────────────────────────────────────────────────────
    // Layer-based rule execution (Phase 2+ for performance optimization)
    // ─────────────────────────────────────────────────────────────────────────

    /// Returns the layer this rule operates on for performance optimization.
    /// Layer 0 = preflight (keyword-based fast rejection)
    /// Layer 1 = structural (AST pattern matching) - default
    /// Layer 2 = semantic (type checking, data flow)
    /// Layer 3 = flow (cross-function, whole-program analysis)
    fn layer(&self) -> u8 { 1 }

    /// Returns keywords that must be present in source for this rule to apply.
    /// Used for Layer 0 preflight filtering - rules without keywords always run.
    fn required_keywords(&self) -> Vec<&str> { vec![] }
}

/// A rule entry for inventory-based registration
#[derive(Clone)]
pub struct RuleEntry {
    /// Factory function to create a rule instance
    pub factory: fn() -> Box<dyn Rule>,
}

impl RuleEntry {
    /// Instantiate a new rule from this entry
    pub fn instantiate(&self) -> Box<dyn Rule> {
        (self.factory)()
    }
}

/// Registry of all available rules with discovery via inventory
pub struct RuleRegistry {
    rules: Vec<Box<dyn Rule>>,
    by_language: HashMap<String, Vec<usize>>,
    by_category: HashMap<Category, Vec<usize>>,
    /// Layer-0 preflight filter for fast keyword-based rule eligibility
    preflight: Option<crate::rules::preflight::PreflightFilter>,
}

impl std::fmt::Debug for RuleRegistry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RuleRegistry")
            .field("rules_count", &self.rules.len())
            .field("by_language", &self.by_language.keys())
            .field("by_category", &self.by_category.keys())
            .finish()
    }
}

impl RuleRegistry {
    /// Discover and load all rules registered via the `declare_rule!` macro
    pub fn discover() -> Self {
        let mut registry = Self {
            rules: Vec::new(),
            by_language: HashMap::new(),
            by_category: HashMap::new(),
            preflight: None,
        };

        // Instantiate the inventory registry
        inventory::collect!(RuleEntry);

        // Iterate over all registered rule entries
        for entry in inventory::iter::<RuleEntry> {
            let rule = entry.instantiate();
            let idx = registry.rules.len();
            registry.rules.push(rule);

            // Index by language
            let lang = registry.rules[idx].language().to_lowercase();
            registry
                .by_language
                .entry(lang.clone())
                .or_default()
                .push(idx);

            // If language is "*", also index under every supported language key
            if lang == "*" {
                for supported_lang in ["python", "javascript", "typescript", "go", "java", "rust"] {
                    registry
                        .by_language
                        .entry(supported_lang.to_string())
                        .or_default()
                        .push(idx);
                }
            }

            // Index by category
            let cat = registry.rules[idx].category();
            registry.by_category.entry(cat).or_default().push(idx);
        }

        // Build preflight filter from all loaded rules
        registry.preflight = Some(crate::rules::preflight::PreflightFilter::new(&registry.rules));

        registry
    }

    /// Returns all registered rules
    pub fn all(&self) -> &[Box<dyn Rule>] {
        &self.rules
    }

    /// Returns rules that apply to a specific language
    pub fn for_language(&self, language: &str) -> Vec<&dyn Rule> {
        let lang = language.to_lowercase();
        self.by_language
            .get(&lang)
            .map(|indices| {
                indices
                    .iter()
                    .map(|&i| &*self.rules[i] as &dyn Rule)
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Returns rules that apply to a specific language, with their global indices.
    /// This is useful for preflight filtering where we need to map back to original indices.
    fn for_language_with_indices(&self, language: &str) -> Vec<(usize, &dyn Rule)> {
        let lang = language.to_lowercase();
        self.by_language
            .get(&lang)
            .map(|indices| {
                indices
                    .iter()
                    .map(|&i| (i, &*self.rules[i] as &dyn Rule))
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Returns rules for a specific category
    pub fn for_category(&self, category: Category) -> Vec<&dyn Rule> {
        self.by_category
            .get(&category)
            .map(|indices| {
                indices
                    .iter()
                    .map(|&i| &*self.rules[i] as &dyn Rule)
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Get the count of registered rules
    pub fn count(&self) -> usize {
        self.rules.len()
    }

    /// Analyze multiple files in parallel using Rayon.
    pub fn analyze_files(&self, paths: &[PathBuf]) -> Vec<Issue> {
        paths
            .par_iter()
            .flat_map(|path| {
                self.analyze_single_file(path)
                    .unwrap_or_default()
            })
            .collect()
    }

    /// Analyze files with early termination on N critical issues
    pub fn analyze_files_with_limit(
        &self,
        paths: &[PathBuf],
        max_critical: usize,
    ) -> Vec<Issue> {
        let mut all_issues = Vec::new();
        for path in paths {
            if let Ok(issues) = self.analyze_single_file(path) {
                let critical_count = issues.iter()
                    .filter(|i| i.severity >= Severity::Critical)
                    .count();
                all_issues.extend(issues);
                if critical_count >= max_critical {
                    break; // Early termination
                }
            }
        }
        all_issues
    }

    /// Analyze a single file and return any issues found.
    pub fn analyze_single_file(&self, path: &Path) -> Result<Vec<Issue>, String> {
        let source = std::fs::read_to_string(path)
            .map_err(|e| format!("Cannot read {}: {}", path.display(), e))?;

        let ext = path.extension();
        let language = Language::from_extension(ext)
            .ok_or_else(|| format!("Unknown language for {}", path.display()))?;

        let ts_lang = language.to_ts_language();
        let mut parser = tree_sitter::Parser::new();
        parser.set_language(&ts_lang)
            .map_err(|e| format!("Failed to set parser language: {}", e))?;

        let tree = parser.parse(&source, None)
            .ok_or_else(|| format!("Failed to parse {}", path.display()))?;

        let metrics = FileMetrics::new();
        let call_graph = CallGraph::new();

        // Build per-file symbol table for semantic analysis (LCPG MVP)
        // This is built once per file and shared with rules via RuleContext
        let symbol_table = crate::rules::symbol_table::SymbolTableBuilder::new()
            .build(&tree, &source);

        let ctx = RuleContext {
            tree: &tree,
            source: &source,
            file_path: path,
            language: &language,
            graph: &call_graph,
            metrics: &metrics,
            symbol_table: Some(&symbol_table),
        };

        let lang_name = language.name();

        // Get language-specific rules with their global indices
        let lang_rules_with_indices = self.for_language_with_indices(lang_name);

        // Layer-0 preflight: determine which of the language-specific rules are eligible
        let eligible_rules: Vec<_> = if let Some(ref preflight) = self.preflight {
            let eligible_global = preflight.eligible_rule_indices(&source);
            let eligible_global_set: std::collections::HashSet<usize> =
                eligible_global.into_iter().collect();

            // Filter to only rules whose global index is in the eligible set
            lang_rules_with_indices
                .into_iter()
                .filter(|(idx, _)| eligible_global_set.contains(idx))
                .map(|(_, rule)| rule)
                .collect()
        } else {
            // Fallback: all language rules are eligible
            lang_rules_with_indices.into_iter().map(|(_, rule)| rule).collect()
        };

        let issues: Vec<Issue> = eligible_rules
            .par_iter()
            .flat_map(|rule| rule.check(&ctx))
            .collect();

        Ok(issues)
    }
}

impl Default for RuleRegistry {
    fn default() -> Self {
        Self::discover()
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// RuleContext helpers for rule implementations
// ─────────────────────────────────────────────────────────────────────────────

impl<'a> RuleContext<'a> {
    /// Count the lines occupied by a node in the source.
    pub fn line_count(&self, node: tree_sitter::Node) -> usize {
        let start = node.start_position().row;
        let end = node.end_position().row;
        end - start + 1
    }

    /// Returns the function/query node type for the current language.
    pub fn function_query(&self) -> String {
        format!("({}) @func", self.language.function_node_type())
    }

    /// Execute a tree-sitter query and return all matching nodes.
    #[allow(dead_code)]
    pub fn query_nodes(&self, query_str: &str) -> Vec<tree_sitter::Node<'a>> {
        let query = match tree_sitter::Query::new(&self.language.to_ts_language(), query_str) {
            Ok(q) => q,
            Err(_) => return Vec::new(),
        };
        let mut cursor = tree_sitter::QueryCursor::new();
        let mut results = Vec::new();
        let mut matches = cursor.matches(&query, self.tree.root_node(), self.source.as_bytes());
        while let Some(m) = matches.next() {
            for cap in m.captures {
                results.push(cap.node);
            }
        }
        results
    }

    /// Execute a tree-sitter query and return the count of matches.
    #[allow(dead_code)]
    pub fn count_matches(&self, query_str: &str) -> usize {
        let query = match tree_sitter::Query::new(&self.language.to_ts_language(), query_str) {
            Ok(q) => q,
            Err(_) => return 0,
        };
        let mut cursor = tree_sitter::QueryCursor::new();
        let mut matches = cursor.matches(&query, self.tree.root_node(), self.source.as_bytes());
        let mut count = 0;
        while let Some(_m) = matches.next() {
            count += 1;
        }
        count
    }

    /// Extract function/method name from its AST node.
    #[allow(dead_code)]
    pub fn function_name(&self, node: tree_sitter::Node) -> Option<&'a str> {
        // Try child_by_field_name("name") first (works for Rust, Python, TypeScript, Java, Go)
        if let Some(name_node) = node.child_by_field_name("name") {
            return name_node.utf8_text(self.source.as_bytes()).ok();
        }
        // Try first named child as fallback
        let mut cursor = node.walk();
        for child in node.named_children(&mut cursor) {
            if child.kind() == "identifier" || child.kind() == "name" {
                return child.utf8_text(self.source.as_bytes()).ok();
            }
        }
        None
    }

    /// Calculate nesting depth (max depth of control structures) with language-aware kinds.
    pub fn nesting_depth(&self, node: tree_sitter::Node) -> usize {
        let nesting_kinds = match *self.language {
            Language::Rust => vec!["if_expression", "while_expression", "for_expression", "loop_expression", "match_expression"],
            Language::Python => vec!["if_statement", "while_statement", "for_statement", "try_statement", "except_clause"],
            Language::JavaScript | Language::TypeScript => vec!["if_statement", "while_statement", "for_statement", "do_statement", "switch_statement"],
            Language::Go => vec!["if_statement", "for_statement", "range_statement"],
            Language::Java => vec!["if_statement", "while_statement", "for_statement", "do_statement", "switch_statement"],
        };
        self.max_nesting_impl(node, 0, &nesting_kinds)
    }

    fn max_nesting_impl(&self, node: tree_sitter::Node, current_depth: usize, nesting_kinds: &[&str]) -> usize {
        let mut max_depth = current_depth;
        for i in 0..node.child_count() {
            if let Some(child) = node.child(i) {
                let kind = child.kind();
                let new_depth = if nesting_kinds.contains(&kind) {
                    current_depth + 1
                } else {
                    current_depth
                };
                let child_max = self.max_nesting_impl(child, new_depth, nesting_kinds);
                max_depth = max_depth.max(child_max);
            }
        }
        max_depth
    }

    /// Calculate cognitive complexity of a node (SonarSource algorithm).
    pub fn cognitive_complexity(&self, node: tree_sitter::Node) -> i32 {
        calculate_cognitive_complexity(node, self.source.as_bytes())
    }

    /// Get all function nodes in the file using language-appropriate node type.
    /// Uses the StreamingIterator pattern required by tree-sitter 0.24+.
    pub fn query_functions(&self) -> Vec<tree_sitter::Node<'a>> {
        let node_type = self.language.function_node_type();
        let query_str = format!("({}) @func", node_type);
        let query = match tree_sitter::Query::new(&self.language.to_ts_language(), &query_str) {
            Ok(q) => q,
            Err(_) => return Vec::new(),
        };
        let mut cursor = tree_sitter::QueryCursor::new();
        let mut matches = cursor.matches(&query, self.tree.root_node(), self.source.as_bytes());
        let mut nodes = Vec::new();
        while let Some(m) = matches.next() {
            for capture in m.captures {
                nodes.push(capture.node);
            }
        }
        nodes
    }

    /// Find all dead code symbols (unreachable functions/types)
    pub fn find_dead_symbols(&self) -> Vec<(String, String)> {
        let dead_ids = self.graph.find_dead_code();
        dead_ids.iter()
            .filter_map(|id| {
                self.graph.get_symbol(id).map(|s| {
                    (s.name().to_string(), s.location().file().to_string())
                })
            })
            .collect()
    }

    /// Get callers of a symbol by name
    pub fn callers_of(&self, symbol_name: &str) -> Vec<String> {
        let matches = self.graph.find_by_name(symbol_name);
        let mut result = Vec::new();
        for sym in matches {
            let fqn = sym.fully_qualified_name();
            let id = SymbolId::new(fqn);
            let callers = self.graph.callers(&id);
            for caller_id in &callers {
                if let Some(caller) = self.graph.get_symbol(caller_id) {
                    result.push(caller.name().to_string());
                }
            }
        }
        result
    }

    /// Get callees of a symbol by name
    pub fn callees_of(&self, symbol_name: &str) -> Vec<String> {
        let matches = self.graph.find_by_name(symbol_name);
        let mut result = Vec::new();
        for sym in matches {
            let fqn = sym.fully_qualified_name();
            let id = SymbolId::new(fqn);
            let callees = self.graph.callees(&id);
            for (target_id, _) in &callees {
                if let Some(target) = self.graph.get_symbol(target_id) {
                    result.push(target.name().to_string());
                }
            }
        }
        result
    }

    /// Get all import/use declarations in the file
    pub fn query_imports(&self) -> Vec<(usize, String)> {
        let mut imports = Vec::new();
        // Use language-aware query based on function_node_type pattern
        let query_str = match *self.language {
            Language::Rust => "(use_declaration) @import",
            Language::Python => "(import_statement) @import\n(from_import_statement) @import",
            Language::JavaScript | Language::TypeScript => "(import_statement) @import",
            _ => "(import_declaration) @import",
        };

        let query = tree_sitter::Query::new(&self.language.to_ts_language(), query_str);
        if let Ok(query) = query {
            let mut cursor = tree_sitter::QueryCursor::new();
            let mut matches = cursor.matches(&query, self.tree.root_node(), self.source.as_bytes());
            let mut seen = std::collections::HashSet::new();
            while let Some(m) = matches.next() {
                for capture in m.captures {
                    if let Ok(text) = capture.node.utf8_text(self.source.as_bytes()) {
                        let pt = capture.node.start_position();
                        let line = pt.row + 1;
                        let key = (line, text.to_string());
                        if seen.insert(key.clone()) {
                            imports.push(key);
                        }
                    }
                }
            }
        }
        imports
    }

    /// Get all class/struct/impl declarations in the file
    pub fn query_classes(&self) -> Vec<(usize, String)> {
        let node_type = self.language.class_node_type();
        let query_str = format!("({}) @class", node_type);

        let query = tree_sitter::Query::new(&self.language.to_ts_language(), &query_str);
        let mut results = Vec::new();
        if let Ok(query) = query {
            let mut cursor = tree_sitter::QueryCursor::new();
            let mut matches = cursor.matches(&query, self.tree.root_node(), self.source.as_bytes());
            while let Some(m) = matches.next() {
                for capture in m.captures {
                    if let Some(name) = self.function_name(capture.node) {
                        let pt = capture.node.start_position();
                        results.push((pt.row + 1, name.to_string()));
                    }
                }
            }
        }
        results
    }

    /// Execute a custom tree-sitter query and return matching nodes with line numbers
    pub fn query_patterns(&self, query_str: &str) -> Vec<(usize, usize, String)> {
        let query = match tree_sitter::Query::new(&self.language.to_ts_language(), query_str) {
            Ok(q) => q,
            Err(_) => return Vec::new(),
        };
        let mut cursor = tree_sitter::QueryCursor::new();
        let mut results = Vec::new();
        let mut matches = cursor.matches(&query, self.tree.root_node(), self.source.as_bytes());
        while let Some(m) = matches.next() {
            for capture in m.captures {
                let pt = capture.node.start_position();
                if let Ok(text) = capture.node.utf8_text(self.source.as_bytes()) {
                    results.push((pt.row + 1, pt.column, text.to_string()));
                }
            }
        }
        results
    }
}

fn calculate_cognitive_complexity(node: tree_sitter::Node, source: &[u8]) -> i32 {
    let mut complexity = 0;
    compute_complexity_recursive(node, source, 0, &mut complexity, false);
    complexity
}

fn compute_complexity_recursive(
    node: tree_sitter::Node,
    source: &[u8],
    depth: usize,
    complexity: &mut i32,
    _in_loop: bool,
) {
    let kind = node.kind();
    
    // Increment for control structures
    if matches!(kind,
        "if_expression" | "match_expression" | "match_arm" |
        "for_expression" | "while_expression" | "loop_expression"
    ) {
        *complexity += 1 + depth as i32;
    }
    
    // Increment for boolean operators in binary expressions
    if kind == "binary_expression"
        && let Ok(text) = node.utf8_text(source)
            && (text.contains("&&") || text.contains("||")) {
                *complexity += 1;
            }
    
    // Recurse into children
    for i in 0..node.child_count() {
        if let Some(child) = node.child(i) {
            let is_loop = matches!(kind,
                "for_expression" | "while_expression" | "loop_expression"
            );
            compute_complexity_recursive(child, source, depth + 1, complexity, is_loop);
        }
    }
}
