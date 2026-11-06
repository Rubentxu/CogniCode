//! TASK-49 — la cobertura y su palabra tienen un dueño.
//!
//! Una respuesta dice dos cosas: QUÉ sabe del árbol y DE QUÉ cobertura
//! salió. Las dos superficies que responden —la CLI y el MCP— lo dicen
//! sobre el mismo árbol, así que tienen que decirlo igual.
//!
//! Este fichero mide esa igualdad sobre el árbol real (el corpus
//! versionado), por los dos caminos REALES —no por los presentadores de
//! laboratorio— y con los TRES estados que un árbol puede alcanzar:
//!
//! - `complete`: el árbol se leyó entero;
//! - `partial`: hay un fichero que no se pudo leer;
//! - `unsupported`: el árbol no tiene nada que este producto sepa leer.
//!
//! La palabra se compara con el TIPO del dueño (`OutcomeState`), no con un
//! `match` que se traga los estados que no conoce: un mapeo con pérdida
//! declara la paridad justo donde hay que probarla.
//!
//! El caso de la CACHÉ es el que falla hoy: el presenter del MCP dice
//! "unknown" para una respuesta que se sirvió de la caché, y el handler
//! vivo dice "complete" para esa misma respuesta — y
//! `cached_coverage_is_honest.rs` ya probó que las dos tienen que ser la
//! MISMA respuesta, campo por campo.

use cognicode_core::application::analysis_basis::{
    AnalysisBasis, AnalysisOp, Analyze, GraphSource, OutcomeState,
};
use cognicode_core::application::consumer_analysis::ServiceAnalyzer;
use cognicode_core::interface::cli::presenter_bridge::present_graph_full;
use cognicode_core::interface::cli::result::GraphResult;
use cognicode_core::interface::mcp::handlers::{
    BuildGraphInput, BuildGraphOutput, HandlerContext, handle_build_graph,
};
use cognicode_core::interface::mcp::presenter_bridge::present_build_graph;
use std::path::{Path, PathBuf};

// ---------------------------------------------------------------------------
// Fixture: el corpus versionado, copiado a un temporal para poder dejar un
// fichero ilegible sin tocar el repositorio.
// ---------------------------------------------------------------------------

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
        if entry.file_type()?.is_dir() {
            copy_dir_recursive(&from, &to)?;
        } else {
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

/// Un fichero que no se puede leer: es un hueco en la cobertura.
fn make_unreadable(root: &Path) {
    use std::os::unix::fs::PermissionsExt;
    let unreadable = root.join("src").join("unreadable.rs");
    std::fs::set_permissions(&unreadable, std::fs::Permissions::from_mode(0o000))
        .expect("set_permissions 000");
}

// ---------------------------------------------------------------------------
// Lo que un consumidor lee de una respuesta.
// ---------------------------------------------------------------------------

/// La palabra publicada, como el TIPO que la nombra.
///
/// Un mapeo con pérdida (`_ => Unknown`) declararía la paridad de los
/// estados que no conoce, y el estado que se traga es justo el que hay que
/// probar. Aquí una palabra desconocida es un fallo, no un `Unknown`.
fn state_of(status: &str) -> OutcomeState {
    match status {
        "complete" => OutcomeState::Complete,
        "partial" => OutcomeState::Partial,
        "unknown" => OutcomeState::Unknown,
        "unsupported" => OutcomeState::Unsupported,
        "failed" => OutcomeState::Failed,
        other => panic!("una palabra de estado que el producto no publica: {other}"),
    }
}

/// Lo que un consumidor lee, y nada más: el mensaje y el tiempo transcurrido
/// dicen de dónde viene la respuesta, no qué es cierto.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Answer {
    state: OutcomeState,
    symbols: usize,
    relationships: usize,
    /// Los ficheros que no se pudieron leer, con su ruta RELATIVA al
    /// proyecto: dos copias del corpus viven en dos directorios, y lo que se
    /// compara es el árbol, no el directorio donde se copió.
    skipped: Vec<String>,
}

impl Answer {
    fn of(state: OutcomeState, symbols: usize, relationships: usize, skipped: Vec<String>) -> Self {
        Self {
            state,
            symbols,
            relationships,
            skipped,
        }
    }
}

/// El outcome de la CLI: la facade, que es el camino que `graph full`
/// recorre en producción.
fn cli_outcome(
    dir: &Path,
    cache: &Path,
    rebuild: bool,
) -> cognicode_core::application::analysis_basis::AnalysisOutcome<
    cognicode_core::application::analysis_basis::AnalysisValue,
> {
    let analyzer = ServiceAnalyzer::with_cache_root(cache.to_path_buf());
    let basis = AnalysisBasis::for_workspace(dir.to_path_buf());
    analyzer.execute(
        &basis,
        AnalysisOp::GraphFull {
            path: dir.to_path_buf(),
            rebuild,
        },
    )
}

/// La respuesta de la CLI, leída del documento que publica.
fn cli_answer(dir: &Path, cache: &Path, rebuild: bool) -> Answer {
    let outcome = cli_outcome(dir, cache, rebuild);

    let GraphResult::Full {
        status,
        symbols,
        dependencies,
        skipped_files,
        ..
    } = present_graph_full(&outcome, &dir.to_string_lossy(), rebuild)
    else {
        panic!("esperaba GraphResult::Full")
    };

    Answer::of(
        state_of(&status),
        symbols,
        dependencies,
        skipped_files
            .iter()
            .map(|file| relative(&file.path, dir))
            .collect(),
    )
}

/// La respuesta del MCP: el handler vivo, con su caché y sus manifests.
async fn mcp_answer(dir: &Path, cache: &Path) -> Answer {
    let ctx = HandlerContext::builder()
        .with_working_dir(dir.to_path_buf())
        .with_cache_root(cache.to_path_buf())
        .build();
    let output: BuildGraphOutput = handle_build_graph(
        &ctx,
        BuildGraphInput {
            directory: Some(dir.to_string_lossy().into_owned()),
        },
    )
    .await
    .expect("the MCP answers with a graph");

    Answer::of(
        state_of(&output.status),
        output.symbols_found,
        output.relationships_found,
        output
            .skipped_files
            .as_deref()
            .unwrap_or_default()
            .iter()
            .map(|file| relative(&file.path, dir))
            .collect(),
    )
}

fn relative(path: &str, root: &Path) -> String {
    Path::new(path)
        .strip_prefix(root)
        .unwrap_or_else(|_| Path::new(path))
        .to_string_lossy()
        .into_owned()
}

// ---------------------------------------------------------------------------
// Ley 1: los tres estados que un árbol alcanza, por los dos caminos reales.
// ---------------------------------------------------------------------------

/// Un árbol que se leyó entero: las dos superficies dicen lo mismo.
#[tokio::test]
async fn both_surfaces_say_complete_over_a_tree_read_whole() {
    let (_tmp, root) = clean_corpus();
    let cache = tempfile::tempdir().expect("a temporary cache");

    let cli = cli_answer(&root, cache.path(), true);
    let mcp = mcp_answer(&root, cache.path()).await;

    assert_eq!(
        cli.state,
        OutcomeState::Complete,
        "el corpus se lee entero: la cobertura es completa: {cli:?}"
    );
    assert_eq!(
        cli, mcp,
        "las dos superficies dicen lo mismo sobre el mismo árbol: {cli:?} / {mcp:?}"
    );
    assert!(
        cli.symbols > 0,
        "el grafo no está vacío: dos respuestas vacías son iguales, y esa \
         igualdad no mide nada: {cli:?}"
    );
}

/// Un árbol con un fichero que no se puede leer: las dos superficies dicen
/// `partial` y NAMBRAN el fichero que no leyeron.
#[tokio::test]
async fn both_surfaces_say_partial_over_a_tree_with_a_file_they_cannot_read() {
    let (_tmp, root) = clean_corpus();
    make_unreadable(&root);
    let cache = tempfile::tempdir().expect("a temporary cache");

    let cli = cli_answer(&root, cache.path(), true);
    let mcp = mcp_answer(&root, cache.path()).await;

    assert_eq!(
        cli.state,
        OutcomeState::Partial,
        "un fichero que no se pudo leer deja la cobertura incompleta: {cli:?}"
    );
    assert_eq!(
        cli, mcp,
        "las dos superficies dicen lo mismo sobre el mismo árbol: {cli:?} / {mcp:?}"
    );
    assert_eq!(
        cli.skipped,
        vec!["src/unreadable.rs".to_string()],
        "el hueco se nombra, y no se cuenta como cobertura: {cli:?}"
    );
}

/// Un árbol sin nada que leer: las dos superficies dicen `unsupported`, y no
/// lo disfracen de un grafo vacío que se sabe completo.
#[tokio::test]
async fn both_surfaces_say_unsupported_over_a_tree_with_nothing_to_read() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let root = tmp.path().join("empty");
    std::fs::create_dir_all(root.join("src")).expect("create the empty tree");
    std::fs::write(root.join("src").join("notes.txt"), "no rust here\n").expect("write");
    let cache = tempfile::tempdir().expect("a temporary cache");

    let cli = cli_answer(&root, cache.path(), true);
    let mcp = mcp_answer(&root, cache.path()).await;

    assert_eq!(
        cli.state,
        OutcomeState::Unsupported,
        "un árbol sin material no es un grafo completo vacío: {cli:?}"
    );
    assert_eq!(
        cli, mcp,
        "las dos superficies dicen lo mismo sobre el mismo árbol: {cli:?} / {mcp:?}"
    );
}

// ---------------------------------------------------------------------------
// Ley 2: la respuesta servida de la caché es la misma respuesta.
// ---------------------------------------------------------------------------

/// El presenter del MCP y el handler vivo dicen la MISMA palabra sobre la
/// MISMA respuesta.
///
/// Aquí falla hoy: el presenter dice "unknown" para lo servido de la caché y
/// el handler dice "complete". Dos presentaciones de la misma respuesta, dos
/// palabras — y `cached_coverage_is_honest.rs` ya probó que la respuesta
/// servida de la caché tiene que ser igual que la construida en frío.
#[tokio::test]
async fn the_presented_answer_says_the_same_word_as_the_handler_that_served_it() {
    let (_tmp, root) = clean_corpus();
    let cache = tempfile::tempdir().expect("a temporary cache");

    // La primera respuesta construye el grafo y persiste la instantánea.
    let cold = cli_answer(&root, cache.path(), true);
    assert_eq!(
        cold.state,
        OutcomeState::Complete,
        "el corpus se lee entero: la primera respuesta es completa: {cold:?}"
    );

    // La segunda se sirve de la instantánea: no hay recorrido en ESTA
    // llamada, pero la cobertura es la del árbol y se comprobó.
    let warm = cli_outcome(&root, cache.path(), false);
    assert_eq!(
        warm.provenance().graph_source,
        Some(GraphSource::Cache),
        "la segunda respuesta vino de la instantánea, o esto no mide nada: {:?}",
        warm.provenance().graph_source
    );
    assert_eq!(
        state_of(warm.state().published_label()),
        OutcomeState::Complete,
        "la respuesta servida de la caché es la misma respuesta: {warm:?}"
    );

    let live = mcp_answer(&root, cache.path()).await;
    let presented = present_build_graph(&warm);

    assert_eq!(
        state_of(&presented.status),
        live.state,
        "el presenter y el handler dicen la misma palabra sobre la misma \
         respuesta: {presented:?} / {live:?}"
    );
}

// ---------------------------------------------------------------------------
// Ley 3: las cinco palabras, sin mapeo con pérdida.
// ---------------------------------------------------------------------------

/// Los dos presentadores dicen la misma palabra para cada uno de los cinco
/// estados.
///
/// Los cinco, no uno: una paridad probada sobre un solo estado no dice nada
/// de los otros cuatro, y el mapeo con pérdida que usaba este contrato
/// (`_ => OutcomeState::Unknown`) convertía cada estado no probado en un
/// `Unknown` que pasaba el test.
#[test]
fn the_two_presenters_agree_on_the_five_words() {
    use cognicode_core::application::analysis_basis::{
        AnalysisError, AnalysisOutcome, AnalysisValue, GraphValue,
    };

    let states = [
        OutcomeState::Complete,
        OutcomeState::Partial,
        OutcomeState::Unknown,
        OutcomeState::Unsupported,
        OutcomeState::Failed,
    ];

    for state in states {
        let value = AnalysisValue::Graph(GraphValue::Full {
            path: "/w".into(),
            symbols: 4,
            dependencies: 2,
            elapsed_ms: 10,
            pending_references: vec![],
            rejected_relations: vec![],
        });
        let outcome = match state {
            OutcomeState::Partial => AnalysisOutcome::partial(
                value,
                vec![AnalysisError::gap("graph", "src/broken.rs: syntax error")],
            ),
            OutcomeState::Complete => AnalysisOutcome::complete(value),
            OutcomeState::Unknown => AnalysisOutcome::unknown(),
            OutcomeState::Unsupported => AnalysisOutcome::unsupported("no hay material"),
            OutcomeState::Failed => {
                AnalysisOutcome::failed(AnalysisError::io_failure("no se pudo leer el árbol"))
            }
        };

        let cli = present_graph_full(&outcome, "/w", false);
        let GraphResult::Full { status, .. } = &cli else {
            panic!("esperaba GraphResult::Full")
        };
        let mcp = present_build_graph(&outcome);

        assert_eq!(
            state_of(status),
            state,
            "el presenter CLI publica la palabra del estado: {status}"
        );
        assert_eq!(
            state_of(&mcp.status),
            state,
            "el presenter MCP publica la palabra del estado: {}",
            mcp.status
        );
    }
}
