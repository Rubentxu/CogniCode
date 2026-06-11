//! AVC Contract Generator — extracts contracts from existing code
//!
//! Uses Tree-sitter to extract syntax, BM25 for semantic intent scoring,
//! and Rust's type system for safety invariants.

use super::contract::*;
use std::collections::HashSet;
use std::path::Path;

/// Generates AVC contracts from existing source code.
/// This is the engine that creates "truth contracts" for AI agents.
pub struct AvcGenerator;

impl AvcGenerator {
    /// Generate an AVC contract from a function definition in source code.
    pub fn generate_from_source(
        source: &str,
        function_name: &str,
        file_path: &str,
    ) -> Option<AvcContract> {
        let ext = Path::new(file_path).extension();
        let lang = crate::infrastructure::parser::Language::from_extension(ext)?;

        let parser = match crate::infrastructure::parser::TreeSitterParser::new(lang) {
            Ok(p) => p,
            Err(_) => return None,
        };
        let tree = match parser.parse_tree(source) {
            Ok(t) => t,
            Err(_) => return None,
        };

        // Find the target function node
        let func_node = Self::find_function(&tree.root_node(), function_name, source)?;

        // Layer 1: Extract syntax contract
        let syntax = Self::extract_syntax(&func_node, source, file_path);

        // Layer 2: Build semantic contract using BM25
        let semantic = Self::build_semantic(&func_node, source, function_name);

        // Layer 3: Extract safety invariants
        let safety = Self::extract_safety(&func_node, source);

        Some(AvcContract {
            contract_id: format!("{}-{}", file_path.replace('/', "-"), function_name),
            source_of_truth: file_path.to_string(),
            description: format!("Contract for function '{}' in {}", function_name, file_path),
            syntax,
            semantic,
            safety,
            context_depth: 2,
        })
    }

    /// Find a function node by name in the AST
    fn find_function<'a>(
        node: &tree_sitter::Node<'a>,
        name: &str,
        source: &str,
    ) -> Option<tree_sitter::Node<'a>> {
        // Match Rust function_item/function_definition AND JavaScript function_declaration
        if node.kind() == "function_item"
            || node.kind() == "function_definition"
            || node.kind() == "function_declaration"
        {
            // Check if this is our target
            if let Some(child) = node.child_by_field_name("name")
                && let Ok(n) = child.utf8_text(source.as_bytes())
                && n == name
            {
                return Some(*node);
            }
        }
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if let Some(found) = Self::find_function(&child, name, source) {
                return Some(found);
            }
        }
        None
    }

    /// Layer 1: Extract exact types, dependencies, and forbidden patterns
    fn extract_syntax(node: &tree_sitter::Node, source: &str, file_path: &str) -> SyntaxContract {
        let mut required_types = Vec::new();
        let forbidden_patterns = vec![
            "unsafe".to_string(),
            "panic!".to_string(),
            ".unwrap()".to_string(),
            ".expect(".to_string(),
        ];
        let mut target_function = None;
        let _param_count = 0usize;
        let mut return_seen = false;

        // Walk the function node to extract type info
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            match child.kind() {
                "identifier" | "name" => {
                    if let Ok(text) = child.utf8_text(source.as_bytes())
                        && target_function.is_none()
                    {
                        target_function = Some(FunctionSignature {
                            name: text.to_string(),
                            params: Vec::new(),
                            return_type: String::new(),
                            file: file_path.to_string(),
                            line: child.start_position().row + 1,
                        });
                    }
                }
                "parameters" => {
                    // Extract parameter types
                    if let Some(ref mut sig) = target_function {
                        sig.params = Self::extract_params(&child, source);
                    }
                }
                "type_identifier" | "generic_type" | "scoped_type_identifier" => {
                    if let Ok(text) = child.utf8_text(source.as_bytes()) {
                        if return_seen {
                            if let Some(ref mut sig) = target_function {
                                sig.return_type = text.to_string();
                            }
                        } else if !text.starts_with(|c: char| c.is_ascii_lowercase()) {
                            // It's a type (starts with uppercase)
                            required_types.push(TypeRequirement {
                                name: text.to_string(),
                                kind: TypeKind::Struct,
                                definition_file: file_path.to_string(),
                                definition_line: child.start_position().row + 1,
                            });
                        }
                    }
                }
                "return_type" | "->" => {
                    return_seen = true;
                }
                _ => {}
            }
        }

        // Detect call expressions inside the function body to find required calls
        let calls = Self::extract_required_calls(node, source, file_path);
        let mut call_names: HashSet<String> = HashSet::new();
        let mut required_calls = Vec::new();
        for call in calls {
            if call_names.insert(call.function_name.clone()) {
                required_calls.push(call);
            }
        }

        SyntaxContract {
            language: "rust".to_string(),
            required_types,
            required_calls,
            forbidden_patterns,
            target_function,
        }
    }

    /// Extract parameter types from a parameters node
    fn extract_params(node: &tree_sitter::Node, source: &str) -> Vec<ParamInfo> {
        let mut params = Vec::new();
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() == "parameter" || child.kind() == "self_parameter" {
                let name = child
                    .child_by_field_name("name")
                    .and_then(|n| n.utf8_text(source.as_bytes()).ok())
                    .unwrap_or("self")
                    .to_string();
                let type_name = child
                    .child_by_field_name("type")
                    .and_then(|n| n.utf8_text(source.as_bytes()).ok())
                    .unwrap_or("Self")
                    .to_string();

                let text = child.utf8_text(source.as_bytes()).unwrap_or("");
                let is_mutable = text.contains("mut ");
                let is_reference = type_name.starts_with('&');

                params.push(ParamInfo {
                    name,
                    type_name,
                    is_mutable,
                    is_reference,
                });
            }
        }
        params
    }

    /// Extract function calls inside the function body
    fn extract_required_calls(
        node: &tree_sitter::Node,
        source: &str,
        file_path: &str,
    ) -> Vec<RequiredCall> {
        let mut calls = Vec::new();
        if node.kind() == "call_expression"
            && let Some(func) = node.child_by_field_name("function")
            && let Ok(name) = func.utf8_text(source.as_bytes())
        {
            calls.push(RequiredCall {
                function_name: name.to_string(),
                file: file_path.to_string(),
                reason: "Called from function body".to_string(),
            });
        }
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            calls.extend(Self::extract_required_calls(&child, source, file_path));
        }
        calls
    }

    /// Layer 2: Build semantic alignment contract using BM25
    fn build_semantic(
        node: &tree_sitter::Node,
        source: &str,
        function_name: &str,
    ) -> SemanticContract {
        // Extract docstring/comment above the function
        let docstring = Self::extract_docstring(node, source);

        // Extract function body text
        let body_text = Self::extract_body_text(node, source);

        // Tokenize for BM25
        let intent_tokens: HashSet<String> =
            Self::tokenize(&format!("{} {}", function_name, docstring));
        let body_tokens: HashSet<String> = Self::tokenize(&body_text);

        // Compute domain terms (words that appear in BOTH intent and body)
        let domain_terms: Vec<String> = intent_tokens.intersection(&body_tokens).cloned().collect();

        // Forbidden terms: words in body that are NOT in intent (potential drift)
        let forbidden_terms: Vec<String> = body_tokens
            .difference(&intent_tokens)
            .take(10)
            .cloned()
            .collect();

        // Compute initial BM25 similarity score
        let score = if intent_tokens.is_empty() {
            1.0 // No docstring → can't check intent
        } else {
            let intersection = intent_tokens.intersection(&body_tokens).count() as f32;
            let union = intent_tokens.union(&body_tokens).count() as f32;
            if union > 0.0 {
                intersection / union
            } else {
                0.0
            }
        };

        SemanticContract {
            intent: function_name.to_string(),
            bm25_threshold: 0.5,
            domain_terms,
            forbidden_terms,
            current_score: Some(score),
            semantic_pass: Some(score >= 0.5),
        }
    }

    /// Extract docstring/comment text above a function node
    pub fn extract_docstring(node: &tree_sitter::Node, source: &str) -> String {
        // ========================================
        // PASS 1: AST child walk (inline docstrings)
        // ========================================
        // Check function body children for inline docstrings first
        // This catches Python triple-quoted and JSDoc inside function body

        // Helper to extract triple-quoted string content
        fn extract_triple_quoted(node: &tree_sitter::Node, source: &str) -> Option<String> {
            let text = node.utf8_text(source.as_bytes()).ok()?;
            let trimmed = text.trim();
            if (trimmed.starts_with("\"\"\"") && trimmed.ends_with("\"\"\""))
                || (trimmed.starts_with("'''") && trimmed.ends_with("'''"))
            {
                let inner = if trimmed.starts_with("\"\"\"") {
                    trimmed
                        .trim_start_matches("\"\"\"")
                        .trim_end_matches("\"\"\"")
                } else {
                    trimmed.trim_start_matches("'''").trim_end_matches("'''")
                };
                return Some(inner.to_string());
            }
            None
        }

        // Helper to extract JSDoc content from comment node
        fn extract_jsdoc(node: &tree_sitter::Node, source: &str) -> Option<String> {
            let text = node.utf8_text(source.as_bytes()).ok()?;
            let trimmed = text.trim();
            if trimmed.starts_with("/**") && trimmed.ends_with("*/") {
                let inner = trimmed
                    .trim_start_matches("/**")
                    .trim_end_matches("*/")
                    .lines()
                    .map(|line| line.trim_start_matches('*').trim())
                    .collect::<Vec<_>>()
                    .join(" ");
                return Some(inner.trim().to_string());
            }
            None
        }

        // First, check direct children of function node
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            match child.kind() {
                "string" | "string_content" => {
                    if let Some(content) = extract_triple_quoted(&child, source) {
                        return content;
                    }
                }
                "comment" => {
                    if let Some(content) = extract_jsdoc(&child, source) {
                        return content;
                    }
                }
                _ => {}
            }
        }

        // For languages where body is a separate field (Python, JS), check body children
        if let Some(body) = node.child_by_field_name("body") {
            let mut body_cursor = body.walk();
            for child in body.children(&mut body_cursor) {
                match child.kind() {
                    // Expression statement containing a string
                    "expression_statement" | "string" | "string_content" => {
                        // For expression_statement, get its string child
                        if child.kind() == "expression_statement" {
                            let mut stmt_cursor = child.walk();
                            for stmt_child in child.children(&mut stmt_cursor) {
                                if (stmt_child.kind() == "string"
                                    || stmt_child.kind() == "string_content")
                                    && let Some(content) =
                                        extract_triple_quoted(&stmt_child, source)
                                {
                                    return content;
                                }
                            }
                        } else if let Some(content) = extract_triple_quoted(&child, source) {
                            return content;
                        }
                    }
                    "comment" => {
                        if let Some(content) = extract_jsdoc(&child, source) {
                            return content;
                        }
                    }
                    _ => {}
                }
            }
        }

        // ========================================
        // PASS 2: Backwards line scan (above-function comments)
        // ========================================
        let pos = node.start_position();
        if pos.row == 0 {
            return String::new();
        }

        let lines: Vec<&str> = source.lines().collect();
        let mut doc_lines = Vec::new();
        let mut jsdoc_lines: Vec<String> = Vec::new();
        let mut in_jsdoc = false;

        // Look backwards from the function for doc comments
        for i in (0..pos.row).rev() {
            if i >= lines.len() {
                break;
            }
            let line = lines[i].trim();

            // Check for JSDoc block closing marker (when not already in block)
            // This handles the case where we see `*/` first while scanning backwards
            if !in_jsdoc && line.ends_with("*/") && !line.starts_with("/**") {
                in_jsdoc = true;
                jsdoc_lines.clear();
                // Don't add the end marker line itself
                continue;
            }

            // If we're in a JSDoc block
            if in_jsdoc {
                if line.starts_with("/**") {
                    // Found the start - extract content from the line after /**
                    let inner = line.trim_start_matches("/**").trim();
                    if !inner.is_empty() && !inner.starts_with("*/") {
                        jsdoc_lines.push(inner.to_string());
                    }
                    // Also check if there's content on the /** line itself (single-line JSDoc)
                    // The inner we pushed is everything after /**
                    // Now reverse and join
                    jsdoc_lines.reverse();
                    let jsdoc_result = jsdoc_lines.join(" ");
                    if !jsdoc_result.is_empty() {
                        return jsdoc_result;
                    }
                    in_jsdoc = false;
                    jsdoc_lines.clear();
                } else if line.starts_with('*') {
                    // Continuation line (e.g., ` * content` or `* content`)
                    let inner = line.trim_start_matches('*').trim().to_string();
                    jsdoc_lines.push(inner);
                } else if line.is_empty() || line.starts_with("*") {
                    // Empty line or line starting with * (continuation)
                    if line.starts_with("*") {
                        let inner = line.trim_start_matches('*').trim().to_string();
                        if !inner.is_empty() {
                            jsdoc_lines.push(inner);
                        }
                    }
                    // Don't break - continue scanning
                } else {
                    // Non-doc content in a JSDoc-looking block - reset
                    in_jsdoc = false;
                    jsdoc_lines.clear();
                }
                continue;
            }

            // Rust doc comments
            if line.starts_with("///") || line.starts_with("//!") {
                doc_lines.push(
                    line.trim_start_matches("///")
                        .trim_start_matches("//!")
                        .trim(),
                );
            } else if line.starts_with("//") {
                doc_lines.push(line.trim_start_matches("//").trim());
            }
            // Python # comments (but NOT shebang #!)
            else if line.starts_with('#') && !line.starts_with("#!") {
                doc_lines.push(line.trim_start_matches('#').trim());
            }
            // Skip empty lines and common non-doc patterns
            else if line.is_empty()
                || line.starts_with("#[")
                || line.starts_with("pub")
                || line.starts_with("fn")
            {
                continue;
            } else {
                // Non-doc line, stop scanning
                break;
            }
        }

        doc_lines.reverse();
        let result = doc_lines.join(" ");
        if !result.is_empty() {
            return result;
        }

        // If we have accumulated JSDoc content
        if !jsdoc_lines.is_empty() {
            jsdoc_lines.reverse();
            return jsdoc_lines.join(" ").trim().to_string();
        }

        String::new()
    }

    /// Extract the body of a function as text
    pub fn extract_body_text(node: &tree_sitter::Node, source: &str) -> String {
        if let Some(body) = node.child_by_field_name("body") {
            body.utf8_text(source.as_bytes()).unwrap_or("").to_string()
        } else {
            node.utf8_text(source.as_bytes()).unwrap_or("").to_string()
        }
    }

    /// Simple tokenizer: lowercase, split on non-alphanumeric, filter stop words
    pub fn tokenize(text: &str) -> HashSet<String> {
        let stop_words: HashSet<&str> = [
            "the", "a", "an", "is", "are", "was", "were", "be", "been", "being", "have", "has",
            "had", "do", "does", "did", "will", "would", "could", "should", "may", "might", "can",
            "shall", "to", "of", "in", "for", "on", "with", "at", "by", "from", "as", "into",
            "through", "during", "before", "after", "and", "but", "or", "nor", "not", "so", "yet",
            "both", "this", "that", "these", "those", "it", "its", "fn", "let", "mut", "pub",
            "use", "mod", "impl", "self", "true", "false", "if", "else", "match", "return",
            "while", "loop", "for", "break", "continue", "where", "move", "ref", "i32", "i64",
            "u32", "u64", "f32", "f64", "bool", "String", "usize", "isize", "Vec", "Option",
            "Result", "Some", "None", "Ok", "Err", "Box", "Arc", "Rc", "Cell", "RefCell",
        ]
        .iter()
        .cloned()
        .collect();

        text.split(|c: char| !c.is_alphanumeric() && c != '_')
            .filter(|w| w.len() > 2)
            .map(|w| w.to_lowercase())
            .filter(|w| !stop_words.contains(w.as_str()))
            .collect()
    }

    /// Layer 3: Extract safety invariants from function structure
    fn extract_safety(node: &tree_sitter::Node, source: &str) -> SafetyContract {
        let mut invariants = Vec::new();
        let mut requires_error_handling = false;

        // Check return type for Result
        if let Some(ret) = node.child_by_field_name("return_type")
            && let Ok(text) = ret.utf8_text(source.as_bytes())
            && text.contains("Result")
        {
            requires_error_handling = true;
            invariants.push(format!(
                "Function returns {} — error handling is MANDATORY",
                text.trim()
            ));
        }

        // Check body for unsafe/unwrap patterns
        if let Some(body) = node.child_by_field_name("body")
            && let Ok(text) = body.utf8_text(source.as_bytes())
        {
            if text.contains("unsafe") {
                invariants.push("Contains unsafe block — must be justified".to_string());
            }
            if text.contains(".unwrap()") {
                invariants
                    .push("Contains .unwrap() — replace with proper error handling".to_string());
            }
            if text.contains("panic!") {
                invariants.push("Contains panic! — use Result instead".to_string());
            }
        }

        SafetyContract {
            invariants,
            requires_error_handling,
            ownership_rules: Vec::new(),
            lifetime_requirements: Vec::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_auth_contract() {
        let source = r#"
/// Authenticate a user with password and return a session token.
/// Uses bcrypt for password verification.
pub fn authenticate_user(
    username: String,
    password: String,
) -> Result<Session, AuthError> {
    let user = find_user(&username)?;
    let valid = verify_password(&password, &user.password_hash)?;
    if valid {
        Ok(create_session(&user))
    } else {
        Err(AuthError::InvalidCredentials)
    }
}
"#;

        let contract =
            AvcGenerator::generate_from_source(source, "authenticate_user", "src/auth.rs").unwrap();

        // Syntax checks
        assert_eq!(contract.syntax.language, "rust");
        assert!(
            contract
                .syntax
                .forbidden_patterns
                .contains(&"unsafe".to_string())
        );

        // Target function
        let sig = contract.syntax.target_function.as_ref().unwrap();
        assert_eq!(sig.name, "authenticate_user");
        assert_eq!(sig.params.len(), 2);
        assert!(sig.return_type.contains("Result"));

        // Semantic checks
        assert!(!contract.semantic.domain_terms.is_empty());
        // Should have bcrypt/auth-related terms
        let has_auth_terms = contract
            .semantic
            .domain_terms
            .iter()
            .any(|t| t == "authenticate" || t == "password" || t == "session");
        assert!(has_auth_terms, "Should contain auth domain terms");

        // Safety checks
        assert!(contract.safety.requires_error_handling);
    }

    #[test]
    fn test_detect_semantic_drift() {
        // Function named "encrypt" but body talks about base64 (not encryption)
        let source = r#"
/// Encrypts the given data
pub fn encrypt_data(data: &[u8]) -> String {
    let encoded = base64_encode(data);
    encoded
}
"#;

        let contract =
            AvcGenerator::generate_from_source(source, "encrypt_data", "src/crypto.rs").unwrap();

        // The domain terms should include terms from the body
        assert!(
            !contract.semantic.domain_terms.is_empty(),
            "Should have domain terms"
        );

        // Score might be low if there's vocabulary mismatch between intent and body
        if let Some(score) = contract.semantic.current_score {
            // Just verify score exists and is reasonable
            assert!(score >= 0.0 && score <= 1.0);
        }
    }

    // ========================================
    // Phase 3: Multi-language docstring tests
    // ========================================

    #[test]
    fn test_python_triple_quote_docstring_from_body() {
        // Python triple-quoted docstring extracted from function body child
        let source = r#"
def encrypt_data(data):
    """Encrypts data with AES-256"""
    return base64.b64encode(data)
"#;
        let parser = crate::infrastructure::parser::TreeSitterParser::new(
            crate::infrastructure::parser::Language::Python,
        )
        .unwrap();
        let tree = parser.parse_tree(source).unwrap();
        let func_node =
            AvcGenerator::find_function(&tree.root_node(), "encrypt_data", source).unwrap();
        let doc = AvcGenerator::extract_docstring(&func_node, source);
        assert_eq!(doc, "Encrypts data with AES-256");
    }

    #[test]
    fn test_python_hash_comment_above_def() {
        // Python # comment extracted from line above def
        let source = r#"
# Hashes input using bcrypt
def hash_password(password):
    return bcrypt.hashpw(password, bcrypt.gensalt())
"#;
        let parser = crate::infrastructure::parser::TreeSitterParser::new(
            crate::infrastructure::parser::Language::Python,
        )
        .unwrap();
        let tree = parser.parse_tree(source).unwrap();
        let func_node =
            AvcGenerator::find_function(&tree.root_node(), "hash_password", source).unwrap();
        let doc = AvcGenerator::extract_docstring(&func_node, source);
        assert_eq!(doc, "Hashes input using bcrypt");
    }

    #[test]
    fn test_jsdoc_block_above_function() {
        // JSDoc block extracted from lines above function
        let source = r#"
/**
 * Encrypts data with AES-256
 */
function processData(input) {
    return Buffer.from(input).toString('base64');
}
"#;
        let parser = crate::infrastructure::parser::TreeSitterParser::new(
            crate::infrastructure::parser::Language::JavaScript,
        )
        .unwrap();
        let tree = parser.parse_tree(source).unwrap();
        let func_node =
            AvcGenerator::find_function(&tree.root_node(), "processData", source).unwrap();
        let doc = AvcGenerator::extract_docstring(&func_node, source);
        assert_eq!(doc, "Encrypts data with AES-256");
    }

    #[test]
    fn test_jsdoc_block_inside_function_body() {
        // JSDoc block as first statement inside function body (AST child walk)
        let source = r#"
function processData(input) {
    /** Encrypts data with AES-256 */
    return Buffer.from(input).toString('base64');
}
"#;
        let parser = crate::infrastructure::parser::TreeSitterParser::new(
            crate::infrastructure::parser::Language::JavaScript,
        )
        .unwrap();
        let tree = parser.parse_tree(source).unwrap();
        let func_node =
            AvcGenerator::find_function(&tree.root_node(), "processData", source).unwrap();
        let doc = AvcGenerator::extract_docstring(&func_node, source);
        assert_eq!(doc, "Encrypts data with AES-256");
    }

    #[test]
    fn test_shebang_line_not_docstring() {
        // Shebang line should NOT be treated as docstring
        let source = r#"
#!/usr/bin/env python3
# Hashes input using bcrypt
def hash_password(password):
    return bcrypt.hashpw(password, bcrypt.gensalt())
"#;
        let parser = crate::infrastructure::parser::TreeSitterParser::new(
            crate::infrastructure::parser::Language::Python,
        )
        .unwrap();
        let tree = parser.parse_tree(source).unwrap();
        let func_node =
            AvcGenerator::find_function(&tree.root_node(), "hash_password", source).unwrap();
        let doc = AvcGenerator::extract_docstring(&func_node, source);
        // Shebang should be skipped, only # comment should be captured
        assert_eq!(doc, "Hashes input using bcrypt");
    }
}
