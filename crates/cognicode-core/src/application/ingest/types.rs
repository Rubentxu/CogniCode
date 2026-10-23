//! Ingest pipeline types — domain types used by the Scan → Extract →
//! PgUpsert → Resolve → Cluster → Analyze → Report → Refresh → Notify
//! pipeline (ADR-017).
//!
//! These types are the contract between pipeline stages. Each stage is a
//! pure function that receives typed input and produces typed output.
// e30.1 clippy baseline reset: pre-existing lint debt (see fix/e30.1-clippy-baseline-reset)

use std::collections::BTreeMap;
use std::path::PathBuf;

use crate::domain::aggregates::GraphNode;
use crate::domain::aggregates::{Symbol, SymbolId};
use crate::domain::value_objects::{DependencyType, PendingReference, Provenance};
use cognicode_graph_algos::algorithms::Statement;
use serde::{Deserialize, Serialize};

// ============================================================================
// Scan stage types
// ============================================================================

/// How a file changed since the last scan.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ChangeKind {
    /// File is new (not in the previous `scan_manifest`).
    New,
    /// File exists but its content hash changed.
    Changed,
    /// File existed in the previous manifest but is now gone.
    Deleted,
}

/// A single file change detected by the Scan stage.
#[derive(Debug, Clone)]
pub struct FileChange {
    /// Repo-relative path of the file.
    pub path: PathBuf,
    /// What kind of change was detected.
    pub kind: ChangeKind,
    /// SHA256 content hash (hex). `None` for `Deleted` files.
    pub content_hash: Option<String>,
    /// File modification time as epoch seconds.
    pub mtime: f64,
    /// Detected file type.
    pub file_type: FileType,
    /// Programming language if the file is code, else `None`.
    pub language: Option<&'static str>,
}

/// Classification of a source file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileType {
    /// Source code parseable by tree-sitter.
    Code,
    /// Markdown or text documentation (future: extracted by LLM).
    Document,
    /// Configuration file (JSON, YAML, TOML, etc.).
    Config,
    /// Unrecognised / unsupported file type.
    Other,
}

// ============================================================================
// Extract stage types
// ============================================================================

/// The result of extracting one file. Contains the `GraphNode`s and
/// `GraphEdge`s discovered in that file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtractionResult {
    /// Repo-relative path of the source file that was extracted.
    pub source_path: PathBuf,
    /// Graph nodes discovered (symbols, file-level nodes).
    pub nodes: Vec<GraphNode>,
    /// Graph edges discovered (calls, imports, contains, etc.).
    pub edges: Vec<ExtractionEdge>,
    /// SHA256 content hash of the file at extraction time.
    pub content_hash: String,
    /// Per-function statement list keyed by the function's NodeId (FQN string).
    /// Populated by the statement walker (M5.1b).
    #[serde(default)]
    pub statements_by_function: BTreeMap<String, Vec<Statement>>,
    /// Error message if extraction failed, else `None`.
    pub error: Option<String>,
}

/// A raw edge from extraction, before cross-file resolution.
///
/// `target_ref` may reference a symbol that is not yet in the graph (an
/// unresolved callee). The Resolve stage resolves these references.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtractionEdge {
    /// Source node ID (always resolved — it's in this file).
    pub source: String,
    /// Target reference — may be a name (unresolved) or an ID (resolved).
    pub target_ref: TargetRef,
    /// Edge kind as a dotted string (e.g. `"dependency.calls"`).
    pub kind: String,
    /// How the edge was obtained.
    pub provenance: Provenance,
    /// Confidence in `[0.0, 1.0]`.
    pub confidence: f64,
    /// Source line number (1-indexed).
    pub line: Option<u32>,
}

/// A reference to a target node — either resolved (by ID) or unresolved
/// (by name, to be resolved later by the Resolve stage).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TargetRef {
    /// The target node ID is known (same-file reference).
    Resolved(String),
    /// The target is known by name only; the Resolve stage will try to
    /// match it against the global symbol index.
    Unresolved(String),
}

impl ExtractionResult {
    /// Create a successful extraction result.
    pub fn ok(
        path: PathBuf,
        hash: String,
        nodes: Vec<GraphNode>,
        edges: Vec<ExtractionEdge>,
    ) -> Self {
        Self {
            source_path: path,
            nodes,
            edges,
            content_hash: hash,
            statements_by_function: BTreeMap::new(),
            error: None,
        }
    }

    /// Create a successful extraction result with a pre-populated statements map (M5.1b).
    pub fn ok_with_statements(
        path: PathBuf,
        hash: String,
        nodes: Vec<GraphNode>,
        edges: Vec<ExtractionEdge>,
        statements_by_function: BTreeMap<String, Vec<Statement>>,
    ) -> Self {
        Self {
            source_path: path,
            nodes,
            edges,
            content_hash: hash,
            statements_by_function,
            error: None,
        }
    }

    /// Create a failed extraction result (error isolation — ADR-023).
    pub fn failed(path: PathBuf, hash: String, error: String) -> Self {
        Self {
            source_path: path,
            nodes: Vec::new(),
            edges: Vec::new(),
            content_hash: hash,
            statements_by_function: BTreeMap::new(),
            error: Some(error),
        }
    }

    /// Returns `true` if extraction succeeded.
    pub fn is_ok(&self) -> bool {
        self.error.is_none()
    }
}

// ============================================================================
// File facts — what one file declares
// ============================================================================

/// What one file declares, in the shape the graph consumes.
///
/// Three kinds of relation live here and they are not the same thing. An
/// identified relation names both of its ends already, because the reader
/// that produced it knew them. A call names only the callee, because the
/// reader heard a name and the project still has to match it. A pending
/// reference names a callee that nobody declared: the reference is a fact
/// about what the file says, and dropping it would hide from the consumer
/// the very thing it asked about. Keeping them apart is what lets a file
/// that was read by a language's own reader enter the graph without a
/// second reader going over it.
#[derive(Debug, Clone, Default)]
pub struct FileFacts {
    /// Symbols the file declares. Their identity is the one the reader
    /// gave them, not one the graph invented.
    pub symbols: Vec<Symbol>,
    /// Relations whose two ends are already identified.
    pub identified: Vec<FileRelation>,
    /// Calls that still need a project-wide lookup by name.
    pub calls: Vec<(Symbol, String)>,
    /// References the file states and nobody declared: the far end is not
    /// identified yet, and the project may still be able to tie it to a
    /// declaration in another file.
    pub pending: Vec<PendingReference>,
}

/// A relation between two declarations, both of them already identified.
#[derive(Debug, Clone)]
pub struct FileRelation {
    /// The declaration that holds the relation.
    pub caller: SymbolId,
    /// The declaration the relation points at.
    pub callee: SymbolId,
    /// What the relation is.
    pub kind: DependencyType,
}

/// What the reader of a language says about one file.
///
/// Three answers, and they are not the same thing. The reader declared
/// something: this is what the file says. The reader does not apply: the
/// language has no reader of its own, or its reader had nothing to say about
/// this file, and the generic reader is the one that answers. The reader
/// failed: it could not read the file, and the cause says why.
///
/// A failure is not an absence. A consumer that receives `Failed` does not
/// fall back to the generic reader: it says so, and the file stays unread.
/// The two are told apart because a file whose reader fell over and a file
/// that declares nothing look the same from the outside, and only one of
/// them is a fact about the file.
#[derive(Debug, Clone)]
pub enum SemanticOutcome {
    /// The reader declared something: these are the facts of the file.
    Handled(FileFacts),
    /// The reader does not apply to this file.
    NotApplicable,
    /// The reader could not read the file; the cause says why.
    Failed(String),
}

// ============================================================================
// Scan manifest entry (scan_manifest table mirror)
// ============================================================================

/// A row in the `scan_manifest` table. Tracks file state for incremental
/// change detection (ADR-017/020).
#[derive(Debug, Clone)]
pub struct ScanManifestEntry {
    pub workspace_id: String,
    pub file_path: String,
    pub file_type: String,
    pub language: Option<String>,
    pub content_hash: String,
    pub mtime: f64,
    pub symbol_count: usize,
    pub edge_count: usize,
    pub status: String,
    pub error_msg: Option<String>,
}

// ============================================================================
// Job progress types
// ============================================================================

/// Progress reported by the async scan job.
#[derive(Debug, Clone, serde::Serialize)]
pub struct ScanProgress {
    /// Current pipeline stage.
    pub stage: ScanStage,
    /// Files processed so far.
    pub processed: usize,
    /// Total files to process.
    pub total: usize,
    /// Files that failed extraction (error isolation).
    pub failed: usize,
}

/// Pipeline stages reported in `ScanProgress`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ScanStage {
    /// Scanning the filesystem and comparing against the manifest.
    Scan,
    /// Parsing files with tree-sitter.
    Extract,
    /// Writing to PostgreSQL.
    PgUpsert,
    /// Resolving cross-file references.
    Resolve,
    /// Running community detection.
    Cluster,
    /// Computing god nodes, dead code, etc.
    Analyze,
    /// Generating the GraphReport.
    Report,
    /// Reloading the in-memory graph cache.
    Refresh,
    /// Job completed.
    Done,
}

/// Final job result.
#[derive(Debug, Clone, serde::Serialize)]
pub struct ScanResult {
    pub symbols: usize,
    pub edges: usize,
    pub duration_ms: u64,
    pub failed_files: Vec<FailedFile>,
    pub community_count: usize,
    pub health_score: f64,
}

/// Information about a file that failed extraction.
#[derive(Debug, Clone, serde::Serialize)]
pub struct FailedFile {
    pub path: String,
    pub error: String,
}

// ============================================================================
// Type reference types
// ============================================================================

/// A type reference extracted from a symbol's type annotations.
/// E.g., a function parameter `user: User` produces `TypeRef { target_name: "User", context: ParamType }`.
#[derive(Debug, Clone)]
pub struct TypeRef {
    /// Name of the referenced type.
    pub target_name: String,
    /// How the type is referenced in the source.
    pub context: TypeRefContext,
    /// Source line number (1-indexed).
    pub line: u32,
}

/// The syntactic context in which a type reference appears.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TypeRefContext {
    /// Function/method parameter type annotation.
    ParamType,
    /// Function/method return type annotation.
    ReturnType,
    /// Struct field type annotation.
    FieldType,
    /// Generic type argument.
    GenericArg,
    /// Variable type annotation.
    VariableType,
    /// Trait bound or supertype.
    TraitBound,
}

impl TypeRefContext {
    pub fn as_str(&self) -> &'static str {
        match self {
            TypeRefContext::ParamType => "param_type",
            TypeRefContext::ReturnType => "return_type",
            TypeRefContext::FieldType => "field_type",
            TypeRefContext::GenericArg => "generic_arg",
            TypeRefContext::VariableType => "variable_type",
            TypeRefContext::TraitBound => "trait_bound",
        }
    }
}
