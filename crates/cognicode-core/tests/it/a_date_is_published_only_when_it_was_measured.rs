//! Una fecha se publica solo si se midió.
//!
//! ## El hueco
//!
//! El producto publica la marca de tiempo de un fichero como un número de
//! segundos desde la época. Cuando el sistema de ficheros no publica una
//! fecha, o cuando la fecha es anterior a la época, el producto publicaba
//! `0`, que dice "este fichero se escribió el 1 de enero de 1970": una
//! fecha que nadie midió.
//!
//! ## Lo que se mide aquí
//!
//! Un fichero con fecha anterior a la época tiene que publicar SU fecha,
//! que es un número negativo, y no la época. Y un fichero con fecha normal
//! tiene que publicar el número que se midió: si el producto publicara
//! siempre `null`, estos tests también pasarían y no dirían nada.
//!
//! ## Por qué se aserta sobre el JSON
//!
//! La aserción va por lo que el cliente ve, no por el campo tipado: así el
//! test compila contra el código anterior y falla por el motivo correcto
//! —el número que sale— y no porque el tipo todavía no exista.

use cognicode_core::interface::mcp::file_ops_handlers::{handle_list_files, handle_read_file};
use cognicode_core::interface::mcp::handlers::HandlerContext;
use cognicode_core::interface::mcp::schemas::{ListFilesInput, ReadFileInput};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// Escribe un fichero y le pone una fecha concreta.
fn write_with_mtime(path: &std::path::Path, body: &str, mtime: SystemTime) {
    std::fs::write(path, body).expect("se escribe el fichero");
    let file = std::fs::OpenOptions::new()
        .write(true)
        .open(path)
        .expect("se abre para la fecha");
    file.set_modified(mtime).expect("se pone la fecha");
}

/// Un contexto con el workspace en un directorio temporal.
fn ctx_for(workspace: &std::path::Path) -> HandlerContext {
    HandlerContext::builder()
        .with_working_dir(workspace.to_path_buf())
        .build()
}

/// La fecha que el producto publica para un fichero, tal y como la ve el
/// cliente: un número, o `null` si no hay ninguna.
fn published_date(json: &serde_json::Value) -> Option<i64> {
    json.as_i64()
}

/// La entrada de un listado que corresponde a un fichero concreto.
fn entry_of<'a>(files: &'a [serde_json::Value], name: &str) -> &'a serde_json::Value {
    files
        .iter()
        .find(|entry| {
            entry["path"]
                .as_str()
                .is_some_and(|path| path.ends_with(name))
        })
        .unwrap_or_else(|| panic!("el listado tiene que traer {name}"))
}

#[tokio::test]
async fn read_file_publishes_the_date_it_measured_even_before_the_epoch() {
    let workspace = tempfile::tempdir().expect("workspace");
    let path = workspace.path().join("notas.txt");
    // Un minuto antes de la época: una fecha real, y anterior a ella.
    write_with_mtime(&path, "contenido\n", UNIX_EPOCH - Duration::from_secs(60));

    let ctx = ctx_for(workspace.path());
    let output = handle_read_file(
        &ctx,
        ReadFileInput {
            path: "notas.txt".to_string(),
            start_line: None,
            end_line: None,
            mode: None,
            chunk_size: None,
            continuation_token: None,
        },
    )
    .await
    .expect("read_file responde");

    let json = serde_json::to_value(&output).expect("la salida es JSON");
    let published = published_date(&json["metadata"]["modified"]);

    assert_eq!(
        published,
        Some(-60),
        "el fichero lleva fecha de un minuto antes de la época: el producto \
         tiene que publicar esa fecha, no la época"
    );
}

#[tokio::test]
async fn list_files_publishes_the_date_it_measured_even_before_the_epoch() {
    let workspace = tempfile::tempdir().expect("workspace");
    let path = workspace.path().join("notas.txt");
    write_with_mtime(&path, "contenido\n", UNIX_EPOCH - Duration::from_secs(60));

    let ctx = ctx_for(workspace.path());
    let output = handle_list_files(
        &ctx,
        ListFilesInput {
            path: None,
            glob: Some("*.txt".to_string()),
            offset: None,
            limit: None,
            recursive: None,
            max_depth: None,
        },
    )
    .await
    .expect("list_files responde");

    let json = serde_json::to_value(&output).expect("la salida es JSON");
    let files = json["files"].as_array().expect("el listado es una lista");
    let entry = entry_of(files, "notas.txt");
    let published = published_date(&entry["modified"]);

    assert_eq!(
        published,
        Some(-60),
        "el listado tiene que publicar la fecha que midió, no la época"
    );
}

#[tokio::test]
async fn a_measured_date_is_published_as_the_number_it_is() {
    // La contraprueba: si el producto publicara siempre `null`, o siempre
    // un número constante, los dos tests anteriores pasarían igual. Aquí se
    // pinna el valor medido.
    let workspace = tempfile::tempdir().expect("workspace");
    let path = workspace.path().join("notas.txt");
    write_with_mtime(
        &path,
        "contenido\n",
        UNIX_EPOCH + Duration::from_secs(1_700_000_000),
    );

    let ctx = ctx_for(workspace.path());
    let output = handle_read_file(
        &ctx,
        ReadFileInput {
            path: "notas.txt".to_string(),
            start_line: None,
            end_line: None,
            mode: None,
            chunk_size: None,
            continuation_token: None,
        },
    )
    .await
    .expect("read_file responde");

    let json = serde_json::to_value(&output).expect("la salida es JSON");
    let published = published_date(&json["metadata"]["modified"]);

    assert_eq!(
        published,
        Some(1_700_000_000),
        "una fecha medida se publica tal cual se midió"
    );
}
