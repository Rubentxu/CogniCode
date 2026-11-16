//! La respuesta dice lo que costó cada etapa que la produjo.
//!
//! ## Por qué existe
//!
//! Un `elapsed_ms` total no distingue un acierto de caché de un parseo en
//! frío: dos respuestas con el mismo total pueden ser una servida y una
//! calculada, y una línea base que no las distingue no se puede comparar
//! con otra. Cada ruta mide las etapas que ejecutó —la construcción mide
//! walk/parse/ingest/resolve y la serialización del snapshot; la ruta de
//! caché mide la lectura del snapshot durable— y lo que no se ejecutó
//! queda ausente, que es "no medido", nunca cero.
//!
//! ## Lo que tiene que seguir siendo verdad
//!
//! Una construcción completa mide sus etapas y deja la medida de
//! serializar su snapshot; una respuesta servida del snapshot durable mide
//! solo la lectura; y la provenance de `graph.full` publica la medida, que
//! es por donde la CLI y el MCP la leen.

use std::fs;
use std::path::{Path, PathBuf};

use cognicode_core::application::analysis_basis::GraphSource;
use cognicode_core::application::graph_snapshot::load_or_build_graph;
use cognicode_core::application::services::analysis_service::AnalysisService;

fn write(path: &Path, content: &str) {
    fs::create_dir_all(path.parent().expect("the file has a directory")).expect("create_dir_all");
    fs::write(path, content).expect("the file is written");
}

fn readable_tree() -> (tempfile::TempDir, PathBuf) {
    let dir = tempfile::tempdir().expect("tempdir");
    let root = dir.path().to_path_buf();
    write(
        &root.join("src/lib.rs"),
        "pub fn helper() -> u32 {\n    1\n}\n\npub fn caller() -> u32 {\n    helper()\n}\n",
    );
    (dir, root)
}

/// Una construcción mide las etapas que ejecutó.
#[test]
fn a_built_graph_measures_its_stages() {
    let (tmp, root) = readable_tree();
    let cache = tmp.path().join("cache");
    let service = AnalysisService::new();

    let loaded =
        load_or_build_graph(&service, &root, Some(&cache), None, false).expect("the graph builds");

    assert_eq!(loaded.source, GraphSource::Built);
    let timings = loaded.timings;
    assert!(timings.walk_ms.is_some(), "the walk is measured: {timings:?}");
    assert!(
        timings.parse_ms.is_some(),
        "the parse is measured: {timings:?}"
    );
    assert!(
        timings.ingest_ms.is_some(),
        "the ingest is measured: {timings:?}"
    );
    assert!(
        timings.resolve_ms.is_some(),
        "the resolution is measured: {timings:?}"
    );
    assert!(
        timings.build_ms.is_some(),
        "the whole build is measured: {timings:?}"
    );
    assert_eq!(
        timings.files_parsed,
        Some(1),
        "the build says what it parsed: {timings:?}"
    );
    assert!(
        timings.snapshot_serialize_ms.is_some(),
        "a complete build measures persisting its snapshot: {timings:?}"
    );
    assert!(
        timings.cache_lookup_ms.is_none(),
        "a build did not look a snapshot up: {timings:?}"
    );
}

/// Una respuesta servida del snapshot mide solo la lectura.
#[test]
fn a_snapshot_served_graph_measures_the_lookup() {
    let (tmp, root) = readable_tree();
    let cache = tmp.path().join("cache");

    // The first session builds and persists.
    let builder = AnalysisService::new();
    let built = load_or_build_graph(&builder, &root, Some(&cache), None, false)
        .expect("the graph builds");
    assert_eq!(built.source, GraphSource::Built);

    // A second session holds nothing: the durable snapshot serves it.
    let server = AnalysisService::new();
    let served = load_or_build_graph(&server, &root, Some(&cache), None, false)
        .expect("the snapshot serves");

    assert_eq!(served.source, GraphSource::Cache);
    let timings = served.timings;
    assert!(
        timings.cache_lookup_ms.is_some(),
        "the lookup that served the answer is measured: {timings:?}"
    );
    assert!(
        timings.build_ms.is_none() && timings.parse_ms.is_none(),
        "a served answer did not build, and says so by measuring nothing: {timings:?}"
    );
}

/// La provenance de `graph.full` publica la medida de las etapas.
#[test]
fn the_provenance_of_graph_full_carries_the_stage_costs() {
    use cognicode_core::application::analysis_basis::{Analyze, AnalysisBasis, AnalysisOp};
    use cognicode_core::application::services::analyze::ServiceAnalyzer;

    let (tmp, root) = readable_tree();
    let cache = tmp.path().join("cache");
    let analyzer = ServiceAnalyzer::with_cache_root(cache.clone());
    let basis = AnalysisBasis::for_workspace(root.clone());

    let outcome = analyzer.execute(
        &basis,
        AnalysisOp::GraphFull {
            path: root,
            rebuild: true,
        },
    );

    let provenance = outcome.provenance();
    assert_eq!(provenance.graph_source, Some(GraphSource::Built));
    let timings = provenance
        .phase_timings
        .expect("a built graph measures its stages");
    assert!(timings.parse_ms.is_some(), "the parse is measured: {timings:?}");

    // The measured answer survives the wire: the MCP and the CLI read the
    // provenance from JSON, and a field that does not serialize is a
    // measurement nobody can consume.
    let json = serde_json::to_value(provenance).expect("the provenance is JSON");
    assert!(
        json["phase_timings"]["parse_ms"].is_number(),
        "the wire carries the stage costs: {json}"
    );
}
