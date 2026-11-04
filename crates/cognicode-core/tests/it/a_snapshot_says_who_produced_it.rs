//! El snapshot dice quién lo produjo.
//!
//! ## El hueco
//!
//! El producto persiste un snapshot del grafo y lo vuelve a servir cuando
//! las fuentes no han cambiado. Lo que el snapshot guardaba era el grafo y
//! el manifiesto de contenido: qué ficheros se leyeron y con qué hash.
//!
//! Lo que el snapshot **no** guardaba era quién lo leyó. Sin ese dato, un
//! binario distinto —otra versión, otro commit, otras capacidades— sirve el
//! snapshot como si fuera suyo: el grafo que el consumidor recibe lo calculó
//! otro analizador, con otra ontología y otras reglas, y nada en la
//! respuesta lo dice. El `basis` que el producto publica declara el
//! workspace, la configuración y las fuentes, y declara `complete: true`,
//! como si la identidad del análisis estuviera completa. No lo está: le falta
//! la parte que decide si el análisis sigue valiendo.
//!
//! ## Lo que se mide aquí
//!
//! 1. La base que el producto publica declara el analizador que produjo el
//!    análisis: su versión, su commit cuando se pudo saber, sus capacidades
//!    y el esquema del snapshot.
//! 2. El snapshot persistido declara lo mismo, y un snapshot cuyo analizador
//!    declarado no es el de ahora **no se sirve**: el producto reconstruye en
//!    vez de publicar el grafo de otro.
//! 3. Un análisis en frío y el mismo análisis servido desde la caché declaran
//!    la misma identidad: servirse no es una razón para dejar de saber quién
//!    calculó la respuesta.
//!
//! ## Por qué se aserta sobre el JSON
//!
//! La aserción va por lo que el cliente ve, no por el campo tipado: así el
//! test compila contra el código anterior y falla por el motivo correcto —
//! el dato que falta— y no porque el tipo todavía no exista.

use cognicode_core::interface::mcp::handlers::{
    BuildGraphInput, HandlerContext, handle_build_graph,
};
use serde_json::Value;
use std::path::Path;

/// Un contexto con el workspace en un directorio.
fn ctx_for(workspace: &Path) -> HandlerContext {
    HandlerContext::builder()
        .with_working_dir(workspace.to_path_buf())
        .build()
}

/// El workspace de trabajo y la raíz temporal que lo sostiene.
///
/// El workspace lleva un subdirectorio de nombre fijo: la ruta completa del
/// workspace acaba dentro del snapshot, y un temporal con nombre aleatorio
/// podría—by descuido—contener los bytes de la versión que el test usa para
/// reconocer al analizador.
fn workspace_with_root() -> (tempfile::TempDir, std::path::PathBuf) {
    let root = tempfile::tempdir().expect("temporal");
    let ws = root.path().join("ws");
    std::fs::create_dir_all(&ws).expect("se crea el workspace");
    std::fs::write(ws.join("a.rs"), "fn a() {}\n").expect("se escribe el fuente");
    (root, ws)
}

/// Dónde persiste el producto el snapshot del grafo.
fn snapshot_path(workspace: &Path) -> std::path::PathBuf {
    workspace.join(".cognicode").join("graph.cache")
}

/// Construye el grafo del workspace y devuelve la salida publicada.
async fn build(workspace: &Path) -> Value {
    let ctx = ctx_for(workspace);
    let output = handle_build_graph(&ctx, BuildGraphInput { directory: None })
        .await
        .expect("build_graph responde");
    serde_json::to_value(output).expect("la salida es JSON")
}

/// Cuántas veces aparece un trozo de bytes en el fichero.
fn occurrences(haystack: &[u8], needle: &[u8]) -> usize {
    if needle.is_empty() || haystack.len() < needle.len() {
        return 0;
    }
    haystack
        .windows(needle.len())
        .filter(|window| *window == needle)
        .count()
}

#[tokio::test]
async fn the_basis_declares_the_analyzer_that_produced_the_analysis() {
    let (_root, workspace) = workspace_with_root();

    let json = build(&workspace).await;
    let basis = &json["basis"];

    assert_eq!(
        basis["analyzer_version"].as_str(),
        Some(env!("CARGO_PKG_VERSION")),
        "la base tiene que declarar la versión del analizador que produjo \
         el análisis; sin ella, `complete: true` describe una identidad a \
         medias. Base publicada: {basis}"
    );

    let features = basis["analyzer_features"].as_array().unwrap_or_else(|| {
        panic!("la base tiene que declarar las capacidades del analizador: {basis}")
    });
    assert!(
        !features.is_empty(),
        "un analizador que no declara ninguna capacidad no se puede identificar"
    );
    assert!(
        features.iter().all(|f| f.is_string()),
        "las capacidades declaradas son texto: {features:?}"
    );
    let mut sorted = features.clone();
    sorted.sort_by(|a, b| a.as_str().cmp(&b.as_str()));
    assert_eq!(
        *features, sorted,
        "las capacidades se declaran ordenadas: el mismo analizador tiene que \
         producir la misma identidad en cualquier orden"
    );

    assert!(
        basis["analyzer_commit"].is_string() || basis["analyzer_commit"].is_null(),
        "el commit del analizador es texto cuando se sabe, y ausente cuando \
         no: nunca un valor inventado. Base publicada: {basis}"
    );

    assert_eq!(
        basis["snapshot_schema"].as_str(),
        Some("cognicode.graph.cache/v4"),
        "la base declara el esquema con el que se escribió el snapshot"
    );
}

#[tokio::test]
async fn the_persisted_snapshot_declares_the_analyzer_that_produced_it() {
    let (_root, workspace) = workspace_with_root();

    let _ = build(&workspace).await;
    let path = snapshot_path(&workspace);
    let bytes = std::fs::read(&path).expect("el snapshot se persiste en el workspace");

    let version = env!("CARGO_PKG_VERSION");
    assert_eq!(
        occurrences(&bytes, version.as_bytes()),
        1,
        "el snapshot persistido declara el analizador que lo produjo, y lo \
         declara una vez. Si aparece más de una vez, la versión está en otro \
         sitio del fichero y el test no puede afirmar nada sobre la identidad."
    );
}

#[tokio::test]
async fn a_snapshot_from_another_analyzer_is_not_served() {
    let (_root, workspace) = workspace_with_root();

    // 1. Un análisis deja un snapshot en el workspace.
    let _ = build(&workspace).await;
    let path = snapshot_path(&workspace);
    let original = std::fs::read(&path).expect("el snapshot se persiste");

    // 2. Se simula otro analizador: la versión que el snapshot declara pasa
    //    a ser la de otro build **de la misma longitud**, para que el
    //    fichero siga siendo legible y lo que cambia sea la identidad, no
    //    la integridad. Un snapshot corrupto sería un caso distinto.
    let version = env!("CARGO_PKG_VERSION").to_string();
    let other = "9.999.99".to_string();
    assert_eq!(
        other.len(),
        version.len(),
        "la versión simulada tiene que medir lo mismo que la real: si no, \
         el fichero deja de ser legible y el test probaría otra cosa"
    );
    let position = original
        .windows(version.len())
        .position(|window| window == version.as_bytes())
        .expect("el snapshot declara la versión del analizador que lo produjo");
    let mut tampered = original.clone();
    tampered[position..position + version.len()].copy_from_slice(other.as_bytes());
    assert_ne!(
        tampered, original,
        "el snapshot tiene que quedar marcado como de otro analizador"
    );
    assert_eq!(
        tampered.len(),
        original.len(),
        "el snapshot marcado mide lo mismo: si no, el fichero deja de ser legible"
    );
    std::fs::write(&path, &tampered).expect("se marca el snapshot");

    // 3. Otra sesión pide el grafo. Las fuentes no han cambiado, así que la
    //    única razón para no servir el snapshot es que lo produjo otro
    //    analizador.
    let _ = build(&workspace).await;

    // 4. Si el producto hubiera servido el snapshot, el fichero seguiría
    //    declarando al otro analizador. Si lo rechazó, lo reconstruyó y el
    //    snapshot vuelve a declarar a quien está ahora.
    let after = std::fs::read(&path).expect("el snapshot sigue en el workspace");
    assert_eq!(
        occurrences(&after, version.as_bytes()),
        1,
        "un snapshot producido por otro analizador no se sirve: el producto \
         reconstruye y vuelve a publicar su propia identidad"
    );
}

#[tokio::test]
async fn a_cold_analysis_and_a_warm_one_declare_the_same_identity() {
    let (_root, workspace) = workspace_with_root();

    // En frío: el grafo se construye desde las fuentes.
    let cold = build(&workspace).await;

    // En caliente: otra sesión, mismo workspace, mismas fuentes. El
    // snapshot está al día, así que se sirve.
    let warm = build(&workspace).await;

    assert_eq!(
        cold["basis"], warm["basis"],
        "servirse de la caché no es una razón para declarar otra identidad: \
         el análisis en frío y el servido dicen lo mismo"
    );
    assert_eq!(
        cold["symbols_found"], warm["symbols_found"],
        "el análisis servido y el construido tienen que ver lo mismo"
    );
}
