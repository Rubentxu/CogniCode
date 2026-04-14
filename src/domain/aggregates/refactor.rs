//! Aggregate for representing refactoring operations
//!
//! Represents a refactoring operation that can be validated and prepared for execution.

use std::fmt;

use super::symbol::Symbol;
use crate::domain::value_objects::{Location, SourceRange};

/// Represents a refactoring operation
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Refactor {
    name: RefactorKind,
    target_symbol: Symbol,
    parameters: RefactorParameters,
    validation_result: Option<ValidationResult>,
    prepared_edits: Vec<TextEdit>,
}

impl Refactor {
    /// Creates a new Refactor for the given symbol with the given refactor kind
    pub fn new(name: RefactorKind, target_symbol: Symbol, parameters: RefactorParameters) -> Self {
        Self {
            name,
            target_symbol,
            parameters,
            validation_result: None,
            prepared_edits: Vec::new(),
        }
    }

    /// Returns the kind of refactor
    pub fn kind(&self) -> &RefactorKind {
        &self.name
    }

    /// Returns the target symbol
    pub fn target_symbol(&self) -> &Symbol {
        &self.target_symbol
    }

    /// Returns the parameters
    pub fn parameters(&self) -> &RefactorParameters {
        &self.parameters
    }

    /// Returns the validation result if validated
    pub fn validation_result(&self) -> Option<&ValidationResult> {
        self.validation_result.as_ref()
    }

    /// Returns the prepared edits if prepared
    pub fn prepared_edits(&self) -> &[TextEdit] {
        &self.prepared_edits
    }

    /// Returns true if this refactor has been validated
    pub fn is_validated(&self) -> bool {
        self.validation_result.is_some()
    }

    /// Returns true if this refactor has been prepared with edits
    pub fn is_prepared(&self) -> bool {
        !self.prepared_edits.is_empty()
    }

    /// Returns true if this refactor is safe to execute (validation passed)
    pub fn is_safe(&self) -> bool {
        self.validation_result
            .as_ref()
            .map(|r| r.is_valid)
            .unwrap_or(false)
    }

    /// Validates this refactor operation
    pub fn validate(
        mut self,
        impacted_symbols: usize,
        has_cycles: bool,
        breaking_changes: Vec<BreakingChange>,
    ) -> Self {
        let is_valid = impacted_symbols <= self.parameters.max_impact.unwrap_or(usize::MAX)
            && !has_cycles
            && breaking_changes.is_empty();

        self.validation_result = Some(ValidationResult {
            is_valid,
            impacted_symbol_count: impacted_symbols,
            has_cycles,
            breaking_changes,
            warnings: Vec::new(),
        });
        self
    }

    /// Prepares the text edits for this refactor
    pub fn prepare_edits(mut self, edits: Vec<TextEdit>) -> Self {
        self.prepared_edits = edits;
        self
    }

    /// Adds a warning to the validation result
    pub fn add_warning(mut self, warning: impl Into<String>) -> Self {
        if let Some(result) = &mut self.validation_result {
            result.warnings.push(warning.into());
        }
        self
    }
}

impl fmt::Display for Refactor {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} on {} at {}",
            self.name,
            self.target_symbol.name(),
            self.target_symbol.location()
        )
    }
}

/// Kind of refactoring operation
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RefactorKind {
    /// Rename a symbol
    Rename,
    /// Extract a function or method
    Extract,
    /// Inline a function or method
    Inline,
    /// Move a symbol to another location
    Move,
    /// Change a function signature
    ChangeSignature,
    /// Extract an interface from a class
    ExtractInterface,
    /// Push up or pull down members
    PullUp,
    PushDown,
}

impl RefactorKind {
    /// Returns a human-readable name for this refactor kind
    pub fn name(&self) -> &'static str {
        match self {
            RefactorKind::Rename => "Rename",
            RefactorKind::Extract => "Extract Function",
            RefactorKind::Inline => "Inline",
            RefactorKind::Move => "Move",
            RefactorKind::ChangeSignature => "Change Signature",
            RefactorKind::ExtractInterface => "Extract Interface",
            RefactorKind::PullUp => "Pull Up",
            RefactorKind::PushDown => "Push Down",
        }
    }
}

impl fmt::Display for RefactorKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.name())
    }
}

/// Parameters for a refactoring operation
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RefactorParameters {
    /// Optional new name (for Rename)
    pub new_name: Option<String>,
    /// Optional extraction target (for Extract)
    pub extraction_target: Option<String>,
    /// Optional inline target (for Inline)
    pub inline_target: Option<Symbol>,
    /// Optional new location (for Move)
    pub new_location: Option<Location>,
    /// Optional new signature (for ChangeSignature)
    pub new_signature: Option<super::symbol::FunctionSignature>,
    /// Optional maximum impact threshold
    pub max_impact: Option<usize>,
    /// Whether to skip validation
    pub skip_validation: bool,
}

impl RefactorParameters {
    /// Creates a new empty RefactorParameters
    pub fn new() -> Self {
        Self {
            new_name: None,
            extraction_target: None,
            inline_target: None,
            new_location: None,
            new_signature: None,
            max_impact: None,
            skip_validation: false,
        }
    }

    /// Sets the new name
    pub fn with_new_name(mut self, name: impl Into<String>) -> Self {
        self.new_name = Some(name.into());
        self
    }

    /// Sets the extraction target
    pub fn with_extraction_target(mut self, target: impl Into<String>) -> Self {
        self.extraction_target = Some(target.into());
        self
    }

    /// Sets the maximum impact threshold
    pub fn with_max_impact(mut self, max: usize) -> Self {
        self.max_impact = Some(max);
        self
    }

    /// Sets whether to skip validation
    pub fn with_skip_validation(mut self, skip: bool) -> Self {
        self.skip_validation = skip;
        self
    }

    /// Sets the new location
    pub fn with_new_location(mut self, location: Location) -> Self {
        self.new_location = Some(location);
        self
    }
}

impl Default for RefactorParameters {
    fn default() -> Self {
        Self::new()
    }
}

/// Result of validating a refactor operation
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidationResult {
    pub is_valid: bool,
    pub impacted_symbol_count: usize,
    pub has_cycles: bool,
    pub breaking_changes: Vec<BreakingChange>,
    pub warnings: Vec<String>,
}

impl ValidationResult {
    /// Returns true if there are any warnings
    pub fn has_warnings(&self) -> bool {
        !self.warnings.is_empty()
    }

    /// Returns the severity level of this validation result
    pub fn severity(&self) -> ValidationSeverity {
        if self.is_valid && !self.has_warnings() {
            ValidationSeverity::Safe
        } else if self.is_valid && self.has_warnings() {
            ValidationSeverity::Warning
        } else {
            ValidationSeverity::Error
        }
    }
}

/// Severity level for validation results
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ValidationSeverity {
    Safe,
    Warning,
    Error,
}

/// A text edit to be applied to source code
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TextEdit {
    pub range: SourceRange,
    pub new_text: String,
}

impl TextEdit {
    /// Creates a new TextEdit
    pub fn new(range: SourceRange, new_text: impl Into<String>) -> Self {
        Self {
            range,
            new_text: new_text.into(),
        }
    }

    /// Creates a TextEdit that inserts text at a location (replaces empty range)
    pub fn insert(location: Location, text: impl Into<String>) -> Self {
        Self::new(SourceRange::new(location.clone(), location), text)
    }

    /// Creates a TextEdit that deletes a range
    pub fn delete(range: SourceRange) -> Self {
        Self::new(range, String::new())
    }

    /// Returns the length of the original text being replaced
    pub fn original_length(&self) -> u32 {
        if self.range.start().line() == self.range.end().line() {
            self.range
                .end()
                .column()
                .saturating_sub(self.range.start().column())
        } else {
            // Multi-line: sum of all characters in range
            self.range.line_count() * 100 // Approximate
        }
    }
}

impl fmt::Display for TextEdit {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "TextEdit({}, {}) -> \"{}\"",
            self.range,
            self.original_length(),
            if self.new_text.len() > 20 {
                format!("{}...", &self.new_text[..20])
            } else {
                self.new_text.clone()
            }
        )
    }
}

/// A type of breaking change
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BreakingChange {
    /// Removing a public API
    RemovesPublicApi { symbol: String },
    /// Changing a function signature
    ChangesSignature { symbol: String },
    /// Changing return type
    ChangesReturnType { symbol: String },
    /// Removing a parameter
    RemovesParameter { symbol: String, parameter: String },
}

impl BreakingChange {
    /// Returns a human-readable description of this breaking change
    pub fn description(&self) -> String {
        match self {
            BreakingChange::RemovesPublicApi { symbol } => {
                format!("Removes public API: {}", symbol)
            }
            BreakingChange::ChangesSignature { symbol } => {
                format!("Changes signature of: {}", symbol)
            }
            BreakingChange::ChangesReturnType { symbol } => {
                format!("Changes return type of: {}", symbol)
            }
            BreakingChange::RemovesParameter { symbol, parameter } => {
                format!("Removes parameter {} from: {}", parameter, symbol)
            }
        }
    }
}

impl fmt::Display for BreakingChange {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.description())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_refactor_new() {
        let symbol = Symbol::new(
            "test_func",
            SymbolKind::Function,
            Location::new("test.rs", 1, 1),
        );
        let refactor = Refactor::new(
            RefactorKind::Rename,
            symbol.clone(),
            RefactorParameters::new().with_new_name("new_name"),
        );
        assert_eq!(refactor.kind(), &RefactorKind::Rename);
        assert_eq!(refactor.target_symbol().name(), "test_func");
        assert!(!refactor.is_validated());
    }

    #[test]
    fn test_refactor_validate_pass() {
        let symbol = Symbol::new(
            "test_func",
            SymbolKind::Function,
            Location::new("test.rs", 1, 1),
        );
        let refactor = Refactor::new(
            RefactorKind::Rename,
            symbol,
            RefactorParameters::new().with_max_impact(10),
        );

        let validated = refactor.validate(5, false, Vec::new());
        assert!(validated.is_validated());
        assert!(validated.is_safe());
    }

    #[test]
    fn test_refactor_validate_fail_impact() {
        let symbol = Symbol::new(
            "test_func",
            SymbolKind::Function,
            Location::new("test.rs", 1, 1),
        );
        let refactor = Refactor::new(
            RefactorKind::Rename,
            symbol,
            RefactorParameters::new().with_max_impact(3),
        );

        let validated = refactor.validate(5, false, Vec::new());
        assert!(!validated.is_safe());
    }

    #[test]
    fn test_refactor_validate_fail_cycles() {
        let symbol = Symbol::new(
            "test_func",
            SymbolKind::Function,
            Location::new("test.rs", 1, 1),
        );
        let refactor = Refactor::new(RefactorKind::Rename, symbol, RefactorParameters::new());

        let validated = refactor.validate(1, true, Vec::new());
        assert!(!validated.is_safe());
    }

    #[test]
    fn test_refactor_validate_fail_breaking() {
        let symbol = Symbol::new(
            "test_func",
            SymbolKind::Function,
            Location::new("test.rs", 1, 1),
        );
        let refactor = Refactor::new(
            RefactorKind::ChangeSignature,
            symbol,
            RefactorParameters::new(),
        );

        let breaking = vec![BreakingChange::ChangesSignature {
            symbol: "test_func".to_string(),
        }];
        let validated = refactor.validate(1, false, breaking);
        assert!(!validated.is_safe());
    }

    #[test]
    fn test_refactor_prepare_edits() {
        let symbol = Symbol::new(
            "test_func",
            SymbolKind::Function,
            Location::new("test.rs", 1, 1),
        );
        let refactor = Refactor::new(RefactorKind::Rename, symbol, RefactorParameters::new());

        let edits = vec![TextEdit::insert(Location::new("test.rs", 1, 1), "new_name")];
        let prepared = refactor.prepare_edits(edits.clone());

        assert!(prepared.is_prepared());
        assert_eq!(prepared.prepared_edits().len(), 1);
    }

    #[test]
    fn test_text_edit() {
        let range = SourceRange::new(
            Location::new("test.rs", 1, 5),
            Location::new("test.rs", 1, 10),
        );
        let edit = TextEdit::new(range.clone(), "replacement");
        assert_eq!(edit.new_text, "replacement");
    }

    #[test]
    fn test_text_edit_insert() {
        let edit = TextEdit::insert(Location::new("test.rs", 1, 5), "hello");
        assert!(edit.new_text == "hello");
    }

    #[test]
    fn test_validation_result_severity() {
        let valid_result = ValidationResult {
            is_valid: true,
            impacted_symbol_count: 5,
            has_cycles: false,
            breaking_changes: Vec::new(),
            warnings: Vec::new(),
        };
        assert_eq!(valid_result.severity(), ValidationSeverity::Safe);

        let warning_result = ValidationResult {
            is_valid: true,
            impacted_symbol_count: 5,
            has_cycles: false,
            breaking_changes: Vec::new(),
            warnings: vec!["Some warning".to_string()],
        };
        assert_eq!(warning_result.severity(), ValidationSeverity::Warning);

        let error_result = ValidationResult {
            is_valid: false,
            impacted_symbol_count: 5,
            has_cycles: false,
            breaking_changes: vec![BreakingChange::RemovesPublicApi {
                symbol: "test".to_string(),
            }],
            warnings: Vec::new(),
        };
        assert_eq!(error_result.severity(), ValidationSeverity::Error);
    }

    use crate::domain::value_objects::{Location, SymbolKind};
}
