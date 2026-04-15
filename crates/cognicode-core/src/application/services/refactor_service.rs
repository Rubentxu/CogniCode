//! Refactor Service - Handles refactoring operations

use crate::application::commands::RenameSymbolCommand;
use crate::application::dto::RefactorPreviewDto;
use crate::application::error::{AppError, AppResult};
use crate::domain::aggregates::call_graph::SymbolId;
use crate::domain::aggregates::refactor::{Refactor, RefactorKind, RefactorParameters, TextEdit};
use crate::domain::aggregates::Symbol;
use crate::domain::traits::{DependencyRepository, TextEdit as VfsTextEdit};
use crate::domain::value_objects::{Location, SourceRange};
use crate::infrastructure::graph::PetGraphStore;
use crate::infrastructure::parser::{IdentifierOccurrence, Language, TreeSitterParser};
use crate::infrastructure::safety::{OperationType, SafetyGate, SafetyOperation};
use crate::infrastructure::vfs::VirtualFileSystem;
use lsp_types::Url;
use std::path::Path;

/// Service for refactoring operations
pub struct RefactorService {
    safety_gate: SafetyGate,
}

impl RefactorService {
    /// Creates a new RefactorService with default safety gate
    pub fn new() -> Self {
        Self {
            safety_gate: SafetyGate::new(),
        }
    }

    /// Creates a new RefactorService with a custom safety gate
    pub fn with_safety_gate(safety_gate: SafetyGate) -> Self {
        Self { safety_gate }
    }

    /// Renames a symbol across the codebase
    ///
    /// This method:
    /// 1. Validates the rename operation with SafetyGate
    /// 2. Finds all occurrences of the symbol
    /// 3. Calculates the impact on the codebase
    /// 4. Returns a preview of the changes
    pub fn rename_symbol(&self, command: RenameSymbolCommand) -> AppResult<RefactorPreviewDto> {
        // First, find the symbol in the project graph
        let symbol = self.find_symbol(&command.old_name, &command.file_path)?;

        // Get the project directory for building the graph
        let project_dir = Path::new(&command.file_path)
            .parent()
            .ok_or_else(|| AppError::InvalidParameter("Invalid file path".to_string()))?;

        // Build a graph of the project
        let mut store = PetGraphStore::new();
        self.build_minimal_graph(&mut store, project_dir, &command.old_name)?;

        let call_graph = store.to_call_graph();

        // Calculate impact
        let symbol_id = SymbolId::new(symbol.fully_qualified_name());
        let impacted_symbols = call_graph.find_all_dependents(&symbol_id);
        let impacted_count = impacted_symbols.len();

        // Create the refactor
        let _refactor = Refactor::new(
            RefactorKind::Rename,
            symbol,
            RefactorParameters::new()
                .with_new_name(&command.new_name)
                .with_max_impact(10),
        );

        // Validate with SafetyGate
        let safety_op = SafetyOperation::new(OperationType::Rename, command.old_name.clone())
            .with_location(command.file_path.clone())
            .with_files_affected(1 + impacted_count / 5); // Estimate based on impact

        let safety_result = self.safety_gate.validate(&safety_op);

        // Check if validation passed
        if !safety_result.is_safe {
            return Err(AppError::SafetyCheckFailed(
                safety_result
                    .violations
                    .iter()
                    .map(|v| v.message.clone())
                    .collect::<Vec<_>>()
                    .join(", "),
            ));
        }

        // Find all occurrences and generate edits
        let occurrences = self.find_all_occurrences(&command.file_path, &command.old_name)?;

        // Build the preview
        let mut preview = RefactorPreviewDto::new(format!(
            "Rename '{}' to '{}' ({} occurrences found)",
            command.old_name,
            command.new_name,
            occurrences.len()
        ))
        .with_files(vec![command.file_path.clone()])
        .with_symbols(
            impacted_symbols
                .iter()
                .map(|id| id.as_str().to_string())
                .collect(),
        )
        .with_risk(format!(
            "{:?} risk ({} symbols impacted)",
            safety_result.risk_level, impacted_count
        ));

        preview.change_count = occurrences.len();
        preview.symbols_affected = vec![command.old_name.clone()];

        Ok(preview)
    }

    /// Changes a function's signature across the codebase
    ///
    /// This method:
    /// 1. Validates the signature change operation with SafetyGate
    /// 2. Finds all call sites of the function
    /// 3. Calculates the impact on the codebase
    /// 4. Returns a preview of the changes
    pub fn change_signature(
        &self,
        command: crate::application::commands::ChangeSignatureCommand,
    ) -> AppResult<RefactorPreviewDto> {
        use crate::domain::aggregates::symbol::{FunctionSignature, Parameter};

        // First, find the symbol in the project graph
        let symbol = self.find_symbol(&command.function_name, &command.file_path)?;

        // Get the project directory for building the graph
        let project_dir = Path::new(&command.file_path)
            .parent()
            .ok_or_else(|| AppError::InvalidParameter("Invalid file path".to_string()))?;

        // Build a graph of the project
        let mut store = PetGraphStore::new();
        self.build_minimal_graph(&mut store, project_dir, &command.function_name)?;

        let call_graph = store.to_call_graph();

        // Calculate impact
        let symbol_id = SymbolId::new(symbol.fully_qualified_name());
        let impacted_symbols = call_graph.find_all_dependents(&symbol_id);
        let impacted_count = impacted_symbols.len();

        // Build the new signature from the command
        let new_parameters: Vec<Parameter> = command
            .new_parameters
            .iter()
            .map(|p| Parameter::new(p.name.clone(), p.type_annotation.clone()))
            .collect();

        let _new_signature = FunctionSignature::new(new_parameters, None, false);

        // Create the refactor
        let _refactor = Refactor::new(
            RefactorKind::ChangeSignature,
            symbol.clone(),
            RefactorParameters::new()
                .with_skip_validation(false)
                .with_max_impact(10),
        );

        // Validate with SafetyGate
        let safety_op = SafetyOperation::new(
            OperationType::ChangeSignature,
            command.function_name.clone(),
        )
        .with_location(command.file_path.clone())
        .with_files_affected(1 + impacted_count / 5); // Estimate based on impact

        let safety_result = self.safety_gate.validate(&safety_op);

        // Check if validation passed
        if !safety_result.is_safe {
            return Err(AppError::SafetyCheckFailed(
                safety_result
                    .violations
                    .iter()
                    .map(|v| v.message.clone())
                    .collect::<Vec<_>>()
                    .join(", "),
            ));
        }

        // Find all call sites and generate edits
        let occurrences = self.find_all_occurrences(&command.file_path, &command.function_name)?;

        // Build the preview
        let mut preview = RefactorPreviewDto::new(format!(
            "Change signature of '{}' ({} call sites found)",
            command.function_name,
            occurrences.len()
        ))
        .with_files(vec![command.file_path.clone()])
        .with_symbols(
            impacted_symbols
                .iter()
                .map(|id| id.as_str().to_string())
                .collect(),
        )
        .with_risk(format!(
            "{:?} risk ({} symbols impacted)",
            safety_result.risk_level, impacted_count
        ));

        preview.change_count = occurrences.len();
        preview.symbols_affected = vec![command.function_name.clone()];

        Ok(preview)
    }

    /// Moves a symbol to a different file/location
    ///
    /// This method:
    /// 1. Validates the move operation with SafetyGate
    /// 2. Finds the target symbol
    /// 3. Calculates the impact on the codebase
    /// 4. Returns a preview of the changes
    pub fn move_symbol(
        &self,
        command: crate::application::commands::MoveSymbolCommand,
    ) -> AppResult<RefactorPreviewDto> {
        use crate::domain::aggregates::RefactorKind;

        // First, find the symbol in the project graph
        let symbol = self.find_symbol(&command.symbol_name, &command.source_path)?;

        // Get the project directory for building the graph
        let project_dir = Path::new(&command.source_path)
            .parent()
            .ok_or_else(|| AppError::InvalidParameter("Invalid file path".to_string()))?;

        // Build a graph of the project
        let mut store = PetGraphStore::new();
        self.build_minimal_graph(&mut store, project_dir, &command.symbol_name)?;

        let call_graph = store.to_call_graph();

        // Calculate impact
        let symbol_id = SymbolId::new(symbol.fully_qualified_name());
        let impacted_symbols = call_graph.find_all_dependents(&symbol_id);
        let impacted_count = impacted_symbols.len();

        // Create the refactor
        let target_location = Location::new(
            command.target_path.clone(),
            0, // line will be determined by the strategy
            0, // column will be determined by the strategy
        );
        let _refactor = Refactor::new(
            RefactorKind::Move,
            symbol.clone(),
            RefactorParameters::new()
                .with_new_location(target_location)
                .with_skip_validation(false)
                .with_max_impact(10),
        );

        // Validate with SafetyGate
        let safety_op = SafetyOperation::new(OperationType::Move, command.symbol_name.clone())
            .with_location(command.source_path.clone())
            .with_files_affected(1 + impacted_count / 5);

        let safety_result = self.safety_gate.validate(&safety_op);

        // Check if validation passed
        if !safety_result.is_safe {
            return Err(AppError::SafetyCheckFailed(
                safety_result
                    .violations
                    .iter()
                    .map(|v| v.message.clone())
                    .collect::<Vec<_>>()
                    .join(", "),
            ));
        }

        // Find all occurrences and generate edits
        let occurrences = self.find_all_occurrences(&command.source_path, &command.symbol_name)?;

        // Build the preview
        let mut preview = RefactorPreviewDto::new(format!(
            "Move '{}' from '{}' to '{}' ({} occurrences found)",
            command.symbol_name,
            command.source_path,
            command.target_path,
            occurrences.len()
        ))
        .with_files(vec![
            command.source_path.clone(),
            command.target_path.clone(),
        ])
        .with_symbols(
            impacted_symbols
                .iter()
                .map(|id| id.as_str().to_string())
                .collect(),
        )
        .with_risk(format!(
            "{:?} risk ({} symbols impacted)",
            safety_result.risk_level, impacted_count
        ));

        preview.change_count = occurrences.len();
        preview.symbols_affected = vec![command.symbol_name.clone()];

        Ok(preview)
    }

    /// Extracts a symbol (function/method) from the codebase
    ///
    /// This method:
    /// 1. Validates the extract operation with SafetyGate
    /// 2. Finds the target function
    /// 3. Uses ExtractStrategy to perform the extraction
    /// 4. Returns a preview of the changes
    pub fn extract_symbol(&self, file_path: &str, new_name: &str) -> AppResult<RefactorPreviewDto> {
        use crate::domain::aggregates::RefactorKind;
        use crate::infrastructure::refactor::ExtractStrategy;
        use std::sync::Arc;

        // Find the symbol
        let symbol = self.find_symbol(new_name, file_path)?;

        // Detect language
        let language =
            Language::from_extension(Path::new(file_path).extension()).ok_or_else(|| {
                AppError::InvalidParameter(format!("Unsupported file type: {}", file_path))
            })?;

        // Create parser and strategy
        let parser = TreeSitterParser::new(language)
            .map_err(|e| AppError::AnalysisError(format!("Failed to create parser: {}", e)))?;

        let strategy = ExtractStrategy::new(Arc::new(parser), self.safety_gate.clone());

        // Create the refactor
        let _refactor = Refactor::new(
            RefactorKind::Extract,
            symbol.clone(),
            RefactorParameters::new()
                .with_extraction_target(new_name)
                .with_skip_validation(false)
                .with_max_impact(10),
        );

        // Validate with SafetyGate
        let safety_op = SafetyOperation::new(OperationType::Extract, new_name.to_string())
            .with_location(file_path.to_string())
            .with_files_affected(1);

        let safety_result = self.safety_gate.validate(&safety_op);

        // Check if validation passed
        if !safety_result.is_safe {
            return Err(AppError::SafetyCheckFailed(
                safety_result
                    .violations
                    .iter()
                    .map(|v| v.message.clone())
                    .collect::<Vec<_>>()
                    .join(", "),
            ));
        }

        // Find extractable blocks
        let source = std::fs::read_to_string(file_path).map_err(|e| {
            AppError::InvalidParameter(format!("Failed to read {}: {}", file_path, e))
        })?;

        let blocks = strategy
            .find_extractable_blocks(&source, file_path)
            .map_err(|e| AppError::AnalysisError(format!("Failed to find blocks: {}", e)))?;

        let block_count = blocks.len();

        // Build the preview
        let mut preview = RefactorPreviewDto::new(format!(
            "Extract function '{}' ({} extractable blocks found)",
            new_name, block_count
        ))
        .with_files(vec![file_path.to_string()])
        .with_symbols(vec![new_name.to_string()])
        .with_risk(format!(
            "{:?} risk (1 file affected)",
            safety_result.risk_level
        ));

        preview.change_count = block_count;
        preview.symbols_affected = vec![new_name.to_string()];

        Ok(preview)
    }

    /// Extracts a block from the target symbol into a new function with the given name.
    ///
    /// Unlike `extract_symbol`, this method looks up the *target* (existing symbol)
    /// and uses *new_name* for the extracted function, which is the correct semantic.
    pub fn extract_symbol_with_target(
        &self,
        file_path: &str,
        target: &str,
        new_name: &str,
    ) -> AppResult<RefactorPreviewDto> {
        use crate::domain::aggregates::RefactorKind;
        use crate::infrastructure::refactor::ExtractStrategy;
        use std::sync::Arc;

        // Find the EXISTING symbol (target), not the new name
        let symbol = self.find_symbol(target, file_path)?;

        // Detect language
        let language =
            Language::from_extension(Path::new(file_path).extension()).ok_or_else(|| {
                AppError::InvalidParameter(format!("Unsupported file type: {}", file_path))
            })?;

        // Create parser and strategy
        let parser = TreeSitterParser::new(language)
            .map_err(|e| AppError::AnalysisError(format!("Failed to create parser: {}", e)))?;
        let strategy = ExtractStrategy::new(Arc::new(parser), self.safety_gate.clone());

        // Create the refactor
        let _refactor = Refactor::new(
            RefactorKind::Extract,
            symbol.clone(),
            RefactorParameters::new()
                .with_extraction_target(new_name)
                .with_skip_validation(false)
                .with_max_impact(10),
        );

        // Validate with SafetyGate
        let safety_op = SafetyOperation::new(OperationType::Extract, target.to_string())
            .with_location(file_path.to_string())
            .with_files_affected(1);
        let safety_result = self.safety_gate.validate(&safety_op);

        if !safety_result.is_safe {
            return Err(AppError::SafetyCheckFailed(
                safety_result
                    .violations
                    .iter()
                    .map(|v| v.message.clone())
                    .collect::<Vec<_>>()
                    .join(", "),
            ));
        }

        // Find extractable blocks
        let source = std::fs::read_to_string(file_path).map_err(|e| {
            AppError::InvalidParameter(format!("Failed to read {}: {}", file_path, e))
        })?;

        let blocks = strategy
            .find_extractable_blocks(&source, file_path)
            .map_err(|e| AppError::AnalysisError(format!("Failed to find blocks: {}", e)))?;

        let block_count = blocks.len();

        let mut preview = RefactorPreviewDto::new(format!(
            "Extract '{}' from '{}' ({} extractable blocks found)",
            new_name, target, block_count
        ))
        .with_files(vec![file_path.to_string()])
        .with_symbols(vec![target.to_string(), new_name.to_string()])
        .with_risk(format!(
            "{:?} risk (1 file affected)",
            safety_result.risk_level
        ));

        preview.change_count = block_count;
        preview.symbols_affected = vec![target.to_string()];

        Ok(preview)
    }

    /// Inlines a symbol (function/method) at its call sites
    ///
    /// This method:
    /// 1. Validates the inline operation with SafetyGate
    /// 2. Finds the target function and its call sites
    /// 3. Uses InlineStrategy to perform the inlining
    /// 4. Returns a preview of the changes
    pub fn inline_symbol(
        &self,
        file_path: &str,
        symbol_name: &str,
    ) -> AppResult<RefactorPreviewDto> {
        use crate::domain::aggregates::RefactorKind;
        use crate::infrastructure::refactor::InlineStrategy;
        use std::sync::Arc;

        // Find the symbol
        let symbol = self.find_symbol(symbol_name, file_path)?;

        // Detect language
        let language =
            Language::from_extension(Path::new(file_path).extension()).ok_or_else(|| {
                AppError::InvalidParameter(format!("Unsupported file type: {}", file_path))
            })?;

        // Create parser and strategy
        let parser = TreeSitterParser::new(language)
            .map_err(|e| AppError::AnalysisError(format!("Failed to create parser: {}", e)))?;

        let strategy = InlineStrategy::new(Arc::new(parser), self.safety_gate.clone());

        // Create the refactor
        let _refactor = Refactor::new(
            RefactorKind::Inline,
            symbol.clone(),
            RefactorParameters::new()
                .with_skip_validation(false)
                .with_max_impact(10),
        );

        // Validate with SafetyGate
        let safety_op = SafetyOperation::new(OperationType::Inline, symbol_name.to_string())
            .with_location(file_path.to_string())
            .with_files_affected(1);

        let safety_result = self.safety_gate.validate(&safety_op);

        // Check if validation passed
        if !safety_result.is_safe {
            return Err(AppError::SafetyCheckFailed(
                safety_result
                    .violations
                    .iter()
                    .map(|v| v.message.clone())
                    .collect::<Vec<_>>()
                    .join(", "),
            ));
        }

        // Find function definition and call sites
        let source = std::fs::read_to_string(file_path).map_err(|e| {
            AppError::InvalidParameter(format!("Failed to read {}: {}", file_path, e))
        })?;

        let _func_def = strategy
            .find_function_definition(&source, symbol_name)
            .map_err(|e| AppError::AnalysisError(format!("Failed to find function: {}", e)))?;

        let call_sites = strategy
            .find_call_sites(&source, symbol_name)
            .map_err(|e| AppError::AnalysisError(format!("Failed to find call sites: {}", e)))?;

        let call_count = call_sites.len();

        // Build the preview
        let mut preview = RefactorPreviewDto::new(format!(
            "Inline function '{}' ({} call sites found)",
            symbol_name, call_count
        ))
        .with_files(vec![file_path.to_string()])
        .with_symbols(vec![symbol_name.to_string()])
        .with_risk(format!(
            "{:?} risk (1 file affected)",
            safety_result.risk_level
        ));

        preview.change_count = call_count + 1; // +1 for removing the function definition
        preview.symbols_affected = vec![symbol_name.to_string()];

        Ok(preview)
    }

    /// Finds a symbol by name and file path
    fn find_symbol(&self, name: &str, file_path: &str) -> AppResult<Symbol> {
        // Read the source file
        let source = std::fs::read_to_string(file_path).map_err(|e| {
            AppError::InvalidParameter(format!("Failed to read {}: {}", file_path, e))
        })?;

        // Detect language
        let language =
            Language::from_extension(Path::new(file_path).extension()).ok_or_else(|| {
                AppError::InvalidParameter(format!("Unsupported file type: {}", file_path))
            })?;

        // Parse with tree-sitter
        let parser = TreeSitterParser::new(language)
            .map_err(|e| AppError::AnalysisError(format!("Failed to create parser: {}", e)))?;

        // Find all symbols in the file
        let symbols = parser
            .find_all_symbols_with_path(&source, file_path)
            .map_err(|e| AppError::AnalysisError(format!("Failed to find symbols: {}", e)))?;

        // Find the matching symbol
        symbols
            .into_iter()
            .find(|s| s.name() == name)
            .ok_or_else(|| {
                AppError::SymbolNotFound(format!("Symbol '{}' not found in {}", name, file_path))
            })
    }

    /// Finds all occurrences of a symbol in a file
    fn find_all_occurrences(
        &self,
        file_path: &str,
        symbol_name: &str,
    ) -> AppResult<Vec<IdentifierOccurrence>> {
        let source = std::fs::read_to_string(file_path).map_err(|e| {
            AppError::InvalidParameter(format!("Failed to read {}: {}", file_path, e))
        })?;

        let language =
            Language::from_extension(Path::new(file_path).extension()).ok_or_else(|| {
                AppError::InvalidParameter(format!("Unsupported file type: {}", file_path))
            })?;

        let parser = TreeSitterParser::new(language)
            .map_err(|e| AppError::AnalysisError(format!("Failed to create parser: {}", e)))?;

        let occurrences = parser
            .find_all_occurrences_of_identifier(&source, symbol_name)
            .map_err(|e| AppError::AnalysisError(format!("Failed to find occurrences: {}", e)))?;

        Ok(occurrences)
    }

    /// Builds a minimal graph focused on a specific symbol
    fn build_minimal_graph(
        &self,
        store: &mut PetGraphStore,
        project_dir: &Path,
        _symbol_name: &str,
    ) -> AppResult<()> {
        use walkdir::WalkDir;

        // Walk the directory and process source files
        for entry in WalkDir::new(project_dir)
            .follow_links(true)
            .into_iter()
            .filter_map(|e| e.ok())
        {
            let path = entry.path();
            if !path.is_file() {
                continue;
            }

            let language = match Language::from_extension(path.extension()) {
                Some(lang) => lang,
                None => continue,
            };

            let source = match std::fs::read_to_string(path) {
                Ok(s) => s,
                Err(_) => continue,
            };

            let file_path_str = path.to_string_lossy().to_string();

            let parser = match TreeSitterParser::new(language) {
                Ok(p) => p,
                Err(_) => continue,
            };

            // Find symbols
            let symbols = match parser.find_all_symbols_with_path(&source, &file_path_str) {
                Ok(s) => s,
                Err(_) => continue,
            };

            // Find call relationships
            let relationships = match parser.find_call_relationships(&source, &file_path_str) {
                Ok(r) => r,
                Err(_) => continue,
            };

            // Add symbols to graph
            for symbol in &symbols {
                let symbol_id = SymbolId::new(symbol.fully_qualified_name());
                let _ = store.add_dependency(
                    &symbol_id,
                    &symbol_id,
                    crate::domain::value_objects::DependencyType::Defines,
                );
            }

            // Add call relationships
            for (caller, callee_name) in relationships {
                let caller_id = SymbolId::new(caller.fully_qualified_name());
                let callee_id = SymbolId::new(format!("{}:0:0", callee_name));
                let _ = store.add_dependency(
                    &caller_id,
                    &callee_id,
                    crate::domain::value_objects::DependencyType::Calls,
                );
            }
        }

        Ok(())
    }

    /// Generates the actual text edits for a rename operation
    pub fn generate_rename_edits(
        &self,
        file_path: &str,
        old_name: &str,
        new_name: &str,
    ) -> AppResult<Vec<TextEdit>> {
        let source = std::fs::read_to_string(file_path).map_err(|e| {
            AppError::InvalidParameter(format!("Failed to read {}: {}", file_path, e))
        })?;

        let language =
            Language::from_extension(Path::new(file_path).extension()).ok_or_else(|| {
                AppError::InvalidParameter(format!("Unsupported file type: {}", file_path))
            })?;

        let parser = TreeSitterParser::new(language)
            .map_err(|e| AppError::AnalysisError(format!("Failed to create parser: {}", e)))?;

        let occurrences = parser
            .find_all_occurrences_of_identifier(&source, old_name)
            .map_err(|e| AppError::AnalysisError(format!("Failed to find occurrences: {}", e)))?;

        let edits: Vec<TextEdit> = occurrences
            .into_iter()
            .map(|occ| {
                TextEdit::new(
                    SourceRange::new(
                        Location::new(file_path, occ.line, occ.column),
                        Location::new(file_path, occ.line, occ.column + occ.length),
                    ),
                    new_name.to_string(),
                )
            })
            .collect();

        Ok(edits)
    }

    /// Validates that proposed edits won't break syntax by applying them to a VFS
    /// and attempting to parse the result with tree-sitter.
    ///
    /// Returns Ok(()) if the modified code parses successfully.
    /// Returns Err with details if the modified code has syntax errors.
    pub fn validate_edits_syntax(
        &self,
        file_path: &str,
        source: &str,
        edits: &[TextEdit],
    ) -> AppResult<()> {
        let language =
            Language::from_extension(Path::new(file_path).extension()).ok_or_else(|| {
                AppError::InvalidParameter(format!("Unsupported file type: {}", file_path))
            })?;

        // Create VFS and clone the file
        let mut vfs = VirtualFileSystem::new();
        let url = Url::from_file_path(file_path)
            .map_err(|_| AppError::InvalidParameter(format!("Invalid file path: {}", file_path)))?;
        vfs.set_content(url.clone(), source.to_string());

        // Convert our TextEdit to VFS TextEdit
        let vfs_edits: Vec<VfsTextEdit> = edits
            .iter()
            .map(|edit| {
                // Calculate byte offsets from line/column
                let start_offset = self.line_column_to_offset(source, edit.range.start());
                let end_offset = self.line_column_to_offset(source, edit.range.end());
                VfsTextEdit {
                    range: (start_offset, end_offset),
                    new_text: edit.new_text.clone(),
                }
            })
            .collect();

        // Apply edits to VFS
        vfs.apply_edits_to_file(&url, vfs_edits.clone())
            .map_err(|e| AppError::AnalysisError(format!("Failed to apply edits: {}", e)))?;

        // Get the modified content
        let modified_content = vfs.get_content(&url).ok_or_else(|| {
            AppError::AnalysisError("Failed to get modified content from VFS".to_string())
        })?;

        // Parse with tree-sitter to validate syntax
        let parser = TreeSitterParser::new(language)
            .map_err(|e| AppError::AnalysisError(format!("Failed to create parser: {}", e)))?;

        // Try to parse - if it fails, the syntax is invalid
        let _tree = parser.parse_tree(&modified_content).map_err(|e| {
            AppError::AnalysisError(format!(
                "Syntax validation failed: the refactored code has syntax errors: {}",
                e
            ))
        })?;

        Ok(())
    }

    /// Converts a Location (line, column) to a byte offset in the source
    fn line_column_to_offset(&self, source: &str, location: &Location) -> u32 {
        let mut offset = 0u32;
        for (i, line) in source.lines().enumerate() {
            if i as u32 == location.line() {
                return offset + location.column().min(line.len() as u32);
            }
            offset += line.len() as u32 + 1; // +1 for newline
        }
        offset
    }

    /// Validates syntax of a file directly (used by validate_syntax handler)
    pub fn validate_file_syntax(&self, file_path: &str) -> AppResult<bool> {
        let source = std::fs::read_to_string(file_path).map_err(|e| {
            AppError::InvalidParameter(format!("Failed to read {}: {}", file_path, e))
        })?;

        let language =
            Language::from_extension(Path::new(file_path).extension()).ok_or_else(|| {
                AppError::InvalidParameter(format!("Unsupported file type: {}", file_path))
            })?;

        let parser = TreeSitterParser::new(language)
            .map_err(|e| AppError::AnalysisError(format!("Failed to create parser: {}", e)))?;

        match parser.parse_tree(&source) {
            Ok(_) => Ok(true),
            Err(_) => Ok(false),
        }
    }
}

impl Default for RefactorService {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[test]
    fn test_rename_symbol_generates_preview() {
        let mut file = NamedTempFile::with_suffix(".py").unwrap();
        writeln!(file, "def foo():").unwrap();
        writeln!(file, "    foo()").unwrap();
        writeln!(file, "    bar()").unwrap();

        let service = RefactorService::new();
        let command = RenameSymbolCommand::new("foo", "baz", file.path().to_str().unwrap());

        let result = service.rename_symbol(command);
        assert!(result.is_ok(), "Rename should succeed");

        let preview = result.unwrap();
        assert_eq!(
            preview.change_count, 2,
            "Should find 2 occurrences of 'foo'"
        );
        assert!(preview.description.contains("foo"));
        assert!(preview.description.contains("baz"));
    }

    #[test]
    fn test_rename_symbol_not_found() {
        let mut file = NamedTempFile::with_suffix(".py").unwrap();
        writeln!(file, "def hello():").unwrap();
        writeln!(file, "    pass").unwrap();

        let service = RefactorService::new();
        let command = RenameSymbolCommand::new("nonexistent", "bar", file.path().to_str().unwrap());

        let result = service.rename_symbol(command);
        assert!(
            result.is_err(),
            "Rename should fail for non-existent symbol"
        );
    }

    #[test]
    fn test_generate_rename_edits() {
        let mut file = NamedTempFile::with_suffix(".py").unwrap();
        writeln!(file, "def foo():").unwrap();
        writeln!(file, "    foo()").unwrap();
        writeln!(file, "    foo()").unwrap();

        let service = RefactorService::new();
        let edits = service
            .generate_rename_edits(file.path().to_str().unwrap(), "foo", "bar")
            .unwrap();

        assert_eq!(edits.len(), 3, "Should generate 3 edits");
    }
}
