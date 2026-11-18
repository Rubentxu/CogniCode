//! What the pack reads, and who it asks.
//!
//! The use case owns the decisions —what is a candidate, what fits, what is
//! omitted— and reads the world through four small ports: the frozen
//! snapshot it is given, the bytes of a source file, the declaration that
//! starts at a position, and the relevance of an intent. None of them is a
//! second source of truth: each is answered by the owner of that fact, and
//! the pack publishes what the owner said.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use crate::domain::aggregates::call_graph::{CallGraph, SymbolId};

/// The frozen snapshot a pack is computed over.
///
/// The graph and the content hash of every file it was built from travel
/// together and are never refreshed: a pack never mixes a symbol of one
/// revision with the text of another. A file whose current bytes do not hash
/// to the value recorded here is refused, not read.
#[derive(Clone)]
pub struct SnapshotView {
    /// The graph the candidates are selected from.
    pub graph: Arc<CallGraph>,
    /// The digest of the sources the graph describes.
    pub digest: String,
    /// Where the graph came from, in the words the rest of the product uses.
    pub graph_source: Option<String>,
    /// The workspace root the relative paths below are relative to.
    pub root: PathBuf,
    /// The SHA-256 (hex) of every file of the snapshot, by relative path.
    pub files: BTreeMap<String, String>,
}

impl SnapshotView {
    /// The path of `file` relative to the snapshot root, in the form the
    /// manifest keys use.
    pub fn relative(&self, file: &str) -> String {
        let path = Path::new(file);
        let relative = path.strip_prefix(&self.root).unwrap_or(path);
        relative.to_string_lossy().replace('\\', "/")
    }
}

/// Reads the bytes of a source file of the snapshot.
pub trait SourceReader {
    /// The current bytes of `relative`, or why they cannot be read.
    fn read(&self, relative: &str) -> Result<Vec<u8>, String>;
}

/// The declaration that starts at a position of a source file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeclarationSpan {
    /// The first line of the declaration, counted from one.
    pub start_line: u32,
    /// The last line of the declaration, counted from one.
    pub end_line: u32,
    /// The declaration up to where its body starts.
    pub header: String,
    /// The line ranges of the statements that are direct children of the
    /// body, counted from one, both ends included, in source order. Empty
    /// when the declaration has no body block or the language does not
    /// expose one.
    pub body_statements: Vec<(u32, u32)>,
}

/// Reads the declaration that encloses a position.
pub trait DeclarationReader {
    /// The declaration of the symbol at `line` and `column` (both counted
    /// from zero) of `source`, or `None` when the language cannot say.
    fn declaration(
        &self,
        file: &str,
        source: &str,
        line: u32,
        column: u32,
    ) -> Option<DeclarationSpan>;
}

/// A symbol and the relevance score the ranker gave it for an intent.
#[derive(Debug, Clone)]
pub struct Ranked {
    /// The symbol.
    pub id: SymbolId,
    /// The ranker's score; higher is more relevant.
    pub score: f64,
}

/// Ranks the symbols of the snapshot for an intent.
pub trait Relevance {
    /// At most `max` symbols, best first, equal scores in symbol id order.
    fn rank(&self, intent: &str, max: usize) -> Vec<Ranked>;
}

/// The relevance of a request that carries no intent: it ranks nothing.
pub struct NoRelevance;

impl Relevance for NoRelevance {
    fn rank(&self, _intent: &str, _max: usize) -> Vec<Ranked> {
        Vec::new()
    }
}
