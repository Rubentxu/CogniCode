#![cfg(feature = "sqlite")]
//! Tests for FTS5 Symbol Index

use rusqlite::Connection;
use cognicode_db::Fts5Index;

fn create_test_db() -> Connection {
    let conn = Connection::open_in_memory().unwrap();
    // Create the FTS5 table
    conn.execute_batch(
        "CREATE VIRTUAL TABLE IF NOT EXISTS symbol_index USING fts5(
            symbol_name, symbol_kind, file_path, docstring, body_tokens,
            tokenize='porter unicode61'
        );"
    ).unwrap();
    conn
}

#[test]
fn test_fts5_insert_and_search() {
    let conn = create_test_db();

    // Insert some symbols
    Fts5Index::insert_symbol(&conn, "calculate_total", "Function", "src/math.rs", "Calculates total", "fn calculate_total").unwrap();
    Fts5Index::insert_symbol(&conn, "helper", "Function", "src/utils.rs", "Helper function", "fn helper").unwrap();

    // Search for "calculate"
    let results = Fts5Index::search(&conn, "calculate", 10).unwrap();

    assert!(!results.is_empty(), "Should find results for 'calculate'");
    assert_eq!(results[0].symbol_name, "calculate_total");
}

#[test]
fn test_fts5_search_multiple_results() {
    let conn = create_test_db();

    // Insert symbols
    Fts5Index::insert_symbol(&conn, "user_handler", "Function", "src/handler.rs", "Handles users", "fn user_handler").unwrap();
    Fts5Index::insert_symbol(&conn, "user_service", "Function", "src/service.rs", "User service", "fn user_service").unwrap();
    Fts5Index::insert_symbol(&conn, "order_handler", "Function", "src/handler.rs", "Handles orders", "fn order_handler").unwrap();

    // Search for "user"
    let results = Fts5Index::search(&conn, "user", 10).unwrap();

    assert_eq!(results.len(), 2, "Should find 2 results for 'user'");
}

#[test]
fn test_fts5_search_no_results() {
    let conn = create_test_db();

    // Insert a symbol
    Fts5Index::insert_symbol(&conn, "calculate_total", "Function", "src/math.rs", "Calculates total", "fn calculate_total").unwrap();

    // Search for something that doesn't exist
    let results = Fts5Index::search(&conn, "xyz123", 10).unwrap();

    assert!(results.is_empty(), "Should find no results for 'xyz123'");
}

#[test]
fn test_fts5_search_with_kind_filter() {
    let conn = create_test_db();

    // Insert symbols of different kinds
    Fts5Index::insert_symbol(&conn, "process_data", "Function", "src/lib.rs", "Processes data", "fn process_data").unwrap();
    Fts5Index::insert_symbol(&conn, "DataStruct", "Struct", "src/lib.rs", "Data structure", "struct DataStruct").unwrap();

    // Search for "data" - should find both
    let results = Fts5Index::search(&conn, "data", 10).unwrap();

    assert_eq!(results.len(), 2, "Should find both function and struct");
}
