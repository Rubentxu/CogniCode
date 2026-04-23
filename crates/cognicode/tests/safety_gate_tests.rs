//! Tests for SafetyGate refactoring with Chain of Responsibility pattern
//!
//! These tests establish the expected behavior for SafetyValidator trait and chaining.

use cognicode::infrastructure::safety::{
    ComplexitySafetyValidator, DependencySafetyValidator, OperationType, PathSafetyValidator,
    RiskLevel, RiskThreshold, SafetyGate, SafetyOperation, SafetyValidation, SafetyValidator,
    ViolationCode, WarningCode,
};

#[test]
fn test_path_safety_validator_detects_deletion() {
    let validator = PathSafetyValidator::new();

    let mut operation = SafetyOperation::new(OperationType::Delete, "func".to_string());
    operation.is_deletion = true;

    let result = validator.validate(&operation);
    assert!(!result.is_safe);
    assert!(result
        .violations
        .iter()
        .any(|v| v.code == ViolationCode::DeletionWithoutBackup));
}

#[test]
fn test_path_safety_validator_allows_safe_operations() {
    let validator = PathSafetyValidator::new();

    let operation = SafetyOperation::new(OperationType::Rename, "func".to_string());

    let result = validator.validate(&operation);
    assert!(result.is_safe);
}

#[test]
fn test_dependency_safety_validator_detects_external_deps() {
    let validator = DependencySafetyValidator::new();

    let mut operation =
        SafetyOperation::new(OperationType::ChangeSignature, "api_func".to_string());
    operation.affects_public_api = true;

    let result = validator.validate(&operation);
    assert!(result.has_warnings());
    assert!(result
        .warnings
        .iter()
        .any(|w| w.code == WarningCode::PublicApiChange));
}

#[test]
fn test_complexity_safety_validator_detects_wide_impact() {
    let validator = ComplexitySafetyValidator::new();
    let config = validator.get_config();

    let mut operation = SafetyOperation::new(OperationType::Extract, "method".to_string());
    operation.files_affected = config.max_files_without_review + 10;

    let result = validator.validate(&operation);
    assert!(result.has_warnings());
    assert!(result
        .warnings
        .iter()
        .any(|w| w.code == WarningCode::WideImpact));
}

#[test]
fn test_safety_validator_chain() {
    // Create chain: Path -> Dependency -> Complexity
    let path_validator = PathSafetyValidator::new();
    let dep_validator = DependencySafetyValidator::new();
    let complexity_validator = ComplexitySafetyValidator::new();

    // Chain them together
    let chain = path_validator
        .chain(dep_validator)
        .chain(complexity_validator);

    // Test a deletion operation that also affects public API and has wide impact
    let mut operation = SafetyOperation::new(OperationType::Delete, "func".to_string());
    operation.is_deletion = true;
    operation.affects_public_api = true;
    operation.files_affected = 50;

    let result = chain.validate(&operation);

    // Should fail due to deletion (first validator in chain)
    assert!(!result.is_safe);
    // But should also have warnings from other validators
    assert!(result.has_warnings());
}

#[test]
fn test_safety_validator_chain_order() {
    // Create chain
    let path_validator = PathSafetyValidator::new();
    let dep_validator = DependencySafetyValidator::new();

    let chain = path_validator.chain(dep_validator);

    // Deletion with public API impact
    let mut operation = SafetyOperation::new(OperationType::Delete, "func".to_string());
    operation.is_deletion = true;
    operation.affects_public_api = true;

    let result = chain.validate(&operation);

    // Should fail (deletion blocked by PathSafetyValidator)
    assert!(!result.is_safe);
    // Should also have warning about public API
    assert!(result.has_warnings());
}

#[test]
fn test_safety_gate_uses_validators() {
    let gate = SafetyGate::new();

    let operation = SafetyOperation::new(OperationType::Rename, "func".to_string());
    let result = gate.validate(&operation);

    assert!(result.is_safe);
}

#[test]
fn test_risk_threshold_is_above() {
    let low = RiskThreshold::LOW;
    let medium = RiskThreshold::MEDIUM;
    let high = RiskThreshold::HIGH;

    // Medium is above Low
    assert!(medium.is_above(&low));
    // High is above Medium
    assert!(high.is_above(&medium));
    // High is above Low
    assert!(high.is_above(&low));

    // Low is NOT above Medium
    assert!(!low.is_above(&medium));
    // Medium is NOT above High
    assert!(!medium.is_above(&high));
}

#[test]
fn test_risk_threshold_from_risk_level() {
    let level = RiskLevel::Medium;
    let threshold = RiskThreshold::from_risk_level(level);

    assert!(threshold.is_above(&RiskThreshold::LOW));
    assert!(!threshold.is_above(&RiskThreshold::HIGH));
}

#[test]
fn test_safety_validation_with_risk_threshold() {
    let mut validation = SafetyValidation::new();
    validation.risk_level = RiskLevel::Medium;

    let _threshold = RiskThreshold::MEDIUM;

    // Validation's risk level should be comparable to threshold
    assert!(RiskThreshold::from_risk_level(validation.risk_level).is_above(&RiskThreshold::LOW));
}

#[test]
fn test_chain_continues_after_validator() {
    let path_validator = PathSafetyValidator::new();
    let dep_validator = DependencySafetyValidator::new();

    // Chain
    let _next = dep_validator.clone();
    let chained = path_validator.chain(dep_validator);

    // Create an operation that passes path validation but has warnings
    let mut operation = SafetyOperation::new(OperationType::Extract, "method".to_string());
    operation.affects_public_api = true; // This should trigger warning from dep_validator

    let result = chained.validate(&operation);

    // Should pass path validation
    assert!(result.is_safe);
    // But should have warnings from dep_validator
    assert!(result.has_warnings());
}

#[test]
fn test_safe_operation_passes_all_validators() {
    let path_validator = PathSafetyValidator::new();
    let dep_validator = DependencySafetyValidator::new();
    let complexity_validator = ComplexitySafetyValidator::new();

    let chain = path_validator
        .chain(dep_validator)
        .chain(complexity_validator);

    // Safe operation
    let operation = SafetyOperation::new(OperationType::Rename, "func".to_string());

    let result = chain.validate(&operation);

    assert!(result.is_safe);
    assert!(!result.has_warnings());
}
