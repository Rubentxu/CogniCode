//! TASK-221 — el índice de símbolos sigue al grafo y al directorio
//! consultado.
//!
//! El índice vivía en un `Mutex<Option<LightweightIndex>>`: se llenaba
//! una vez (quien llegara primero), nunca se invalidaba tras un
//! reparse o un rebuild, cada consulta lo clonaba entero y no sabía
//! nada del directorio que se le preguntaba. Aquí se prueba que el
//! índice describe el árbol tal como la revisión actual del grafo lo
//! vio, acotado al directorio que la consulta nombra, y que la
//! respuesta declara esa base temporal.

use std::fs;
use std::path::PathBuf;

use cognicode_core::application::services::analysis_service::AnalysisService;

use tempfile::TempDir;

/// Workspace con un solo fichero y un solo símbolo.
fn workspace_with(label: &str, source: &str) -> (TempDir, PathBuf) {
    let dir = TempDir::new().expect("tempdir");
    let root = dir.path().canonicalize().expect("canonical root");
    let src = root.join("src");
    fs::create_dir(&src).expect("mkdir src");
    let file = src.join(format!("{label}.rs"));
    fs::write(&file, source).expect("write source");
    (dir, root)
}

/// AC #1 — tras editar un fichero y reconstruir el grafo, `find`
/// devuelve el símbolo nuevo y no el borrado. El índice que no sigue
/// al grafo responde con el símbolo que ya no existe.
#[test]
fn find_returns_the_new_symbol_after_a_rebuild_not_the_deleted_one() {
    let (_tmp, root) = workspace_with("alpha", "fn alpha() {}\n");
    let service = AnalysisService::new();
    service
        .build_project_graph(&root)
        .expect("the first build produces a graph");

    // El índice se llena aquí (primera consulta) con el árbol tal como
    // estaba: alpha existe, beta no.
    assert!(
        !service.find_symbol("alpha", &root).locations.is_empty(),
        "alpha exists before the edit"
    );

    // La edición: alpha deja de existir, beta nace.
    fs::write(root.join("src").join("alpha.rs"), "fn beta() {}\n").expect("edit source");
    service
        .build_project_graph(&root)
        .expect("the rebuild produces a new graph");

    assert!(
        service.find_symbol("alpha", &root).locations.is_empty(),
        "a deleted symbol must not survive the rebuild: the index follows \
         the graph, and the graph no longer has alpha"
    );
    assert!(
        !service.find_symbol("beta", &root).locations.is_empty(),
        "the symbol the edit introduced must be found after the rebuild"
    );
}

/// AC #2 — una consulta sobre el directorio B no devuelve símbolos de
/// A, aunque el índice ya se haya llenado consultando A.
#[test]
fn a_query_on_directory_b_does_not_return_directory_a_symbols() {
    let dir = TempDir::new().expect("tempdir");
    let root = dir.path().canonicalize().expect("canonical root");
    let a = root.join("a");
    let b = root.join("b");
    fs::create_dir(&a).expect("mkdir a");
    fs::create_dir(&b).expect("mkdir b");
    fs::write(a.join("has_alpha.rs"), "fn alpha() {}\n").expect("write a");
    fs::write(b.join("has_beta.rs"), "fn beta() {}\n").expect("write b");

    let service = AnalysisService::new();
    service.build_project_graph(&root).expect("graph");

    // La primera consulta llena el índice para A.
    let in_a = service.find_symbol("alpha", &a);
    assert!(!in_a.locations.is_empty(), "alpha lives in A");
    assert_eq!(in_a.directory, a, "the basis names the directory asked");

    // La consulta sobre B no puede responder con A.
    let in_b = service.find_symbol("alpha", &b);
    assert!(
        in_b.locations.is_empty(),
        "a query scoped to B must not answer with A's symbols, got {:?}",
        in_b.locations
    );
    let in_b_beta = service.find_symbol("beta", &b);
    assert!(
        !in_b_beta.locations.is_empty(),
        "beta lives in B and the scoped query must find it, got empty"
    );
}

/// AC #3 — el contenido del índice no depende del orden de las
/// llamadas: la única ruta de llenado es el grafo, así que consultar
/// antes o después de (re)construir el grafo produce lo mismo, y dos
/// servicios que construyen el mismo árbol responden lo mismo.
#[test]
fn the_index_content_does_not_depend_on_the_call_order() {
    let dir = TempDir::new().expect("tempdir");
    let root = dir.path().canonicalize().expect("canonical root");
    fs::write(root.join("lib.rs"), "fn alpha() {}\nfn beta() {}\n").expect("write");

    // Ruta 1: construir y luego consultar.
    let first = AnalysisService::new();
    first.build_project_graph(&root).expect("graph");
    let built_first = first.find_symbol("alpha", &root);

    // Ruta 2: el mismo árbol, otro servicio, mismo orden de construcción.
    let second = AnalysisService::new();
    second.build_project_graph(&root).expect("graph");
    let built_second = second.find_symbol("alpha", &root);

    assert_eq!(
        built_first.locations, built_second.locations,
        "the same tree, the same graph, the same answer"
    );

    // Y reconstruir sin cambios no cambia la respuesta: el orden
    // build→query y query→build(otra vez) coinciden.
    second.build_project_graph(&root).expect("rebuild");
    let rebuilt = second.find_symbol("alpha", &root);
    assert_eq!(
        built_first.locations, rebuilt.locations,
        "an idempotent rebuild does not change the answer"
    );
}

/// AC #4 — una consulta no clona el índice entero: dos consultas al
/// mismo directorio sobre la misma revisión comparten el mismo `Arc`.
#[test]
fn a_query_shares_the_index_instead_of_cloning_it() {
    let (_tmp, root) = workspace_with("alpha", "fn alpha() {}\n");
    let service = AnalysisService::new();
    service.build_project_graph(&root).expect("graph");

    let first = service.shared_symbol_index(&root);
    let second = service.shared_symbol_index(&root);
    assert!(
        std::sync::Arc::ptr_eq(&first, &second),
        "two queries on the same directory and revision share one index"
    );
}

/// AC #5 — la respuesta declara su base temporal: la revisión del
/// grafo que alimentó el índice. Un rebuild publica otra revisión.
#[test]
fn the_answer_declares_the_graph_revision_it_was_built_from() {
    let (_tmp, root) = workspace_with("alpha", "fn alpha() {}\n");
    let service = AnalysisService::new();

    let before = service.find_symbol("alpha", &root);
    assert_eq!(
        before.graph_revision, 0,
        "a service with no graph head declares revision 0"
    );

    service.build_project_graph(&root).expect("graph");
    let after = service.find_symbol("alpha", &root);
    assert!(
        after.graph_revision > 0,
        "a built graph publishes a revision the answer declares"
    );
}

/// AC #4 — medición antes/después sobre un fixture: la ruta nueva
/// consulta el `Arc` compartido; la vieja clonaba el índice entero en
/// cada consulta (el bucle de clonación reproduce exactamente ese
/// comportamiento). Fuera de la suite porque mide tiempo, no
/// comportamiento: se ejecuta a mano para el recibo
/// (`cargo nextest run -p cognicode-core --test it index_query_cost -- --ignored`
/// o `cargo test -p cognicode-core --test it index_query_cost -- --ignored --nocapture`).
#[test]
#[ignore = "mide tiempo, no comportamiento: recibo de TASK-221 AC#4"]
fn index_query_cost_before_and_after() {
    use std::time::Instant;

    let dir = TempDir::new().expect("tempdir");
    let root = dir.path().canonicalize().expect("canonical root");
    // Fixture: 2.000 ficheros con 10 símbolos cada uno.
    for i in 0..2_000 {
        let mut src = String::new();
        for j in 0..10 {
            src.push_str(&format!("fn symbol_{i}_{j}() {{}}\n"));
        }
        fs::write(root.join(format!("m{i}.rs")), src).expect("write fixture");
    }
    let service = AnalysisService::new();
    service.build_project_graph(&root).expect("graph");

    const QUERIES: usize = 1_000;
    let target = "symbol_10_5";

    // DESPUÉS (TASK-221): la consulta comparte el Arc.
    let start = Instant::now();
    for _ in 0..QUERIES {
        let _ = service.find_symbol(target, &root);
    }
    let shared = start.elapsed();

    // ANTES: cada consulta clonaba el índice entero y consultaba la copia.
    let index = service.shared_symbol_index(&root);
    let start = Instant::now();
    for _ in 0..QUERIES {
        let snapshot = index.as_ref().clone();
        let _ = snapshot.find_symbol(target);
    }
    let cloned = start.elapsed();

    println!(
        "TASK-221 AC#4 fixture=2.000 ficheros/20.000 símbolos queries={QUERIES} \
         arc_compartido={shared:?} clon_por_consulta={cloned:?} \
         ratio={:.1}x",
        cloned.as_secs_f64() / shared.as_secs_f64().max(1e-9)
    );
    assert!(
        cloned > shared,
        "cloning the whole index per query must not be cheaper than \
         sharing it: shared={shared:?} cloned={cloned:?}"
    );
}
