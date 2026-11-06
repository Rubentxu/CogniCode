//! Un reindex incremental dice lo que costó.
//!
//! ## El hueco
//!
//! `reparse_on_edit` publicaba cuántos ficheros tocó y cuántos símbolos
//! cambiaron, y nada más. Un consumidor que quisiera saber si el rebuild
//! incremental merece la pena no tenía con qué responder: el trabajo se
//! repartía entre el escaneo del estado, la comprobación de cada fichero,
//! el parseo, la escritura en el índice y la reconstrucción del grafo, y
//! el producto publicaba una sola suma.
//!
//! ## Lo que se mide aquí
//!
//! La respuesta dice las fases que corrieron y cuánto tardó cada una, los
//! ficheros que tocó, el coste típico de las últimas operaciones de la
//! sesión y qué tipo de rebuild fue: `cache_hit` (no se interpretó nada
//! otra vez), `full` (se interpretó todo), `partial` (se interpretó una
//! parte y el resto se reutilizó) o `error` (el rebuild no terminó).
//!
//! Y nada de eso lleva un camino: el coste es un hecho sobre el trabajo,
//! no sobre el árbol.
//!
//! ## Por qué se aserta sobre el JSON
//!
//! La aserción va por lo que el cliente ve, no por el campo tipado: así el
//! test compila contra el código anterior y falla por el motivo correcto
//! —el documento no lo dice— y no porque el campo todavía no exista.

#![cfg(feature = "persistence")]

use cognicode_core::interface::mcp::handlers::aix_handlers::handle_reparse_on_edit;
use cognicode_core::interface::mcp::handlers::{
    BuildGraphInput, HandlerContext, handle_build_graph,
};
use cognicode_core::interface::mcp::schemas::ReparseOnEditInput;
use serde_json::Value;
use std::path::Path;
use std::time::Duration;

/// Las fases que la herramienta puede publicar.
const PHASES: [&str; 5] = ["scan", "hash", "parse", "index", "graph"];

/// Un contexto con el workspace en un directorio temporal.
fn ctx_for(workspace: &Path) -> HandlerContext {
    HandlerContext::builder()
        .with_working_dir(workspace.to_path_buf())
        .build()
}

/// Un workspace con `files` ficheros Rust y su grafo construido.
///
/// El grafo se construye antes de medir nada: es lo que deja la caché por
/// fichero con lo que los ficheros dicen de sí mismos, y sin eso el
/// rebuild siguiente interpretaría el árbol entero otra vez.
async fn analysed_workspace(files: usize) -> (tempfile::TempDir, HandlerContext) {
    let workspace = tempfile::tempdir().expect("workspace");
    let src = workspace.path().join("src");
    std::fs::create_dir_all(&src).expect("se crea el directorio");
    for index in 0..files {
        std::fs::write(
            src.join(format!("file_{index}.rs")),
            format!("fn funcion_{index}() {{}}\nfn otra_{index}() {{}}\n"),
        )
        .expect("se escribe el fichero");
    }

    let ctx = ctx_for(workspace.path());
    let build = handle_build_graph(&ctx, BuildGraphInput { directory: None })
        .await
        .expect("build_graph responde");
    assert_eq!(
        build.status, "complete",
        "el grafo tiene que estar completo: si no, el manifiesto no se guarda y \
         los ficheros se tratan como nuevos en vez de como modificados"
    );

    (workspace, ctx)
}

/// Lo que la herramienta publica, tal y como lo ve el cliente.
async fn reparse(ctx: &HandlerContext, files: &[&str]) -> Value {
    let output = handle_reparse_on_edit(
        ctx,
        ReparseOnEditInput {
            file_paths: files.iter().map(|file| file.to_string()).collect(),
            edit_ranges: None,
        },
    )
    .await
    .expect("reparse_on_edit responde");
    serde_json::to_value(&output).expect("la salida es JSON")
}

/// Los nombres de las fases que el documento publica, en el orden en que
/// las publica.
fn phases_of(document: &Value) -> Vec<String> {
    document["cost"]["phases"]
        .as_array()
        .unwrap_or_else(|| {
            panic!(
                "el documento no dice qué fases corrieron: {document}\n\
                 una operación sin fases es una operación sin coste"
            )
        })
        .iter()
        .map(|phase| {
            phase["phase"]
                .as_str()
                .unwrap_or_else(|| panic!("una fase sin nombre: {phase}"))
                .to_string()
        })
        .collect()
}

/// La palabra del rebuild, tal y como la publica el documento.
fn rebuild_kind_of(document: &Value) -> String {
    document["cost"]["rebuild_kind"]
        .as_str()
        .unwrap_or_else(|| {
            panic!(
                "el documento no dice qué rebuild fue: {document}\n\
                 un rebuild sin nombre es un rebuild sin dueño"
            )
        })
        .to_string()
}

/// Una llamada que no cambia nada no reconstruye nada: es un acierto de
/// caché, y el documento lo dice.
#[tokio::test]
async fn a_reparse_that_changes_nothing_is_a_cache_hit() {
    let (_workspace, ctx) = analysed_workspace(2).await;

    let published = reparse(&ctx, &["src/file_0.rs"]).await;

    assert_eq!(
        rebuild_kind_of(&published),
        "cache_hit",
        "nada cambió: el grafo ya lo decía y no hubo rebuild: {published}"
    );
    assert_eq!(
        phases_of(&published),
        vec!["scan".to_string(), "hash".to_string()],
        "sin cambios no hay parseo, ni escritura, ni rebuild: {published}"
    );
    assert_eq!(
        published["cost"]["affected_files"].as_u64(),
        Some(0),
        "un fichero que no cambió no es un fichero afectado: {published}"
    );
    assert_eq!(
        published["cost"]["considered_files"].as_u64(),
        Some(1),
        "el documento dice cuántos ficheros se consideraron: {published}"
    );
}

/// Un cambio en un fichero de un árbol ya construido reutiliza el resto:
/// el rebuild es parcial, y el documento lo dice.
#[tokio::test]
async fn a_reparse_that_changes_one_file_reuses_the_rest() {
    let (workspace, ctx) = analysed_workspace(3).await;

    tokio::time::sleep(Duration::from_millis(20)).await;
    std::fs::write(
        workspace.path().join("src/file_0.rs"),
        "fn funcion_0() {}\nfn otra_0() {}\nfn tercera_0() {}\n",
    )
    .expect("se reescribe el fichero");

    let published = reparse(&ctx, &["src/file_0.rs"]).await;

    assert_eq!(
        rebuild_kind_of(&published),
        "partial",
        "un fichero interpretado y dos reutilizados: rebuild parcial: {published}"
    );
    assert_eq!(
        phases_of(&published),
        vec![
            "scan".to_string(),
            "hash".to_string(),
            "parse".to_string(),
            "index".to_string(),
            "graph".to_string()
        ],
        "un cambio lee, interpreta, escribe y reconstruye: {published}"
    );
    assert_eq!(
        published["cost"]["affected_files"].as_u64(),
        Some(1),
        "un solo fichero cambió: {published}"
    );
}

/// Una sesión que no ha construido nada antes interpreta el árbol entero:
/// el rebuild es completo.
#[tokio::test]
async fn a_reparse_over_a_cold_session_rebuilds_everything() {
    let workspace = tempfile::tempdir().expect("workspace");
    let src = workspace.path().join("src");
    std::fs::create_dir_all(&src).expect("se crea el directorio");
    for index in 0..3 {
        std::fs::write(src.join(format!("file_{index}.rs")), "fn f() {}\n")
            .expect("se escribe el fichero");
    }

    let ctx = ctx_for(workspace.path());
    let published = reparse(&ctx, &["src/file_0.rs"]).await;

    assert_eq!(
        rebuild_kind_of(&published),
        "full",
        "sin caché previa se interpreta el árbol entero: rebuild completo: {published}"
    );
}

/// El coste no lleva un camino: el trabajo se mide, el árbol no se cuenta.
#[tokio::test]
async fn the_published_cost_carries_no_path() {
    let (workspace, ctx) = analysed_workspace(2).await;

    tokio::time::sleep(Duration::from_millis(20)).await;
    std::fs::write(
        workspace.path().join("src/file_0.rs"),
        "fn funcion_0() {}\n",
    )
    .expect("se reescribe el fichero");

    let published = reparse(&ctx, &["src/file_0.rs"]).await;
    let text = published.to_string();

    assert!(
        published["cost"].is_object(),
        "el documento dice lo que costó: {published}"
    );
    assert!(
        !text.contains(workspace.path().to_string_lossy().as_ref()),
        "el coste no lleva el camino del árbol: {published}"
    );
    assert!(
        !text.contains("file_0.rs"),
        "el coste no lleva el nombre de ningún fichero: {published}"
    );
}

/// Los percentiles son de las operaciones de la sesión, y no existen
/// mientras no haya operaciones suficientes para tenerlos.
#[tokio::test]
async fn the_percentiles_need_more_than_one_operation() {
    let (workspace, ctx) = analysed_workspace(2).await;

    tokio::time::sleep(Duration::from_millis(20)).await;
    std::fs::write(
        workspace.path().join("src/file_0.rs"),
        "fn funcion_0() {}\nfn otra_0() {}\n",
    )
    .expect("se reescribe el fichero");

    let first = reparse(&ctx, &["src/file_0.rs"]).await;
    assert_eq!(
        first["cost"]["sample_size"].as_u64(),
        Some(1),
        "la ventana cuenta las operaciones observadas: {first}"
    );
    assert!(
        first["cost"]["duration_p50_ms"].is_null(),
        "una sola operación no es una distribución: {first}"
    );
    assert!(
        first["cost"]["duration_p95_ms"].is_null(),
        "una sola operación no es una distribución: {first}"
    );

    let second = reparse(&ctx, &["src/file_0.rs"]).await;
    assert_eq!(
        second["cost"]["sample_size"].as_u64(),
        Some(2),
        "la ventana cuenta las operaciones observadas: {second}"
    );
    let p50 = second["cost"]["duration_p50_ms"].as_u64();
    let p95 = second["cost"]["duration_p95_ms"].as_u64();
    assert!(p50.is_some(), "con dos operaciones hay mediana: {second}");
    assert!(
        p95.is_some(),
        "con dos operaciones hay percentil 95: {second}"
    );
    assert!(
        p50 <= p95,
        "la mediana no puede estar por encima del percentil 95: {second}"
    );
}

/// La memoria alta se publica cuando la plataforma la dice, y no se
/// inventa un cero cuando no la dice.
///
/// El kernel de Linux lleva la marca de agua del proceso en
/// `VmHWM`, así que ahí la memoria alta es un número. Donde la
/// plataforma no la publica, el documento dice que no la publica: una
/// ausencia, no un cero.
#[cfg(target_os = "linux")]
#[tokio::test]
async fn the_peak_memory_is_published_when_the_platform_knows_it() {
    let (_workspace, ctx) = analysed_workspace(1).await;

    let published = reparse(&ctx, &["src/file_0.rs"]).await;

    match &published["cost"]["peak_memory_bytes"] {
        Value::Number(bytes) => assert!(
            bytes.as_u64().unwrap_or(0) > 0,
            "la memoria alta de un proceso vivo no es cero: {published}"
        ),
        other => panic!("la memoria alta no es un número: {other}: {published}"),
    }
}

/// Donde la plataforma no publica la memoria alta, el documento la
/// publica como una ausencia.
#[cfg(not(target_os = "linux"))]
#[tokio::test]
async fn the_peak_memory_is_an_absence_where_the_platform_has_none() {
    let (_workspace, ctx) = analysed_workspace(1).await;

    let published = reparse(&ctx, &["src/file_0.rs"]).await;

    assert!(
        published["cost"]["peak_memory_bytes"].is_null(),
        "una memoria alta que la plataforma no publica no es un cero: {published}"
    );
}

/// Cada fase que el documento publica es una de las cinco, y todas las
/// que publicaron tienen un tiempo medido.
#[tokio::test]
async fn every_published_phase_is_one_the_product_runs() {
    let (workspace, ctx) = analysed_workspace(2).await;

    tokio::time::sleep(Duration::from_millis(20)).await;
    std::fs::write(
        workspace.path().join("src/file_0.rs"),
        "fn funcion_0() {}\nfn otra_0() {}\n",
    )
    .expect("se reescribe el fichero");

    let published = reparse(&ctx, &["src/file_0.rs"]).await;
    let phases = published["cost"]["phases"]
        .as_array()
        .expect("las fases son una lista");

    for phase in phases {
        let name = phase["phase"].as_str().expect("la fase tiene nombre");
        assert!(
            PHASES.contains(&name),
            "una fase que el producto no corre: {name}: {published}"
        );
        assert!(
            phase["duration_ms"].as_u64().is_some(),
            "una fase sin tiempo medido no es una fase: {phase}: {published}"
        );
    }

    let names = phases_of(&published);
    let mut position = 0usize;
    for name in &names {
        let found = PHASES[position..]
            .iter()
            .position(|candidate| candidate == name)
            .unwrap_or_else(|| {
                panic!("las fases no se publican en su orden: {names:?}: {published}")
            });
        position += found + 1;
    }
}
