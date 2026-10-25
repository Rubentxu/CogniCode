//! Cache del servicio: el indice de simbolos y la cache de ficheros.
//!
//! El indice se construye la primera vez que se pide; la cache de ficheros se invalida por ruta cuando el codigo cambia.

#![allow(
    clippy::doc_overindented_list_items,
    clippy::len_zero,
    clippy::type_complexity,
    clippy::useless_vec,
    unused_comparisons
)]

use super::AnalysisService;
use crate::infrastructure::graph::{LightweightIndex, SymbolLocation};

impl AnalysisService {
    /// Replaces the symbol index with a pre-built one (e.g. from spawn_blocking).
    pub fn set_symbol_index(&self, index: LightweightIndex) {
        *self.symbol_index.lock().unwrap() = Some(index);
    }

    /// Checks whether a symbol index has been built.
    pub fn has_symbol_index(&self) -> bool {
        self.symbol_index.lock().unwrap().is_some()
    }

    /// Returns a snapshot of the symbol index for fast symbol lookups.
    ///
    /// Builds the index on first call (lazy init, protected by Mutex).
    /// Returns a cloned `LightweightIndex` so callers don't need to hold the lock.
    pub(super) fn ensure_symbol_index(&self) -> LightweightIndex {
        let mut guard = self.symbol_index.lock().unwrap();
        if guard.is_none() {
            let graph = self.get_project_graph();
            let mut index = LightweightIndex::new();
            for symbol in graph.symbols() {
                let location = SymbolLocation::from_location(symbol.location(), *symbol.kind());
                let name_lower = symbol.name().to_lowercase();
                index.insert(name_lower, location);
            }
            *guard = Some(index);
        }
        guard.as_ref().unwrap().clone()
    }

    /// Builds a call graph by scanning all source files in a directory
    ///
    /// Walks the directory recursively, parses each supported source file,
    /// extracts symbols and call relationships, and builds a project-wide call graph.
    /// Invalidates `file_cache` entries for specific absolute paths.
    /// Must be called before `build_project_graph` when files have been modified
    /// externally so that the mtime-based cache does not return stale symbols.
    pub fn invalidate_file_cache_for(&self, paths: &[std::path::PathBuf]) {
        let mut cache = self.file_cache.lock().unwrap();
        for path in paths {
            cache.remove(&path.to_string_lossy().into_owned());
        }
    }
}
