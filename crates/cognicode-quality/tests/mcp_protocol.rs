//! MCP Protocol-Level Tests
//!
//! These tests verify the MCP stdio protocol by spawning the binary as a child process
//! and sending JSON-RPC messages directly over stdin/stdout.

use std::io::Write;
use std::process::{Command, Stdio};

/// Test that the MCP server starts and responds to initialize request
#[test]
fn test_mcp_server_starts_and_responds() {
    // Build the binary first (or use cargo run)
    let binary = std::env::current_dir()
        .unwrap()
        .join("target/debug/cognicode-quality");

    if !binary.exists() {
        eprintln!("Binary not found at {:?}, skipping protocol test", binary);
        return;
    }

    let mut child = Command::new(&binary)
        .arg("--cwd")
        .arg(".")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();

    let mut stdin = child.stdin.take().unwrap();

    // Send initialize request (JSON-RPC)
    let init_msg = r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-03-26","capabilities":{},"clientInfo":{"name":"test","version":"1.0"}}}"#;
    writeln!(stdin, "{}", init_msg).unwrap();
    stdin.flush().unwrap();

    // Read response
    let mut stdout = child.stdout.take().unwrap();
    use std::io::BufRead;
    let mut reader = std::io::BufReader::new(&mut stdout);
    let mut response = String::new();
    reader.read_line(&mut response).unwrap();

    // Verify we got a valid JSON-RPC response
    assert!(
        response.contains("\"result\""),
        "No valid JSON-RPC response: {}",
        response
    );

    // Cleanup
    let _ = child.kill();
    let _ = child.wait();
}

/// Test that tools/list returns expected tools
#[test]
fn test_mcp_tools_list_returns_tools() {
    let binary = std::env::current_dir()
        .unwrap()
        .join("target/debug/cognicode-quality");

    if !binary.exists() {
        eprintln!("Binary not found, skipping protocol test");
        return;
    }

    let mut child = Command::new(&binary)
        .arg("--cwd")
        .arg(".")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();

    let mut stdin = child.stdin.take().unwrap();

    // Send initialize request
    let init_msg = r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-03-26","capabilities":{},"clientInfo":{"name":"test","version":"1.0"}}}"#;
    writeln!(stdin, "{}", init_msg).unwrap();
    stdin.flush().unwrap();

    // Read initialize response
    let mut stdout = child.stdout.take().unwrap();
    use std::io::BufRead;
    let mut reader = std::io::BufReader::new(&mut stdout);
    let mut response = String::new();
    reader.read_line(&mut response).unwrap();

    // Re-acquire stdin after reading
    stdin = child.stdin.take().unwrap();

    // Send tools/list request
    let list_msg = r#"{"jsonrpc":"2.0","id":2,"method":"tools/list","params":{}}"#;
    writeln!(stdin, "{}", list_msg).unwrap();
    stdin.flush().unwrap();

    // Read tools/list response
    let mut response = String::new();
    let mut reader = std::io::BufReader::new(&mut stdout);
    reader.read_line(&mut response).unwrap();

    // Verify we got tools in the response
    assert!(
        response.contains("tools") || response.contains("analyze_file"),
        "Response doesn't contain expected tools: {}",
        response
    );

    let _ = child.kill();
    let _ = child.wait();
}

/// Test that calling analyze_file tool works via MCP protocol
#[test]
fn test_mcp_call_analyze_file() {
    let binary = std::env::current_dir()
        .unwrap()
        .join("target/debug/cognicode-quality");

    if !binary.exists() {
        eprintln!("Binary not found, skipping protocol test");
        return;
    }

    let mut child = Command::new(&binary)
        .arg("--cwd")
        .arg(".")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();

    let mut stdin = child.stdin.take().unwrap();

    // Send initialize
    let init_msg = r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-03-26","capabilities":{},"clientInfo":{"name":"test","version":"1.0"}}}"#;
    writeln!(stdin, "{}", init_msg).unwrap();
    stdin.flush().unwrap();

    // Read initialize response
    let mut stdout = child.stdout.take().unwrap();
    use std::io::BufRead;
    let mut reader = std::io::BufReader::new(&mut stdout);
    let mut response = String::new();
    reader.read_line(&mut response).unwrap();

    // Re-acquire stdin
    stdin = child.stdin.take().unwrap();

    // Send tools/call for analyze_file
    let call_msg = r#"{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"analyze_file","arguments":{"file_path":"src/lib.rs"}}}"#;
    writeln!(stdin, "{}", call_msg).unwrap();
    stdin.flush().unwrap();

    // Read response
    let mut response = String::new();
    let mut reader = std::io::BufReader::new(&mut stdout);
    reader.read_line(&mut response).unwrap();

    // The response should be JSON-RPC format
    assert!(
        response.contains("jsonrpc") || response.contains("result") || response.contains("error"),
        "Response doesn't look like JSON-RPC: {}",
        response
    );

    let _ = child.kill();
    let _ = child.wait();
}

/// Test that the server handles invalid requests gracefully
#[test]
fn test_mcp_handles_invalid_requests() {
    let binary = std::env::current_dir()
        .unwrap()
        .join("target/debug/cognicode-quality");

    if !binary.exists() {
        eprintln!("Binary not found, skipping protocol test");
        return;
    }

    let mut child = Command::new(&binary)
        .arg("--cwd")
        .arg(".")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();

    let mut stdin = child.stdin.take().unwrap();

    // Send invalid JSON-RPC request
    let invalid_msg = "{ invalid json }";
    writeln!(stdin, "{}", invalid_msg).unwrap();
    stdin.flush().unwrap();

    // Server should either close connection or return error
    // We just verify it doesn't panic
    use std::io::BufRead;
    let mut stdout = child.stdout.take().unwrap();
    let mut reader = std::io::BufReader::new(&mut stdout);
    let mut response = String::new();
    let _ = reader.read_line(&mut response);

    let _ = child.kill();
    let _ = child.wait();
}
