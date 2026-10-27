//! K1 (línea Kotlin) — el contrato de símbolos.
//!
//! MEDIDO: `tree-sitter-kotlin-sg` 0.4.1 no expone NINGÚN
//! campo `name`: el nombre de una declaración es un HIJO, y su posición
//! depende del constructo. Por eso Kotlin necesita una regla de nombre
//! por lenguaje (`NameRule`), y por eso los DOS lectores de un fichero
//! Kotlin —el extractor genérico y el parser anterior— tienen que dar
//! la MISMA respuesta. Si no, el grafo y el índice de símbolos dirían
//! cosas distintas del mismo fichero, que es la forma exacta del
//! defecto que ya se pagó una vez con `EvidenceStore`.
//!
//! Este fichero fija cuatro cosas:
//!
//!   1. Los dos lectores publican lo mismo.
//!   2. La clasificación es explícita: función top-level, clase,
//!      interface, data class, `object`, companion con nombre y
//!      companion anónimo, clase anidada, enum, propiedad, constructor
//!      secundario, función de extensión, sobrecarga y paquete.
//!   3. La identidad es canónica: dos sobrecargas son DOS símbolos, y
//!      el constructor secundario no se funde con la clase que lo
//!      contiene.
//!   4. Un fichero Kotlin que NO parsea no se publica como leído.
//!
//! Y una quinta, que es la del lenguaje y no del parser: un script
//! `.kts` se lee como fuente Kotlin, pero con SUS LÍMITES —el producto
//! no simula la compilación de la DSL de Gradle.
//!
//! El corpus es `sandbox/fixtures/kotlin/Shapes.kt`: el mismo fichero
//! que consume la UAT efímera, para que el contrato y la aceptación
//! miren el mismo código.

use cognicode_core::application::ingest::extractor::extract_file;
use cognicode_core::application::services::analysis_service::AnalysisService;
use cognicode_core::domain::value_objects::{NodeKind, SymbolKind};
use cognicode_core::infrastructure::graph::per_file_graph::{BuildStatus, SkipReason};
use cognicode_core::infrastructure::parser::language_config::LanguageConfig;
use cognicode_core::infrastructure::parser::{Language, TreeSitterParser};
use cognicode_core::infrastructure::semantic::{OutlineNode, SymbolCodeService, build_outline};
use std::path::{Path, PathBuf};

/// El corpus de Kotlin del repo.
fn fixture() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../sandbox/fixtures/kotlin/Shapes.kt")
        .canonicalize()
        .expect("el corpus Kotlin está donde el repo dice que está")
}

/// Un script de Gradle: Kotlin, pero NO Kotlin de proyecto.
fn gradle_script() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../sandbox/fixtures/kotlin/setup.kts")
        .canonicalize()
        .expect("el script Kotlin está donde el repo dice que está")
}

/// Un script de pipeline: la otra forma de script que el contrato
/// nombra (`.gradle.kts`/`.pipeline.kts`).
fn pipeline_script() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../sandbox/fixtures/kotlin/.pipeline.kts")
        .canonicalize()
        .expect("el script de pipeline está donde el repo dice que está")
}

/// Una declaración: su clase, su nombre y su línea.
type Declaration = (SymbolKind, String, u32);

/// El orden de las clases, para que dos listas se puedan comparar.
///
/// `SymbolKind` no es ordenable —no hace falta que lo sea en el
/// producto—, así que el orden lo pone esta función y no el tipo.
fn kind_rank(kind: SymbolKind) -> u8 {
    match kind {
        SymbolKind::Class => 0,
        SymbolKind::Function => 1,
        SymbolKind::Variable => 2,
        _ => 3,
    }
}

fn sort_declarations(declarations: &mut [Declaration]) {
    declarations.sort_by_key(|(kind, name, line)| (kind_rank(*kind), name.clone(), *line));
}

/// Lo que un lector publica de un fichero.
///
/// El lector antiguo da la LÍNEA en base 0 y el extractor genérico en
/// base 1 (la convención fact-side). Para comparar los dos se normaliza
/// a base 1: lo que se compara es QUÉ se declara, no cómo cada lado
/// cuenta las líneas.
fn declarations_from_extractor(path: &Path, source: &str) -> Vec<Declaration> {
    let config = LanguageConfig::for_language(Language::Kotlin);
    let result = extract_file(config, path, source, "hash");
    assert!(
        result.error.is_none(),
        "el extractor no puede fallar sobre un fichero Kotlin válido: {:?}",
        result.error
    );
    let mut out: Vec<Declaration> = result
        .nodes
        .iter()
        .filter_map(|node| match node.kind {
            NodeKind::Symbol(SymbolKind::File) => None,
            NodeKind::Symbol(kind) => Some((
                kind,
                node.label.clone(),
                node.properties
                    .get("line")
                    .and_then(|value| value.as_str())
                    .and_then(|text| text.parse::<u32>().ok())
                    .unwrap_or_default(),
            )),
            _ => None,
        })
        .collect();
    sort_declarations(&mut out);
    out
}

fn declarations_from_parser(source: &str, path: &str) -> Vec<Declaration> {
    let parser = TreeSitterParser::new(Language::Kotlin).expect("la gramática de Kotlin se carga");
    let tree = parser.parse_tree(source).expect("el corpus Kotlin parsea");
    assert!(
        !TreeSitterParser::has_error_nodes(&tree),
        "el corpus Kotlin tiene que parsear sin nodos de error"
    );
    let symbols = parser
        .find_all_symbols_in_tree(&tree, source, path)
        .expect("los símbolos se leen del árbol");
    let mut out: Vec<Declaration> = symbols
        .iter()
        .map(|symbol| {
            (
                *symbol.kind(),
                symbol.name().to_string(),
                symbol.location().line() + 1,
            )
        })
        .collect();
    sort_declarations(&mut out);
    out
}

#[test]
fn the_two_readers_publish_the_same_kotlin_declarations() {
    // La garantía que sostiene todo lo demás: un fichero Kotlin tiene
    // UNA respuesta sobre qué declara, no dos. Si un lector publica
    // algo que el otro no, el grafo y el índice de símbolos cuentan
    // ficheros distintos.
    let path = fixture();
    let source = std::fs::read_to_string(&path).unwrap();

    let from_extractor = declarations_from_extractor(&path, &source);
    let from_parser = declarations_from_parser(&source, path.to_str().unwrap());

    assert!(
        !from_extractor.is_empty(),
        "un fichero Kotlin con contenido declara algo; un lector vacío \
         significa que la gramática no se está leyendo"
    );
    assert_eq!(
        from_extractor, from_parser,
        "los dos lectores de Kotlin tienen que publicar lo mismo"
    );
}

/// Lo que el tercer lector —el esquema jerárquico— publica de un fichero.
///
/// El esquema es un lector más del MISMO fichero. Si publica otra cosa,
/// el grafo, el índice de símbolos y el esquema cuentan tres ficheros
/// distintos, y el consumidor ve tres respuestas sobre un código que
/// tiene una.
///
/// La línea no se compara: el esquema la cuenta en base 0 y el extractor
/// en base 1. Lo que tiene que coincidir es QUÉ se declara y de qué
/// clase.
fn declarations_from_outline(path: &Path, source: &str) -> Vec<(SymbolKind, String)> {
    fn walk(nodes: &[OutlineNode], out: &mut Vec<(SymbolKind, String)>) {
        for node in nodes {
            out.push((node.kind, node.name.clone()));
            walk(&node.children, out);
        }
    }

    let outline = build_outline(source, path.to_str().unwrap(), Language::Kotlin, true, true);
    let mut out = Vec::new();
    walk(&outline, &mut out);
    out.sort_by_key(|(kind, name)| (kind_rank(*kind), name.clone()));
    out
}

#[test]
fn the_outline_reader_publishes_the_same_kotlin_declarations() {
    // MEDIDO: el esquema era un TERCER lector que
    // preguntaba «un tipo por categoría» y nombraba buscando un
    // `identifier`. La gramática de Kotlin no tiene ninguno de los dos:
    // declara tres tipos de función y tres de clase, y nombra con
    // `simple_identifier` o `type_identifier` según el constructo. El
    // resultado era un esquema VACÍO de un fichero que sí declara
    // símbolos —la respuesta falsa que K0 vowed no dar.
    let path = fixture();
    let source = std::fs::read_to_string(&path).unwrap();

    let from_extractor: Vec<(SymbolKind, String)> = declarations_from_extractor(&path, &source)
        .into_iter()
        .map(|(kind, name, _)| (kind, name))
        .collect();
    let from_outline = declarations_from_outline(&path, &source);

    assert!(
        !from_outline.is_empty(),
        "el esquema de un fichero Kotlin que declara símbolos no puede \
         estar vacío: el lector no está leyendo la gramática"
    );
    assert_eq!(
        from_extractor, from_outline,
        "el esquema y el extractor tienen que publicar lo mismo del mismo \
         fichero Kotlin"
    );
}

#[test]
fn the_kotlin_declaration_vocabulary_is_classified_explicitly() {
    // El vocabulario completo, con su clase y su LÍNEA. La línea está
    // porque la identidad canónica es `fichero:nombre:línea`: sin ella,
    // dos declaraciones con el mismo nombre son la misma, y dos
    // sobrecargas dejarían de ser dos.
    let path = fixture();
    let source = std::fs::read_to_string(&path).unwrap();
    let declarations = declarations_from_extractor(&path, &source);

    let mut expected: Vec<Declaration> = vec![
        // Clases: interface, data class, clase anidada, clase,
        // companion con nombre, companion anónimo, enum, `object`,
        // sealed y value class.
        (SymbolKind::Class, "Circle".to_string(), 25),
        (SymbolKind::Class, "Color".to_string(), 36),
        (SymbolKind::Class, "Factory".to_string(), 29),
        (SymbolKind::Class, "Filled".to_string(), 18),
        (SymbolKind::Class, "Money".to_string(), 69),
        (SymbolKind::Class, "Node".to_string(), 67),
        (SymbolKind::Class, "Plain".to_string(), 43),
        (SymbolKind::Class, "Plain.Companion".to_string(), 44),
        (SymbolKind::Class, "Rect".to_string(), 12),
        (SymbolKind::Class, "Registry".to_string(), 49),
        (SymbolKind::Class, "Shape".to_string(), 7),
        // Funciones: método de interface, override, constructor
        // secundario, método de companion, método de object, función
        // top-level, extensión y las dos sobrecargas.
        (SymbolKind::Function, "Rect".to_string(), 16),
        (SymbolKind::Function, "describe".to_string(), 9),
        (SymbolKind::Function, "describe".to_string(), 22),
        (SymbolKind::Function, "largest".to_string(), 59),
        (SymbolKind::Function, "make".to_string(), 45),
        (SymbolKind::Function, "overload".to_string(), 63),
        (SymbolKind::Function, "overload".to_string(), 65),
        (SymbolKind::Function, "register".to_string(), 52),
        (SymbolKind::Function, "shout".to_string(), 61),
        (SymbolKind::Function, "unit".to_string(), 32),
        // Propiedades: de interface, de constructor (`val` en el
        // constructor primario), de clase, de clase anidada, `const`,
        // de enum, de value class y top-level.
        (SymbolKind::Variable, "DEFAULT_RADIUS".to_string(), 30),
        (SymbolKind::Variable, "DEFAULT_SHAPE".to_string(), 57),
        (SymbolKind::Variable, "amount".to_string(), 69),
        (SymbolKind::Variable, "area".to_string(), 8),
        (SymbolKind::Variable, "area".to_string(), 13),
        (SymbolKind::Variable, "area".to_string(), 26),
        (SymbolKind::Variable, "height".to_string(), 12),
        (SymbolKind::Variable, "label".to_string(), 19),
        (SymbolKind::Variable, "radius".to_string(), 25),
        (SymbolKind::Variable, "rgb".to_string(), 36),
        (SymbolKind::Variable, "shapes".to_string(), 50),
        (SymbolKind::Variable, "width".to_string(), 12),
        // El paquete: la declaración a la que pertenece el fichero. K-SYM-03
        // la nombra la primera, y sin ella la identidad de un símbolo
        // Kotlin no sabe de qué paquete habla.
        (SymbolKind::Package, "com.example.shapes".to_string(), 1),
    ];
    sort_declarations(&mut expected);

    assert_eq!(
        declarations, expected,
        "el vocabulario de Kotlin cambió. Si el cambio es a propósito, \
         actualiza ESTA lista y explica por qué; si no lo es, el lector \
         ha dejado de declarar algo o ha declarado algo de más."
    );
}

#[test]
fn two_overloads_are_two_symbols_and_the_constructor_is_not_its_class() {
    // K-SYM-05: la identidad es canónica y las sobrecargas NO se
    // unifican por nombre simple. Y el constructor secundario lleva el
    // nombre de la clase que lo envuelve, pero NO es la clase: son dos
    // símbolos con dos identidades.
    let path = fixture();
    let source = std::fs::read_to_string(&path).unwrap();
    let declarations = declarations_from_extractor(&path, &source);

    let overloads: Vec<&Declaration> = declarations
        .iter()
        .filter(|(_, name, _)| name == "overload")
        .collect();
    assert_eq!(
        overloads.len(),
        2,
        "las dos sobrecargas de `overload` son dos símbolos: unif\
         icarlas por nombre simple pierde una declaración"
    );
    assert_ne!(
        overloads[0].2, overloads[1].2,
        "las dos sobrecargas están en líneas distintas: con la misma \
         línea serían el MISMO símbolo por la identidad `fichero:nombre:línea`"
    );

    let rect: Vec<&Declaration> = declarations
        .iter()
        .filter(|(_, name, _)| name == "Rect")
        .collect();
    assert_eq!(
        rect.len(),
        2,
        "`Rect` aparece dos veces: como clase y como constructor \
         secundario. Publicar el constructor primario además de la clase \
         produciría dos símbolos con la MISMA identidad, porque comparten \
         nombre y línea."
    );
    assert!(
        rect.iter().any(|(kind, _, _)| *kind == SymbolKind::Class),
        "una de las dos es la clase"
    );
    assert!(
        rect.iter()
            .any(|(kind, _, _)| *kind == SymbolKind::Function),
        "la otra es el constructor secundario"
    );
}

#[test]
fn the_kotlin_package_is_published_as_its_own_declaration() {
    // K-SYM-03 nombra `package` la PRIMERA de las declarations que hay
    // que reconocer, y K-SYM-05 la pone en la identidad: «package +
    // owner + kind + signature». Un fichero Kotlin que no publica su
    // paquete no puede decir de qué paquete habla ninguno de sus
    // símbolos.
    //
    // MEDIDO: ANTES de esto el paquete NO se publicaba
    // y este test fallaba. La gramática lo soporta —`package_header` con
    // un único `identifier` que lleva la ruta con puntos—; lo que faltaba
    // era que un LENGUAJE dijera cuál es su nodo de paquete, para que los
    // tres lectores lo publicaran sin saber qué es Kotlin.
    let path = fixture();
    let source = std::fs::read_to_string(&path).unwrap();
    let declarations = declarations_from_extractor(&path, &source);

    let packages: Vec<&Declaration> = declarations
        .iter()
        .filter(|(kind, _, _)| *kind == SymbolKind::Package)
        .collect();
    assert_eq!(
        packages.len(),
        1,
        "un fichero Kotlin declara un paquete, y el paquete es una \
         declaración más"
    );
    assert_eq!(
        packages[0].1, "com.example.shapes",
        "el paquete se publica con su ruta con puntos, no con su última \
         parte: `com.example.shapes` y `shapes` son paquetes distintos"
    );
    assert_eq!(
        packages[0].2, 1,
        "el paquete está en la línea en la que se declara"
    );
}

/// El código que el producto publica para el símbolo de ese nombre.
///
/// La columna es la que publica el lector —base 0, como la línea que
/// cuenta—, porque lo que se prueba aquí es QUÉ código se devuelve, no
/// en qué base se cuentan las coordenadas.
fn code_of(service: &SymbolCodeService, path: &Path, source: &str, name: &str) -> String {
    let parser = TreeSitterParser::new(Language::Kotlin).expect("la gramática de Kotlin se carga");
    let tree = parser.parse_tree(source).expect("el corpus Kotlin parsea");
    let symbols = parser
        .find_all_symbols_in_tree(&tree, source, path.to_str().unwrap())
        .expect("los símbolos se leen del árbol");
    let symbol = symbols
        .iter()
        .find(|symbol| symbol.name() == name)
        .unwrap_or_else(|| panic!("el símbolo `{name}` no está en el corpus Kotlin"));
    let location = symbol.location();
    service
        .get_symbol_code(
            path.to_str().unwrap(),
            location.line() + 1,
            location.column(),
        )
        .unwrap_or_else(|error| panic!("el código de `{name}` se lee: {error}"))
        .code
}

#[test]
fn the_code_of_a_kotlin_symbol_is_its_declaration() {
    // K-SYM-06 nombra `get_symbol_code` entre las herramientas que tienen
    // que funcionar con Kotlin, y U-K1 exige «código de símbolo
    // correcto»: el código de un `object` es su declaración, no la
    // palabra `object` suelta.
    //
    // MEDIDO: ANTES de esto el código de un `object`
    // era "object", el de un companion era su nombre suelto, el de una
    // propiedad era su identificador y el del paquete era "package";
    // este test fallaba. La causa era una lista de nombres de nodo que
    // conocía Rust, JavaScript y Python y no la gramática de Kotlin:
    // una segunda tabla semántica, que es justo lo que K-SYM-04
    // prohíbe. Ahora la respuesta sale de los conjuntos que declara el
    // lenguaje.
    let path = fixture();
    let source = std::fs::read_to_string(&path).unwrap();
    let service = SymbolCodeService::new();

    let package = code_of(&service, &path, &source, "com.example.shapes");
    assert!(
        package.contains("package com.example.shapes"),
        "el código del paquete es su declaración: {package}"
    );

    let registry = code_of(&service, &path, &source, "Registry");
    assert!(
        registry.contains("object Registry"),
        "el código de un `object` es el `object` con su cuerpo, no la palabra clave: {registry}"
    );

    let factory = code_of(&service, &path, &source, "Factory");
    assert!(
        factory.contains("companion object Factory"),
        "el código de un companion es su declaración, no su nombre: {factory}"
    );

    let shape = code_of(&service, &path, &source, "DEFAULT_SHAPE");
    assert!(
        shape.contains("DEFAULT_SHAPE: Shape"),
        "el código de una propiedad es su binding con su tipo, no su \
         identificador: {shape}"
    );
}

#[test]
fn the_code_of_a_kotlin_symbol_keeps_the_unicode_it_declares() {
    // El texto Unicode no se pierde por el camino: el lector no lo
    // toca, pero el código que se publica tiene que salir de la fuente
    // y no de una reconstrucción. `Unicode.kt` es el fichero con
    // acentos, eñes y emoji.
    //
    // MEDIDO: ANTES de esto el código del `object` era
    // la palabra "object" y el Unicode no aparecía en ninguna parte;
    // este test fallaba.
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../sandbox/fixtures/kotlin/Unicode.kt")
        .canonicalize()
        .expect("el corpus Unicode está donde el repo dice que está");
    let source = std::fs::read_to_string(&path).unwrap();
    let service = SymbolCodeService::new();

    let labels = code_of(&service, &path, &source, "Labels");
    assert!(
        labels.contains("日本語") && labels.contains("añejo"),
        "el Unicode de la fuente se conserva en el código que se publica: {labels}"
    );
}

#[test]
fn a_kotlin_file_that_does_not_parse_is_not_published_as_read() {
    // El caso negativo. Un fichero Kotlin que no parsea no se publica:
    // publicar lo que un árbol roto tiene a mano sería una respuesta
    // parcial vestida de entera. El observable es el reporte de la
    // ruta por fichero: `Partial` con el fichero en `skipped` y su
    // razón de parseo.
    let dir = tempfile::tempdir().expect("tempdir");
    let broken = dir.path().join("Broken.kt");
    std::fs::write(&broken, "class Broken {\n    fun oops( {\n}\n")
        .expect("se escribe el fichero roto");

    let service = AnalysisService::new();
    let report = service
        .build_graph_per_file_with_report(dir.path())
        .expect("la ruta por fichero termina");

    let skipped = match report.status {
        BuildStatus::Partial { skipped, .. } => skipped,
        other => panic!("un Kotlin que no parsea NO puede ser un build completo: {other:?}"),
    };
    assert!(
        skipped
            .iter()
            .any(|s| s.path.ends_with("Broken.kt") && matches!(s.reason, SkipReason::Parse(_))),
        "el fichero roto se nombra con su razón de parseo: {skipped:?}"
    );
}

#[test]
fn a_pipeline_script_is_read_as_source_and_does_not_invent_a_service() {
    // La segunda forma de script que el contrato nombra: `.pipeline.kts`.
    // Se lee como Kotlin —tiene binding de nivel superior— y NO se
    // convierte en un servicio: el producto no ejecuta el pipeline ni
    // publica los servicios que el pipeline declara.
    let path = pipeline_script();
    let source = std::fs::read_to_string(&path).unwrap();
    let declarations = declarations_from_extractor(&path, &source);

    assert_eq!(
        declarations
            .iter()
            .map(|(_, name, _)| name.as_str())
            .collect::<Vec<_>>(),
        vec!["pipelineName"],
        "un script de pipeline declara sus bindings de nivel superior; \
         las tareas y los servicios del pipeline son LLAMADAS, no \
         declaraciones, y el producto no los compila"
    );
}

#[test]
fn a_gradle_script_is_read_as_source_with_its_own_limits() {
    // Un `.kts` es Kotlin —un script, que se ejecuta de arriba abajo— y
    // el producto lo lee como tal: sin fingir que compila la DSL de
    // Gradle. Lo que se afirma aquí es lo que se PUEDE afirmar: el
    // script se lee, su binding de nivel superior se declara, y no se
    // inventa ninguna declaración para las llamadas de la DSL.
    let path = gradle_script();
    let source = std::fs::read_to_string(&path).unwrap();
    let declarations = declarations_from_extractor(&path, &source);

    assert_eq!(
        declarations
            .iter()
            .map(|(_, name, _)| name.as_str())
            .collect::<Vec<_>>(),
        vec!["greeting"],
        "un script Kotlin declara sus bindings de nivel superior; la DSL \
         de Gradle (plugins, repositories, tasks) son LLAMADAS, no \
         declaraciones, y el producto no las compila"
    );
}
