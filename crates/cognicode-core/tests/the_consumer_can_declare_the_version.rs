//! El consumidor del protocolo puede decir con que version esta trabajando.
//!
//! Un cliente lee un fichero, recibe la version que leyo, y al escribir
//! declara esa version. Si el fichero ya no es el que leyo, la escritura se
//! niega: el trabajo de otro no se pisa.
//!
//! El test habla el mismo idioma que el consumidor —JSON— porque es la
//! frontera que cambia. Si el campo se perdiera por el camino, el JSON es lo
//! que lo delata.

use cognicode_core::interface::mcp::file_ops_handlers::{
    handle_edit_file, handle_read_file, handle_write_file,
};
use cognicode_core::interface::mcp::handlers::HandlerContext;
use cognicode_core::interface::mcp::schemas::{EditFileInput, ReadFileInput, WriteFileInput};
use serde_json::{Value, json};

/// El contexto que atiende las peticiones: un directorio de trabajo.
fn context(dir: &tempfile::TempDir) -> HandlerContext {
    HandlerContext::builder()
        .with_working_dir(dir.path().to_path_buf())
        .build()
}

/// Lee el fichero y devuelve la salida tal y como la ve el consumidor.
async fn read(ctx: &HandlerContext, path: &str) -> Value {
    let input: ReadFileInput = serde_json::from_value(json!({ "path": path })).unwrap();
    let output = handle_read_file(ctx, input).await.unwrap();
    serde_json::to_value(output).unwrap()
}

/// La version que el consumidor recibe al leer: el digest del fichero entero.
///
/// Sin esto, el consumidor no tiene forma de nombrar lo que leyo, y la
/// escritura condicional no se puede pedir.
async fn published_version(ctx: &HandlerContext, path: &str) -> String {
    let output = read(ctx, path).await;
    output["content_hash"]
        .as_str()
        .unwrap_or_else(|| {
            panic!("read_file no publica la version del fichero: la salida es {output}")
        })
        .to_string()
}

/// Escribe declarando una version.
async fn write_declaring(ctx: &HandlerContext, path: &str, content: &str, version: &str) -> bool {
    let input: WriteFileInput = serde_json::from_value(json!({
        "path": path,
        "content": content,
        "expected_content_hash": version,
    }))
    .unwrap();
    handle_write_file(ctx, input).await.is_ok()
}

/// Edita declarando una version.
async fn edit_declaring(ctx: &HandlerContext, path: &str, version: &str) -> bool {
    let input: EditFileInput = serde_json::from_value(json!({
        "path": path,
        "edits": [{ "oldString": "primera", "newString": "segunda" }],
        "expected_content_hash": version,
    }))
    .unwrap();
    handle_edit_file(ctx, input).await.is_ok()
}

#[tokio::test]
async fn read_file_publishes_the_version_of_the_file_it_read() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("notas.txt");
    std::fs::write(&path, "primera linea\nsegunda linea\n").unwrap();
    let ctx = context(&dir);

    let output = read(&ctx, path.to_str().unwrap()).await;

    let version = output["content_hash"].as_str().unwrap_or_else(|| {
        panic!("read_file no publica la version del fichero: la salida es {output}")
    });
    assert_eq!(
        version.len(),
        64,
        "la version es un sha256 en hexadecimal: {version}"
    );
    assert!(
        version
            .chars()
            .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase()),
        "la version es hexadecimal en minusculas: {version}"
    );
}

#[tokio::test]
async fn the_published_version_changes_when_the_file_changes() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("notas.txt");
    std::fs::write(&path, "primera linea\n").unwrap();
    let ctx = context(&dir);
    let path = path.to_str().unwrap().to_string();

    let before = published_version(&ctx, &path).await;
    std::fs::write(&path, "otro contenido\n").unwrap();
    let after = published_version(&ctx, &path).await;

    assert_ne!(
        before, after,
        "la version sigue al contenido: si no, no dice que version se leyo"
    );
}

#[tokio::test]
async fn a_write_that_declares_the_version_just_read_is_applied() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("notas.txt");
    std::fs::write(&path, "primera linea\n").unwrap();
    let ctx = context(&dir);
    let path = path.to_str().unwrap().to_string();

    let version = published_version(&ctx, &path).await;
    let applied = write_declaring(&ctx, &path, "contenido nuevo\n", &version).await;

    assert!(
        applied,
        "una escritura que declara la version leida se aplica"
    );
    assert_eq!(
        std::fs::read_to_string(&path).unwrap(),
        "contenido nuevo\n",
        "el contenido escrito es el que se pidio"
    );
}

#[tokio::test]
async fn the_version_the_write_checks_is_the_one_read_file_publishes() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("notas.txt");
    std::fs::write(&path, "primera linea\n").unwrap();
    let ctx = context(&dir);
    let path = path.to_str().unwrap().to_string();

    let before = published_version(&ctx, &path).await;
    write_declaring(&ctx, &path, "contenido nuevo\n", &before).await;
    let after = published_version(&ctx, &path).await;

    assert_ne!(
        before, after,
        "la version que la escritura comprueba tiene que ser la que read_file \
         publica: si una midiese el trozo y la otra el fichero entero, \
         ningun cliente podria declarar la version que leyo"
    );
}

#[tokio::test]
async fn a_write_that_declares_a_version_the_file_no_longer_has_is_refused() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("notas.txt");
    std::fs::write(&path, "primera linea\n").unwrap();
    let ctx = context(&dir);
    let path = path.to_str().unwrap().to_string();

    let version = published_version(&ctx, &path).await;
    // Otro agente escribe mientras el primero prepara su escritura.
    std::fs::write(&path, "lo escribio otro\n").unwrap();

    let applied = write_declaring(&ctx, &path, "contenido nuevo\n", &version).await;

    assert!(
        !applied,
        "una escritura que declara una version que el fichero ya no tiene se niega"
    );
    assert_eq!(
        std::fs::read_to_string(&path).unwrap(),
        "lo escribio otro\n",
        "el trabajo de otro no se pisa"
    );
}

#[tokio::test]
async fn an_edit_that_declares_a_version_the_file_no_longer_has_is_refused() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("notas.txt");
    std::fs::write(&path, "primera linea\n").unwrap();
    let ctx = context(&dir);
    let path = path.to_str().unwrap().to_string();

    let version = published_version(&ctx, &path).await;
    std::fs::write(&path, "lo escribio otro\n").unwrap();

    let applied = edit_declaring(&ctx, &path, &version).await;

    assert!(
        !applied,
        "una edicion que declara una version que el fichero ya no tiene se niega"
    );
    assert_eq!(
        std::fs::read_to_string(&path).unwrap(),
        "lo escribio otro\n",
        "el trabajo de otro no se pisa"
    );
}

#[tokio::test]
async fn a_write_that_declares_no_version_is_still_written() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("notas.txt");
    let ctx = context(&dir);
    let path = path.to_str().unwrap().to_string();

    let input: WriteFileInput =
        serde_json::from_value(json!({ "path": path, "content": "sin declarar\n" })).unwrap();
    handle_write_file(&ctx, input).await.unwrap();

    assert_eq!(
        std::fs::read_to_string(&path).unwrap(),
        "sin declarar\n",
        "declarar la version es opcional: lo que no la declara sigue escribiendo"
    );
}
