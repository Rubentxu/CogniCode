//! Safety Gate Infrastructure Component
//!
//! Provides safety checks and validations for refactoring operations.
//! Uses Chain of Responsibility pattern for extensible validation.

use std::sync::Arc;

// =============================================================================
// RiskThreshold - Value Object for Risk Level Comparison
// =============================================================================

/// Risk threshold value object that encapsulates risk level comparison logic.
///
/// This removes ordinal assumptions from comparison code by providing
/// explicit comparison methods.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RiskThreshold {
    /// The numeric value of the threshold
    value: u8,
    /// Human-readable name
    name: &'static str,
}

impl RiskThreshold {
    /// No risk threshold
    pub const NONE: RiskThreshold = RiskThreshold {
        value: 0,
        name: "None",
    };
    /// Low risk threshold
    pub const LOW: RiskThreshold = RiskThreshold {
        value: 1,
        name: "Low",
    };
    /// Medium risk threshold
    pub const MEDIUM: RiskThreshold = RiskThreshold {
        value: 2,
        name: "Medium",
    };
    /// High risk threshold
    pub const HIGH: RiskThreshold = RiskThreshold {
        value: 3,
        name: "High",
    };
    /// Critical risk threshold
    pub const CRITICAL: RiskThreshold = RiskThreshold {
        value: 4,
        name: "Critical",
    };

    /// Creates a new risk threshold with the given value
    pub fn new(value: u8) -> Self {
        match value {
            0 => Self::NONE,
            1 => Self::LOW,
            2 => Self::MEDIUM,
            3 => Self::HIGH,
            _ => Self::CRITICAL,
        }
    }

    /// Returns true if this threshold is above the other threshold.
    ///
    /// This method removes ordinal assumptions by explicitly comparing
    /// threshold values rather than using derived Ord trait.
    pub fn is_above(&self, other: &RiskThreshold) -> bool {
        self.value > other.value
    }

    /// Returns true if this threshold is at or above the other threshold.
    pub fn is_at_or_above(&self, other: &RiskThreshold) -> bool {
        self.value >= other.value
    }

    /// Returns the numeric value of this threshold
    pub fn value(&self) -> u8 {
        self.value
    }

    /// Returns the name of this threshold
    pub fn name(&self) -> &'static str {
        self.name
    }

    /// Creates a RiskThreshold from a RiskLevel
    pub fn from_risk_level(level: RiskLevel) -> Self {
        match level {
            RiskLevel::None => Self::NONE,
            RiskLevel::Low => Self::LOW,
            RiskLevel::Medium => Self::MEDIUM,
            RiskLevel::High => Self::HIGH,
            RiskLevel::Critical => Self::CRITICAL,
        }
    }
}

impl Default for RiskThreshold {
    fn default() -> Self {
        Self::NONE
    }
}

// =============================================================================
// Safety Validation Result
// =============================================================================

/// Safety validation result
#[derive(Debug, Clone)]
pub struct SafetyValidation {
    /// Whether the operation passed safety checks
    pub is_safe: bool,
    /// List of safety warnings
    pub warnings: Vec<SafetyWarning>,
    /// List of safety violations (errors)
    pub violations: Vec<SafetyViolation>,
    /// Overall risk level
    pub risk_level: RiskLevel,
}

impl SafetyValidation {
    /// Creates a new empty safety validation (passed)
    pub fn new() -> Self {
        Self {
            is_safe: true,
            warnings: Vec::new(),
            violations: Vec::new(),
            risk_level: RiskLevel::None,
        }
    }

    /// Creates a failed safety validation
    pub fn failed(violations: Vec<SafetyViolation>) -> Self {
        Self {
            is_safe: false,
            warnings: Vec::new(),
            violations,
            risk_level: RiskLevel::Critical,
        }
    }

    /// Creates an unsafe validation with given risk level
    pub fn unsafe_with_level(level: RiskLevel, violations: Vec<SafetyViolation>) -> Self {
        Self {
            is_safe: false,
            warnings: Vec::new(),
            violations,
            risk_level: level,
        }
    }

    /// Adds a warning to the validation
    pub fn with_warning(mut self, warning: SafetyWarning) -> Self {
        self.warnings.push(warning);
        self
    }

    /// Adds a violation to the validation
    pub fn with_violation(mut self, violation: SafetyViolation) -> Self {
        self.is_safe = false;
        self.violations.push(violation);
        self
    }

    /// Merges another validation into this one
    pub fn merge(&mut self, other: SafetyValidation) {
        if !other.is_safe {
            self.is_safe = false;
        }
        self.warnings.extend(other.warnings);
        self.violations.extend(other.violations);
        // Keep the higher risk level
        let self_threshold = RiskThreshold::from_risk_level(self.risk_level);
        let other_threshold = RiskThreshold::from_risk_level(other.risk_level);
        if other_threshold.is_above(&self_threshold) {
            self.risk_level = other.risk_level;
        }
    }

    /// Returns true if there are any warnings
    pub fn has_warnings(&self) -> bool {
        !self.warnings.is_empty()
    }
}

impl Default for SafetyValidation {
    fn default() -> Self {
        Self::new()
    }
}

// =============================================================================
// Safety Warning and Violation Types
// =============================================================================

/// A safety warning (non-blocking issue)
#[derive(Debug, Clone)]
pub struct SafetyWarning {
    /// Warning message
    pub message: String,
    /// Location related to the warning (if applicable)
    pub location: Option<String>,
    /// Warning code for categorization
    pub code: WarningCode,
}

/// Warning codes for categorization
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WarningCode {
    /// Operation may affect external users
    PublicApiChange,
    /// Operation may break existing functionality
    PotentialBreakage,
    /// Operation affects many files
    WideImpact,
    /// Operation is not easily reversible
    HardToReverse,
}

/// A safety violation (blocking issue)
#[derive(Debug, Clone)]
pub struct SafetyViolation {
    /// Violation message
    pub message: String,
    /// Location related to the violation (if applicable)
    pub location: Option<String>,
    /// Violation code for categorization
    pub code: ViolationCode,
}

/// Violation codes for categorization
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ViolationCode {
    /// Operation would delete code without backup
    DeletionWithoutBackup,
    /// Operation affects generated code
    AffectsGeneratedCode,
    /// Operation affects test code
    AffectsTestCode,
    /// Operation affects external dependencies
    AffectsExternalDeps,
    /// Operation would cause data loss
    DataLoss,
    /// Operation violates safety policy
    PolicyViolation,
}

// =============================================================================
// Risk Level (refactored to use RiskThreshold)
// =============================================================================

/// Risk level assessment
///
/// Note: This enum uses explicit ordinal values but comparisons should
/// use RiskThreshold::is_above() to avoid ordinal assumptions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default)]
pub enum RiskLevel {
    #[default]
    None = 0,
    /// Low risk operation
    Low = 1,
    /// Medium risk operation
    Medium = 2,
    /// High risk operation
    High = 3,
    /// Critical risk operation
    Critical = 4,
}

// =============================================================================
// Safety Validator Trait - Chain of Responsibility Pattern
// =============================================================================

/// Trait for safety validators using Chain of Responsibility pattern.
///
/// Each validator handles specific safety concerns and can chain to the next validator.
pub trait SafetyValidator: Send + Sync {
    /// Returns the name of this validator
    fn name(&self) -> &str;

    /// Validates the operation and returns a SafetyValidation result.
    fn validate(&self, operation: &SafetyOperation) -> SafetyValidation;

    /// Returns the next validator in the chain, if any
    fn get_next(&self) -> Option<Arc<dyn SafetyValidator>>;

    /// Sets the next validator in the chain.
    /// Returns a new chained validator.
    fn chain(self, next: impl SafetyValidator + 'static) -> ChainedSafetyValidator
    where
        Self: Sized + 'static,
    {
        ChainedSafetyValidator {
            current: Arc::new(self),
            next: Some(Arc::new(next)),
        }
    }
}

/// A chained safety validator that delegates to the next validator
#[derive(Clone)]
pub struct ChainedSafetyValidator {
    current: Arc<dyn SafetyValidator>,
    next: Option<Arc<dyn SafetyValidator>>,
}

impl SafetyValidator for ChainedSafetyValidator {
    fn name(&self) -> &str {
        "ChainedSafetyValidator"
    }

    fn validate(&self, operation: &SafetyOperation) -> SafetyValidation {
        // Validate with current validator
        let mut result = self.current.validate(operation);

        // If there's a next validator, continue the chain
        if let Some(ref next) = self.next {
            let next_result = next.validate(operation);
            result.merge(next_result);
        }

        result
    }

    fn get_next(&self) -> Option<Arc<dyn SafetyValidator>> {
        self.next.clone()
    }
}

// =============================================================================
// Individual Safety Validators
// =============================================================================

/// Validator for path-related safety concerns (deletions, etc.)
#[derive(Debug, Clone)]
pub struct PathSafetyValidator {
    config: SafetyConfig,
}

impl PathSafetyValidator {
    pub fn new() -> Self {
        Self {
            config: SafetyConfig::default(),
        }
    }

    pub fn with_config(config: SafetyConfig) -> Self {
        Self { config }
    }

    pub fn get_config(&self) -> &SafetyConfig {
        &self.config
    }
}

impl Default for PathSafetyValidator {
    fn default() -> Self {
        Self::new()
    }
}

impl SafetyValidator for PathSafetyValidator {
    fn name(&self) -> &str {
        "PathSafetyValidator"
    }

    fn validate(&self, operation: &SafetyOperation) -> SafetyValidation {
        let mut validation = SafetyValidation::new();

        // Check for deletions
        if operation.is_deletion && !self.config.allow_destructive {
            validation = validation.with_violation(SafetyViolation {
                message: "Deletion operations require explicit confirmation".to_string(),
                location: operation.target_location.clone(),
                code: ViolationCode::DeletionWithoutBackup,
            });
        }

        // Check for generated code modifications
        if operation.affects_generated_code && !self.config.allow_generated_modifications {
            validation = validation.with_violation(SafetyViolation {
                message: "Modification of generated code is not allowed".to_string(),
                location: operation.target_location.clone(),
                code: ViolationCode::AffectsGeneratedCode,
            });
        }

        // Check for test code modifications
        if operation.affects_test_code && !self.config.allow_test_modifications {
            validation = validation.with_violation(SafetyViolation {
                message: "Modification of test code is not allowed".to_string(),
                location: operation.target_location.clone(),
                code: ViolationCode::AffectsTestCode,
            });
        }

        // Set risk level based on findings
        if !validation.is_safe {
            validation.risk_level = RiskLevel::Critical;
        } else if validation.has_warnings() {
            validation.risk_level = RiskLevel::Low;
        }

        validation
    }

    fn get_next(&self) -> Option<Arc<dyn SafetyValidator>> {
        None
    }
}

/// Validator for dependency-related safety concerns (public API, etc.)
#[derive(Debug, Clone)]
pub struct DependencySafetyValidator {
    config: SafetyConfig,
}

impl DependencySafetyValidator {
    pub fn new() -> Self {
        Self {
            config: SafetyConfig::default(),
        }
    }

    pub fn with_config(config: SafetyConfig) -> Self {
        Self { config }
    }

    pub fn get_config(&self) -> &SafetyConfig {
        &self.config
    }
}

impl Default for DependencySafetyValidator {
    fn default() -> Self {
        Self::new()
    }
}

impl SafetyValidator for DependencySafetyValidator {
    fn name(&self) -> &str {
        "DependencySafetyValidator"
    }

    fn validate(&self, operation: &SafetyOperation) -> SafetyValidation {
        let mut validation = SafetyValidation::new();

        // Check for public API changes
        if operation.affects_public_api {
            validation = validation.with_warning(SafetyWarning {
                message: "This operation affects public API".to_string(),
                location: operation.target_location.clone(),
                code: WarningCode::PublicApiChange,
            });
        }

        // Set risk level
        if validation.has_warnings() {
            validation.risk_level = RiskLevel::Low;
        }

        validation
    }

    fn get_next(&self) -> Option<Arc<dyn SafetyValidator>> {
        None
    }
}

/// Validator for complexity-related safety concerns (wide impact, etc.)
#[derive(Debug, Clone)]
pub struct ComplexitySafetyValidator {
    config: SafetyConfig,
}

impl ComplexitySafetyValidator {
    pub fn new() -> Self {
        Self {
            config: SafetyConfig::default(),
        }
    }

    pub fn with_config(config: SafetyConfig) -> Self {
        Self { config }
    }

    pub fn get_config(&self) -> &SafetyConfig {
        &self.config
    }
}

impl Default for ComplexitySafetyValidator {
    fn default() -> Self {
        Self::new()
    }
}

impl SafetyValidator for ComplexitySafetyValidator {
    fn name(&self) -> &str {
        "ComplexitySafetyValidator"
    }

    fn validate(&self, operation: &SafetyOperation) -> SafetyValidation {
        let mut validation = SafetyValidation::new();

        // Check for wide impact
        if operation.files_affected > self.config.max_files_without_review {
            validation = validation.with_warning(SafetyWarning {
                message: format!(
                    "This operation affects {} files (threshold: {})",
                    operation.files_affected, self.config.max_files_without_review
                ),
                location: None,
                code: WarningCode::WideImpact,
            });

            // Elevate risk level based on impact
            if operation.files_affected > self.config.max_files_soft_limit {
                validation.risk_level = RiskLevel::High;
            } else {
                validation.risk_level = RiskLevel::Medium;
            }
        }

        validation
    }

    fn get_next(&self) -> Option<Arc<dyn SafetyValidator>> {
        None
    }
}

// =============================================================================
// Safety Gate
// =============================================================================

/// Safety gate that performs pre-refactoring safety checks.
/// Uses a chain of validators for extensible safety validation.
#[derive(Clone)]
pub struct SafetyGate {
    validators: Vec<Arc<dyn SafetyValidator>>,
}

impl SafetyGate {
    /// Creates a new SafetyGate with default configuration
    pub fn new() -> Self {
        Self::default()
    }

    /// Creates a new SafetyGate with a custom chain of validators
    pub fn with_validators(validators: Vec<Arc<dyn SafetyValidator>>) -> Self {
        Self { validators }
    }

    /// Creates a SafetyGate with the standard validator chain:
    /// PathSafetyValidator -> DependencySafetyValidator -> ComplexitySafetyValidator
    pub fn with_standard_chain(config: SafetyConfig) -> Self {
        let path = Arc::new(PathSafetyValidator::with_config(config.clone()));
        let dep = Arc::new(DependencySafetyValidator::with_config(config.clone()));
        let complexity = Arc::new(ComplexitySafetyValidator::with_config(config));

        Self {
            validators: vec![path, dep, complexity],
        }
    }

    /// Validates a refactoring operation before execution
    pub fn validate(&self, operation: &SafetyOperation) -> SafetyValidation {
        let mut result = SafetyValidation::new();

        for validator in &self.validators {
            let validation = validator.validate(operation);
            result.merge(validation);
        }

        // Set final risk level
        if result.is_safe && !result.has_warnings() {
            result.risk_level = RiskLevel::Low;
        }

        result
    }
}

impl Default for SafetyGate {
    fn default() -> Self {
        Self::with_standard_chain(SafetyConfig::default())
    }
}

// =============================================================================
// Safety Configuration
// =============================================================================

/// Configuration for safety checks
#[derive(Debug, Clone)]
pub struct SafetyConfig {
    /// Maximum files that can be affected without explicit review
    pub max_files_without_review: usize,
    /// Soft limit for files affected (warns above this)
    pub max_files_soft_limit: usize,
    /// Whether to allow modifications to test code
    pub allow_test_modifications: bool,
    /// Whether to allow modifications to generated code
    pub allow_generated_modifications: bool,
    /// Whether to allow destructive operations
    pub allow_destructive: bool,
}

impl Default for SafetyConfig {
    fn default() -> Self {
        Self {
            max_files_without_review: 5,
            max_files_soft_limit: 10,
            allow_test_modifications: false,
            allow_generated_modifications: false,
            allow_destructive: false,
        }
    }
}

// =============================================================================
// Safety Operation
// =============================================================================

/// Represents an operation to validate
#[derive(Debug, Clone)]
pub struct SafetyOperation {
    /// Type of operation
    pub operation_type: OperationType,
    /// Target of the operation
    pub target: String,
    /// Location of the target
    pub target_location: Option<String>,
    /// Number of files affected
    pub files_affected: usize,
    /// Whether this is a deletion operation
    pub is_deletion: bool,
    /// Whether this affects public API
    pub affects_public_api: bool,
    /// Whether this affects test code
    pub affects_test_code: bool,
    /// Whether this affects generated code
    pub affects_generated_code: bool,
}

impl SafetyOperation {
    /// Creates a new safety operation
    pub fn new(operation_type: OperationType, target: String) -> Self {
        Self {
            operation_type,
            target,
            target_location: None,
            files_affected: 1,
            is_deletion: false,
            affects_public_api: false,
            affects_test_code: false,
            affects_generated_code: false,
        }
    }

    /// Sets the target location
    pub fn with_location(mut self, location: String) -> Self {
        self.target_location = Some(location);
        self
    }

    /// Sets the files affected count
    pub fn with_files_affected(mut self, count: usize) -> Self {
        self.files_affected = count;
        self
    }
}

/// Type of operation being validated
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OperationType {
    Rename,
    Extract,
    Inline,
    Move,
    Delete,
    ChangeSignature,
    AddParameter,
    RemoveParameter,
    Other,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_safety_validation_pass() {
        let validation = SafetyValidation::new();
        assert!(validation.is_safe);
        assert!(!validation.has_warnings());
    }

    #[test]
    fn test_safety_validation_with_warning() {
        let validation = SafetyValidation::new().with_warning(SafetyWarning {
            message: "Test warning".to_string(),
            location: None,
            code: WarningCode::PublicApiChange,
        });
        assert!(validation.is_safe);
        assert!(validation.has_warnings());
    }

    #[test]
    fn test_safety_gate_pass() {
        let gate = SafetyGate::new();
        let operation = SafetyOperation::new(OperationType::Rename, "func".to_string());
        let result = gate.validate(&operation);
        assert!(result.is_safe);
    }

    #[test]
    fn test_safety_gate_deletion_blocked() {
        let gate = SafetyGate::new();
        let mut operation = SafetyOperation::new(OperationType::Delete, "func".to_string());
        operation.is_deletion = true;
        let result = gate.validate(&operation);
        assert!(!result.is_safe);
        assert!(!result.violations.is_empty());
    }

    #[test]
    fn test_path_safety_validator_deletion() {
        let validator = PathSafetyValidator::new();
        let mut operation = SafetyOperation::new(OperationType::Delete, "func".to_string());
        operation.is_deletion = true;

        let result = validator.validate(&operation);
        assert!(!result.is_safe);
        assert!(!result.violations.is_empty());
    }

    #[test]
    fn test_dependency_safety_validator_public_api() {
        let validator = DependencySafetyValidator::new();
        let mut operation =
            SafetyOperation::new(OperationType::ChangeSignature, "api_func".to_string());
        operation.affects_public_api = true;

        let result = validator.validate(&operation);
        assert!(result.has_warnings());
    }

    #[test]
    fn test_complexity_safety_validator_wide_impact() {
        let validator = ComplexitySafetyValidator::new();
        let mut operation = SafetyOperation::new(OperationType::Extract, "method".to_string());
        operation.files_affected = 50;

        let result = validator.validate(&operation);
        assert!(result.has_warnings());
    }

    #[test]
    fn test_risk_threshold_is_above() {
        let low = RiskThreshold::LOW;
        let medium = RiskThreshold::MEDIUM;
        let high = RiskThreshold::HIGH;

        assert!(medium.is_above(&low));
        assert!(high.is_above(&medium));
        assert!(high.is_above(&low));
        assert!(!low.is_above(&medium));
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
    fn test_chained_validators() {
        let path = PathSafetyValidator::new();
        let dep = DependencySafetyValidator::new();

        let chain = path.chain(dep);

        let mut operation = SafetyOperation::new(OperationType::Delete, "func".to_string());
        operation.is_deletion = true;
        operation.affects_public_api = true;

        let result = chain.validate(&operation);
        assert!(!result.is_safe); // Deletion blocked
        assert!(result.has_warnings()); // Public API warning
    }

    #[test]
    fn test_validation_merge() {
        let mut validation = SafetyValidation::new();
        validation.risk_level = RiskLevel::Low;

        let other = SafetyValidation::unsafe_with_level(
            RiskLevel::High,
            vec![SafetyViolation {
                message: "Test".to_string(),
                location: None,
                code: ViolationCode::DeletionWithoutBackup,
            }],
        );

        validation.merge(other);

        assert!(!validation.is_safe);
        assert_eq!(validation.risk_level, RiskLevel::High);
    }
}
