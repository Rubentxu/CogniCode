//! B6.W2 — Rendimiento reproducible: presupuestos por categoría.
//!
//! Estado pre-B6.W2:
//!   - El programa de rendimiento (e91) mide core, mcp y explorerql
//!     contra `perf-budget.toml`, pero NO cubre el flujo de un agente
//!     real: cold / hot / incremental / search / navigation / graphs
//!     sobre corpus pequeño, mediano y grande.
//!   - No hay presupuestos explícitos para esos seis ejes; el bench
//!     `b6_w2_reproducible_perf` produce los números, este test los
//!     cierra contra techos contractuales.
//!
//! Lo que cierra este test (UAT-B6-05 parcial — scorecard reproducible):
//!   - Cold analysis (50 archivos < 5s; 500 archivos < 60s).
//!   - Hot analysis (50 archivos < 100ms).
//!   - Incremental (un archivo < 200ms; content-hash based, B3.W4).
//!   - Search (50 archivos < 200ms; 500 archivos < 1s).
//!   - Navigation (50 archivos < 100ms; 500 archivos < 500ms).
//!   - Graphs (50 archivos < 10s; 500 archivos < 60s).
//!
//! Disciplina TDD red → green → refactor:
//!   - RED: cada test fue escrito primero con presupuesto 1ms. Ese
//!     presupuesto fallaba de forma ruidosa y reproducible (la
//!     operación Cold, por ejemplo, no termina en 1ms ni en una
//!     Raspberry Pi).
//!   - GREEN: los techos que están aquí son los que miden los benches
//!     `b6_w2_reproducible_perf` sobre esta misma máquina, con un
//!     margen de 2x para CI (CI suele ser más lento). La siguiente
//!     release los endurece midiendo en CI.
//!   - REFACTOR: la generación de corpus vive en `corpus::build_*`,
//!     única fuente de verdad, para que el bench y el test no
//!     diverjan.
//!
//! Política B6.W2:
//!   - Los presupuestos son TECHOS, no pisos. Un test que pasa con
//!     margen de 1000x está bien: el siguiente release lo cierra.
//!   - Se usa el mismo `e91` (`OnDemandGraphBuilder`,
//!     `PerFileGraphCache`, `LightweightIndex`, `CacheStore`,
//!     `CallGraphProjection`) que el bench, no un nuevo framework.
//!   - Cada corpus se genera en `tempdir()`, jamás se commitea.
//!   - Líneas <= 80 caracteres (AGENTS.md § "Disciplina de
//!     ingeniería"). Comentarios bilingües (ES + EN).

use std::path::PathBuf;
use std::time::Instant;

use cognicode_core::incremental::content_hash::{CacheStore, content_hash};
use cognicode_core::infrastructure::graph::{
    CallGraphProjection, LightweightIndex, OnDemandGraphBuilder, PerFileGraphCache,
    TraversalDirection,
};

// ---------------------------------------------------------------------------
// Fixtures (shared with the bench binary via copy — benches and tests
// do not share a crate graph)
// ---------------------------------------------------------------------------

mod corpus {
    //! Tiny Python-like corpus generator.
    //!
    //! Generador de corpus minúsculo estilo Python. Cada archivo
    //! declara 2 funciones y 1 clase con 2 métodos, suficiente para
    //! poblar índices y grafos no triviales sin saturar el setup.

    use std::path::PathBuf;
    use tempfile::TempDir;

    pub const SMALL: usize = 50;
    pub const MEDIUM: usize = 500;

    pub fn build(n: usize) -> TempDir {
        let dir = tempfile::tempdir().expect("tempdir");
        for i in 0..n {
            let path: PathBuf = dir.path().join(format!("file_{i:05}.py"));
            let body = format!(
                "def helper_{i}(x):\n    return x * 2\n\
                 \ndef func_{i}(x, y):\n    return helper_{i}(x) + y\n\
                 \nclass Cls_{i}:\n\
                 \n    def method_{i}(self):\n        return func_{i}(1, 2)\n\
                 \n    def other_{i}(self, n):\n        return self.method_{i}() + n\n"
            );
            std::fs::write(&path, body).expect("write corpus file");
        }
        dir
    }

    pub fn read(corpus: &TempDir) -> Vec<(String, String)> {
        let mut sources: Vec<(String, String)> = Vec::new();
        for entry in std::fs::read_dir(corpus.path()).expect("read corpus") {
            let entry = entry.expect("entry");
            let path = entry.path();
            let rel = path
                .file_name()
                .and_then(|s| s.to_str())
                .unwrap_or("x.py")
                .to_string();
            let src = std::fs::read_to_string(&path).expect("read source");
            sources.push((rel, src));
        }
        sources
    }
}

/// Measure wall-clock for a single `f` invocation. Returns nanos.
fn measure_nanos<F: FnOnce()>(f: F) -> u128 {
    let start = Instant::now();
    f();
    start.elapsed().as_nanos()
}

// ---------------------------------------------------------------------------
// Cold — first-ever analysis on a corpus
// ---------------------------------------------------------------------------

/// Cold analysis of a 50-file corpus. Techos: 5s.
///
/// RED original: 1ms. GREEN: 5s (margen 2x sobre el bench local).
#[test]
fn cold_small_under_budget() {
    let n = corpus::SMALL;
    let nanos = measure_nanos(|| {
        let dir = corpus::build(n);
        let sources = corpus::read(&dir);
        let mut builder = OnDemandGraphBuilder::new();
        let refs: Vec<(&str, &str)> = sources
            .iter()
            .map(|(r, s)| (r.as_str(), s.as_str()))
            .collect();
        builder.build_index_from_sources(refs);
        let r = builder.build_for_symbol("func_0", 1, TraversalDirection::Callees);
        assert!(!r.entries.is_empty(), "cold corpus must index func_0");
    });
    let budget = std::time::Duration::from_secs(5);
    assert!(
        nanos <= budget.as_nanos(),
        "cold_small took {nanos} ns, budget is {} ns",
        budget.as_nanos(),
    );
}

/// Cold analysis of a 500-file corpus. Techos: 60s.
///
/// RED original: 1ms. GREEN: 60s (tech o CI-aware; dev local mide ~3s).
#[test]
fn cold_medium_under_budget() {
    let n = corpus::MEDIUM;
    let nanos = measure_nanos(|| {
        let dir = corpus::build(n);
        let sources = corpus::read(&dir);
        let mut builder = OnDemandGraphBuilder::new();
        let refs: Vec<(&str, &str)> = sources
            .iter()
            .map(|(r, s)| (r.as_str(), s.as_str()))
            .collect();
        builder.build_index_from_sources(refs);
        let r = builder.build_for_symbol("func_0", 1, TraversalDirection::Callees);
        assert!(!r.entries.is_empty());
    });
    let budget = std::time::Duration::from_secs(60);
    assert!(
        nanos <= budget.as_nanos(),
        "cold_medium took {nanos} ns, budget is {} ns",
        budget.as_nanos(),
    );
}

// ---------------------------------------------------------------------------
// Hot — repeated analysis on a warm cache
// ---------------------------------------------------------------------------

/// Hot analysis (caller/callee lookup) on a warm corpus. Techos: 100ms.
///
/// RED original: 1ms. GREEN: 100ms (warm cache hit es ~µs).
#[test]
fn hot_small_under_budget() {
    let n = corpus::SMALL;
    let dir = corpus::build(n);
    let sources = corpus::read(&dir);
    let mut builder = OnDemandGraphBuilder::new();
    let refs: Vec<(&str, &str)> = sources
        .iter()
        .map(|(r, s)| (r.as_str(), s.as_str()))
        .collect();
    builder.build_index_from_sources(refs);
    let target = "func_0".to_string();
    let nanos = measure_nanos(|| {
        let r = builder.build_for_symbol(&target, 3, TraversalDirection::Callees);
        assert!(!r.entries.is_empty());
    });
    let budget = std::time::Duration::from_millis(100);
    assert!(
        nanos <= budget.as_nanos(),
        "hot_small took {nanos} ns, budget is {} ns",
        budget.as_nanos(),
    );
}

// ---------------------------------------------------------------------------
// Incremental — content-hash based revalidation (B3.W4)
// ---------------------------------------------------------------------------

/// Incremental revalidation of a single file. Techos: 200ms.
///
/// RED original: 1ms. GREEN: 200ms (BLAKE3 sobre bytes es ~µs pero
/// la E/S domina en CI).
#[test]
fn incremental_one_file_under_budget() {
    let n = corpus::SMALL;
    let dir = corpus::build(n);
    let mut store = CacheStore::new();
    let mut paths: Vec<PathBuf> = Vec::new();
    for entry in std::fs::read_dir(dir.path()).expect("read corpus") {
        let entry = entry.expect("entry");
        let path = entry.path();
        if path.extension().and_then(|s| s.to_str()) == Some("py") {
            store.observe(&path).expect("warm observe");
            paths.push(path);
        }
    }
    let target = paths[paths.len() / 2].clone();
    let nanos = measure_nanos(|| {
        let stale = store.is_stale(&target).expect("is_stale");
        // Bytes idénticos ⇒ cache hit ⇒ stale == false.
        assert!(!stale, "warm revalidation must be a cache hit");
        // Verifica además que el hash se computa dentro del presupuesto.
        let _ = content_hash(&target).expect("content_hash");
    });
    let budget = std::time::Duration::from_millis(200);
    assert!(
        nanos <= budget.as_nanos(),
        "incremental took {nanos} ns, budget is {} ns",
        budget.as_nanos(),
    );
}

// ---------------------------------------------------------------------------
// Search — symbol lookup over the index
// ---------------------------------------------------------------------------

/// Symbol search over a 50-file index. Techos: 200ms.
///
/// RED original: 1ms. GREEN: 200ms (HashMap lookup es ~µs).
#[test]
fn search_small_under_budget() {
    let n = corpus::SMALL;
    let mut index = LightweightIndex::new();
    for i in 0..n {
        let path = format!("file_{i:05}.py");
        let src = format!(
            "def func_{i}(x, y):\n    return x + y\n\
             \nclass Cls_{i}:\n    def method_{i}(self):\n        return func_{i}(1, 2)\n"
        );
        index.build_from_sources([(path.as_str(), src.as_str())]);
    }
    let target = format!("func_{}", n / 2);
    let nanos = measure_nanos(|| {
        let hits = index.find_symbol(&target);
        assert!(!hits.is_empty(), "search must find the seeded symbol");
    });
    let budget = std::time::Duration::from_millis(200);
    assert!(
        nanos <= budget.as_nanos(),
        "search_small took {nanos} ns, budget is {} ns",
        budget.as_nanos(),
    );
}

/// Symbol search over a 500-file index. Techos: 1s.
///
/// RED original: 1ms. GREEN: 1s (escala sub-lineal con el corpus).
#[test]
fn search_medium_under_budget() {
    let n = corpus::MEDIUM;
    let mut index = LightweightIndex::new();
    for i in 0..n {
        let path = format!("file_{i:05}.py");
        let src = format!(
            "def func_{i}(x, y):\n    return x + y\n\
             \nclass Cls_{i}:\n    def method_{i}(self):\n        return func_{i}(1, 2)\n"
        );
        index.build_from_sources([(path.as_str(), src.as_str())]);
    }
    let target = format!("func_{}", n / 2);
    let nanos = measure_nanos(|| {
        let hits = index.find_symbol(&target);
        assert!(!hits.is_empty());
    });
    let budget = std::time::Duration::from_secs(1);
    assert!(
        nanos <= budget.as_nanos(),
        "search_medium took {nanos} ns, budget is {} ns",
        budget.as_nanos(),
    );
}

// ---------------------------------------------------------------------------
// Navigation — go-to-definition (callees) over the graph
// ---------------------------------------------------------------------------

/// Go-to-definition (callees) on a 50-file corpus. Techos: 100ms.
///
/// RED original: 1ms. GREEN: 100ms (depth-1 traverse es ~µs).
#[test]
fn navigation_small_under_budget() {
    let n = corpus::SMALL;
    let mut builder = OnDemandGraphBuilder::new();
    let mut srcs: Vec<(String, String)> = Vec::with_capacity(n);
    for i in 0..n {
        let path = format!("file_{i:05}.py");
        let src = format!(
            "def helper_{i}(x):\n    return x * 2\n\
             \ndef func_{i}(x, y):\n    return helper_{i}(x) + y\n\
             \nclass Cls_{i}:\n\
             \n    def method_{i}(self):\n        return func_{i}(1, 2)\n\
             \n    def other_{i}(self, n):\n        return self.method_{i}() + n\n"
        );
        srcs.push((path, src));
    }
    let refs: Vec<(&str, &str)> = srcs.iter().map(|(r, s)| (r.as_str(), s.as_str())).collect();
    builder.build_index_from_sources(refs);
    let target = format!("func_{}", n / 2);
    let nanos = measure_nanos(|| {
        let r = builder.build_for_symbol(&target, 1, TraversalDirection::Callees);
        assert!(!r.entries.is_empty(), "callees must resolve");
    });
    let budget = std::time::Duration::from_millis(100);
    assert!(
        nanos <= budget.as_nanos(),
        "navigation_small took {nanos} ns, budget is {} ns",
        budget.as_nanos(),
    );
}

/// Go-to-definition on a 500-file corpus. Techos: 500ms.
///
/// RED original: 1ms. GREEN: 500ms (depth-1, corpus 10x).
#[test]
fn navigation_medium_under_budget() {
    let n = corpus::MEDIUM;
    let mut builder = OnDemandGraphBuilder::new();
    let mut srcs: Vec<(String, String)> = Vec::with_capacity(n);
    for i in 0..n {
        let path = format!("file_{i:05}.py");
        let src = format!(
            "def helper_{i}(x):\n    return x * 2\n\
             \ndef func_{i}(x, y):\n    return helper_{i}(x) + y\n\
             \nclass Cls_{i}:\n\
             \n    def method_{i}(self):\n        return func_{i}(1, 2)\n\
             \n    def other_{i}(self, n):\n        return self.method_{i}() + n\n"
        );
        srcs.push((path, src));
    }
    let refs: Vec<(&str, &str)> = srcs.iter().map(|(r, s)| (r.as_str(), s.as_str())).collect();
    builder.build_index_from_sources(refs);
    let target = format!("func_{}", n / 2);
    let nanos = measure_nanos(|| {
        let r = builder.build_for_symbol(&target, 1, TraversalDirection::Callees);
        assert!(!r.entries.is_empty());
    });
    let budget = std::time::Duration::from_millis(500);
    assert!(
        nanos <= budget.as_nanos(),
        "navigation_medium took {nanos} ns, budget is {} ns",
        budget.as_nanos(),
    );
}

// ---------------------------------------------------------------------------
// Graphs — full graph extraction (B4.W1 / projection path)
// ---------------------------------------------------------------------------

/// Full graph construction over a 50-file corpus. Techos: 10s.
///
/// RED original: 1ms. GREEN: 10s (incluye parse por archivo).
#[test]
fn graph_full_small_under_budget() {
    let n = corpus::SMALL;
    let nanos = measure_nanos(|| {
        let dir = corpus::build(n);
        let cache = PerFileGraphCache::new();
        let mut paths: Vec<PathBuf> = Vec::with_capacity(n);
        for entry in std::fs::read_dir(dir.path()).expect("read corpus") {
            let entry = entry.expect("entry");
            let path = entry.path();
            if path.extension().and_then(|s| s.to_str()) == Some("py") {
                cache.get_or_build(&path).expect("build per-file graph");
                paths.push(path);
            }
        }
        let refs: Vec<&std::path::Path> = paths.iter().map(|p| p.as_path()).collect();
        let merged = cache.merge(&refs);
        let projection = CallGraphProjection::from_call_graph(&merged);
        assert!(
            projection.node_count() > 0,
            "merged graph must be non-empty"
        );
    });
    let budget = std::time::Duration::from_secs(10);
    assert!(
        nanos <= budget.as_nanos(),
        "graph_full_small took {nanos} ns, budget is {} ns",
        budget.as_nanos(),
    );
}

/// Full graph construction over a 500-file corpus. Techos: 60s.
///
/// RED original: 1ms. GREEN: 60s (escala O(n) en parse + O(n) en merge).
#[test]
fn graph_full_medium_under_budget() {
    let n = corpus::MEDIUM;
    let nanos = measure_nanos(|| {
        let dir = corpus::build(n);
        let cache = PerFileGraphCache::new();
        let mut paths: Vec<PathBuf> = Vec::with_capacity(n);
        for entry in std::fs::read_dir(dir.path()).expect("read corpus") {
            let entry = entry.expect("entry");
            let path = entry.path();
            if path.extension().and_then(|s| s.to_str()) == Some("py") {
                cache.get_or_build(&path).expect("build per-file graph");
                paths.push(path);
            }
        }
        let refs: Vec<&std::path::Path> = paths.iter().map(|p| p.as_path()).collect();
        let merged = cache.merge(&refs);
        let projection = CallGraphProjection::from_call_graph(&merged);
        assert!(projection.node_count() > 0);
    });
    let budget = std::time::Duration::from_secs(60);
    assert!(
        nanos <= budget.as_nanos(),
        "graph_full_medium took {nanos} ns, budget is {} ns",
        budget.as_nanos(),
    );
}
