//! La cobertura dice lo mismo en caliente que en frío.
//!
//! La métrica de cobertura cuenta `parsed_files`: los ficheros que el
//! recorrido volvió a interpretar. Un fichero recuperado de la caché de
//! ficheros no cuenta. Así, la segunda construcción sobre el mismo árbol —
//! donde todo sale de la caché — informaba de **0 % de cobertura sobre un
//! grafo completo**, y la primera construcción informaba de un porcentaje
//! que tampoco era el de un grafo completo, porque el denominador contaba
//! también los ficheros que el grafo nunca iba a cubrir.
//!
//! El producto no puede publicar dos números distintos para el mismo árbol.
//! Y el número tiene que decir *qué* ficheros cuenta: los cubiertos, los
//! interpretados de nuevo y los reutilizados de la caché son tres hechos,
//! no uno.
//!
//! Este fichero fija ese contrato:
//!
//! - un build en caliente informa la misma cobertura que uno en frío;
//! - el informe separa cubiertos, reinterpretados y reutilizados;
//! - un fichero que no se pudo leer no cuenta como cubierto;
//! - la RESPUESTA que el MCP publica en caliente es la misma que publica
//!   en frío, campo por campo.
//!
//! El corpus es el real —`testdata/silent_errors_corpus`—, copiado a un
//! directorio temporal para poder dejar un fichero ilegible sin tocar el
//! repositorio.

use cognicode_core::application::dto::GraphCoverageMetrics;
use cognicode_core::application::services::analysis_service::AnalysisService;
use cognicode_core::interface::mcp::handlers::{
    BuildGraphInput, BuildGraphOutput, HandlerContext, handle_build_graph,
};
use std::path::{Path, PathBuf};

/// El corpus versionado: tres ficheros Rust que se leen bien.
fn corpus_path() -> PathBuf {
    let manifest = std::env::var("CARGO_MANIFEST_DIR")
        .expect("CARGO_MANIFEST_DIR must be set during cargo test");
    PathBuf::from(manifest).join("testdata/silent_errors_corpus")
}

fn copy_dir_recursive(src: &Path, dst: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(dst)?;
    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
        let from = entry.path();
        let to = dst.join(entry.file_name());
        let file_type = entry.file_type()?;
        if file_type.is_dir() {
            copy_dir_recursive(&from, &to)?;
        } else if file_type.is_file() {
            std::fs::copy(&from, &to)?;
        }
    }
    Ok(())
}

/// Una copia limpia del corpus en un directorio temporal.
fn clean_corpus() -> (tempfile::TempDir, PathBuf) {
    let tmp = tempfile::tempdir().expect("tempdir");
    let dst = tmp.path().join("corpus");
    copy_dir_recursive(&corpus_path(), &dst).expect("copy corpus");
    (tmp, dst)
}

fn build(root: &Path, service: &AnalysisService) -> GraphCoverageMetrics {
    service
        .build_project_graph(root)
        .expect("build_project_graph returns Ok even with skipped files");
    service
        .get_coverage_metrics()
        .expect("coverage metrics are populated after a build")
}

/// Un fichero que no se pudo leer no es un fichero cubierto.
#[test]
fn a_file_that_could_not_be_read_is_not_covered() {
    use std::os::unix::fs::PermissionsExt;

    let (_tmp, root) = clean_corpus();
    let unreadable = root.join("src").join("unreadable.rs");
    std::fs::set_permissions(&unreadable, std::fs::Permissions::from_mode(0o000))
        .expect("set_permissions 000");

    let service = AnalysisService::new();
    let coverage = build(&root, &service);

    assert_eq!(
        coverage.total_source_files, 3,
        "the three Rust files of the corpus are the files the graph is built from"
    );
    assert_eq!(
        coverage.covered_files, 2,
        "the file that could not be read is not covered; got coverage={coverage:?}"
    );
    assert!(
        coverage.coverage_percent < 100.0,
        "a file that could not be read leaves the coverage below 100 %; got {coverage:?}"
    );
}

/// El informe separa los ficheros cubiertos, los interpretados de nuevo y
/// los reutilizados de la caché.
#[test]
fn the_report_separates_reparsed_from_reused() {
    let (_tmp, root) = clean_corpus();
    let service = AnalysisService::new();

    let cold = build(&root, &service);
    assert_eq!(
        cold.parsed_files, 3,
        "a cold build interprets every file; got {cold:?}"
    );
    assert_eq!(
        cold.cached_files, 0,
        "a cold build reuses nothing; got {cold:?}"
    );
    assert_eq!(
        cold.covered_files, 3,
        "a cold build covers every file; got {cold:?}"
    );

    let warm = build(&root, &service);
    assert_eq!(
        warm.parsed_files, 0,
        "a warm build interprets nothing; got {warm:?}"
    );
    assert_eq!(
        warm.cached_files, 3,
        "a warm build reuses every file; got {warm:?}"
    );
    assert_eq!(
        warm.covered_files, 3,
        "a warm build covers the same files as a cold one; got {warm:?}"
    );
}

/// Un build en caliente sobre el mismo árbol informa la misma cobertura que
/// uno en frío. El árbol no ha cambiado: la cobertura tampoco.
#[test]
fn a_warm_build_reports_the_same_coverage_as_a_cold_one() {
    let (_tmp, root) = clean_corpus();
    let service = AnalysisService::new();

    let cold = build(&root, &service);
    let warm = build(&root, &service);

    assert_eq!(
        warm.covered_files, cold.covered_files,
        "the same tree covers the same files warm and cold; cold={cold:?} warm={warm:?}"
    );
    assert_eq!(
        warm.coverage_percent, cold.coverage_percent,
        "the same tree reports the same coverage warm and cold; cold={cold:?} warm={warm:?}"
    );
    assert_eq!(
        cold.coverage_percent, 100.0,
        "a corpus whose files are all readable is fully covered; got {cold:?}"
    );
}

// ─── La respuesta que el MCP publica en caliente ───────────────────────
//
// `build_graph` tiene dos maneras de responder: construir el grafo, o
// servir el que ya tiene. Las dos respuestas tienen la MISMA forma, así
// que un cliente no puede decir cuál recibió —y no tiene por qué poder:
// el cliente lee la RESPUESTA, no el modo en que se produjo. Por eso las
// dos respuestas tienen que ser la misma respuesta.
//
// Lo de arriba mide la cobertura que informa el servicio. Esto mide lo
// que lee el CONSUMIDOR: el veredicto, los ficheros omitidos, las
// referencias pendientes. Una respuesta servida de la caché que dice
// `complete` donde una construcción en frío del MISMO árbol diría
// `partial` es una respuesta hueca: el consumidor cree conocer un
// proyecto que sólo ha visto a medias.
//
// El hueco que la comprobación de frescura NO puede ver es el del
// recorrido: el manifiesto cubre los ficheros de los que se construye el
// grafo, y un enlace simbólico que el recorrido DECLINA no es uno de
// ellos. El árbol puede cambiar de una forma que hace que una
// construcción en frío sea parcial mientras la caché sigue creyendo que
// está fresca.

/// Lo que un consumidor lee de la respuesta, y nada más: el mensaje y el
/// tiempo transcurrido dicen de dónde viene la respuesta, no qué es
/// cierto.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Answer {
    status: String,
    symbols_found: usize,
    relationships_found: usize,
    /// Los ficheros omitidos, con su ruta RELATIVA al proyecto: dos
    /// copias del corpus viven en dos directorios, y lo que se compara es
    /// el árbol, no el directorio donde se copió.
    skipped: Vec<(String, String)>,
    pending: usize,
    rejected: usize,
}

fn answer_of(output: &BuildGraphOutput, root: &Path) -> Answer {
    Answer {
        status: output.status.clone(),
        symbols_found: output.symbols_found,
        relationships_found: output.relationships_found,
        skipped: output
            .skipped_files
            .as_deref()
            .unwrap_or_default()
            .iter()
            .map(|file| {
                let path = Path::new(&file.path);
                let relative = path
                    .strip_prefix(root)
                    .unwrap_or(path)
                    .to_string_lossy()
                    .into_owned();
                (relative, file.reason_kind.to_string())
            })
            .collect(),
        pending: output
            .pending_references
            .as_deref()
            .unwrap_or_default()
            .len(),
        rejected: output
            .rejected_relationships
            .as_deref()
            .unwrap_or_default()
            .len(),
    }
}

/// La respuesta del MCP para un proyecto, como la recibe un consumidor.
async fn mcp_answer(dir: &Path) -> BuildGraphOutput {
    let ctx = HandlerContext::builder().with_working_dir(dir).build();
    handle_build_graph(
        &ctx,
        BuildGraphInput {
            directory: Some(dir.to_string_lossy().into_owned()),
        },
    )
    .await
    .expect("the MCP answers with a graph")
}

/// Un enlace simbólico que el recorrido DECLINA: es un hueco en la
/// cobertura, y no es uno de los ficheros de los que se construye el
/// grafo, así que tampoco está en el manifiesto.
fn decline_a_symlink(root: &Path) {
    std::os::unix::fs::symlink("ok.rs", root.join("src").join("link.rs"))
        .expect("the corpus can hold a symlink");
}

/// La respuesta servida de la caché es la que una construcción en frío
/// daría del mismo árbol.
#[tokio::test]
async fn the_cached_answer_is_the_answer_a_fresh_build_would_have_given() {
    let (_tmp, root) = clean_corpus();

    let cold = mcp_answer(&root).await;
    assert_eq!(
        cold.status, "complete",
        "the corpus is read whole, so the first answer is a complete one: {cold:?}"
    );
    assert!(
        cold.symbols_found > 0,
        "the graph is not empty: two empty answers are equal, and that \
         equality measures nothing"
    );

    let warm = mcp_answer(&root).await;
    assert!(
        warm.message.contains("cache"),
        "the second answer really came from the cache and not from a \
         rebuild, or this measures nothing: {}",
        warm.message
    );

    assert_eq!(
        answer_of(&warm, &root),
        answer_of(&cold, &root),
        "a consumer cannot tell the cached answer from the built one, so \
         they have to be the same answer"
    );
}

/// Un árbol con un hueco que el recorrido deja NO se sirve `complete`
/// desde la caché: la caché no puede ver el hueco, y por eso no puede
/// afirmar que no lo hay.
#[tokio::test]
async fn a_tree_the_walk_declines_is_not_complete_when_it_is_served_from_the_cache() {
    let (_tmp, root) = clean_corpus();

    // La primera respuesta construye el grafo y persiste la instantánea.
    let cold = mcp_answer(&root).await;
    assert_eq!(
        cold.status, "complete",
        "the corpus is read whole, so the first answer is a complete one: {cold:?}"
    );

    // Ahora el árbol cambia de una forma que la comprobación de frescura
    // no puede ver: el recorrido DECLINA un enlace simbólico, y un
    // enlace declinado no es un fichero del que se construye el grafo,
    // así que tampoco está en el manifiesto.
    decline_a_symlink(&root);

    // La verdad: lo que dice una construcción en frío de ESTE árbol. Una
    // segunda copia, para que la caché no pueda responder por él.
    let (_tmp_truth, truth_root) = clean_corpus();
    decline_a_symlink(&truth_root);
    let truth = mcp_answer(&truth_root).await;
    assert_eq!(
        truth.status, "partial",
        "a hole the walk leaves is not a complete coverage: {truth:?}"
    );
    // The ABSOLUTE truth of the hole, not only the equality between the
    // two answers: a comparison of two answers cannot see a mistake they
    // share, and the mapping that says WHY a path was left out is one of
    // those shared things.
    assert_eq!(
        answer_of(&truth, &truth_root).skipped,
        vec![("src/link.rs".to_string(), "symlink".to_string())],
        "the hole is the declined symlink, and it is published as such: {truth:?}"
    );

    // La respuesta de la caché, sobre el árbol que tiene el hueco.
    let warm = mcp_answer(&root).await;
    assert!(
        warm.message.contains("cache"),
        "the second answer really came from the cache and not from a \
         rebuild, or this measures nothing: {}",
        warm.message
    );
    assert!(
        warm.symbols_found > 0,
        "the graph is not empty: two empty answers are equal, and that \
         equality measures nothing"
    );

    assert_eq!(
        answer_of(&warm, &root),
        answer_of(&truth, &truth_root),
        "the cached answer and the fresh answer describe the same tree, so \
         they have to be the same answer"
    );
}
