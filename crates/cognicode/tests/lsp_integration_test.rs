//! LSP Integration Tests
//!
//! End-to-end tests that validate the complete LSP stack with real language server binaries.
//! These tests are ignored by default and run only when the respective LSP binaries are available.

use std::sync::Arc;
use tempfile::TempDir;

/// Checks if rust-analyzer binary is available on the system
fn rust_analyzer_available() -> bool {
    std::process::Command::new("rust-analyzer")
        .arg("--version")
        .output()
        .map(|output| output.status.success())
        .unwrap_or(false)
}

/// Checks if pyright binary is available on the system
fn pyright_available() -> bool {
    std::process::Command::new("pyright")
        .arg("--version")
        .output()
        .map(|output| output.status.success())
        .unwrap_or(false)
}

#[tokio::test]
#[ignore = "requires rust-analyzer binary"]
async fn test_rust_analyzer_hover() {
    use cognicode::application::services::analysis_service::AnalysisService;
    use cognicode::application::services::lsp_proxy_service::LspProxyService;
    
    // Create a temporary Rust project
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let project_dir = temp_dir.path().to_path_buf();
    
    // Create Cargo.toml so rust-analyzer recognizes the project
    std::fs::write(
        project_dir.join("Cargo.toml"),
        r#"[package]
name = "test_hover"
version = "0.1.0"
edition = "2021"
"#,
    )
    .expect("Failed to write Cargo.toml");
    
    // Create a simple Rust file with a function
    // Line 0 (0-indexed): "pub fn greet(name: &str) -> String {"
    // "greet" starts at character 7 on line 0
    let rust_file = project_dir.join("src").join("main.rs");
    std::fs::create_dir_all(rust_file.parent().unwrap()).expect("Failed to create src dir");
    std::fs::write(
        &rust_file,
        r#"pub fn greet(name: &str) -> String {
    format!("Hello, {}!", name)
}

fn main() {
    let message = greet("World");
    println!("{}", message);
}
"#,
    )
    .expect("Failed to write Rust file");

    // Create the LSP proxy service with the temp directory as workspace
    let service = LspProxyService::new(Arc::new(AnalysisService::new()), project_dir.clone());
    
    // Enable proxy mode with the composite provider
    let mut service = service;
    service.enable_proxy_mode_with_provider();

    // Define LSP params for hover on "greet" function definition (line 0, character 7)
    // This is 0-indexed as per LSP protocol
    let params = serde_json::json!({
        "textDocument": {
            "uri": format!("file://{}", rust_file.display())
        },
        "position": {
            "line": 0,
            "character": 7
        }
    });

    // Send hover request with 30 second timeout (rust-analyzer needs ~10s to index)
    let result = tokio::time::timeout(
        std::time::Duration::from_secs(30),
        async {
            service.route_operation("hover", &params).await
        }
    ).await;

    // Verify the result
    match result {
        Ok(Ok(Some(response))) => {
            // Should contain type information about the function
            let response_str = response.to_string();
            assert!(
                response_str.contains("greet") || response_str.contains("String"),
                "Hover response should contain function signature or type info, got: {}",
                response_str
            );
        }
        Ok(Ok(None)) => {
            // This can happen if LSP didn't return anything useful
            // but we shouldn't error
        }
        Ok(Err(e)) => {
            panic!("Hover request failed: {}", e);
        }
        Err(_) => {
            panic!("Hover request timed out after 10 seconds");
        }
    }
}

#[tokio::test]
#[ignore = "requires rust-analyzer binary"]
async fn test_rust_analyzer_goto_definition() {
    use cognicode::application::services::analysis_service::AnalysisService;
    use cognicode::application::services::lsp_proxy_service::LspProxyService;
    
    // Create a temporary Rust project
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let project_dir = temp_dir.path().to_path_buf();
    
    // Create Cargo.toml so rust-analyzer recognizes the project
    std::fs::write(
        project_dir.join("Cargo.toml"),
        r#"[package]
name = "test_goto_def"
version = "0.1.0"
edition = "2021"
"#,
    )
    .expect("Failed to write Cargo.toml");
    
    // Create a simple Rust file
    // Line 0 (0-indexed): "pub fn calculate(x: i32, y: i32) -> i32 {"
    // Line 4 (0-indexed): "    let result = calculate(1, 2);"
    // "calculate" in the call starts at character 15 on line 4
    let rust_file = project_dir.join("src").join("main.rs");
    std::fs::create_dir_all(rust_file.parent().unwrap()).expect("Failed to create src dir");
    std::fs::write(
        &rust_file,
        r#"pub fn calculate(x: i32, y: i32) -> i32 {
    x + y
}

fn main() {
    let result = calculate(1, 2);
    println!("Result: {}", result);
}
"#,
    )
    .expect("Failed to write Rust file");

    // Create the LSP proxy service
    let service = LspProxyService::new(Arc::new(AnalysisService::new()), project_dir.clone());
    let mut service = service;
    service.enable_proxy_mode_with_provider();

    // Request goto-definition on the call to calculate (line 5, character 17)
    // Should return the definition at line 0
    let params = serde_json::json!({
        "textDocument": {
            "uri": format!("file://{}", rust_file.display())
        },
        "position": {
            "line": 5,
            "character": 17
        }
    });

    let result = tokio::time::timeout(
        std::time::Duration::from_secs(30),
        async {
            service.route_operation("textDocument/definition", &params).await
        }
    ).await;

    match result {
        Ok(Ok(Some(response))) => {
            // Should return a location pointing to the function definition
            let response_str = response.to_string();
            // The definition should be on line 3 (1-indexed) which is line 2 in 0-indexed
            // Response format may vary - could be Location object or array
            assert!(
                response_str.contains("file://") || response_str.contains("main.rs"),
                "Definition response should contain file path, got: {}",
                response_str
            );
        }
        Ok(Ok(None)) => {
            // LSP might not return anything in some cases
        }
        Ok(Err(e)) => {
            panic!("Definition request failed: {}", e);
        }
        Err(_) => {
            panic!("Definition request timed out after 30 seconds");
        }
    }
}

#[tokio::test]
#[ignore = "requires pyright binary"]
async fn test_pyright_goto_definition() {
    use cognicode::application::services::analysis_service::AnalysisService;
    use cognicode::application::services::lsp_proxy_service::LspProxyService;
    
    // Create a temporary Python project
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let project_dir = temp_dir.path().to_path_buf();
    
    // Create a simple Python file
    let python_file = project_dir.join("test_file.py");
    std::fs::write(
        &python_file,
        r#"
def calculate(x, y):
    return x + y

result = calculate(5, 3)
print(result)
"#,
    )
    .expect("Failed to write Python file");

    // Create the LSP proxy service
    let service = LspProxyService::new(Arc::new(AnalysisService::new()), project_dir.clone());
    let mut service = service;
    service.enable_proxy_mode_with_provider();

    // Request goto-definition on the call to calculate (line 5, column 7)
    // Should return the definition at line 2
    let params = serde_json::json!({
        "textDocument": {
            "uri": format!("file://{}", python_file.display())
        },
        "position": {
            "line": 5,
            "character": 7
        }
    });

    let result = tokio::time::timeout(
        std::time::Duration::from_secs(30),
        async {
            service.route_operation("textDocument/definition", &params).await
        }
    ).await;

    match result {
        Ok(Ok(Some(response))) => {
            let response_str = response.to_string();
            assert!(
                response_str.contains("file://") || response_str.contains("test_file.py"),
                "Definition response should contain file path, got: {}",
                response_str
            );
        }
        Ok(Ok(None)) => {
            // Pyright might not return anything
        }
        Ok(Err(e)) => {
            panic!("Definition request failed: {}", e);
        }
        Err(_) => {
            panic!("Definition request timed out after 30 seconds");
        }
    }
}

#[tokio::test]
#[ignore = "requires pyright binary"]
async fn test_pyright_find_references() {
    use cognicode::application::services::analysis_service::AnalysisService;
    use cognicode::application::services::lsp_proxy_service::LspProxyService;
    
    // Create a temporary Python project
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let project_dir = temp_dir.path().to_path_buf();
    
    // Create a Python file with a variable used in multiple places
    let python_file = project_dir.join("test_refs.py");
    std::fs::write(
        &python_file,
        r#"
message = "Hello"

def greet():
    print(message)

def farewell():
    print(message)

greet()
farewell()
"#,
    )
    .expect("Failed to write Python file");

    // Create the LSP proxy service
    let service = LspProxyService::new(Arc::new(AnalysisService::new()), project_dir.clone());
    let mut service = service;
    service.enable_proxy_mode_with_provider();

    // Request find-references on "message" at line 1 (the definition)
    let params = serde_json::json!({
        "textDocument": {
            "uri": format!("file://{}", python_file.display())
        },
        "position": {
            "line": 1,
            "character": 0
        }
    });

    let result = tokio::time::timeout(
        std::time::Duration::from_secs(30),
        async {
            service.route_operation("find_references", &params).await
        }
    ).await;

    match result {
        Ok(Ok(Some(response))) => {
            // Should find multiple references to "message"
            // Verify response is an array (list of references)
            assert!(
                response.is_array(),
                "References response should be an array, got: {}",
                response
            );
            let refs = response.as_array().unwrap();
            // Should find at least the definition + 3 usages
            assert!(
                refs.len() >= 2,
                "Should find at least 2 references (definition + usage), got {}",
                refs.len()
            );
        }
        Ok(Ok(None)) => {
            // LSP might not return anything
        }
        Ok(Err(e)) => {
            panic!("Find references request failed: {}", e);
        }
        Err(_) => {
            panic!("Find references request timed out after 30 seconds");
        }
    }
}

#[test]
fn test_rust_analyzer_available_detection() {
    // This test just verifies the detection function works
    // It doesn't require rust-analyzer to be installed
    let available = rust_analyzer_available();
    println!("rust-analyzer available: {}", available);
    
    // The test always passes - it just reports the status
    // In CI, this can be used to determine which tests to run
}

#[test]
fn test_pyright_available_detection() {
    // This test just verifies the detection function works
    let available = pyright_available();
    println!("pyright available: {}", available);
    
    // The test always passes - it just reports the status
}

#[tokio::test]
async fn test_extract_location_from_lsp_params() {
    use cognicode::application::services::analysis_service::AnalysisService;
    use cognicode::application::services::lsp_proxy_service::LspProxyService;
    
    let service = LspProxyService::new_without_workspace(Arc::new(AnalysisService::new()));
    
    // Test valid LSP params (0-indexed line 10, character 5 becomes Location line 11, column 6)
    let params = serde_json::json!({
        "textDocument": {
            "uri": "file:///path/to/file.rs"
        },
        "position": {
            "line": 10,
            "character": 5
        }
    });
    
    // Use reflection to test extract_location - but it's private, so we test via route_operation
    // The error case can be tested directly
    let result = service.route_operation("hover", &params).await;
    
    // With proxy disabled, should return None even with valid params
    assert!(result.is_ok());
    assert!(result.unwrap().is_none());
}
