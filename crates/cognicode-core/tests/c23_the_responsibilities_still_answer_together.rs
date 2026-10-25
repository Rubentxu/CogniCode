//! Las cuatro responsabilidades del servicio siguen contestando del mismo grafo.
//!
//! MEDIDO: el servicio de analisis era un unico fichero con la construccion
//! del grafo, la cache, la resolucion de simbolos y las metricas dentro. El
//! troceo las movio a cuatro ficheros, y este archivo es el que dice que
//! moverlas no cambio lo que el servicio contesta.
//!
//! La ley que este archivo fija: **construir, medir, resolver y leer el
//! indice hablan del mismo grafo.** Un servicio que construye un grafo y
//! consulta otro ya no es un servicio: son dos, y el consumidor no puede
//! compararlos. Un test por responsabilidad no lo diria —cada una pasaria
//! sola— asi que el test es sobre el servicio entero.

use cognicode_core::application::services::analysis_service::{AnalysisService, UsageSearchParams};
use cognicode_core::infrastructure::graph::per_file_graph::BuildStatus;
use std::path::Path;

/// Un proyecto con dos fuentes que se llaman entre si, y dos ficheros que el
/// grafo no analiza: un manifiesto y un readme.
///
/// Los dos ficheros de mas estan a proposito: el denominador de la cobertura
/// son los ficheros de los que se construyo el grafo, asi que un consumidor
/// que cuenta el manifiesto esta contando otra pregunta.
fn corpus(root: &Path) {
    std::fs::create_dir_all(root.join("src")).expect("create src");
    std::fs::write(
        root.join("Cargo.toml"),
        "[package]\nname = \"fixture\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
    )
    .expect("write manifest");
    std::fs::write(root.join("README.md"), "# Fixture\n\nUn proyecto normal.\n")
        .expect("write readme");
    std::fs::write(
        root.join("src").join("main.rs"),
        "mod inside;\nfn main() { inside::inside_fn(); }\n",
    )
    .expect("write main");
    std::fs::write(
        root.join("src").join("inside.rs"),
        "pub fn inside_fn() -> u32 { 1 }\n",
    )
    .expect("write inside");
}

/// El grafo del corpus, construido.
fn service_over(root: &Path) -> AnalysisService {
    let service = AnalysisService::new();
    service
        .build_project_graph(root)
        .expect("the project graph builds");
    service
}

/// Las metricas describen el arbol que el servicio construyo.
///
/// La cobertura cuenta los ficheros de los que se construyo el grafo: los
/// dos fuentes del corpus, y no el manifiesto ni el readme.
#[test]
fn the_metrics_describe_the_tree_the_service_built() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let root = tmp.path().join("project");
    corpus(&root);

    let service = service_over(&root);

    let coverage = service
        .get_coverage_metrics()
        .expect("a built graph publishes its coverage");
    assert_eq!(
        coverage.total_source_files, 2,
        "the coverage counts the sources the graph was built from, and the \
         corpus has two of them"
    );
    assert_eq!(
        coverage.covered_files, 2,
        "both sources were read, so both are covered: {:?}",
        coverage
    );

    let report = service
        .get_last_build_report()
        .expect("a built graph publishes its report");
    assert_eq!(
        report.status,
        BuildStatus::Complete,
        "the corpus has nothing the walk declines, so the build is complete: {:?}",
        report.status
    );

    let stats = service.get_graph_stats();
    assert_eq!(
        stats.file_count, 2,
        "the statistics count the files the graph holds, and the graph holds \
         two sources"
    );
    assert!(
        stats.symbol_count > 0,
        "the corpus declares symbols and the graph reports none"
    );
}

/// El indice resuelve el simbolo que el grafo tiene.
///
/// El indice se construye la primera vez que se pide, a partir del grafo que
/// el servicio acaba de construir. Si el indice se construyera sobre otro
/// grafo —o sobre ninguno— resolveria un simbolo que el grafo no tiene.
#[test]
fn the_index_resolves_the_symbol_the_graph_holds() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let root = tmp.path().join("project");
    corpus(&root);

    let service = service_over(&root);

    let located = service.find_symbol("inside_fn");
    assert_eq!(
        located.len(),
        1,
        "the corpus declares `inside_fn` once, and the index reported {:?}",
        located
    );
    assert!(
        located[0].file().ends_with("inside.rs"),
        "the index pointed at {}, which is not the file that declares it",
        located[0].file()
    );

    let held = service
        .get_project_graph()
        .find_by_name("inside_fn")
        .iter()
        .map(|symbol| symbol.location().file().to_string())
        .collect::<Vec<_>>();
    assert_eq!(
        held.len(),
        1,
        "the graph holds `inside_fn` once, and it holds {:?}",
        held
    );
    assert_eq!(
        located[0].file(),
        held[0],
        "the index and the graph point at different files for the same symbol: \
         {} vs {}",
        located[0].file(),
        held[0]
    );
}

/// La busqueda de usos encuentra la llamada que el grafo conoce.
///
/// La busqueda de usos camina el proyecto por su cuenta; el grafo se
/// construye por su cuenta. Si los dos no cuentan la misma historia, el
/// consumidor que pregunta "donde se usa esto" recibe una respuesta que no
/// tiene nada que ver con el grafo que le sirvio antes.
#[test]
fn the_usage_search_finds_the_call_the_graph_knew() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let root = tmp.path().join("project");
    corpus(&root);

    let service = service_over(&root);

    let usages = service
        .find_symbol_usages(UsageSearchParams {
            project_dir: root.clone(),
            symbol_name: "inside_fn".to_string(),
            include_declaration: true,
            context_lines: Some(0),
            first_only_definition: false,
        })
        .expect("the usage search answers");

    let files = usages
        .iter()
        .map(|usage| usage.file.clone())
        .collect::<Vec<_>>();
    assert!(
        files.iter().any(|file| file.ends_with("main.rs")),
        "the corpus calls `inside_fn` from main.rs, and the search reported {files:?}"
    );
}
