//! Las relaciones Kotlin↔Java sólo cuando se pueden probar.
//!
//! MEDIDO sobre el corpus versionado `sandbox/fixtures/kotlin-java/`: el
//! producto lee SEIS llamadas y publica CUATRO relaciones. Las dos que no
//! publica son exactamente las dos que no se pueden probar:
//!
//!   - `demo/gamma/Caller.kt` llama a `run`, y `run` está declarado tres
//!     veces: `demo/alpha/Service.kt`, `demo/alpha/Service.java` y
//!     `demo/beta/Service.kt` — dos en Kotlin y una en Java, en dos
//!     paquetes. El nombre no dice cuál, y el grafo no puede elegir uno
//!     sin inventarlo.
//!   - `demo/gamma/UseHandler.kt` llama a `invoke`, y `invoke` está
//!     declarado dos veces en `demo/delta/JHandler.java`. El cuerpo de la
//!     llamada dice `invoke()` y el otro dice `invoke(int)`, pero el
//!     pipeline sólo transporta el NOMBRE del callee —`FileFacts.calls`
//!     es `Vec<(Symbol, String)>`—, así que la aridad que sí distinguiría
//!     los dos overloads no llega al resolvedor.
//!
//! El primer caso NO lleva import a propósito: `demo.alpha` declara una
//! clase `Service` en Kotlin y otra en Java, así que cualquier import de
//! `run` sería ambiguo en el mismo paquete que lo declara. La ambigüedad
//! no es un descuido del corpus: es lo que el caso mide.
//!
//! Las otras cuatro son probables y se publican: Kotlin→Java, Java→Kotlin,
//! Kotlin→Kotlin y fichero a fichero. Son las que hacen que este fichero
//! NO sea un PASS vacío: sin ellas, «no sale ninguna arista» se cumpliría
//! igual con un corpus que no se lee.
//!
//! Lo que este fichero fija:
//!
//!   1. El corpus declara los homónimos y los overloads que dice
//!      declarar, y los llamadores existen (si no, la negativa sería
//!      vacía).
//!   2. Las llamadas que no se pueden probar SÍ se extraen: la ausencia
//!      de la arista es una negativa del resolvedor, no un olvido del
//!      extractor.
//!   3. Ninguna relación nace de un homónimo ni de un overload.
//!   4. Las relaciones probables se publican, y son las ÚNICAS que se
//!      publican.
//!   5. `TargetResolution` tiene TRES variantes, sólo `Resolved` nombra un
//!      destino, y el fichero dice por qué no hay una cuarta.
//!
//! Lo que este fichero NO dice:
//!
//!   - Que el corpus Java↔Kotlin completo esté resuelto: el corpus es un
//!     oráculo, no una muestra.
//!   - Que la pérdida de una relación se reporte. MEDIDO: las dos
//!     referencias refusadas no aparecen ni como arista ni como
//!     pendiente, y el estado del build dice `Complete`. Ese hueco está
//!     enrutado a otra unidad, fuera del alcance de esta; este contrato
//!     fija las aristas, no la cobertura.
//!   - Que la CLI y el MCP publiquen lo mismo que este contrato. La
//!     equivalencia CLI/MCP medida hoy cubre Terraform, no este corpus.

use cognicode_core::application::services::analysis_service::AnalysisService;
use cognicode_core::domain::aggregates::call_graph::{CallGraph, SymbolId};
use cognicode_core::domain::aggregates::symbol::Symbol;
use cognicode_core::domain::services::resolve_target;
use cognicode_core::domain::value_objects::TargetResolution;
use cognicode_core::infrastructure::parser::{Language, TreeSitterParser};
use std::collections::{BTreeSet, HashMap};
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// El corpus Kotlin/Java del repo: doce ficheros, dos lenguajes.
fn corpus() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../sandbox/fixtures/kotlin-java")
        .canonicalize()
        .expect("el corpus Kotlin/Java está donde el repo dice que está")
}

/// Un camino del corpus, relativo a su raíz.
///
/// Las etiquetas de este fichero son `nombre@camino-relativo`: relative
/// para que el corpus se pueda mover sin reescribir el contrato, y
/// relativo para que `demo/alpha/Service.kt` y `demo/beta/Service.kt` no
/// se confundan sólo por el nombre del fichero.
fn rel_of(file: &str) -> String {
    let root = corpus().display().to_string();
    file.strip_prefix(&format!("{root}/"))
        .unwrap_or(file)
        .to_string()
}

/// La etiqueta de un símbolo: su nombre y su fichero.
fn label_of(symbol: &Symbol) -> String {
    format!("{}@{}", symbol.name(), rel_of(symbol.location().file()))
}

/// Los ficheros del corpus, en orden.
fn corpus_files() -> Vec<PathBuf> {
    fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
        let Ok(entries) = std::fs::read_dir(dir) else {
            return;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                walk(&path, out);
            } else {
                out.push(path);
            }
        }
    }
    let mut out = Vec::new();
    walk(&corpus(), &mut out);
    out.sort();
    out
}

/// El lenguaje de un fichero del corpus.
fn language_of(path: &Path) -> Language {
    match path.extension().and_then(|ext| ext.to_str()) {
        Some("kt") => Language::Kotlin,
        Some("java") => Language::Java,
        other => panic!("el corpus sólo tiene ficheros .kt y .java, no {:?}", other),
    }
}

/// Las llamadas que el extractor lee de UN fichero.
///
/// Se usa el MISMO lector que el producto usa (`TreeSitterParser`), no
/// un grep sobre el texto: lo que se afirma es que el extractor VE la
/// llamada, no que la llamada está escrita en el fichero.
fn calls_of(path: &Path) -> Vec<(String, String)> {
    let source = std::fs::read_to_string(path).expect("el corpus se lee");
    let parser = TreeSitterParser::new(language_of(path)).expect("la gramática se carga");
    let tree = parser.parse_tree(&source).expect("el corpus parsea");
    assert!(
        !TreeSitterParser::has_error_nodes(&tree),
        "{} tiene que parsear sin nodos de error",
        rel_of(path.to_str().unwrap())
    );
    let calls = parser
        .find_call_relationships_in_tree(&tree, &source, path.to_str().unwrap())
        .expect("las llamadas se leen del árbol");
    calls
        .into_iter()
        .map(|(caller, callee)| {
            (
                format!("{}@{}", caller.name(), rel_of(caller.location().file())),
                callee,
            )
        })
        .collect()
}

/// TODAS las llamadas del corpus: `(llamador, nombre-del-callee)`.
fn corpus_calls() -> Vec<(String, String)> {
    let mut out: Vec<(String, String)> = corpus_files()
        .iter()
        .flat_map(|file| calls_of(file))
        .collect();
    out.sort();
    out
}

/// El grafo que publica la estrategia de proyecto completo.
fn full_graph() -> Arc<CallGraph> {
    let service = AnalysisService::new();
    service
        .build_graph_with_strategy(&corpus(), "full")
        .expect("el grafo de proyecto se construye");
    service.get_project_graph()
}

/// El grafo que publica la estrategia por fichero.
fn per_file_graph() -> Arc<CallGraph> {
    let service = AnalysisService::new();
    Arc::new(
        service
            .build_graph_per_file_with_report(&corpus())
            .expect("el grafo por fichero se construye")
            .graph,
    )
}

/// Las relaciones del grafo, como pares de etiquetas.
fn relations_of(graph: &CallGraph) -> BTreeSet<(String, String)> {
    let labels: HashMap<String, String> = graph
        .symbol_ids()
        .map(|(id, symbol)| (id.as_str().to_string(), label_of(symbol)))
        .collect();
    graph
        .all_dependencies()
        .map(|(from, to, _)| {
            let source = labels
                .get(from.as_str())
                .cloned()
                .unwrap_or_else(|| from.as_str().to_string());
            let target = labels
                .get(to.as_str())
                .cloned()
                .unwrap_or_else(|| to.as_str().to_string());
            (source, target)
        })
        .collect()
}

/// Los símbolos del grafo que se llaman así.
fn declarations_of(graph: &CallGraph, name: &str) -> Vec<String> {
    let mut out: Vec<String> = graph
        .symbols()
        .filter(|symbol| symbol.name() == name)
        .map(label_of)
        .collect();
    out.sort();
    out
}

/// Las CUATRO relaciones que el corpus permite probar.
///
/// Kotlin→Java, Java→Kotlin, Kotlin→Kotlin y fichero a fichero. Las
/// dos negaciones del contrato NO aparecen aquí, y su ausencia es
/// justamente lo que los otros dos ficheros comprueban.
fn provable_relations() -> BTreeSet<(String, String)> {
    [
        (
            "consumeJava@demo/provable/JavaConsumer.kt".to_string(),
            "uniqueJavaTarget@demo/provable/JavaTarget.java".to_string(),
        ),
        (
            "call@demo/provable/JavaCaller.java".to_string(),
            "uniqueKotlinTarget@demo/provable/KotlinTarget.kt".to_string(),
        ),
        (
            "consumeKotlin@demo/provable/KotlinConsumer.kt".to_string(),
            "uniqueKotlinTarget@demo/provable/KotlinTarget.kt".to_string(),
        ),
        (
            "localCaller@demo/local/Local.kt".to_string(),
            "helper@demo/local/Local.kt".to_string(),
        ),
    ]
    .into_iter()
    .collect()
}

/// Lo que el dominio dice de un nombre: sólo `Resolved` nombra un
/// destino.
///
/// El `match` NO tiene brazo `_`. Eso es la forma más fuerte de fijar la
/// forma de un enum: una cuarta variante rompe la COMPILACIÓN, que es un
/// fallo que ni el CI puede dejar pasar por alto.
fn named_target(resolution: &TargetResolution) -> Option<&SymbolId> {
    match resolution {
        TargetResolution::Resolved(id) => Some(id),
        TargetResolution::Ambiguous { .. } => None,
        TargetResolution::Unresolved(_) => None,
    }
}

#[test]
fn the_corpus_declares_the_homonyms_and_the_overloads_it_is_about() {
    let graphs = [("full", full_graph()), ("per_file", per_file_graph())];

    for (strategy, graph) in &graphs {
        let runs = declarations_of(graph, "run");
        assert_eq!(
            runs,
            vec![
                "run@demo/alpha/Service.java".to_string(),
                "run@demo/alpha/Service.kt".to_string(),
                "run@demo/beta/Service.kt".to_string(),
            ],
            "[{strategy}] `run` tiene que estar declarado tres veces —dos en Kotlin y una en Java— o la negativa de este contrato no se puede probar"
        );

        let invokes = declarations_of(graph, "invoke");
        assert_eq!(
            invokes,
            vec![
                "invoke@demo/delta/JHandler.java".to_string(),
                "invoke@demo/delta/JHandler.java".to_string(),
            ],
            "[{strategy}] `invoke` tiene que estar declarado dos veces en el mismo fichero Java o el caso del overload no se puede probar"
        );

        for caller in [
            "caller@demo/gamma/Caller.kt",
            "useHandler@demo/gamma/UseHandler.kt",
        ] {
            assert!(
                declarations_of(graph, caller.split('@').next().unwrap())
                    .iter()
                    .any(|label| label == caller),
                "[{strategy}] el llamador `{caller}` tiene que existir en el grafo: sin él, «no sale ninguna arista» se cumpliría por falta de quien llama"
            );
        }
    }
}

#[test]
fn the_calls_that_cannot_be_proved_are_still_extracted() {
    let calls = corpus_calls();

    assert_eq!(
        calls.len(),
        6,
        "el corpus tiene que declarar seis llamadas: cuatro probables y dos no probables. Si el extractor deja de ver alguna, este contrato deja de poder distinguir una negativa de un olvido"
    );

    for expected in [
        ("caller@demo/gamma/Caller.kt", "run"),
        ("useHandler@demo/gamma/UseHandler.kt", "invoke"),
        (
            "consumeKotlin@demo/provable/KotlinConsumer.kt",
            "uniqueKotlinTarget",
        ),
        (
            "consumeJava@demo/provable/JavaConsumer.kt",
            "uniqueJavaTarget",
        ),
        ("call@demo/provable/JavaCaller.java", "uniqueKotlinTarget"),
        ("localCaller@demo/local/Local.kt", "helper"),
    ] {
        assert!(
            calls.contains(&(expected.0.to_string(), expected.1.to_string())),
            "el extractor tiene que ver la llamada {expected:?}, con el nombre exacto"
        );
    }
}

#[test]
fn no_relation_is_born_for_a_homonym() {
    for (strategy, graph) in [("full", full_graph()), ("per_file", per_file_graph())] {
        let relations = relations_of(&graph);

        let born: Vec<(String, String)> = relations
            .iter()
            .filter(|(source, _)| source == "caller@demo/gamma/Caller.kt")
            .cloned()
            .collect();
        assert!(
            born.is_empty(),
            "[{strategy}] `run` está declarado tres veces y el grafo no puede probar cuál: no puede nacer ninguna arista desde `caller`, y nacieron {born:?}"
        );
    }
}

#[test]
fn no_relation_is_born_for_an_overload() {
    for (strategy, graph) in [("full", full_graph()), ("per_file", per_file_graph())] {
        let relations = relations_of(&graph);

        let born: Vec<(String, String)> = relations
            .iter()
            .filter(|(source, _)| source == "useHandler@demo/gamma/UseHandler.kt")
            .cloned()
            .collect();
        assert!(
            born.is_empty(),
            "[{strategy}] `invoke` está declarado dos veces y el pipeline no transporta la aridad que los distinguiría: no puede nacer ninguna arista desde `useHandler`, y nacieron {born:?}"
        );
    }
}

#[test]
fn the_provable_relations_are_published() {
    for (strategy, graph) in [("full", full_graph()), ("per_file", per_file_graph())] {
        let relations = relations_of(&graph);

        for expected in provable_relations() {
            assert!(
                relations.contains(&expected),
                "[{strategy}] la relación {expected:?} es probable y tiene que estar en el grafo; las que hay son {relations:?}"
            );
        }
    }
}

#[test]
fn the_graph_publishes_no_relation_beyond_the_ones_it_can_prove() {
    for (strategy, graph) in [("full", full_graph()), ("per_file", per_file_graph())] {
        let relations = relations_of(&graph);

        assert_eq!(
            relations,
            provable_relations(),
            "[{strategy}] el grafo tiene que publicar exactamente las relaciones que puede probar y NINGUNA más"
        );
    }
}

#[test]
fn every_call_is_published_or_is_provably_unpublishable() {
    let graph = full_graph();
    let calls = corpus_calls();

    // Las relaciones publicadas, con el NOMBRE del callee y no su
    // etiqueta completa: lo que se compara es la llamada, no el fichero.
    let published: BTreeSet<(String, String)> = relations_of(&graph)
        .iter()
        .map(|(source, target)| {
            let callee = target.split('@').next().unwrap_or(target).to_string();
            (source.clone(), callee)
        })
        .collect();

    for (caller, callee) in &calls {
        if published.contains(&(caller.clone(), callee.clone())) {
            continue;
        }

        match resolve_target(&graph, callee) {
            TargetResolution::Ambiguous { candidates, .. } => assert!(
                candidates.len() > 1,
                "la llamada {caller} -> {callee} no se publica, y el dominio dice que su nombre es ambiguo con {} candidatos: un solo candidato se podría publicar",
                candidates.len()
            ),
            other => panic!(
                "la llamada {caller} -> {callee} no se publica pero su nombre NO es ambiguo ({other:?}): una llamada no publicada tiene que ser una negativa probable, no un olvido"
            ),
        }
    }
}

#[test]
fn only_resolved_names_a_target() {
    let graph = full_graph();

    let resolved = resolve_target(&graph, "uniqueKotlinTarget");
    let target = named_target(&resolved).expect("un nombre único nombra un destino");
    assert!(
        graph.get_symbol(target).is_some(),
        "el destino que nombra `Resolved` tiene que estar en el grafo"
    );

    let ambiguous = resolve_target(&graph, "run");
    assert!(
        named_target(&ambiguous).is_none(),
        "una llamada ambigua NO nombra un destino: ése es el mecanismo que impide que nazca una arista por coincidencia de nombre"
    );
    match &ambiguous {
        TargetResolution::Ambiguous { name, candidates } => {
            assert_eq!(
                name, "run",
                "la variante ambigua dice qué nombre no se pudo probar"
            );
            assert_eq!(
                candidates.len(),
                3,
                "la variante ambigua CONSERVA los candidatos: un consumidor tiene que poder surfazar el hueco"
            );
        }
        other => panic!("`run` es ambiguo en este corpus, no {:?}", other),
    }

    let unresolved = resolve_target(&graph, "nombre_que_no_existe_en_este_corpus");
    assert!(
        named_target(&unresolved).is_none(),
        "un nombre que no existe NO nombra un destino"
    );
}

#[test]
fn the_shape_is_three_variants_and_the_reason_says_why() {
    let source = include_str!("../src/domain/value_objects/target_resolution.rs");

    let body = source
        .split_once("pub enum TargetResolution {")
        .expect("el enum TargetResolution existe")
        .1;

    assert!(
        body.contains("Resolved(SymbolId)"),
        "`TargetResolution` tiene que seguir declarando la variante `Resolved`"
    );
    assert!(
        body.contains("Ambiguous {"),
        "`TargetResolution` tiene que seguir declarando la variante `Ambiguous`"
    );
    assert!(
        body.contains("Unresolved(String)"),
        "`TargetResolution` tiene que seguir declarando la variante `Unresolved`"
    );
    assert!(
        !body.contains("External"),
        "`TargetResolution` NO tiene una variante `External`: no hay tabla de símbolos externos, y una variante que nadie puede construir es una hipótesis que sobrevive más que el código que debería haberla llenado"
    );

    assert!(
        source.contains("## Why no `External` variant"),
        "la razón de por qué son tres y no cuatro no se puede borrar en silencio: es la mitad de la respuesta que la unidad debe dar"
    );
}
