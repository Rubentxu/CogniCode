//! Rename Strategy - Strategy pattern implementation for rename refactoring

use crate::domain::aggregates::refactor::{Refactor, RefactorKind, RefactorParameters, TextEdit};
use crate::domain::traits::refactor_strategy::{
    PreparedEdits, RefactorError, RefactorStrategy, RefactorValidation, ValidationError,
    ValidationErrorCode,
};
use crate::domain::value_objects::{Location, SourceRange};
use crate::infrastructure::parser::{TreeSitterParser};
use crate::infrastructure::safety::{OperationType, SafetyGate, SafetyOperation};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

/// Strategy implementation for rename refactoring operations
pub struct RenameStrategy {
    parser: Arc<TreeSitterParser>,
    safety_gate: SafetyGate,
}

impl RenameStrategy {
    /// Creates a new RenameStrategy with the given parser and safety gate
    pub fn new(parser: Arc<TreeSitterParser>, safety_gate: SafetyGate) -> Self {
        Self {
            parser,
            safety_gate,
        }
    }

    /// Finds all occurrences of a symbol in the source code
    pub fn find_all_occurrences(
        &self,
        source: &str,
        symbol_name: &str,
    ) -> Result<Vec<Occurrence>, RefactorError> {
        let tree = self
            .parser
            .parse_tree(source)
            .map_err(|e| RefactorError::PreparationFailed(format!("Parse failed: {}", e)))?;

        let lines: Vec<&str> = source.lines().collect();
        let source_bytes = source.as_bytes();
        let mut occurrences = Vec::new();
        self.find_identifier_occurrences(
            tree.root_node(),
            source_bytes,
            &lines,
            symbol_name,
            &mut occurrences,
        );
        Ok(occurrences)
    }

    /// Recursively finds all occurrences of an identifier
    fn find_identifier_occurrences(
        &self,
        node: tree_sitter::Node,
        source_bytes: &[u8],
        lines: &[&str],
        target_name: &str,
        occurrences: &mut Vec<Occurrence>,
    ) {
        // Check if this node is an identifier matching our target
        if node.kind() == "identifier" || node.kind() == "type_identifier" {
            if let Ok(text) = node.utf8_text(source_bytes) {
                if text == target_name {
                    let start = node.start_position();
                    let end = node.end_position();
                    occurrences.push(Occurrence {
                        location: Location::new("source", start.row as u32, start.column as u32),
                        range: SourceRange::new(
                            Location::new("source", start.row as u32, start.column as u32),
                            Location::new("source", end.row as u32, end.column as u32),
                        ),
                        context: self.extract_context(lines, start.row as u32),
                    });
                }
            }
        }

        // Recurse into children
        for i in 0..node.child_count() {
            if let Some(child) = node.child(i) {
                self.find_identifier_occurrences(
                    child,
                    source_bytes,
                    lines,
                    target_name,
                    occurrences,
                );
            }
        }
    }

    /// Extracts context around a location (the line of text)
    fn extract_context(&self, lines: &[&str], line: u32) -> String {
        if line as usize >= lines.len() {
            return String::new();
        }
        lines[line as usize].to_string()
    }

    /// Validates that the new name doesn't conflict with existing symbols
    pub fn check_name_conflict(&self, source: &str, new_name: &str) -> Result<bool, RefactorError> {
        let occurrences = self.find_all_occurrences(source, new_name)?;
        Ok(!occurrences.is_empty())
    }
}

/// Represents a location where a symbol occurs
#[derive(Debug, Clone)]
pub struct Occurrence {
    /// The location of the occurrence
    pub location: Location,
    /// The source range of the occurrence
    pub range: SourceRange,
    /// Context (the line of code)
    pub context: String,
}

impl RefactorStrategy for RenameStrategy {
    fn validate(&self, refactor: &Refactor) -> RefactorValidation {
        let mut errors = Vec::new();
        let _warnings: Vec<String> = Vec::new();

        // Extract parameters
        let new_name = refactor.parameters().new_name.as_ref().ok_or_else(|| {
            RefactorValidation::failure(
                vec![ValidationError::new(
                    ValidationErrorCode::InvalidParameters,
                    "New name not provided",
                )],
                refactor.clone(),
            )
        });

        let new_name = match new_name {
            Ok(n) => n,
            Err(e) => return e,
        };

        let target_symbol = refactor.target_symbol();

        // Validate new name format (basic validation)
        if new_name.is_empty() {
            errors.push(ValidationError::new(
                ValidationErrorCode::InvalidParameters,
                "New name cannot be empty",
            ));
        }

        if new_name.contains(' ') {
            errors.push(ValidationError::new(
                ValidationErrorCode::InvalidParameters,
                "New name cannot contain spaces",
            ));
        }

        // Check if new name is the same as old name
        if new_name == target_symbol.name() {
            errors.push(ValidationError::new(
                ValidationErrorCode::InvalidParameters,
                "New name is the same as current name",
            ));
        }

        // Build safety operation for validation
        let safety_op =
            SafetyOperation::new(OperationType::Rename, target_symbol.name().to_string())
                .with_location(target_symbol.location().file().to_string())
                .with_files_affected(1); // We'll update this later based on graph

        let _safety_result = self.safety_gate.validate(&safety_op);

        if !errors.is_empty() {
            return RefactorValidation::failure(errors, refactor.clone());
        }

        RefactorValidation::success(refactor.clone()).with_warning("Rename validation complete")
    }

    fn prepare_edits(&self, refactor: &Refactor) -> Result<PreparedEdits, RefactorError> {
        let new_name =
            refactor.parameters().new_name.as_ref().ok_or_else(|| {
                RefactorError::PreparationFailed("New name not provided".to_string())
            })?;

        let target_symbol = refactor.target_symbol();
        let file_path = target_symbol.location().file();

        // Read the source file
        let source = std::fs::read_to_string(file_path)
            .map_err(|e| RefactorError::IoError(format!("Failed to read file: {}", e)))?;

        // Find all occurrences
        let occurrences = self.find_all_occurrences(&source, target_symbol.name())?;

        if occurrences.is_empty() {
            return Ok(PreparedEdits::empty());
        }

        // Generate text edits for each occurrence
        let edits: Vec<RefactorParameters> = occurrences
            .iter()
            .map(|_occ| RefactorParameters::new().with_new_name(new_name.clone()))
            .collect();

        let mut files_to_modify = HashMap::new();
        files_to_modify.insert(PathBuf::from(file_path), occurrences.len());

        Ok(PreparedEdits {
            edits,
            files_to_modify: files_to_modify.into_keys().collect(),
            files_to_create: Vec::new(),
            files_to_delete: Vec::new(),
        })
    }

    fn execute(
        &self,
        refactor: &Refactor,
    ) -> Result<crate::domain::traits::refactor_strategy::RefactorResult, RefactorError> {
        // First validate
        let validation = self.validate(refactor);
        if !validation.is_valid {
            return Err(RefactorError::ValidationFailed(
                validation
                    .errors
                    .iter()
                    .map(|e| e.message.clone())
                    .collect::<Vec<_>>()
                    .join(", "),
            ));
        }

        // Prepare edits
        let prepared = self.prepare_edits(refactor)?;

        if prepared.edits.is_empty() {
            return Ok(
                crate::domain::traits::refactor_strategy::RefactorResult::success(refactor.clone()),
            );
        }

        // Execute would apply the edits - for now return success with files that would be modified
        Ok(crate::domain::traits::refactor_strategy::RefactorResult::success(refactor.clone()))
    }

    fn supported_kinds(&self) -> Vec<RefactorKind> {
        vec![RefactorKind::Rename]
    }
}

/// Helper function to create a TextEdit for renaming
pub fn create_rename_edit(
    source: &str,
    old_name: &str,
    new_name: &str,
    location: Location,
) -> Result<TextEdit, RefactorError> {
    // Find the identifier at the location
    let lines: Vec<&str> = source.lines().collect();
    let line_idx = location.line() as usize;

    if line_idx >= lines.len() {
        return Err(RefactorError::PreparationFailed(
            "Location out of bounds".to_string(),
        ));
    }

    let line = lines[line_idx];
    let col = location.column() as usize;

    // Find the word at this position
    let mut start = col;
    let mut end = col;

    while start > 0 && !line.is_char_boundary(start - 1) {
        start -= 1;
    }
    while end < line.len() && !line.is_char_boundary(end) {
        end += 1;
    }

    // Check if the word matches
    let word = &line[start..end];
    if word != old_name {
        return Err(RefactorError::PreparationFailed(format!(
            "Expected to find '{}' at {} but found '{}'",
            old_name, location, word
        )));
    }

    let range = SourceRange::new(
        Location::new(location.file(), location.line(), location.column()),
        Location::new(
            location.file(),
            location.line(),
            location.column() + (end - start) as u32,
        ),
    );

    Ok(TextEdit::new(range, new_name.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::infrastructure::parser::Language;

    #[test]
    fn test_find_occurrences_python() {
        let parser = TreeSitterParser::new(Language::Python).unwrap();
        let safety_gate = SafetyGate::new();
        let strategy = RenameStrategy::new(Arc::new(parser), safety_gate);

        let source = r#"
def foo():
    foo()
    bar()

def baz():
    foo()
"#;

        let occurrences = strategy.find_all_occurrences(source, "foo").unwrap();
        assert_eq!(occurrences.len(), 3, "Should find 3 occurrences of 'foo'");
    }

    #[test]
    fn test_find_occurrences_rust() {
        let parser = TreeSitterParser::new(Language::Rust).unwrap();
        let safety_gate = SafetyGate::new();
        let strategy = RenameStrategy::new(Arc::new(parser), safety_gate);

        let source = r#"
fn foo() {
    foo();
    bar();
}

fn baz() {
    foo();
}
"#;

        let occurrences = strategy.find_all_occurrences(source, "foo").unwrap();
        assert_eq!(occurrences.len(), 3, "Should find 3 occurrences of 'foo'");
    }

    #[test]
    fn test_check_name_conflict() {
        let parser = TreeSitterParser::new(Language::Python).unwrap();
        let safety_gate = SafetyGate::new();
        let strategy = RenameStrategy::new(Arc::new(parser), safety_gate);

        let source = r#"
def foo():
    pass

def bar():
    pass
"#;

        // 'bar' exists in source, should return conflict
        let has_conflict = strategy.check_name_conflict(source, "bar").unwrap();
        assert!(
            has_conflict,
            "Should detect conflict with existing symbol 'bar'"
        );

        // 'baz' doesn't exist
        let has_conflict = strategy.check_name_conflict(source, "baz").unwrap();
        assert!(
            !has_conflict,
            "Should not detect conflict with non-existent symbol 'baz'"
        );
    }
}
