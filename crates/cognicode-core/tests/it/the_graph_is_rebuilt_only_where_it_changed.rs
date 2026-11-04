//! El grafo en caliente no se rehace entero.
//!
//! Una construcción en frío lee el proyecto entero y resuelve todas sus
//! relaciones. Una construcción en caliente, con el proyecto sin cambios,
//! no tiene por qué repetir ese trabajo: lo que no cambió se conserva.
//!
//! El defecto que este fichero fija es de **coste**, no de resultado: el
//! grafo que se publica en caliente es el mismo que se publicaría en frío,
//! pero se rehace entero para llegar a él. Con eso, el consumidor que
//! cambió un fichero paga por todo el proyecto, y el que no cambió nada
//! también —aunque el servidor MCP ya se salte la construcción cuando el
//! manifiesto dice que nada ha cambiado, el camino que reconstruye es el
//! mismo para todos—.
//!
//! Por eso el producto **publica** cuánto de la construcción anterior se
//! conservó: sin ese número, un consumidor no puede distinguir «el grafo
//! está completo» de «el grafo se rehizo entero», y las dos cosas son
//! completas.
//!
//! Este fichero fija ese contrato:
//!
//! - una construcción en frío resuelve todas las relaciones y no conserva
//!   ninguna;
//! - al cambiar **un** fichero, solo se vuelven a resolver las relaciones
//!   que ese cambio puede afectar; las demás se conservan;
//! - el grafo publicado es el mismo grafo: en frío y en caliente, los
//!   mismos símbolos y las mismas aristas;
//! - lo que el cambio tocó se ve en el grafo: el símbolo nuevo está y el
//!   viejo no.
//!
//! El corpus son tres ficheros Rust que se llaman entre sí, para que la
//! resolución tenga algo que resolver y para que el cambio en uno de ellos
//! no pueda arrastrar a los otros.

use cognicode_core::application::services::analysis_service::AnalysisService;
use cognicode_core::domain::aggregates::call_graph::CallGraph;
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

/// Un proyecto de tres ficheros que se llaman entre sí:
///
/// - `uno.rs` declara `uno`;
/// - `dos.rs` declara `dos` y llama a `uno`;
/// - `tres.rs` declara `tres` y llama a `dos`.
///
/// Las relaciones del proyecto son dos: `dos → uno` y `tres → dos`.
fn proyecto() -> (tempfile::TempDir, PathBuf) {
    let tmp = tempfile::tempdir().expect("tempdir");
    let root = tmp.path().join("proyecto");
    std::fs::create_dir_all(&root).expect("mkdir");
    std::fs::write(root.join("uno.rs"), "pub fn uno() -> i32 {\n    1\n}\n").expect("write uno");
    std::fs::write(root.join("dos.rs"), "pub fn dos() -> i32 {\n    uno()\n}\n")
        .expect("write dos");
    std::fs::write(
        root.join("tres.rs"),
        "pub fn tres() -> i32 {\n    dos()\n}\n",
    )
    .expect("write tres");
    (tmp, root)
}

/// Los símbolos del grafo, por identidad.
fn simbolos(graph: &CallGraph) -> BTreeSet<String> {
    graph
        .symbols()
        .map(|s| s.fully_qualified_name().to_string())
        .collect()
}

/// Las aristas del grafo, como `(origen, destino, tipo)`.
fn aristas(graph: &CallGraph) -> BTreeSet<(String, String, String)> {
    graph
        .all_dependencies()
        .map(|(s, t, k)| {
            (
                s.as_str().to_string(),
                t.as_str().to_string(),
                format!("{k:?}"),
            )
        })
        .collect()
}

/// Escribe un fichero con bytes distintos, para que el cambio sea real.
fn reescribir(path: &Path, cuerpo: &str) {
    std::fs::write(path, cuerpo).expect("write");
}

/// El identificador con el que el grafo nombra un símbolo de un fichero.
///
/// El grafo nombra un símbolo por su ruta, su nombre y el orden en que
/// apareció, así que el identificador se busca en el grafo y no se escribe
/// a mano: un test que fija la forma del identificador falla cuando la
/// forma cambia y no cuando la relación cambia.
fn id_de(graph: &CallGraph, fichero: &str, nombre: &str) -> Option<String> {
    graph
        .symbols()
        .find(|s| s.location().file().ends_with(fichero) && s.name() == nombre)
        .map(|s| s.fully_qualified_name().to_string())
}

/// Una construcción en frío resuelve todo y no conserva nada: no hay
/// construcción anterior de la que conservar.
#[test]
fn una_construccion_en_frio_no_conserva_nada() {
    let (_tmp, root) = proyecto();
    let service = AnalysisService::new();

    service.build_project_graph(&root).expect("build");
    let reuse = service
        .get_reuse_metrics()
        .expect("the build publishes how much it reused");

    assert_eq!(
        reuse.relationships_reused, 0,
        "a cold build has no previous graph to reuse; got {reuse:?}"
    );
    assert_eq!(
        reuse.relationships_resolved, 2,
        "the corpus declares two relationships and both are resolved; got {reuse:?}"
    );
}

/// El hueco que la unidad cierra: al cambiar **un** fichero, el grafo se
/// rehace entero y no solo lo que el cambio toca.
#[test]
fn al_cambiar_un_fichero_solo_se_resuelve_lo_que_el_cambio_toca() {
    let (_tmp, root) = proyecto();
    let service = AnalysisService::new();

    service.build_project_graph(&root).expect("cold build");

    // `tres.rs` cambia. Cambia lo que declara —de `tres` a `tres` y
    // `cuatro`— y nada más: la relación `dos → uno` no puede verse
    // afectada, porque `dos.rs` no ha cambiado y `uno.rs` tampoco.
    reescribir(
        &root.join("tres.rs"),
        "pub fn tres() -> i32 {\n    dos()\n}\n\npub fn cuatro() -> i32 {\n    4\n}\n",
    );
    service.build_project_graph(&root).expect("warm build");

    let reuse = service
        .get_reuse_metrics()
        .expect("the build publishes how much it reused");

    assert_eq!(
        reuse.relationships_reused, 1,
        "the relationship `dos → uno` does not depend on what `tres.rs` \
         declares, so it is kept; got {reuse:?}"
    );
    assert_eq!(
        reuse.relationships_resolved, 1,
        "only `tres.rs` is affected, and it declares one relationship; \
         got {reuse:?}"
    );
}

/// Las tres rutas que construyen el grafo dicen lo mismo del grafo, así que
/// también tienen que decir lo mismo de cuánto conservaron.
///
/// Una ruta que publicara su propio número —o ninguno— haría que un
/// consumidor que llegara por ella no pudiera comparar su respuesta con la
/// de las otras: dos interfaces que contestan distinto son dos grafos.
#[tokio::test]
async fn las_tres_rutas_publican_la_misma_medida_de_reutilizacion() {
    let (_tmp, root) = proyecto();

    let whole = {
        let service = AnalysisService::new();
        service
            .build_project_graph(&root)
            .expect("the whole graph builds");
        service
            .get_reuse_metrics()
            .expect("the whole route publishes how much it reused")
    };

    let filtered = {
        let service = AnalysisService::new();
        service
            .build_project_graph_filtered(&root, &[&root])
            .expect("the filtered graph builds");
        service
            .get_reuse_metrics()
            .expect("the filtered route publishes how much it reused")
    };

    let asynchronous = {
        let service = AnalysisService::new();
        service
            .build_project_graph_async(&root)
            .await
            .expect("the async graph builds");
        service
            .get_reuse_metrics()
            .expect("the async route publishes how much it reused")
    };

    assert_eq!(
        whole, filtered,
        "the whole route and the filtered route published a different measure \
         of the same tree"
    );
    assert_eq!(
        whole, asynchronous,
        "the whole route and the async route published a different measure of \
         the same tree"
    );
}

/// El grafo publicado es el mismo grafo: en frío y en caliente, los mismos
/// símbolos y las mismas aristas. La reutilización es un ahorro de trabajo,
/// no un grafo distinto.
#[test]
fn el_grafo_en_caliente_es_el_mismo_grafo() {
    let (_tmp, root) = proyecto();
    let service = AnalysisService::new();

    service.build_project_graph(&root).expect("cold build");
    let frio_simbolos = simbolos(&service.get_project_graph());
    let frio_aristas = aristas(&service.get_project_graph());

    service.build_project_graph(&root).expect("warm build");
    let caliente_simbolos = simbolos(&service.get_project_graph());
    let caliente_aristas = aristas(&service.get_project_graph());

    assert_eq!(
        caliente_simbolos, frio_simbolos,
        "the same tree yields the same symbols warm and cold"
    );
    assert_eq!(
        caliente_aristas, frio_aristas,
        "the same tree yields the same edges warm and cold"
    );

    // Every edge names symbols the graph publishes. A kept answer names
    // the identity a declaration had when it was resolved, so an answer
    // that outlived its declaration would publish an edge to a symbol
    // that is not there — the graph would look complete and would have a
    // hole in it.
    let publicados: BTreeSet<String> = simbolos(&service.get_project_graph());
    for (origen, destino, _) in &caliente_aristas {
        assert!(
            publicados.contains(origen) && publicados.contains(destino),
            "an edge names a symbol the graph does not publish: {origen} -> {destino}"
        );
    }
}

/// El caso peligroso: un cambio puede **invalidar** una resolución que
/// antes valía. Si `tres.rs` pasa a declarar `uno` —el nombre que
/// `dos.rs` llama—, la llamada de `dos.rs` deja de poder probarse: ahora
/// hay dos homónimos y el grafo no puede cablearla.
///
/// Una reutilización que conservase esa respuesta publicaría una arista
/// que ya no existe. El grafo estaría «completo» y sería falso.
#[test]
fn un_cambio_puede_invalidar_una_resolucion_que_valia() {
    let (_tmp, root) = proyecto();
    let service = AnalysisService::new();

    service.build_project_graph(&root).expect("cold build");
    let antes = aristas(&service.get_project_graph());
    let dos = id_de(&service.get_project_graph(), "dos.rs", "dos").expect("dos");
    let uno = id_de(&service.get_project_graph(), "uno.rs", "uno").expect("uno");
    assert!(
        antes.contains(&(dos.clone(), uno.clone(), "Calls".to_string())),
        "the corpus starts with one proven relationship; before={antes:?}"
    );

    // `tres.rs` empieza a declarar `uno`, el nombre que `dos.rs` llama.
    reescribir(
        &root.join("tres.rs"),
        "pub fn tres() -> i32 {\n    dos()\n}\n\npub fn uno() -> i32 {\n    1\n}\n",
    );
    service.build_project_graph(&root).expect("warm build");

    let despues = aristas(&service.get_project_graph());
    let dos = id_de(&service.get_project_graph(), "dos.rs", "dos").expect("dos");
    let uno = id_de(&service.get_project_graph(), "uno.rs", "uno").expect("uno");
    assert!(
        !despues.contains(&(dos, uno, "Calls".to_string())),
        "the call names two homonyms now, so the edge cannot be proven; after={despues:?}"
    );

    let reuse = service
        .get_reuse_metrics()
        .expect("the build publishes how much it reused");
    assert_eq!(
        reuse.relationships_reused, 0,
        "the change touched the name the kept resolution looked for, so nothing \
         of it survives; got {reuse:?}"
    );
    assert_eq!(
        reuse.relationships_resolved, 2,
        "both relationships had to be looked up again; got {reuse:?}"
    );
}

/// Y el revés: si el homónimo desaparece, la relación vuelve a poder
/// probarse. Las dos caras del mismo cambio.
#[test]
fn un_cambio_puede_restaurar_una_resolucion_que_se_perdio() {
    let (_tmp, root) = proyecto();
    let service = AnalysisService::new();

    // `tres.rs` declara `uno`: la llamada de `dos.rs` no puede probarse.
    reescribir(
        &root.join("tres.rs"),
        "pub fn tres() -> i32 {\n    dos()\n}\n\npub fn uno() -> i32 {\n    1\n}\n",
    );
    service.build_project_graph(&root).expect("cold build");
    let antes = aristas(&service.get_project_graph());
    let dos = id_de(&service.get_project_graph(), "dos.rs", "dos").expect("dos");
    let uno = id_de(&service.get_project_graph(), "uno.rs", "uno").expect("uno");
    assert!(
        !antes.contains(&(dos.clone(), uno.clone(), "Calls".to_string())),
        "the corpus starts with two homonyms; before={antes:?}"
    );

    // `tres.rs` deja de declararlo: la llamada vuelve a tener un destino.
    reescribir(
        &root.join("tres.rs"),
        "pub fn tres() -> i32 {\n    dos()\n}\n",
    );
    service.build_project_graph(&root).expect("warm build");

    let despues = aristas(&service.get_project_graph());
    let dos = id_de(&service.get_project_graph(), "dos.rs", "dos").expect("dos");
    let uno = id_de(&service.get_project_graph(), "uno.rs", "uno").expect("uno");
    assert!(
        despues.contains(&(dos, uno, "Calls".to_string())),
        "the homonym is gone, so the edge can be proven again; after={despues:?}"
    );
}

/// Lo que el cambio tocó se ve en el grafo: el símbolo nuevo está y el
/// viejo no. Un grafo que se reutiliza sin mirar el cambio es un grafo
/// viejo.
#[test]
fn el_cambio_se_ve_en_el_grafo_publicado() {
    let (_tmp, root) = proyecto();
    let service = AnalysisService::new();

    service.build_project_graph(&root).expect("cold build");
    let antes = simbolos(&service.get_project_graph());

    reescribir(
        &root.join("tres.rs"),
        "pub fn tres() -> i32 {\n    dos()\n}\n\npub fn cuatro() -> i32 {\n    4\n}\n",
    );
    service.build_project_graph(&root).expect("warm build");
    let despues = simbolos(&service.get_project_graph());

    assert!(
        despues.len() == antes.len() + 1,
        "the edited file declares one more symbol; before={antes:?} after={despues:?}"
    );
    assert!(
        despues.iter().any(|s| s.contains("cuatro")),
        "the symbol the edit added is in the published graph; after={despues:?}"
    );
}

/// The rule that tells identity from mere presence.
///
/// A name that a changed file still declares is not necessarily a name
/// that changed: the file can change its BODY and declare exactly the
/// same declarations, at exactly the same lines. The identity of a
/// declaration is its path, its name and its line, and a body that
/// changes does not move the declaration.
///
/// A rule that invalidated every name a changed file declares would throw
/// away every answer that names one of them — for nothing. This test
/// pins the difference: changing the body of `uno` re-resolves nothing.
#[test]
fn cambiar_el_cuerpo_de_una_funcion_no_resuelve_nada() {
    let (_tmp, root) = proyecto();
    let service = AnalysisService::new();

    service.build_project_graph(&root).expect("cold build");

    // The body of `uno` changes; its declaration does not move.
    reescribir(&root.join("uno.rs"), "pub fn uno() -> i32 {\n    2\n}\n");
    service.build_project_graph(&root).expect("warm build");

    let reuse = service
        .get_reuse_metrics()
        .expect("the build publishes how much it reused");

    assert_eq!(
        reuse.relationships_resolved, 0,
        "the declaration of `uno` did not move, so nothing that names it          changed; got {reuse:?}"
    );
    assert_eq!(
        reuse.relationships_reused, 2,
        "both relationships name declarations that did not move; got {reuse:?}"
    );
}

/// The other half: a declaration that MOVES is another declaration, even
/// though the name is the same.
///
/// The identity of a declaration includes the line it sits on. When the
/// declaration moves, the answer the ledger kept names an identity that
/// no longer exists — keeping it would publish an edge to a symbol that
/// is not in the graph. So the name is touched and the relationship is
/// looked up again.
#[test]
fn una_declaracion_que_se_muda_cambia_su_identidad() {
    let (_tmp, root) = proyecto();
    let service = AnalysisService::new();

    service.build_project_graph(&root).expect("cold build");

    // A line in front of the declaration moves it down one line.
    reescribir(
        &root.join("uno.rs"),
        "// the declaration moves down one line\npub fn uno() -> i32 {\n    1\n}\n",
    );
    service.build_project_graph(&root).expect("warm build");

    let reuse = service
        .get_reuse_metrics()
        .expect("the build publishes how much it reused");

    assert_eq!(
        reuse.relationships_resolved, 1,
        "the declaration of `uno` moved, so the call that names it has to be          looked up again; got {reuse:?}"
    );
    assert_eq!(
        reuse.relationships_reused, 1,
        "the other relationship names a declaration that did not move; got {reuse:?}"
    );

    // And the edge the graph publishes points at the declaration that is
    // there now, not at the one that was there before.
    let graph = service.get_project_graph();
    let dos = id_de(&graph, "dos.rs", "dos").expect("dos");
    let uno = id_de(&graph, "uno.rs", "uno").expect("uno");
    assert!(
        aristas(&graph).contains(&(dos, uno, "Calls".to_string())),
        "the edge points at the declaration that exists now"
    );
}

/// A file that arrives is not a change to the whole project.
///
/// The declarations a new file brings are new to the project, so the names
/// they answer to are touched. The names nobody changed are not: a file
/// that declares something no one calls does not alter any resolution.
#[test]
fn un_fichero_nuevo_no_invalida_lo_que_no_toca() {
    let (_tmp, root) = proyecto();
    let service = AnalysisService::new();

    service.build_project_graph(&root).expect("cold build");

    reescribir(
        &root.join("cuatro.rs"),
        "pub fn cuatro() -> i32 {\n    4\n}\n",
    );
    service.build_project_graph(&root).expect("warm build");

    let reuse = service
        .get_reuse_metrics()
        .expect("the build publishes how much it reused");

    assert_eq!(
        reuse.relationships_resolved, 0,
        "the new file declares a name that no call looks for; got {reuse:?}"
    );
    assert_eq!(
        reuse.relationships_reused, 2,
        "both relationships name declarations that did not change; got {reuse:?}"
    );
}
