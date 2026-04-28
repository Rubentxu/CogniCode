use super::*;

pub async fn handle_safe_refactor(
    ctx: &HandlerContext,
    input: SafeRefactorInput,
) -> HandlerResult<SafeRefactorOutput> {
    

    // Validate input
    ctx.validator.validate_query(&input.target)?;
    if let Some(params) = &input.params {
        ctx.validator.validate_query(&params.to_string())?;
    }

    // Handle different refactor actions
    match input.action {
        RefactorAction::Rename => {
            // Extract new_name from params
            let new_name = input.params.as_ref()
                .and_then(|p| p.get("new_name"))
                .and_then(|v| v.as_str())
                .ok_or_else(|| HandlerError::InvalidInput("Missing 'new_name' parameter for rename".to_string()))?
                .to_string();

            // Get the file path from params or use working_dir, resolving relative paths
            let file_path = input.params.as_ref()
                .and_then(|p| p.get("file_path"))
                .and_then(|v| v.as_str())
                .map(|s| resolve_file_path(s, &ctx.working_dir))
                .unwrap_or_else(|| ctx.working_dir.join(&input.target));

            let file_path_str = file_path.to_string_lossy().to_string();

            // Create rename command
            let command = RenameSymbolCommand::new(&input.target, &new_name, &file_path_str);

            // Execute rename via RefactorService
            match ctx.refactor_service.rename_symbol(command) {
                Ok(preview) => {
                    // Generate the actual edits for the preview
                    let edits = ctx.refactor_service.generate_rename_edits(
                        &file_path_str,
                        &input.target,
                        &new_name,
                    ).unwrap_or_default();

                    // Convert edits to ChangeEntry
                    let changes: Vec<ChangeEntry> = edits.iter().map(|edit| {
                        let start_loc = edit.range.start();
                        ChangeEntry {
                            file: start_loc.file().to_string(),
                            old_text: input.target.clone(),
                            new_text: new_name.clone(),
                            location: SourceLocation {
                                file: start_loc.file().to_string(),
                                line: start_loc.line(),
                                column: start_loc.column(),
                            },
                        }
                    }).collect();

                    Ok(SafeRefactorOutput {
                        action: input.action,
                        success: true,
                        changes,
                        validation_result: ValidationResult {
                            is_valid: true,
                            warnings: vec![format!("Impact: {} symbols affected", preview.symbols_affected.len())],
                            errors: Vec::new(),
                        },
                        error_message: None,
                    })
                }
                Err(e) => {
                    Ok(SafeRefactorOutput {
                        action: input.action,
                        success: false,
                        changes: Vec::new(),
                        validation_result: ValidationResult {
                            is_valid: false,
                            warnings: Vec::new(),
                            errors: vec![e.to_string()],
                        },
                        error_message: Some(e.to_string()),
                    })
                }
            }
        }
        RefactorAction::Extract => {
            // target is the existing symbol to extract from
            // new_name is the name for the extracted function
            let new_name = input.params.as_ref()
                .and_then(|p| p.get("new_name"))
                .and_then(|v| v.as_str())
                .ok_or_else(|| HandlerError::InvalidInput("Missing 'new_name' parameter for extract".to_string()))?
                .to_string();

            // Get the file path from params or use working_dir/target, resolving relative paths
            let file_path = input.params.as_ref()
                .and_then(|p| p.get("file_path"))
                .and_then(|v| v.as_str())
                .map(|s| resolve_file_path(s, &ctx.working_dir))
                .unwrap_or_else(|| ctx.working_dir.join(&input.target));

            let file_path_str = file_path.to_string_lossy().to_string();

            // Execute extract via RefactorService
            // Pass input.target (existing symbol) so extract_symbol can find it
            match ctx.refactor_service.extract_symbol_with_target(&file_path_str, &input.target, &new_name) {
                Ok(preview) => {
                    Ok(SafeRefactorOutput {
                        action: input.action,
                        success: true,
                        changes: vec![ChangeEntry {
                            file: file_path_str.clone(),
                            old_text: format!("// {} block", new_name),
                            new_text: format!("fn {}() {{ ... }}", new_name),
                            location: SourceLocation {
                                file: file_path_str.clone(),
                                line: 0,
                                column: 0,
                            },
                        }],
                        validation_result: ValidationResult {
                            is_valid: true,
                            warnings: vec![preview.description],
                            errors: Vec::new(),
                        },
                        error_message: None,
                    })
                }
                Err(e) => {
                    Ok(SafeRefactorOutput {
                        action: input.action,
                        success: false,
                        changes: Vec::new(),
                        validation_result: ValidationResult {
                            is_valid: false,
                            warnings: Vec::new(),
                            errors: vec![e.to_string()],
                        },
                        error_message: Some(e.to_string()),
                    })
                }
            }
        }
        RefactorAction::Inline => {
            // Get the file path from params or use working_dir, resolving relative paths
            let file_path = input.params.as_ref()
                .and_then(|p| p.get("file_path"))
                .and_then(|v| v.as_str())
                .map(|s| resolve_file_path(s, &ctx.working_dir))
                .unwrap_or_else(|| ctx.working_dir.join(&input.target));

            let file_path_str = file_path.to_string_lossy().to_string();

            // Execute inline via RefactorService
            match ctx.refactor_service.inline_symbol(&file_path_str, &input.target) {
                Ok(preview) => {
                    Ok(SafeRefactorOutput {
                        action: input.action,
                        success: true,
                        changes: vec![ChangeEntry {
                            file: file_path_str.clone(),
                            old_text: input.target.clone(),
                            new_text: "// inlined".to_string(),
                            location: SourceLocation {
                                file: file_path_str,
                                line: 0,
                                column: 0,
                            },
                        }],
                        validation_result: ValidationResult {
                            is_valid: true,
                            warnings: vec![preview.description],
                            errors: Vec::new(),
                        },
                        error_message: None,
                    })
                }
                Err(e) => {
                    Ok(SafeRefactorOutput {
                        action: input.action,
                        success: false,
                        changes: Vec::new(),
                        validation_result: ValidationResult {
                            is_valid: false,
                            warnings: Vec::new(),
                            errors: vec![e.to_string()],
                        },
                        error_message: Some(e.to_string()),
                    })
                }
            }
        }
        RefactorAction::Move => {
            // Extract source_path and target_path from params, resolving relative paths
            let source_path = input.params.as_ref()
                .and_then(|p| p.get("source_path"))
                .and_then(|v| v.as_str())
                .map(|s| resolve_file_path(s, &ctx.working_dir).to_string_lossy().to_string())
                .ok_or_else(|| HandlerError::InvalidInput("Missing 'source_path' parameter for move".to_string()))?;

            let target_path = input.params.as_ref()
                .and_then(|p| p.get("target_path"))
                .and_then(|v| v.as_str())
                .map(|s| resolve_file_path(s, &ctx.working_dir).to_string_lossy().to_string())
                .ok_or_else(|| HandlerError::InvalidInput("Missing 'target_path' parameter for move".to_string()))?;

            // Create move command
            let command = MoveSymbolCommand::new(&input.target, &source_path, &target_path);

            // Execute move via RefactorService
            match ctx.refactor_service.move_symbol(command) {
                Ok(preview) => {
                    Ok(SafeRefactorOutput {
                        action: input.action,
                        success: true,
                        changes: vec![ChangeEntry {
                            file: source_path.clone(),
                            old_text: input.target.clone(),
                            new_text: format!("// moved to {}", target_path),
                            location: SourceLocation {
                                file: source_path.clone(),
                                line: 0,
                                column: 0,
                            },
                        }],
                        validation_result: ValidationResult {
                            is_valid: true,
                            warnings: vec![preview.description],
                            errors: Vec::new(),
                        },
                        error_message: None,
                    })
                }
                Err(e) => {
                    Ok(SafeRefactorOutput {
                        action: input.action,
                        success: false,
                        changes: Vec::new(),
                        validation_result: ValidationResult {
                            is_valid: false,
                            warnings: Vec::new(),
                            errors: vec![e.to_string()],
                        },
                        error_message: Some(e.to_string()),
                    })
                }
            }
        }
        RefactorAction::ChangeSignature => {
            // Extract new_parameters from params
            let new_parameters_json = input.params.as_ref()
                .and_then(|p| p.get("new_parameters"))
                .ok_or_else(|| HandlerError::InvalidInput("Missing 'new_parameters' parameter for change_signature".to_string()))?;

            let new_parameters: Vec<ParameterDefinition> = serde_json::from_value(new_parameters_json.clone())
                .map_err(|e| HandlerError::InvalidInput(format!("Invalid new_parameters: {}", e)))?;

            // Get the file path from params or use working_dir, resolving relative paths
            let file_path = input.params.as_ref()
                .and_then(|p| p.get("file_path"))
                .and_then(|v| v.as_str())
                .map(|s| resolve_file_path(s, &ctx.working_dir))
                .unwrap_or_else(|| ctx.working_dir.join(&input.target));

            let file_path_str = file_path.to_string_lossy().to_string();

            // Create change signature command
            let command = ChangeSignatureCommand {
                function_name: input.target.clone(),
                new_parameters,
                file_path: file_path_str.clone(),
            };

            // Execute change_signature via RefactorService
            match ctx.refactor_service.change_signature(command) {
                Ok(preview) => {
                    Ok(SafeRefactorOutput {
                        action: input.action,
                        success: true,
                        changes: vec![ChangeEntry {
                            file: file_path_str.clone(),
                            old_text: input.target.clone(),
                            new_text: "// signature changed".to_string(),
                            location: SourceLocation {
                                file: file_path_str,
                                line: 0,
                                column: 0,
                            },
                        }],
                        validation_result: ValidationResult {
                            is_valid: true,
                            warnings: vec![preview.description],
                            errors: Vec::new(),
                        },
                        error_message: None,
                    })
                }
                Err(e) => {
                    Ok(SafeRefactorOutput {
                        action: input.action,
                        success: false,
                        changes: Vec::new(),
                        validation_result: ValidationResult {
                            is_valid: false,
                            warnings: Vec::new(),
                            errors: vec![e.to_string()],
                        },
                        error_message: Some(e.to_string()),
                    })
                }
            }
        }
    }
}

/// Handler for validate_syntax tool
pub async fn handle_validate_syntax(
    ctx: &HandlerContext,
    input: ValidateSyntaxInput,
) -> HandlerResult<ValidateSyntaxOutput> {
    // Resolve the file path relative to working directory
    let file_path = resolve_file_path(&input.file_path, &ctx.working_dir);

    // Validate file path
    ctx.validator.validate_file_path(&file_path.to_string_lossy())?;

    // Use VFS-based tree-sitter validation
    match ctx.refactor_service.validate_file_syntax(&file_path.to_string_lossy()) {
        Ok(is_valid) => Ok(ValidateSyntaxOutput {
            file_path: input.file_path,
            is_valid,
            errors: Vec::new(),
            warnings: Vec::new(),
        }),
        Err(e) => Ok(ValidateSyntaxOutput {
            file_path: input.file_path,
            is_valid: false,
            errors: vec![crate::interface::mcp::schemas::SyntaxError {
                line: 1,
                column: 1,
                message: e.to_string(),
                severity: "error".to_string(),
            }],
            warnings: Vec::new(),
        }),
    }
}

