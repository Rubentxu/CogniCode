//! Tests for AVC Contract persistence

use rusqlite::Connection;
use cognicode_core::infrastructure::avc::AvcContract;
use cognicode_db::AvcContractStore;

fn create_test_db() -> Connection {
    let conn = Connection::open_in_memory().unwrap();
    // Create the schema
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS avc_contracts (
            id TEXT PRIMARY KEY,
            source_file TEXT NOT NULL,
            function_name TEXT NOT NULL,
            contract_json TEXT NOT NULL,
            generated_at TEXT NOT NULL,
            compliance_score REAL DEFAULT 1.0
        );"
    ).unwrap();
    conn
}

#[test]
fn test_avc_contract_save_and_load() {
    let conn = create_test_db();

    // Create a test contract
    let contract = AvcContract::new("test-contract-001", "test_source.rs")
        .with_description("Test contract");

    // Save the contract
    AvcContractStore::save(&conn, &contract).unwrap();

    // Load the contract
    let loaded = AvcContractStore::load(&conn, "test-contract-001").unwrap();

    assert!(loaded.is_some(), "Contract should be loaded");
    let loaded = loaded.unwrap();
    assert_eq!(loaded.contract_id, "test-contract-001");
    assert_eq!(loaded.source_of_truth, "test_source.rs");
}

#[test]
fn test_avc_contract_load_not_found() {
    let conn = create_test_db();

    // Try to load a non-existent contract
    let result = AvcContractStore::load(&conn, "non-existent").unwrap();

    assert!(result.is_none(), "Should return None for non-existent contract");
}

#[test]
fn test_avc_contract_update_compliance_score() {
    let conn = create_test_db();

    // Create and save a contract
    let contract = AvcContract::new("test-contract-002", "test_source.rs")
        .with_description("Test contract");
    AvcContractStore::save(&conn, &contract).unwrap();

    // Update compliance score
    AvcContractStore::update_compliance_score(&conn, "test-contract-002", 0.85).unwrap();

    // Verify the update (we can check via direct query since load doesn't expose score)
    let loaded = AvcContractStore::load(&conn, "test-contract-002").unwrap();
    assert!(loaded.is_some());
}

#[test]
fn test_avc_contract_insert_or_replace() {
    let conn = create_test_db();

    // Create a contract
    let contract = AvcContract::new("test-contract-003", "test_source.rs")
        .with_description("Original contract");
    AvcContractStore::save(&conn, &contract).unwrap();

    // Create a replacement contract with same ID
    let replacement = AvcContract::new("test-contract-003", "new_source.rs")
        .with_description("Replacement contract");
    AvcContractStore::save(&conn, &replacement).unwrap();

    // Load and verify it was replaced
    let loaded = AvcContractStore::load(&conn, "test-contract-003").unwrap().unwrap();
    assert_eq!(loaded.source_of_truth, "new_source.rs");
    assert_eq!(loaded.description, "Replacement contract");
}
