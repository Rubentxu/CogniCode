//! Evidence aggregation — builds the four canonical evidence kinds
//! (`symbol_metadata`, `call_graph`, `source_file`, `fs_index`) for a symbol.
//!
//! Phase 1C: every block carries a `freshness` signal. For now the heuristic
//! is a file-existence check on `source_file` / `fs_index`; the graph-backed
//! blocks (`symbol_metadata`, `call_graph`) report `"unknown"` because the
//! call-graph build time is not currently exposed through the explorer port.

use std::path::Path;

use crate::dto::{EvidenceBlock, LineRange};
use crate::ports::source_reader::SourceReader;
use crate::ports::symbol_repository::{ResolvedSymbol, SymbolRepository};

/// Build the full evidence chain for a symbol.
///
/// Returns 4 [`EvidenceBlock`]s by default; `fs_index` is always emitted
/// even when the underlying file is missing (confidence drops to 0.0).
pub fn build_evidence_blocks(
    symbol: &ResolvedSymbol,
    repo: &dyn SymbolRepository,
    reader: &dyn SourceReader,
) -> Vec<EvidenceBlock> {
    vec![
        symbol_metadata_evidence(symbol),
        call_graph_evidence(symbol, repo),
        source_file_evidence(symbol, reader),
        fs_index_evidence(symbol),
    ]
}

/// Heuristic freshness: `"fresh"` if the file exists on disk, `"stale"`
/// otherwise. The repository's `SourceReader::root` is not directly
/// accessible here, so we use the raw `symbol.file` path. This matches the
/// long-standing assumption that symbol files are workspace-relative paths.
fn file_freshness(file: &str) -> Option<String> {
    Some(if Path::new(file).exists() {
        "fresh".to_string()
    } else {
        "stale".to_string()
    })
}

fn symbol_metadata_evidence(symbol: &ResolvedSymbol) -> EvidenceBlock {
    EvidenceBlock {
        id: "evidence:symbol_metadata".into(),
        kind: "symbol_metadata".into(),
        title: format!("Symbol metadata: {}", symbol.name),
        file: Some(symbol.file.clone()),
        line_range: Some(LineRange {
            start: symbol.line,
            end: symbol.line,
        }),
        source_tool_or_query: "CallGraphRepository::resolve".into(),
        confidence: Some(1.0),
        // Graph has no build-time exposed through the explorer port yet.
        freshness: Some("unknown".into()),
    }
}

fn call_graph_evidence(symbol: &ResolvedSymbol, repo: &dyn SymbolRepository) -> EvidenceBlock {
    let fan_in = repo.fan_in(&symbol.id);
    let fan_out = repo.fan_out(&symbol.id);
    EvidenceBlock {
        id: "evidence:call_graph".into(),
        kind: "call_graph".into(),
        title: format!("Call graph of {}", symbol.name),
        file: Some(symbol.file.clone()),
        line_range: Some(LineRange {
            start: symbol.line,
            end: symbol.line,
        }),
        source_tool_or_query: format!("CallGraph(fan_in={fan_in}, fan_out={fan_out})"),
        confidence: Some(1.0),
        // Same as symbol_metadata: graph build time not exposed.
        freshness: Some("unknown".into()),
    }
}

fn source_file_evidence(
    symbol: &ResolvedSymbol,
    reader: &dyn SourceReader,
) -> EvidenceBlock {
    let result = reader.read_source(&symbol.file);
    let (confidence, tool) = match result {
        Ok(_) => (
            Some(1.0),
            format!("SourceReader::read_source({})", symbol.file),
        ),
        Err(_) => (Some(0.0), format!("SourceReader::read_source(MISSING: {})", symbol.file)),
    };
    // `read_source` already proved the file is reachable; the freshness
    // signal matches the same heuristic used by `fs_index_evidence` so the
    // two blocks agree.
    let freshness = if result.is_ok() {
        Some("fresh".to_string())
    } else {
        Some("stale".to_string())
    };
    EvidenceBlock {
        id: "evidence:source_file".into(),
        kind: "source_file".into(),
        title: format!("Source file: {}", symbol.file),
        file: Some(symbol.file.clone()),
        line_range: Some(LineRange {
            start: 1,
            end: symbol.line,
        }),
        source_tool_or_query: tool,
        confidence,
        freshness,
    }
}

fn fs_index_evidence(symbol: &ResolvedSymbol) -> EvidenceBlock {
    EvidenceBlock {
        id: "evidence:fs_index".into(),
        kind: "fs_index".into(),
        title: format!("Filesystem index for {}", symbol.file),
        file: Some(symbol.file.clone()),
        line_range: None,
        source_tool_or_query: "FsSourceReader::root".into(),
        confidence: Some(1.0),
        freshness: file_freshness(&symbol.file),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ExplorerResult;
    use crate::ports::source_reader::SourceReader;
    use crate::ports::symbol_repository::{GraphStats, ResolvedSymbol, SymbolRepository};
    use cognicode_core::domain::aggregates::SymbolId;
    use cognicode_core::domain::value_objects::SymbolKind;
    use std::collections::HashMap as StdHashMap;
    use std::sync::Mutex;
    use tempfile::tempdir;

    fn make_resolved(file: &str, name: &str, line: u32) -> ResolvedSymbol {
        ResolvedSymbol {
            id: SymbolId::new(format!("{file}:{name}:{line}")),
            name: name.into(),
            kind: SymbolKind::Function,
            file: file.into(),
            line,
            signature: None,
        }
    }

    struct StubRepo;
    impl SymbolRepository for StubRepo {
        fn resolve(&self, _id: &SymbolId) -> ExplorerResult<Option<ResolvedSymbol>> { Ok(None) }
        fn callers(&self, _id: &SymbolId) -> Vec<crate::ports::RelationTarget> { Vec::new() }
        fn callees(&self, _id: &SymbolId) -> Vec<crate::ports::RelationTarget> { Vec::new() }
        fn fan_in(&self, _id: &SymbolId) -> usize { 0 }
        fn fan_out(&self, _id: &SymbolId) -> usize { 0 }
        fn find_symbols_by_name(&self, _name: &str) -> ExplorerResult<Vec<ResolvedSymbol>> { Ok(Vec::new()) }
        fn find_symbols_by_file(&self, _file: &str) -> ExplorerResult<Vec<ResolvedSymbol>> { Ok(Vec::new()) }
        fn module_list(&self) -> Vec<String> { Vec::new() }
        fn all_symbols(&self) -> ExplorerResult<Vec<ResolvedSymbol>> { Ok(Vec::new()) }
        fn graph_stats(&self) -> GraphStats { GraphStats::default() }
    }

    struct StubReader {
        files: Mutex<StdHashMap<String, String>>,
    }
    impl StubReader {
        fn new() -> Self {
            Self { files: Mutex::new(StdHashMap::new()) }
        }
    }
    impl SourceReader for StubReader {
        fn read_source(&self, file: &str) -> ExplorerResult<String> {
            self.files
                .lock()
                .unwrap()
                .get(file)
                .cloned()
                .ok_or_else(|| crate::error::ExplorerError::SourceUnavailable {
                    file: file.into(),
                    object_id: file.into(),
                })
        }
        fn read_lines(
            &self,
            file: &str,
            start: u32,
            end: u32,
        ) -> ExplorerResult<Vec<(u32, String)>> {
            let content = self.read_source(file)?;
            Ok(content
                .lines()
                .enumerate()
                .map(|(i, l)| ((i + 1) as u32, l.to_string()))
                .filter(|(n, _)| *n >= start && *n <= end)
                .collect())
        }
    }

    #[test]
    fn symbol_metadata_freshness_is_unknown() {
        let sym = make_resolved("src/missing.rs", "x", 1);
        let blocks = build_evidence_blocks(&sym, &StubRepo, &StubReader::new());
        let meta = blocks.iter().find(|b| b.kind == "symbol_metadata").unwrap();
        assert_eq!(meta.freshness.as_deref(), Some("unknown"));
    }

    #[test]
    fn call_graph_freshness_is_unknown() {
        let sym = make_resolved("src/missing.rs", "x", 1);
        let blocks = build_evidence_blocks(&sym, &StubRepo, &StubReader::new());
        let cg = blocks.iter().find(|b| b.kind == "call_graph").unwrap();
        assert_eq!(cg.freshness.as_deref(), Some("unknown"));
    }

    #[test]
    fn source_file_freshness_fresh_when_file_exists() {
        let dir = tempdir().expect("tempdir");
        let file = dir.path().join("real.rs");
        std::fs::write(&file, "fn real() {}\n").expect("write");
        let rel = file
            .strip_prefix(dir.path())
            .unwrap()
            .to_str()
            .unwrap()
            .to_string();

        // Use a stub reader backed by an in-memory map that matches the
        // reader-side "file exists" check; the freshness logic relies on
        // `reader.read_source` succeeding, not on `Path::exists`.
        let mut files = StdHashMap::new();
        files.insert(rel.clone(), "fn real() {}\n".to_string());
        let reader = StubReader { files: Mutex::new(files) };

        let sym = make_resolved(&rel, "real", 1);
        let blocks = build_evidence_blocks(&sym, &StubRepo, &reader);
        let src = blocks.iter().find(|b| b.kind == "source_file").unwrap();
        assert_eq!(src.freshness.as_deref(), Some("fresh"));
    }

    #[test]
    fn source_file_freshness_stale_when_file_missing() {
        let reader = StubReader::new();
        let sym = make_resolved("does_not_exist.rs", "x", 1);
        let blocks = build_evidence_blocks(&sym, &StubRepo, &reader);
        let src = blocks.iter().find(|b| b.kind == "source_file").unwrap();
        assert_eq!(src.freshness.as_deref(), Some("stale"));
    }

    #[test]
    fn fs_index_freshness_reflects_path_exists() {
        let dir = tempdir().expect("tempdir");
        let file = dir.path().join("present.rs");
        std::fs::write(&file, "fn p() {}\n").expect("write");
        // The fs_index check uses the raw path; use an absolute path so the
        // heuristic actually sees the file we just created.
        let abs = file.to_str().unwrap().to_string();
        let sym = make_resolved(&abs, "p", 1);
        let blocks = build_evidence_blocks(&sym, &StubRepo, &StubReader::new());
        let fs = blocks.iter().find(|b| b.kind == "fs_index").unwrap();
        assert_eq!(fs.freshness.as_deref(), Some("fresh"));

        // Now try a path that does not exist — must be "stale".
        let sym2 = make_resolved("__definitely_not_there__.rs", "q", 1);
        let blocks2 = build_evidence_blocks(&sym2, &StubRepo, &StubReader::new());
        let fs2 = blocks2.iter().find(|b| b.kind == "fs_index").unwrap();
        assert_eq!(fs2.freshness.as_deref(), Some("stale"));
    }
}
