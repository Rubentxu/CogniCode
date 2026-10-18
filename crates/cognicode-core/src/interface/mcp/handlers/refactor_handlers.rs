use super::*;

#[cognicode_macros::aix_tool(
    name = "safe_refactor",
    description = "Perform safe refactoring with validation and preview.",
    input_schema = SafeRefactorInput
)]
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
            let new_name = input
                .params
                .as_ref()
                .and_then(|p| p.get("new_name"))
                .and_then(|v| v.as_str())
                .ok_or_else(|| {
                    HandlerError::InvalidInput(
                        "Missing 'new_name' parameter for rename".to_string(),
                    )
                })?
                .to_string();

            // Get the file path from params or use working_dir, resolving relative paths
            let file_path = input
                .params
                .as_ref()
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
                    let edits = ctx
                        .refactor_service
                        .generate_rename_edits(&file_path_str, &input.target, &new_name)
                        .unwrap_or_default();

                    // Convert edits to ChangeEntry
                    let changes: Vec<ChangeEntry> = edits
                        .iter()
                        .map(|edit| {
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
                        })
                        .collect();

                    Ok(SafeRefactorOutput {
                        action: input.action,
                        success: true,
                        changes,
                        validation_result: ValidationResult {
                            is_valid: true,
                            warnings: vec![format!(
                                "Impact: {} symbols affected",
                                preview.symbols_affected.len()
                            )],
                            errors: Vec::new(),
                        },
                        error_message: None,
                    })
                }
                Err(e) => Ok(SafeRefactorOutput {
                    action: input.action,
                    success: false,
                    changes: Vec::new(),
                    validation_result: ValidationResult {
                        is_valid: false,
                        warnings: Vec::new(),
                        errors: vec![e.to_string()],
                    },
                    error_message: Some(e.to_string()),
                }),
            }
        }
        RefactorAction::Extract => {
            // target is the existing symbol to extract from
            // new_name is the name for the extracted function
            let new_name = input
                .params
                .as_ref()
                .and_then(|p| p.get("new_name"))
                .and_then(|v| v.as_str())
                .ok_or_else(|| {
                    HandlerError::InvalidInput(
                        "Missing 'new_name' parameter for extract".to_string(),
                    )
                })?
                .to_string();

            // Get the file path from params or use working_dir/target, resolving relative paths
            let file_path = input
                .params
                .as_ref()
                .and_then(|p| p.get("file_path"))
                .and_then(|v| v.as_str())
                .map(|s| resolve_file_path(s, &ctx.working_dir))
                .unwrap_or_else(|| ctx.working_dir.join(&input.target));

            let file_path_str = file_path.to_string_lossy().to_string();

            // Execute extract via RefactorService
            // Pass input.target (existing symbol) so extract_symbol can find it
            match ctx.refactor_service.extract_symbol_with_target(
                &file_path_str,
                &input.target,
                &new_name,
            ) {
                Ok(preview) => Ok(SafeRefactorOutput {
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
                }),
                Err(e) => Ok(SafeRefactorOutput {
                    action: input.action,
                    success: false,
                    changes: Vec::new(),
                    validation_result: ValidationResult {
                        is_valid: false,
                        warnings: Vec::new(),
                        errors: vec![e.to_string()],
                    },
                    error_message: Some(e.to_string()),
                }),
            }
        }
        RefactorAction::Inline => {
            // Get the file path from params or use working_dir, resolving relative paths
            let file_path = input
                .params
                .as_ref()
                .and_then(|p| p.get("file_path"))
                .and_then(|v| v.as_str())
                .map(|s| resolve_file_path(s, &ctx.working_dir))
                .unwrap_or_else(|| ctx.working_dir.join(&input.target));

            let file_path_str = file_path.to_string_lossy().to_string();

            // Execute inline via RefactorService
            match ctx
                .refactor_service
                .inline_symbol(&file_path_str, &input.target)
            {
                Ok(preview) => Ok(SafeRefactorOutput {
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
                }),
                Err(e) => Ok(SafeRefactorOutput {
                    action: input.action,
                    success: false,
                    changes: Vec::new(),
                    validation_result: ValidationResult {
                        is_valid: false,
                        warnings: Vec::new(),
                        errors: vec![e.to_string()],
                    },
                    error_message: Some(e.to_string()),
                }),
            }
        }
        RefactorAction::Move => {
            // Extract source_path and target_path from params, resolving relative paths
            let source_path = input
                .params
                .as_ref()
                .and_then(|p| p.get("source_path"))
                .and_then(|v| v.as_str())
                .map(|s| {
                    resolve_file_path(s, &ctx.working_dir)
                        .to_string_lossy()
                        .to_string()
                })
                .ok_or_else(|| {
                    HandlerError::InvalidInput(
                        "Missing 'source_path' parameter for move".to_string(),
                    )
                })?;

            let target_path = input
                .params
                .as_ref()
                .and_then(|p| p.get("target_path"))
                .and_then(|v| v.as_str())
                .map(|s| {
                    resolve_file_path(s, &ctx.working_dir)
                        .to_string_lossy()
                        .to_string()
                })
                .ok_or_else(|| {
                    HandlerError::InvalidInput(
                        "Missing 'target_path' parameter for move".to_string(),
                    )
                })?;

            // Create move command
            let command = MoveSymbolCommand::new(&input.target, &source_path, &target_path);

            // Execute move via RefactorService
            match ctx.refactor_service.move_symbol(command) {
                Ok(preview) => Ok(SafeRefactorOutput {
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
                }),
                Err(e) => Ok(SafeRefactorOutput {
                    action: input.action,
                    success: false,
                    changes: Vec::new(),
                    validation_result: ValidationResult {
                        is_valid: false,
                        warnings: Vec::new(),
                        errors: vec![e.to_string()],
                    },
                    error_message: Some(e.to_string()),
                }),
            }
        }
        RefactorAction::ChangeSignature => {
            // Extract new_parameters from params
            let new_parameters_json = input
                .params
                .as_ref()
                .and_then(|p| p.get("new_parameters"))
                .ok_or_else(|| {
                    HandlerError::InvalidInput(
                        "Missing 'new_parameters' parameter for change_signature".to_string(),
                    )
                })?;

            let new_parameters: Vec<ParameterDefinition> =
                serde_json::from_value(new_parameters_json.clone()).map_err(|e| {
                    HandlerError::InvalidInput(format!("Invalid new_parameters: {}", e))
                })?;

            // Get the file path from params or use working_dir, resolving relative paths
            let file_path = input
                .params
                .as_ref()
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
                Ok(preview) => Ok(SafeRefactorOutput {
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
                }),
                Err(e) => Ok(SafeRefactorOutput {
                    action: input.action,
                    success: false,
                    changes: Vec::new(),
                    validation_result: ValidationResult {
                        is_valid: false,
                        warnings: Vec::new(),
                        errors: vec![e.to_string()],
                    },
                    error_message: Some(e.to_string()),
                }),
            }
        }
    }
}

/// Handler for validate_syntax tool
#[cognicode_macros::aix_tool(
    name = "validate_syntax",
    description = "Validate the syntax of a source file.",
    input_schema = ValidateSyntaxInput
)]
pub async fn handle_validate_syntax(
    ctx: &HandlerContext,
    input: ValidateSyntaxInput,
) -> HandlerResult<ValidateSyntaxOutput> {
    // Resolve the file path relative to working directory
    let file_path = resolve_file_path(&input.file_path, &ctx.working_dir);

    // Validate file path
    ctx.validator
        .validate_file_path(&file_path.to_string_lossy())?;

    // Use VFS-based tree-sitter validation
    match ctx
        .refactor_service
        .validate_file_syntax(&file_path.to_string_lossy())
    {
        Ok(is_valid) => {
            if is_valid {
                Ok(ValidateSyntaxOutput {
                    file_path: input.file_path,
                    is_valid: true,
                    errors: Vec::new(),
                    warnings: Vec::new(),
                })
            } else {
                Ok(ValidateSyntaxOutput {
                    file_path: input.file_path,
                    is_valid: false,
                    errors: vec![crate::interface::mcp::schemas::SyntaxIssue {
                        line: 1,
                        column: 1,
                        message: "Syntax error detected by tree-sitter parser".to_string(),
                        severity: "error".to_string(),
                    }],
                    warnings: Vec::new(),
                })
            }
        }
        Err(e) => Ok(ValidateSyntaxOutput {
            file_path: input.file_path,
            is_valid: false,
            errors: vec![crate::interface::mcp::schemas::SyntaxIssue {
                line: 1,
                column: 1,
                message: e.to_string(),
                severity: "error".to_string(),
            }],
            warnings: Vec::new(),
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // =========================================================================
    // handle_safe_refactor Tests - Rename Action
    // =========================================================================

    #[tokio::test]
    async fn test_handle_safe_refactor_rename_with_new_name_extraction() {
        let tempdir = tempfile::tempdir().unwrap();
        let tempdir_path = tempdir.path();

        // Create a test Rust file
        let rust_file = tempdir_path.join("test.rs");
        std::fs::write(&rust_file, "fn foo() {}\nfn bar() { foo(); }").unwrap();

        let ctx = HandlerContext::builder()
            .with_working_dir(tempdir_path.to_path_buf())
            .build();

        let input = SafeRefactorInput {
            action: RefactorAction::Rename,
            target: "foo".to_string(),
            params: Some(serde_json::json!({
                "new_name": "foo_renamed",
                "file_path": rust_file.to_str().unwrap()
            })),
        };

        let result = handle_safe_refactor(&ctx, input).await;
        // Result should be Ok - either success with changes or failure due to symbol not found
        assert!(result.is_ok());
        let output = result.unwrap();
        assert_eq!(output.action, RefactorAction::Rename);
    }

    #[tokio::test]
    async fn test_handle_safe_refactor_rename_missing_new_name() {
        let tempdir = tempfile::tempdir().unwrap();
        let tempdir_path = tempdir.path();

        let ctx = HandlerContext::builder()
            .with_working_dir(tempdir_path.to_path_buf())
            .build();

        let input = SafeRefactorInput {
            action: RefactorAction::Rename,
            target: "foo".to_string(),
            params: None, // Missing new_name
        };

        let result = handle_safe_refactor(&ctx, input).await;
        // Should fail with InvalidInput error about missing new_name
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(matches!(err, HandlerError::InvalidInput(_)));
        let error_msg = match err {
            HandlerError::InvalidInput(msg) => msg,
            _ => String::new(),
        };
        assert!(error_msg.contains("new_name") || error_msg.contains("rename"));
    }

    #[tokio::test]
    async fn test_handle_safe_refactor_rename_invalid_target() {
        let tempdir = tempfile::tempdir().unwrap();
        let tempdir_path = tempdir.path();

        // Create a test Rust file
        let rust_file = tempdir_path.join("test.rs");
        std::fs::write(&rust_file, "fn existing_func() {}\n").unwrap();

        let ctx = HandlerContext::builder()
            .with_working_dir(tempdir_path.to_path_buf())
            .build();

        let input = SafeRefactorInput {
            action: RefactorAction::Rename,
            target: "nonexistent_symbol_xyz".to_string(),
            params: Some(serde_json::json!({
                "new_name": "new_name",
                "file_path": rust_file.to_str().unwrap()
            })),
        };

        let result = handle_safe_refactor(&ctx, input).await;
        // Should return Ok with success=false since symbol won't be found
        assert!(result.is_ok());
        let output = result.unwrap();
        assert!(!output.success);
    }

    // =========================================================================
    // handle_safe_refactor Tests - Extract Action
    // =========================================================================

    #[tokio::test]
    async fn test_handle_safe_refactor_extract_valid() {
        let tempdir = tempfile::tempdir().unwrap();
        let tempdir_path = tempdir.path();

        // Create a test Rust file
        let rust_file = tempdir_path.join("test.rs");
        std::fs::write(
            &rust_file,
            r#"
fn process() {
    let total = 10;
    let tax = total * 0.1;
    let final_total = total + tax;
    println!("{}", final_total);
}
"#,
        )
        .unwrap();

        let ctx = HandlerContext::builder()
            .with_working_dir(tempdir_path.to_path_buf())
            .build();

        let input = SafeRefactorInput {
            action: RefactorAction::Extract,
            target: "process".to_string(),
            params: Some(serde_json::json!({
                "new_name": "calculate_tax",
                "file_path": rust_file.to_str().unwrap()
            })),
        };

        let result = handle_safe_refactor(&ctx, input).await;
        // Result should be Ok - extraction may succeed or fail based on implementation
        assert!(result.is_ok());
        let output = result.unwrap();
        assert_eq!(output.action, RefactorAction::Extract);
    }

    #[tokio::test]
    async fn test_handle_safe_refactor_extract_missing_new_name() {
        let tempdir = tempfile::tempdir().unwrap();
        let tempdir_path = tempdir.path();

        let ctx = HandlerContext::builder()
            .with_working_dir(tempdir_path.to_path_buf())
            .build();

        let input = SafeRefactorInput {
            action: RefactorAction::Extract,
            target: "some_func".to_string(),
            params: None, // Missing new_name
        };

        let result = handle_safe_refactor(&ctx, input).await;
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(matches!(err, HandlerError::InvalidInput(_)));
    }

    #[tokio::test]
    async fn test_handle_safe_refactor_extract_invalid_selection() {
        let tempdir = tempfile::tempdir().unwrap();
        let tempdir_path = tempdir.path();

        // Create a minimal test file
        let rust_file = tempdir_path.join("test.rs");
        std::fs::write(&rust_file, "fn foo() {}\n").unwrap();

        let ctx = HandlerContext::builder()
            .with_working_dir(tempdir_path.to_path_buf())
            .build();

        let input = SafeRefactorInput {
            action: RefactorAction::Extract,
            target: "nonexistent_func".to_string(),
            params: Some(serde_json::json!({
                "new_name": "extracted_func"
            })),
        };

        let result = handle_safe_refactor(&ctx, input).await;
        // Should return Ok with success=false for invalid selection
        assert!(result.is_ok());
        let output = result.unwrap();
        assert!(!output.success);
    }

    // =========================================================================
    // handle_safe_refactor Tests - Inline Action
    // =========================================================================

    #[tokio::test]
    async fn test_handle_safe_refactor_inline_valid() {
        let tempdir = tempfile::tempdir().unwrap();
        let tempdir_path = tempdir.path();

        // Create a test Rust file with a simple helper function
        let rust_file = tempdir_path.join("test.rs");
        std::fs::write(
            &rust_file,
            r#"
fn helper(x: i32) -> i32 {
    x * 2
}

fn main() {
    let y = helper(5);
}
"#,
        )
        .unwrap();

        let ctx = HandlerContext::builder()
            .with_working_dir(tempdir_path.to_path_buf())
            .build();

        let input = SafeRefactorInput {
            action: RefactorAction::Inline,
            target: "helper".to_string(),
            params: Some(serde_json::json!({
                "file_path": rust_file.to_str().unwrap()
            })),
        };

        let result = handle_safe_refactor(&ctx, input).await;
        assert!(result.is_ok());
        let output = result.unwrap();
        assert_eq!(output.action, RefactorAction::Inline);
    }

    #[tokio::test]
    async fn test_handle_safe_refactor_inline_invalid_target() {
        let tempdir = tempfile::tempdir().unwrap();
        let tempdir_path = tempdir.path();

        let rust_file = tempdir_path.join("test.rs");
        std::fs::write(&rust_file, "fn foo() {}\n").unwrap();

        let ctx = HandlerContext::builder()
            .with_working_dir(tempdir_path.to_path_buf())
            .build();

        let input = SafeRefactorInput {
            action: RefactorAction::Inline,
            target: "nonexistent_function".to_string(),
            params: Some(serde_json::json!({
                "file_path": rust_file.to_str().unwrap()
            })),
        };

        let result = handle_safe_refactor(&ctx, input).await;
        assert!(result.is_ok());
        let output = result.unwrap();
        assert!(!output.success);
    }

    // =========================================================================
    // handle_safe_refactor Tests - ChangeSignature Action
    // =========================================================================

    #[tokio::test]
    async fn test_handle_safe_refactor_change_signature_valid() {
        let tempdir = tempfile::tempdir().unwrap();
        let tempdir_path = tempdir.path();

        let rust_file = tempdir_path.join("test.rs");
        std::fs::write(
            &rust_file,
            r#"
fn process(x: i32, y: i32) -> i32 {
    x + y
}
"#,
        )
        .unwrap();

        let ctx = HandlerContext::builder()
            .with_working_dir(tempdir_path.to_path_buf())
            .build();

        let input = SafeRefactorInput {
            action: RefactorAction::ChangeSignature,
            target: "process".to_string(),
            params: Some(serde_json::json!({
                "new_parameters": [
                    {"name": "a", "type_annotation": "i32"},
                    {"name": "b", "type_annotation": "i32"},
                    {"name": "c", "type_annotation": "i32", "default_value": "0"}
                ],
                "file_path": rust_file.to_str().unwrap()
            })),
        };

        let result = handle_safe_refactor(&ctx, input).await;
        assert!(result.is_ok());
        let output = result.unwrap();
        assert_eq!(output.action, RefactorAction::ChangeSignature);
    }

    #[tokio::test]
    async fn test_handle_safe_refactor_change_signature_missing_params() {
        let tempdir = tempfile::tempdir().unwrap();
        let tempdir_path = tempdir.path();

        let ctx = HandlerContext::builder()
            .with_working_dir(tempdir_path.to_path_buf())
            .build();

        let input = SafeRefactorInput {
            action: RefactorAction::ChangeSignature,
            target: "some_func".to_string(),
            params: None, // Missing new_parameters
        };

        let result = handle_safe_refactor(&ctx, input).await;
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(matches!(err, HandlerError::InvalidInput(_)));
        let error_msg = match err {
            HandlerError::InvalidInput(msg) => msg,
            _ => String::new(),
        };
        assert!(error_msg.contains("new_parameters") || error_msg.contains("change_signature"));
    }

    #[tokio::test]
    async fn test_handle_safe_refactor_change_signature_invalid_params() {
        let tempdir = tempfile::tempdir().unwrap();
        let tempdir_path = tempdir.path();

        let ctx = HandlerContext::builder()
            .with_working_dir(tempdir_path.to_path_buf())
            .build();

        let input = SafeRefactorInput {
            action: RefactorAction::ChangeSignature,
            target: "some_func".to_string(),
            params: Some(serde_json::json!({
                "new_parameters": "not_an_array" // Should be array
            })),
        };

        let result = handle_safe_refactor(&ctx, input).await;
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(matches!(err, HandlerError::InvalidInput(_)));
    }

    // =========================================================================
    // handle_safe_refactor Tests - Move Action
    // =========================================================================

    #[tokio::test]
    async fn test_handle_safe_refactor_move_valid() {
        let tempdir = tempfile::tempdir().unwrap();
        let tempdir_path = tempdir.path();

        // Create source and target files
        let source_file = tempdir_path.join("source.rs");
        let target_file = tempdir_path.join("target.rs");
        std::fs::write(&source_file, "pub struct MyStruct {}\n").unwrap();
        std::fs::write(&target_file, "// target file\n").unwrap();

        let ctx = HandlerContext::builder()
            .with_working_dir(tempdir_path.to_path_buf())
            .build();

        let input = SafeRefactorInput {
            action: RefactorAction::Move,
            target: "MyStruct".to_string(),
            params: Some(serde_json::json!({
                "source_path": source_file.to_str().unwrap(),
                "target_path": target_file.to_str().unwrap()
            })),
        };

        let result = handle_safe_refactor(&ctx, input).await;
        assert!(result.is_ok());
        let output = result.unwrap();
        assert_eq!(output.action, RefactorAction::Move);
    }

    #[tokio::test]
    async fn test_handle_safe_refactor_move_missing_source_path() {
        let tempdir = tempfile::tempdir().unwrap();
        let tempdir_path = tempdir.path();

        let ctx = HandlerContext::builder()
            .with_working_dir(tempdir_path.to_path_buf())
            .build();

        let input = SafeRefactorInput {
            action: RefactorAction::Move,
            target: "SomeStruct".to_string(),
            params: Some(serde_json::json!({
                // Missing source_path
                "target_path": "/some/path.rs"
            })),
        };

        let result = handle_safe_refactor(&ctx, input).await;
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(matches!(err, HandlerError::InvalidInput(_)));
        let error_msg = match err {
            HandlerError::InvalidInput(msg) => msg,
            _ => String::new(),
        };
        assert!(error_msg.contains("source_path"));
    }

    #[tokio::test]
    async fn test_handle_safe_refactor_move_missing_target_path() {
        let tempdir = tempfile::tempdir().unwrap();
        let tempdir_path = tempdir.path();

        let ctx = HandlerContext::builder()
            .with_working_dir(tempdir_path.to_path_buf())
            .build();

        let input = SafeRefactorInput {
            action: RefactorAction::Move,
            target: "SomeStruct".to_string(),
            params: Some(serde_json::json!({
                "source_path": "/some/source.rs"
                // Missing target_path
            })),
        };

        let result = handle_safe_refactor(&ctx, input).await;
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(matches!(err, HandlerError::InvalidInput(_)));
        let error_msg = match err {
            HandlerError::InvalidInput(msg) => msg,
            _ => String::new(),
        };
        assert!(error_msg.contains("target_path"));
    }

    // =========================================================================
    // Error Cases - Validation Failures
    // =========================================================================

    /// A refactor target that is not an identifier must be refused
    /// with a reason, and must never report success or report edits.
    ///
    /// This used to assert `result.is_ok() || result.is_err()`, which
    /// is true for every `Result` — the SQL-injection probe was
    /// asserting that the function returned something. What it
    /// actually does: `ctx.validator.validate_query` only checks the
    /// query *length*, so the payload passes validation untouched; the
    /// rename then resolves `working_dir/<payload>` as a file path,
    /// fails to read it, and surfaces as `success: false` with the
    /// I/O reason. That is safe — nothing interprets the string, and
    /// CogniCode has no SQL layer — but it is safe by accident of the
    /// payload naming a nonexistent file, not by validation.
    #[tokio::test]
    async fn test_handle_safe_refactor_invalid_query() {
        let tempdir = tempfile::tempdir().unwrap();
        let tempdir_path = tempdir.path();

        let ctx = HandlerContext::builder()
            .with_working_dir(tempdir_path.to_path_buf())
            .build();

        let payload = "'; DROP TABLE users;--";
        let input = SafeRefactorInput {
            action: RefactorAction::Rename,
            target: payload.to_string(),
            params: Some(serde_json::json!({
                "new_name": "safe_name"
            })),
        };

        let output = handle_safe_refactor(&ctx, input)
            .await
            .expect("a SQL-injection-shaped target must be answered, not error the whole call");

        assert!(
            !output.success,
            "a refactor whose target names no existing symbol must not \
             report success; got {output:?}"
        );
        let reason = output
            .error_message
            .expect("a failed refactor must carry an error_message");
        assert!(
            reason.contains("Failed to read"),
            "the refusal must come from resolving the target, not from \
             silently accepting it; got: {reason}"
        );
        assert!(
            output.changes.is_empty(),
            "a refused refactor must not report edits; got {:?}",
            output.changes
        );
        assert!(
            !output.validation_result.is_valid && !output.validation_result.errors.is_empty(),
            "validation_result must record the failure; got {:?}",
            output.validation_result
        );

        // Nothing on disk may have been created for the payload name.
        assert!(
            !tempdir_path.join(payload).exists(),
            "the payload must never have been treated as a real path"
        );
    }

    /// An empty target is the same class of input: it must be refused
    /// with a reason, not resolved against the workspace root and
    /// reported as a successful rename of nothing.
    #[tokio::test]
    async fn test_handle_safe_refactor_empty_target() {
        let tempdir = tempfile::tempdir().unwrap();
        let tempdir_path = tempdir.path();

        let ctx = HandlerContext::builder()
            .with_working_dir(tempdir_path.to_path_buf())
            .build();

        let input = SafeRefactorInput {
            action: RefactorAction::Rename,
            target: "".to_string(),
            params: Some(serde_json::json!({
                "new_name": "new_name"
            })),
        };

        let output = handle_safe_refactor(&ctx, input)
            .await
            .expect("an empty target must be answered, not error the whole call");

        assert!(
            !output.success,
            "an empty refactor target must not report success; got {output:?}"
        );
        assert!(
            output.error_message.is_some(),
            "an empty target must carry an error_message; got {output:?}"
        );
        assert!(
            output.changes.is_empty(),
            "a refused refactor must not report edits; got {:?}",
            output.changes
        );
        assert!(
            !output.validation_result.is_valid,
            "validation_result must be invalid; got {:?}",
            output.validation_result
        );
    }

    // =========================================================================
    // handle_validate_syntax Tests
    // =========================================================================

    #[tokio::test]
    async fn test_handle_validate_syntax_valid_rust_file() {
        let tempdir = tempfile::tempdir().unwrap();
        let tempdir_path = tempdir.path();

        let rust_file = tempdir_path.join("valid.rs");
        std::fs::write(
            &rust_file,
            r#"
fn main() {
    println!("Hello, world!");
}
"#,
        )
        .unwrap();

        let ctx = HandlerContext::builder()
            .with_working_dir(tempdir_path.to_path_buf())
            .build();

        let input = ValidateSyntaxInput {
            file_path: rust_file.to_str().unwrap().to_string(),
        };

        let result = handle_validate_syntax(&ctx, input).await;
        assert!(result.is_ok());
        let output = result.unwrap();
        assert_eq!(output.file_path, rust_file.to_str().unwrap());
    }

    #[tokio::test]
    async fn test_handle_validate_syntax_invalid_rust_file() {
        let tempdir = tempfile::tempdir().unwrap();
        let tempdir_path = tempdir.path();

        let rust_file = tempdir_path.join("invalid.rs");
        // Write invalid Rust code (missing closing brace)
        std::fs::write(&rust_file, "fn main() { println!(\"Hello\"); ").unwrap();

        let ctx = HandlerContext::builder()
            .with_working_dir(tempdir_path.to_path_buf())
            .build();

        let input = ValidateSyntaxInput {
            file_path: rust_file.to_str().unwrap().to_string(),
        };

        let result = handle_validate_syntax(&ctx, input).await;
        assert!(result.is_ok());
        let output = result.unwrap();
        assert!(!output.is_valid);
        assert!(!output.errors.is_empty());
    }

    #[tokio::test]
    async fn test_handle_validate_syntax_nonexistent_file() {
        let tempdir = tempfile::tempdir().unwrap();
        let tempdir_path = tempdir.path();

        let ctx = HandlerContext::builder()
            .with_working_dir(tempdir_path.to_path_buf())
            .build();

        let input = ValidateSyntaxInput {
            file_path: tempdir_path
                .join("nonexistent.rs")
                .to_str()
                .unwrap()
                .to_string(),
        };

        let result = handle_validate_syntax(&ctx, input).await;
        // Should return Ok with is_valid=false for nonexistent file
        assert!(result.is_ok());
        let output = result.unwrap();
        assert!(!output.is_valid);
    }

    #[tokio::test]
    async fn test_handle_validate_syntax_python_file() {
        let tempdir = tempfile::tempdir().unwrap();
        let tempdir_path = tempdir.path();

        let py_file = tempdir_path.join("valid.py");
        std::fs::write(
            &py_file,
            r#"
def main():
    print("Hello, world!")

main()
"#,
        )
        .unwrap();

        let ctx = HandlerContext::builder()
            .with_working_dir(tempdir_path.to_path_buf())
            .build();

        let input = ValidateSyntaxInput {
            file_path: py_file.to_str().unwrap().to_string(),
        };

        let result = handle_validate_syntax(&ctx, input).await;
        assert!(result.is_ok());
        let output = result.unwrap();
        // Python files should be valid
        assert!(output.is_valid);
    }
}

/// Which side of the line a preview failure falls on.
///
/// Five of `PreviewError`'s seven variants describe a plan the caller has to
/// fix: a file outside the workspace, a missing file, a symlink escape, two
/// overlapping edits, a range past end-of-file. Reporting those as `Internal`
/// tells the operator their server is broken when the request is what is —
/// and per the project contract an `Internal` answer must never stand in for
/// a clean one. The other two — no usable workspace root, and a read that
/// failed for a reason we did not classify — genuinely are this side's
/// problem, so they stay `Internal`.
///
/// The split is exhaustive on purpose: a new variant fails to compile here
/// rather than silently inheriting one of the two answers.
fn classify_preview_error(e: crate::refactor::preview::PreviewError) -> HandlerError {
    use crate::refactor::preview::PreviewError;
    match e {
        PreviewError::PathEscapesWorkspace(_)
        | PreviewError::AffectedFileMissing(_)
        | PreviewError::SymlinkEscape(_)
        | PreviewError::OverlappingEdits { .. }
        | PreviewError::RangeOutOfBounds { .. } => {
            HandlerError::InvalidInput(format!("refactor plan rejected: {e}"))
        }
        PreviewError::InvalidWorkspaceRoot(_) | PreviewError::IoError(_) => {
            HandlerError::Internal(format!("refactor preview failed: {e}"))
        }
    }
}

/// `safe_refactor_preview`: render a `RefactorPlan` as a deterministic,
/// non-mutating preview with a unified diff per file.
///
/// MEDIDO 2026-10-18. This capability has existed in `refactor::preview`
/// since B5.W2, fully implemented and tested, with no caller outside tests:
/// it was announced in `product/tools.json` and had no dispatch arm, so a
/// client that trusted `tools/list` got `ToolNotFound`. The gap was not the
/// preview — it was the wire.
///
/// The handler is read-only by construction: `render_preview` writes nothing
/// (pinned by `preview_does_not_modify_files`), and the input carries no
/// authorization field, so a `--read-only` session can serve it.
#[cognicode_macros::aix_tool(
    name = "safe_refactor_preview",
    description = "Render a deterministic, non-mutating preview of a RefactorPlan.",
    input_schema = SafeRefactorPreviewInput
)]
pub async fn handle_safe_refactor_preview(
    ctx: &HandlerContext,
    input: SafeRefactorPreviewInput,
) -> HandlerResult<crate::refactor::preview::RefactorPreview> {
    // Exactly one source for the plan. Falling back from a missing `plan_json`
    // to an empty plan, or preferring `plan_path` when the caller sent the
    // other, would answer with a preview of something nobody asked for.
    let plan = match (input.plan_path, input.plan_json) {
        (Some(_), Some(_)) => {
            return Err(HandlerError::InvalidInput(
                "safe_refactor_preview takes `plan_path` or `plan_json`, not both".to_string(),
            ));
        }
        (Some(path), None) => {
            let raw = std::path::Path::new(&path);
            let absolute = if raw.is_absolute() {
                raw.to_path_buf()
            } else {
                ctx.working_dir.join(raw)
            };
            ctx.validator.validate_path(&absolute)?;
            crate::refactor::plan::load_plan_from_file(&absolute).map_err(|e| match e {
                // Every variant here is the caller's file being wrong —
                // absent, unreadable, malformed, empty, or edited since it was
                // written. None of them is a fault of this server, and calling
                // one `Internal` would hand the caller a fault report for
                // their own typo.
                crate::refactor::plan::PlanIoError::Io(ref io)
                    if io.kind() == std::io::ErrorKind::NotFound =>
                {
                    HandlerError::NotFound(format!("no RefactorPlan at {}", absolute.display()))
                }
                other => HandlerError::InvalidInput(format!("refactor plan unusable: {other}")),
            })?
        }
        (None, Some(raw)) => serde_json::from_str::<crate::refactor::plan::RefactorPlan>(
            raw.as_str(),
        )
        .map_err(|e| HandlerError::InvalidInput(format!("invalid RefactorPlan JSON: {e}")))?,
        (None, None) => {
            return Err(HandlerError::InvalidInput(
                "safe_refactor_preview needs either `plan_path` or `plan_json`".to_string(),
            ));
        }
    };

    // The workspace is the session's anchor, not a caller-supplied path: a
    // preview reads files to render diffs, and the caller choosing which
    // directory that is would put the read boundary in the wrong hands.
    crate::refactor::preview::render_preview(&plan, &ctx.working_dir)
        .map_err(classify_preview_error)
}
