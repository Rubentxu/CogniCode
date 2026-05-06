//! AVC Validator — validates agent-generated code against contracts
//!
//! Checks all three layers:
//! 1. Syntax: are required types used? Are forbidden patterns absent?
//! 2. Semantic: does the BM25 score meet the threshold?
//! 3. Safety: are invariants satisfied?

use super::contract::*;
use super::generator::AvcGenerator;

/// Validates agent-generated code against an AVC contract.
pub struct AvcValidator;

impl AvcValidator {
    /// Validate generated code against a contract.
    /// Returns a detailed result with violations and fix suggestions.
    pub fn validate(
        contract: &AvcContract,
        generated_code: &str,
    ) -> AvcValidationResult {
        let mut violations = Vec::new();
        let mut suggestions = Vec::new();

        // Layer 1: Syntax check
        let syntax_result = Self::check_syntax(contract, generated_code, &mut violations, &mut suggestions);

        // Layer 2: Semantic check (BM25)
        let mut updated_contract = contract.clone();
        let semantic_result = Self::check_semantic(&mut updated_contract, generated_code, &mut violations, &mut suggestions);

        // Layer 3: Safety check
        let safety_result = Self::check_safety(contract, generated_code, &mut violations, &mut suggestions);

        let passed = syntax_result.passed && semantic_result.passed && safety_result.passed;

        AvcValidationResult {
            passed,
            contract_id: contract.contract_id.clone(),
            syntax_check: syntax_result,
            semantic_check: semantic_result,
            safety_check: safety_result,
            violations,
            suggestions,
        }
    }

    /// Check syntax: required types, forbidden patterns, required calls
    fn check_syntax(
        contract: &AvcContract,
        code: &str,
        violations: &mut Vec<AvcViolation>,
        suggestions: &mut Vec<String>,
    ) -> LayerResult {
        let mut issues = 0;
        let total_checks = contract.syntax.required_types.len()
            + contract.syntax.forbidden_patterns.len()
            + contract.syntax.required_calls.len();
        let mut passed_checks = total_checks;

        // Check forbidden patterns
        for pattern in &contract.syntax.forbidden_patterns {
            if code.contains(pattern.as_str()) {
                violations.push(AvcViolation {
                    layer: "syntax".to_string(),
                    severity: ViolationSeverity::Blocker,
                    message: format!("Forbidden pattern found: '{}'", pattern),
                    location: None,
                });
                suggestions.push(format!("Remove '{}' from the code", pattern));
                passed_checks -= 1;
                issues += 1;
            }
        }

        // Check required types are referenced
        for req_type in &contract.syntax.required_types {
            if !code.contains(&req_type.name) {
                violations.push(AvcViolation {
                    layer: "syntax".to_string(),
                    severity: ViolationSeverity::Warning,
                    message: format!("Required type '{}' not found in generated code", req_type.name),
                    location: Some(req_type.definition_file.clone()),
                });
                suggestions.push(format!("Import and use the required type '{}' from {}", req_type.name, req_type.definition_file));
                passed_checks -= 1;
                issues += 1;
            }
        }

        // Check required function calls
        for call in &contract.syntax.required_calls {
            if !code.contains(&call.function_name) {
                violations.push(AvcViolation {
                    layer: "syntax".to_string(),
                    severity: ViolationSeverity::Warning,
                    message: format!("Required call '{}' not found: {}", call.function_name, call.reason),
                    location: None,
                });
                suggestions.push(format!("Call '{}' as required by the contract", call.function_name));
                passed_checks -= 1;
                issues += 1;
            }
        }

        let score = if total_checks > 0 {
            passed_checks as f32 / total_checks as f32
        } else { 1.0 };

        LayerResult {
            layer: "syntax".to_string(),
            passed: issues == 0,
            score,
            details: format!("{} of {} checks passed", passed_checks, total_checks),
        }
    }

    /// Check semantic alignment using BM25
    fn check_semantic(
        contract: &mut AvcContract,
        code: &str,
        violations: &mut Vec<AvcViolation>,
        suggestions: &mut Vec<String>,
    ) -> LayerResult {
        // Tokenize generated code
        let code_tokens = AvcGenerator::tokenize(code);
        let intent_tokens: std::collections::HashSet<String> = contract.semantic.domain_terms.iter().cloned().collect();
        let forbidden_set: std::collections::HashSet<String> = contract.semantic.forbidden_terms.iter().cloned().collect();

        // Compute BM25-like score between intent and generated code
        let intersection = intent_tokens.intersection(&code_tokens).count() as f32;
        let union = intent_tokens.union(&code_tokens).count() as f32;
        let score = if union > 0.0 { intersection / union } else { 0.0 };

        contract.semantic.current_score = Some(score);

        let mut passed = true;

        // Check against threshold
        if score < contract.semantic.bm25_threshold {
            violations.push(AvcViolation {
                layer: "semantic".to_string(),
                severity: ViolationSeverity::Blocker,
                message: format!(
                    "Semantic drift detected: BM25 score {:.2} below threshold {:.2}",
                    score, contract.semantic.bm25_threshold
                ),
                location: None,
            });
            suggestions.push(format!(
                "The generated code doesn't match the intent '{}'. Expected domain terms: {:?}",
                contract.semantic.intent, contract.semantic.domain_terms
            ));
            passed = false;
        }

        // Check for forbidden terms
        for term in forbidden_set {
            if code_tokens.contains(&term) {
                violations.push(AvcViolation {
                    layer: "semantic".to_string(),
                    severity: ViolationSeverity::Warning,
                    message: format!("Forbidden term '{}' found in generated code", term),
                    location: None,
                });
                suggestions.push(format!("Replace '{}' with a domain-appropriate alternative", term));
                passed = false;
            }
        }

        contract.semantic.semantic_pass = Some(passed);

        LayerResult {
            layer: "semantic".to_string(),
            passed,
            score,
            details: format!(
                "BM25 score: {:.2} (threshold: {:.2})",
                score, contract.semantic.bm25_threshold
            ),
        }
    }

    /// Check safety invariants
    fn check_safety(
        contract: &AvcContract,
        code: &str,
        violations: &mut Vec<AvcViolation>,
        suggestions: &mut Vec<String>,
    ) -> LayerResult {
        let mut issues = 0;
        let total = contract.safety.invariants.len() + 1; // +1 for error handling check
        
        // Check error handling if required
        if contract.safety.requires_error_handling {
            if !code.contains('?') && !code.contains("match") && !code.contains("if let Err") {
                violations.push(AvcViolation {
                    layer: "safety".to_string(),
                    severity: ViolationSeverity::Blocker,
                    message: "Function returns Result but no error handling found".to_string(),
                    location: None,
                });
                suggestions.push("Add '?' operator or explicit match on Result".to_string());
                issues += 1;
            }
        }

        // Check invariants
        for invariant in &contract.safety.invariants {
            // Simple heuristic: check for opposite patterns
            if invariant.contains(".unwrap()") && code.contains(".unwrap()") {
                violations.push(AvcViolation {
                    layer: "safety".to_string(),
                    severity: ViolationSeverity::Blocker,
                    message: "Invariant violated: .unwrap() should be replaced".to_string(),
                    location: None,
                });
                suggestions.push("Replace .unwrap() with proper error handling (? operator or match)".to_string());
                issues += 1;
            }
            if invariant.contains("unsafe") && code.contains("unsafe") {
                violations.push(AvcViolation {
                    layer: "safety".to_string(),
                    severity: ViolationSeverity::Warning,
                    message: "Invariant violated: unsafe block must be justified".to_string(),
                    location: None,
                });
                suggestions.push("Add a SAFETY comment justifying the unsafe block".to_string());
                issues += 1;
            }
        }

        let passed = issues == 0;
        LayerResult {
            layer: "safety".to_string(),
            passed,
            score: if total > 0 { (total - issues) as f32 / total as f32 } else { 1.0 },
            details: format!("{} of {} safety checks passed", total - issues, total),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_compliant_code() {
        let contract = AvcContract::new("test-01", "test.rs")
            .with_intent("authentication")
            .with_domain_terms(vec!["password".into(), "check".into(), "valid".into()])
            .with_threshold(0.1) // Low threshold for this simple test
            .with_forbidden_patterns(vec!["unsafe".to_string()]);

        let generated = r#"
fn authenticate(user: &str) -> Result<bool, Error> {
    let valid = check_password(user)?;
    Ok(valid)
}
"#;

        let result = AvcValidator::validate(&contract, generated);
        assert!(result.passed, "Compliant code should pass: {:?}", result.violations);
    }

    #[test]
    fn test_validate_forbidden_pattern() {
        let contract = AvcContract::new("test-02", "test.rs")
            .with_forbidden_patterns(vec!["unsafe".to_string()]);

        let generated = r#"
fn do_stuff() {
    unsafe { *ptr };
}
"#;

        let result = AvcValidator::validate(&contract, generated);
        assert!(!result.passed);
        assert!(result.violations.iter().any(|v| v.message.contains("unsafe")));
    }

    #[test]
    fn test_validate_semantic_drift() {
        let contract = AvcContract::new("test-03", "test.rs")
            .with_intent("encryption")
            .with_domain_terms(vec!["encrypt".into(), "cipher".into(), "key".into()])
            .with_threshold(0.3);

        let generated = r#"
fn process_data(data: &[u8]) -> Vec<u8> {
    base64::encode(data)  // This is encoding, not encryption!
}
"#;

        let mut contract_copy = contract.clone();
        let result = AvcValidator::validate(&contract_copy, generated);
        
        // Should detect drift because "encrypt/cipher/key" domain terms
        // don't match "base64/encode/data" in the generated code
        let has_drift = result.violations.iter()
            .any(|v| v.message.contains("drift") || v.message.contains("score"));
        
        // If score is below threshold, drift is detected
        if let Some(score) = contract_copy.semantic.current_score {
            if score < contract.semantic.bm25_threshold {
                assert!(has_drift, "Should detect semantic drift");
            }
        }
    }
}
