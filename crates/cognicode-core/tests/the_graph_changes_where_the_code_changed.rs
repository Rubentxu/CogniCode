//! El grafo cambia donde cambió el código, y se queda donde no.
//!
//! Una instantánea de un grafo dice cómo lucía una vez. No puede decir que
//! el grafo *siga* al código: un grafo que contestara lo mismo antes y
//! después de un renombrado pasaría todas las instantáneas tomadas antes
//! del renombrado y fallaría todas las tomadas después. Las dos serían
//! ciertas sobre su momento y ciegas al cambio.
//!
//! Así que esto es un escenario que cambia el código y hace dos preguntas a
//! la vez. **Qué se movió**: la identidad del símbolo renombrado ya no está
//! y la nueva sí, y la relación que apuntaba al nombre viejo apunta al
//! nuevo. **Qué se quedó**: cada otra identidad y cada otra relación está
//! donde estaba. Y la caché de ficheros olvidó el fichero que cambió, y
//! sólo ese.

use cognicode_core::application::services::analysis_service::AnalysisService;
use cognicode_core::domain::aggregates::call_graph::CallGraph;
use std::collections::BTreeSet;
use std::path::Path;

/// La identidad de un símbolo tal como la ve un consumidor: su nombre y el
/// fichero relativo donde vive. La ruta absoluta cambiaría con el
/// directorio, y lo que se compara aquí es el código, no dónde está.
type Identity = (String, String);

/// Un proyecto con dos ficheros que no se tocan entre sí y uno que sí.
fn proyecto() -> tempfile::TempDir {
    let dir = tempfile::TempDir::new().expect("tempdir");
    let src = dir.path().join("src");
    std::fs::create_dir_all(&src).expect("mkdir");
    std::fs::write(src.join("lib.rs"), "pub mod alpha;\npub mod beta;\n").expect("write");
    std::fs::write(
        src.join("alpha.rs"),
        "pub fn helper() -> u32 {\n    42\n}\n",
    )
    .expect("write");
    std::fs::write(
        src.join("beta.rs"),
        "pub fn value() -> u32 {\n    42\n}\n\npub fn total() -> u32 {\n    value()\n}\n",
    )
    .expect("write");
    dir
}

/// Renombra un símbolo y hace que la llamada que lo usaba siga al nombre
/// nuevo.
fn renombrar(dir: &Path) {
    std::fs::write(
        dir.join("src/beta.rs"),
        "pub fn answer() -> u32 {\n    42\n}\n\npub fn total() -> u32 {\n    answer()\n}\n",
    )
    .expect("write");
}

fn identidades(grafo: &CallGraph, raiz: &Path) -> BTreeSet<Identity> {
    grafo
        .symbols()
        .map(|s| {
            let ruta = Path::new(s.location().file())
                .strip_prefix(raiz)
                .unwrap_or_else(|_| Path::new(s.location().file()))
                .to_string_lossy()
                .to_string();
            (s.name().to_string(), ruta)
        })
        .collect()
}

fn relaciones(grafo: &CallGraph) -> BTreeSet<(String, String)> {
    grafo
        .all_dependencies()
        .map(|(origen, destino, _)| {
            let nombre = |id: &cognicode_core::domain::aggregates::call_graph::SymbolId| {
                grafo
                    .get_symbol(id)
                    .map(|s| s.name().to_string())
                    .unwrap_or_default()
            };
            (nombre(origen), nombre(destino))
        })
        .collect()
}

#[test]
fn el_grafo_cambia_donde_cambio_el_codigo_y_se_queda_donde_no() {
    let dir = proyecto();
    let service = AnalysisService::new();
    service
        .build_project_graph(dir.path())
        .expect("la primera lectura del proyecto");
    let antes = service.get_project_graph();

    let identidades_antes = identidades(&antes, dir.path());
    let relaciones_antes = relaciones(&antes);
    let metricas_antes = service.get_coverage_metrics().expect("cobertura");

    assert_eq!(
        identidades_antes,
        BTreeSet::from([
            ("helper".to_string(), "src/alpha.rs".to_string()),
            ("total".to_string(), "src/beta.rs".to_string()),
            ("value".to_string(), "src/beta.rs".to_string()),
        ]),
        "la primera lectura publica lo que el proyecto declara"
    );
    assert_eq!(
        relaciones_antes,
        BTreeSet::from([("total".to_string(), "value".to_string())]),
        "la llamada del fichero modificado es la relación que existe"
    );
    assert_eq!(metricas_antes.parsed_files, 3, "la primera lee todo");
    assert_eq!(
        metricas_antes.cached_files, 0,
        "la primera no tiene nada en caché"
    );

    // El código cambia: un nombre se convierte en otro y la llamada que lo
    // usaba lo sigue.
    renombrar(dir.path());

    service
        .build_project_graph(dir.path())
        .expect("la segunda lectura del proyecto");
    let despues = service.get_project_graph();

    let identidades_despues = identidades(&despues, dir.path());
    let relaciones_despues = relaciones(&despues);
    let metricas_despues = service.get_coverage_metrics().expect("cobertura");

    // --- Qué se movió ---
    assert!(
        !identidades_despues.contains(&("value".to_string(), "src/beta.rs".to_string())),
        "la identidad renombrada ya no se publica: {identidades_despues:?}"
    );
    assert!(
        identidades_despues.contains(&("answer".to_string(), "src/beta.rs".to_string())),
        "la identidad nueva se publica: {identidades_despues:?}"
    );
    assert!(
        !relaciones_despues.contains(&("total".to_string(), "value".to_string())),
        "la relación ya no apunta al nombre viejo: {relaciones_despues:?}"
    );
    assert!(
        relaciones_despues.contains(&("total".to_string(), "answer".to_string())),
        "la relación sigue al nombre nuevo: {relaciones_despues:?}"
    );

    // --- Qué se quedó ---
    let movidas: Vec<&Identity> = identidades_antes.difference(&identidades_despues).collect();
    assert_eq!(
        movidas.len(),
        1,
        "sólo se mueve la identidad renombrada, y se mueve exactamente una: {movidas:?}"
    );
    assert_eq!(
        identidades_despues.len(),
        identidades_antes.len(),
        "un renombrado no añade ni quita símbolos"
    );
    assert!(
        identidades_despues.contains(&("helper".to_string(), "src/alpha.rs".to_string())),
        "el fichero que no cambió conserva su identidad: {identidades_despues:?}"
    );
    assert!(
        identidades_despues.contains(&("total".to_string(), "src/beta.rs".to_string())),
        "el símbolo que no cambió conserva su identidad: {identidades_despues:?}"
    );
    assert_eq!(
        relaciones_despues.len(),
        relaciones_antes.len(),
        "una relación que cambia de destino sigue siendo una relación: {relaciones_despues:?}"
    );

    // --- La caché olvidó exactamente un fichero ---
    assert_eq!(
        metricas_despues.parsed_files, 1,
        "sólo el fichero que cambió se vuelve a leer"
    );
    assert_eq!(
        metricas_despues.cached_files, 2,
        "los otros dos se reutilizan: el cambio olvidó un fichero y sólo uno"
    );
    assert_eq!(
        metricas_despues.total_source_files, metricas_antes.total_source_files,
        "el conjunto de ficheros observados no cambia: el código cambió, no el proyecto"
    );
}
