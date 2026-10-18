//! H0.1 (delivery-first-radical, Entry 56 WU-1) — RED test.
//!
//! Defecto observado en `crates/cognicode-core/src/infrastructure/graph/per_file_graph.rs`:
//!   - `get_cached` (líneas 288-294) NO consulta `entry.valid`. Sólo lee
//!     por clave de ruta y devuelve `entry.graph` aunque el flag
//!     `valid` sea `false`.
//!   - `invalidate` (líneas 297-304) sí pone `entry.valid = false`,
//!     pero su efecto queda anulado por `get_cached`.
//!   - `merge_with_report` (línea 366) usa `get_cached`, por tanto
//!     entrega grafos stale hasta el siguiente `get_or_build`.
//!
//! Esta RED test pina el contrato: tras `invalidate()`, `get_cached`
//! debe devolver `None`. El paquete afirma "un hit nunca ignora
//! invalidate" (H0.1). Si la implementación ya cumple, el assert
//! verde y el defecto no existe; si falla, el defecto es real.
//!
//! Tras el fix (planned, bounded): una línea, `.filter(|e| e.valid)`
//! en `get_cached`. Sin cambios de firma pública.

use cognicode_core::infrastructure::graph::per_file_graph::PerFileGraphCache;

#[test]
fn per_file_graph_cache_invalidate_makes_get_cached_return_none() {
    let tmp = tempfile::tempdir().expect("create tempdir");
    let src_dir = tmp.path().join("src");
    std::fs::create_dir_all(&src_dir).expect("create src dir");
    std::fs::write(
        src_dir.join("lib.rs"),
        "pub fn helper() -> u32 { 1 }\n",
    )
    .expect("write lib.rs");
    let project_dir = tmp.path().to_string_lossy().into_owned();
    let file_path = src_dir.join("lib.rs");

    let cache = PerFileGraphCache::with_project_dir(project_dir);

    // Populate cache via get_or_build (valida fingerprint, marca entry.valid = true).
    cache
        .get_or_build(&file_path)
        .expect("build graph for fixture lib.rs");

    // Precondición: el cache recién poblado devuelve Some.
    assert!(
        cache.get_cached(&file_path).is_some(),
        "freshly built cache must return Some; precondition para la RED test"
    );

    // Invalidate: la implementación existente pone entry.valid = false (línea 302).
    cache.invalidate(&file_path);

    // RED: get_cached debe devolver None tras invalidate().
    // Implementación actual devuelve el grafo stale porque get_cached
    // no consulta entry.valid (líneas 288-294).
    assert!(
        cache.get_cached(&file_path).is_none(),
        "tras invalidate(), get_cached debe devolver None (contrato H0.1); \
         la implementación actual devuelve el grafo cacheado ignorando \
         entry.valid; ver per_file_graph.rs:288-294"
    );
}