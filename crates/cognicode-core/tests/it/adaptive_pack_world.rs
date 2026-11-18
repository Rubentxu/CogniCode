//! A small world to select context from: real Rust sources, the graph the
//! extractor would draw over them, the product's own IDF ranker and the
//! product's own declaration reader.
//!
//! The graph is drawn by hand, edge by edge, so a test says exactly which
//! relations the selection may believe. Everything else —the ranking, the
//! declarations, the token measure, the pack— is the real one.

use std::collections::{BTreeMap, HashMap};
use std::path::PathBuf;
use std::sync::Arc;

use cognicode_core::application::context_pack::{
    Adaptive, ContextPack, ContextPackService, PackRequest, Ranked, Relevance, SelectionRequest,
    SnapshotView, SourceReader, Utf8UpperBound,
};
use cognicode_core::domain::aggregates::call_graph::{CallGraph, SymbolId};
use cognicode_core::domain::aggregates::symbol::Symbol;
use cognicode_core::domain::findings::digest::sha256_hex;
use cognicode_core::domain::services::ExtractionContext;
use cognicode_core::domain::value_objects::{DependencyType, Location, SymbolKind};
use cognicode_core::infrastructure::graph::analytics::ranking_stats::RankingStats;
use cognicode_core::infrastructure::semantic::TreeSitterDeclarations;

/// The product's IDF ranker over the graph of the world.
pub struct Idf(pub Arc<RankingStats>);

impl Relevance for Idf {
    fn rank(&self, intent: &str, max: usize) -> Vec<Ranked> {
        self.0
            .rank(intent, max)
            .into_iter()
            .map(|scored| Ranked {
                id: scored.symbol_id,
                score: scored.idf_score,
            })
            .collect()
    }
}

struct Sources(BTreeMap<String, String>);

impl SourceReader for Sources {
    fn read(&self, relative: &str) -> Result<Vec<u8>, String> {
        self.0
            .get(relative)
            .map(|text| text.as_bytes().to_vec())
            .ok_or_else(|| "missing".to_string())
    }
}

/// Builds a world: files, symbols and the relations between them.
#[derive(Default)]
pub struct WorldBuilder {
    graph: CallGraph,
    texts: BTreeMap<String, String>,
    ids: HashMap<String, SymbolId>,
}

impl WorldBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds a source file.
    pub fn file(mut self, path: &str, text: &str) -> Self {
        self.texts.insert(path.to_string(), text.to_string());
        self
    }

    fn place(&self, path: &str, name: &str, needle: &str) -> Location {
        let text = &self.texts[path];
        let (row, line) = text
            .lines()
            .enumerate()
            .find(|(_, line)| line.contains(needle))
            .unwrap_or_else(|| panic!("`{needle}` is not in {path}"));
        let column = line[line.find(needle).unwrap()..]
            .find(name)
            .map(|at| at + line.find(needle).unwrap())
            .unwrap_or_else(|| panic!("`{name}` is not in `{needle}`"));
        Location::new(path, row as u32, column as u32)
    }

    /// Adds a symbol, placed at the first line of `path` that has `needle`.
    pub fn symbol(
        mut self,
        key: &str,
        path: &str,
        name: &str,
        kind: SymbolKind,
        needle: &str,
    ) -> Self {
        let location = self.place(path, name, needle);
        let id = self.graph.add_symbol(Symbol::new(name, kind, location));
        self.ids.insert(key.to_string(), id);
        self
    }

    /// Adds a symbol the reader marked as a test.
    pub fn test(mut self, key: &str, path: &str, name: &str, needle: &str) -> Self {
        let location = self.place(path, name, needle);
        let id = self
            .graph
            .add_symbol(Symbol::new(name, SymbolKind::Function, location).mark_as_test());
        self.ids.insert(key.to_string(), id);
        self
    }

    /// Draws a relation the extractor believes.
    pub fn edge(mut self, from: &str, dependency: DependencyType, to: &str) -> Self {
        self.graph
            .add_dependency(&self.ids[from], &self.ids[to], dependency)
            .expect("both symbols exist");
        self
    }

    /// Draws a relation with the provenance the extractor gave it.
    pub fn edge_with(
        mut self,
        from: &str,
        dependency: DependencyType,
        to: &str,
        context: ExtractionContext,
    ) -> Self {
        self.graph
            .add_dependency_with_provenance(&self.ids[from], &self.ids[to], dependency, context)
            .expect("both symbols exist");
        self
    }

    pub fn build(self) -> World {
        let hashes: BTreeMap<String, String> = self
            .texts
            .iter()
            .map(|(path, text)| (path.clone(), sha256_hex(text.as_bytes())))
            .collect();
        let graph = Arc::new(self.graph);
        let stats = Arc::new(RankingStats::build(&graph));
        World {
            snapshot: SnapshotView {
                graph,
                digest: sha256_hex(b"adaptive-pack-world"),
                graph_source: Some("test".to_string()),
                root: PathBuf::from("/"),
                files: hashes,
            },
            sources: Sources(self.texts),
            ids: self.ids,
            ranker: Idf(stats),
        }
    }
}

/// A frozen world.
pub struct World {
    pub snapshot: SnapshotView,
    sources: Sources,
    ids: HashMap<String, SymbolId>,
    ranker: Idf,
}

impl World {
    /// The key a symbol id was registered under.
    pub fn key_of(&self, id: &str) -> String {
        self.ids
            .iter()
            .find(|(_, value)| value.as_str() == id)
            .map_or_else(|| id.to_string(), |(key, _)| key.clone())
    }

    /// The text of a file.
    pub fn text(&self, path: &str) -> &str {
        &self.sources.0[path]
    }

    pub fn service(&self) -> ContextPackService<'_> {
        ContextPackService {
            snapshot: &self.snapshot,
            relevance: &self.ranker,
            sources: &self.sources,
            declarations: &TreeSitterDeclarations,
            measure: &Utf8UpperBound,
            token_scope: "",
        }
    }

    /// The pack of a request, in one page.
    pub fn pack(
        &self,
        anchors: &[&str],
        intent: Option<&str>,
        adaptive: Option<Adaptive>,
        max_tokens: u64,
    ) -> ContextPack {
        self.service()
            .build(&PackRequest {
                selection: SelectionRequest {
                    anchors: anchors.iter().map(|a| self.resolve(a)).collect(),
                    intent: intent.map(str::to_string),
                    hot_paths: 0,
                },
                max_tokens,
                continuation: None,
                adaptive,
            })
            .expect("a first page is always buildable")
    }

    /// An anchor is a registered key or a bare name.
    fn resolve(&self, anchor: &str) -> String {
        self.ids
            .get(anchor)
            .map_or_else(|| anchor.to_string(), SymbolId::to_string)
    }

    /// The keys of the items of a pack, in order.
    pub fn keys(&self, pack: &ContextPack) -> Vec<String> {
        pack.items
            .iter()
            .map(|item| self.key_of(&item.id))
            .collect()
    }
}
