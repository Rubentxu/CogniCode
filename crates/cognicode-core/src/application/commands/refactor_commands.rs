//! Refactor Commands - Command definitions for refactoring operations

use serde::{Deserialize, Serialize};

/// Command to rename a symbol
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RenameSymbolCommand {
    pub old_name: String,
    pub new_name: String,
    pub file_path: String,
    pub location: (u32, u32),
}

impl RenameSymbolCommand {
    /// Creates a new rename command
    pub fn new(
        old_name: impl Into<String>,
        new_name: impl Into<String>,
        file_path: impl Into<String>,
    ) -> Self {
        Self {
            old_name: old_name.into(),
            new_name: new_name.into(),
            file_path: file_path.into(),
            location: (0, 0),
        }
    }

    /// Sets the location
    pub fn with_location(mut self, line: u32, column: u32) -> Self {
        self.location = (line, column);
        self
    }
}

/// Command to extract a function
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtractFunctionCommand {
    pub source_range: (u32, u32, u32, u32), // start_line, start_col, end_line, end_col
    pub new_function_name: String,
    pub file_path: String,
}

impl ExtractFunctionCommand {
    /// Creates a new extract function command
    pub fn new(
        source_range: (u32, u32, u32, u32),
        new_function_name: impl Into<String>,
        file_path: impl Into<String>,
    ) -> Self {
        Self {
            source_range,
            new_function_name: new_function_name.into(),
            file_path: file_path.into(),
        }
    }
}

/// Command to move a symbol
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MoveSymbolCommand {
    pub symbol_name: String,
    pub source_path: String,
    pub target_path: String,
}

impl MoveSymbolCommand {
    /// Creates a new move symbol command
    pub fn new(
        symbol_name: impl Into<String>,
        source_path: impl Into<String>,
        target_path: impl Into<String>,
    ) -> Self {
        Self {
            symbol_name: symbol_name.into(),
            source_path: source_path.into(),
            target_path: target_path.into(),
        }
    }
}

/// Command to change a function signature
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChangeSignatureCommand {
    pub function_name: String,
    pub new_parameters: Vec<ParameterDefinition>,
    pub file_path: String,
}

/// Definition of a parameter
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParameterDefinition {
    pub name: String,
    pub type_annotation: Option<String>,
    pub default_value: Option<String>,
}

impl ParameterDefinition {
    /// Creates a new parameter definition
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            type_annotation: None,
            default_value: None,
        }
    }

    /// Sets the type annotation
    pub fn with_type(mut self, type_annotation: impl Into<String>) -> Self {
        self.type_annotation = Some(type_annotation.into());
        self
    }

    /// Sets the default value
    pub fn with_default(mut self, default_value: impl Into<String>) -> Self {
        self.default_value = Some(default_value.into());
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rename_symbol_command_new() {
        let cmd = RenameSymbolCommand::new("old", "new", "/path/file.rs");
        assert_eq!(cmd.old_name, "old");
        assert_eq!(cmd.new_name, "new");
        assert_eq!(cmd.file_path, "/path/file.rs");
        assert_eq!(cmd.location, (0, 0));
    }

    #[test]
    fn test_rename_symbol_command_with_location() {
        let cmd = RenameSymbolCommand::new("old", "new", "/path/file.rs").with_location(10, 25);
        assert_eq!(cmd.location, (10, 25));
    }

    #[test]
    fn test_extract_function_command_new() {
        let range = (1, 0, 10, 20);
        let cmd = ExtractFunctionCommand::new(range, "new_func", "/path/file.rs");
        assert_eq!(cmd.source_range, range);
        assert_eq!(cmd.new_function_name, "new_func");
        assert_eq!(cmd.file_path, "/path/file.rs");
    }

    #[test]
    fn test_move_symbol_command_new() {
        let cmd = MoveSymbolCommand::new("MyStruct", "/src/lib.rs", "/dst/lib.rs");
        assert_eq!(cmd.symbol_name, "MyStruct");
        assert_eq!(cmd.source_path, "/src/lib.rs");
        assert_eq!(cmd.target_path, "/dst/lib.rs");
    }

    #[test]
    fn test_parameter_definition_new() {
        let param = ParameterDefinition::new("name");
        assert_eq!(param.name, "name");
        assert!(param.type_annotation.is_none());
        assert!(param.default_value.is_none());
    }

    #[test]
    fn test_parameter_definition_with_type() {
        let param = ParameterDefinition::new("name").with_type("String");
        assert_eq!(param.type_annotation, Some("String".to_string()));
    }

    #[test]
    fn test_parameter_definition_with_default() {
        let param = ParameterDefinition::new("name").with_default("\"default\"");
        assert_eq!(param.default_value, Some("\"default\"".to_string()));
    }

    #[test]
    fn test_parameter_definition_builder_chaining() {
        let param = ParameterDefinition::new("count")
            .with_type("i32")
            .with_default("0");
        assert_eq!(param.name, "count");
        assert_eq!(param.type_annotation, Some("i32".to_string()));
        assert_eq!(param.default_value, Some("0".to_string()));
    }

    #[test]
    fn test_rename_symbol_command_serialization() {
        let cmd = RenameSymbolCommand::new("foo", "bar", "/file.rs").with_location(5, 10);
        let json = serde_json::to_string(&cmd).unwrap();
        let deserialized: RenameSymbolCommand = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.old_name, cmd.old_name);
        assert_eq!(deserialized.new_name, cmd.new_name);
        assert_eq!(deserialized.file_path, cmd.file_path);
        assert_eq!(deserialized.location, cmd.location);
    }

    #[test]
    fn test_parameter_definition_serialization() {
        let param = ParameterDefinition::new("x")
            .with_type("i32")
            .with_default("42");
        let json = serde_json::to_string(&param).unwrap();
        let deserialized: ParameterDefinition = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.name, param.name);
        assert_eq!(deserialized.type_annotation, param.type_annotation);
        assert_eq!(deserialized.default_value, param.default_value);
    }
}
