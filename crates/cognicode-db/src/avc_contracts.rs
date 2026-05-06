//! AVC Contract persistence layer
//!
//! Provides SQLite persistence for AVC (Agent-Generated Code Validation) contracts.

use rusqlite::{Connection, params};
use anyhow::Result;
use cognicode_core::infrastructure::avc::AvcContract;

/// Store for AVC contract persistence
pub struct AvcContractStore;

impl AvcContractStore {
    /// Save contract via INSERT OR REPLACE; failures logged, not propagated
    pub fn save(conn: &Connection, contract: &AvcContract) -> Result<()> {
        let json = serde_json::to_string(contract)?;
        conn.execute(
            "INSERT OR REPLACE INTO avc_contracts (id, source_file, function_name, contract_json, generated_at, compliance_score) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                contract.contract_id,
                contract.source_of_truth,
                "", // function_name extracted from contract if needed
                json,
                chrono::Utc::now().to_rfc3339(),
                1.0
            ],
        )?;
        Ok(())
    }

    /// Load contract by contract_id; returns None if not found
    pub fn load(conn: &Connection, contract_id: &str) -> Result<Option<AvcContract>> {
        let result = conn.query_row(
            "SELECT contract_json FROM avc_contracts WHERE id = ?1",
            params![contract_id],
            |row| row.get::<_, String>(0),
        );
        match result {
            Ok(json) => Ok(Some(serde_json::from_str(&json)?)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    /// Update compliance_score for existing contract
    pub fn update_compliance_score(conn: &Connection, contract_id: &str, score: f64) -> Result<()> {
        conn.execute(
            "UPDATE avc_contracts SET compliance_score = ?1 WHERE id = ?2",
            params![score, contract_id],
        )?;
        Ok(())
    }
}
