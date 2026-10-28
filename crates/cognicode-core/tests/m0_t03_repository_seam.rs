//! M0.T03 — the seam that lets a caller observe the repository.
//!
//! The defect this unit closes is that a relation the repository refused
//! was discarded with `.ok()`: the failure was counted nowhere and
//! published nowhere. The product's own repository accepts every
//! relation, so the refusal path was unreachable — and a path nobody can
//! reach is a path nobody can observe.
//!
//! This commit adds the seam and proves it changes nothing about what
//! the product publishes: a build that draws its repository from the
//! seam publishes exactly the graph the default build publishes.
//!
//! The refusal itself is the next commit. Here the seam is only shown
//! to be wired (the injected repository is the one the build used) and
//! to be behaviour-preserving (the published numbers do not move).

use cognicode_core::application::services::analysis_service::AnalysisService;
use cognicode_core::domain::aggregates::{CallGraph, Symbol, SymbolId};
use cognicode_core::domain::services::CycleDetectionResult;
use cognicode_core::domain::traits::{DependencyError, DependencyRepository};
use cognicode_core::domain::value_objects::DependencyType;
use cognicode_core::infrastructure::graph::PetGraphStore;
use std::collections::HashSet;
use std::sync::{Arc, Mutex};

// ---------------------------------------------------------------------------
// A repository that behaves exactly like the product's own, and counts the
// relations it was offered.
// ---------------------------------------------------------------------------

struct CountingRepository {
    inner: PetGraphStore,
    offered: Arc<Mutex<usize>>,
}

impl CountingRepository {
    fn new(offered: Arc<Mutex<usize>>) -> Self {
        Self {
            inner: PetGraphStore::new(),
            offered,
        }
    }
}

impl DependencyRepository for CountingRepository {
    fn add_dependency(
        &mut self,
        source_id: &SymbolId,
        target_id: &SymbolId,
        dependency_type: DependencyType,
    ) -> Result<(), DependencyError> {
        *self.offered.lock().unwrap() += 1;
        self.inner
            .add_dependency(source_id, target_id, dependency_type)
    }

    fn add_symbol(&mut self, id: &SymbolId, symbol: &Symbol) -> Result<(), DependencyError> {
        self.inner.add_symbol(id, symbol)
    }

    fn remove_symbol(&mut self, id: &SymbolId) -> Option<Symbol> {
        self.inner.remove_symbol(id)
    }

    fn get_symbol(&self, id: &SymbolId) -> Option<&Symbol> {
        self.inner.get_symbol(id)
    }

    fn get_all_symbols(&self) -> Vec<Symbol> {
        self.inner.get_all_symbols()
    }

    fn find_impact_scope(&self, id: &SymbolId) -> HashSet<SymbolId> {
        self.inner.find_impact_scope(id)
    }

    fn find_dependents(&self, id: &SymbolId) -> HashSet<SymbolId> {
        self.inner.find_dependents(id)
    }

    fn find_dependencies(&self, id: &SymbolId) -> HashSet<SymbolId> {
        self.inner.find_dependencies(id)
    }

    fn detect_cycles(&self) -> CycleDetectionResult {
        self.inner.detect_cycles()
    }

    fn has_path(&self, source: &SymbolId, target: &SymbolId) -> bool {
        self.inner.has_path(source, target)
    }

    fn get_call_graph(&self) -> CallGraph {
        self.inner.get_call_graph()
    }

    fn symbol_count(&self) -> usize {
        self.inner.symbol_count()
    }

    fn dependency_count(&self) -> usize {
        self.inner.dependency_count()
    }
}

// ---------------------------------------------------------------------------
// Fixture: one file with three relations, so the comparison is not vacuous.
// ---------------------------------------------------------------------------

fn fixture(dir: &std::path::Path) {
    std::fs::create_dir_all(dir.join("src")).unwrap();
    std::fs::write(
        dir.join("Cargo.toml"),
        "[package]\nname = \"m0-t03-fixture\"\nversion = \"0.0.0\"\nedition = \"2021\"\n",
    )
    .unwrap();
    std::fs::write(
        dir.join("src").join("main.rs"),
        "fn main() { helper(); other(); }\nfn helper() -> u32 { 1 }\nfn other() -> u32 { helper() }\n",
    )
    .unwrap();
}

// ---------------------------------------------------------------------------
// Ley 1: el seam está cableado — el build ofrece sus relaciones al
// repositorio inyectado, no a otro.
// ---------------------------------------------------------------------------

#[test]
fn the_build_offers_its_relations_to_the_injected_repository() {
    let tmp = tempfile::tempdir().unwrap();
    fixture(tmp.path());

    let offered = Arc::new(Mutex::new(0usize));
    let counter = offered.clone();
    let service = AnalysisService::new().with_repository_factory(move || {
        Box::new(CountingRepository::new(counter.clone())) as Box<dyn DependencyRepository>
    });

    service.build_project_graph(tmp.path()).unwrap();

    assert!(
        *offered.lock().unwrap() > 0,
        "el build debe ofrecer sus relaciones al repositorio inyectado; \
         un seam que nadie usa no es un seam"
    );
}

// ---------------------------------------------------------------------------
// Ley 2: el seam no mueve lo que el producto publica. Un repositorio
// inyectado que se comporta como el propio publica el mismo grafo.
// ---------------------------------------------------------------------------

#[test]
fn the_injected_repository_publishes_the_same_graph_as_the_default_one() {
    let tmp = tempfile::tempdir().unwrap();
    fixture(tmp.path());

    let default_service = AnalysisService::new();
    default_service.build_project_graph(tmp.path()).unwrap();
    let default_stats = default_service.get_graph_stats();

    let offered = Arc::new(Mutex::new(0usize));
    let counter = offered.clone();
    let injected = AnalysisService::new().with_repository_factory(move || {
        Box::new(CountingRepository::new(counter.clone())) as Box<dyn DependencyRepository>
    });
    injected.build_project_graph(tmp.path()).unwrap();
    let injected_stats = injected.get_graph_stats();

    assert!(
        default_stats.edge_count > 0,
        "el fixture debe producir relaciones; si no, la comparación es vacua"
    );
    assert_eq!(
        default_stats.symbol_count, injected_stats.symbol_count,
        "el seam no debe cambiar los símbolos que el producto publica"
    );
    assert_eq!(
        default_stats.edge_count, injected_stats.edge_count,
        "el seam no debe cambiar las aristas que el producto publica"
    );
}
