//! Common DTOs - Shared types for application layer
//!
//! These types are transport-neutral and can be used by any interface
//! (MCP, REST, gRPC, etc.) without coupling to a specific protocol.

use crate::domain::value_objects::Location;
use serde::{Deserialize, Serialize};

/// Represents a location in source code (1-indexed for display)
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SourceLocation {
    pub file: String,
    pub line: u32,
    pub column: u32,
}

impl From<&Location> for SourceLocation {
    /// Converts from domain Location (zero-indexed) to DTO SourceLocation (1-indexed for display)
    fn from(loc: &Location) -> Self {
        Self {
            file: loc.file().to_string(),
            line: loc.line() + 1,
            column: loc.column() + 1,
        }
    }
}

/// Metadata for analysis operations
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AnalysisMetadata {
    pub total_calls: usize,
    pub analysis_time_ms: u64,
}

/// Risk level for impact analysis
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum RiskLevel {
    Low,
    Medium,
    High,
    Critical,
}

/// Kind of symbol in source code
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum SymbolKind {
    Module,
    Class,
    Struct,
    Enum,
    Trait,
    Function,
    Method,
    Field,
    Variable,
    Constant,
    Constructor,
    Interface,
    TypeAlias,
    Parameter,
    Unknown,
}

/// Summary of a symbol for display purposes
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SymbolSummary {
    pub name: String,
    pub kind: SymbolKind,
    pub location: SourceLocation,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub signature: Option<String>,
}

impl SymbolSummary {
    /// Creates a new SymbolSummary
    pub fn new(name: String, kind: SymbolKind, location: SourceLocation) -> Self {
        Self {
            name,
            kind,
            location,
            signature: None,
        }
    }

    /// Creates a SymbolSummary with a signature
    pub fn with_signature(mut self, signature: Option<String>) -> Self {
        self.signature = signature;
        self
    }
}
