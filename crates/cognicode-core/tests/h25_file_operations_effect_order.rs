//! Las operaciones sobre archivos: el efecto viene después de la promesa, y
//! una validación que no ocurrió lo dice.
//!
//! Estos checks son de caracterización: **miden el defecto antes de que
//! exista el arreglo**, y por eso los cinco primeros están escritos contra la
//! API que había, no contra la que hay después. Si el arreglo cambia la forma
//! de la respuesta, esos cinco se quedan atrás a propósito: el discriminante
//! lo pone el contrato nuevo, no un test que se adaptó solo. Los que van
//! después sí fijan el contrato nuevo.
//!
//! Lo que miden:
//!
//! 1. **Una petición rechazada no deja directorios.** `write_file` crea los
//!    directorios padre antes de validar la ruta, y resuelve la ruta
//!    relativa contra el directorio del proceso mientras la validación la
//!    resuelve contra el workspace. Son dos sitios distintos para la misma
//!    ruta, y el efecto ocurre antes de que nadie haya dicho que sí.
//! 2. **Una ruta relativa se resuelve contra el workspace.** El árbol de
//!    directorios que se crea tiene que estar dentro del root que el
//!    contexto declara, no en el directorio desde el que corre el proceso.
//! 3. **Una validación que no ocurrió no se reporta como validación.** Un
//!    fichero en un lenguaje que el producto no conoce se escribe, pero la
//!    respuesta no puede decir que se comprobó.
//! 4. **Una escritura no puede ser bloqueada por el temporal de otra.**
//!    El temporal se nombra con el PID del proceso y nada más, así que dos
//!    escrituras al mismo camino dentro del mismo proceso comparten el
//!    fichero.
//!
//! La forma que se busca y el orden que se busca son los que fija la
//! decisión de arquitectura sobre las operaciones de fichero.

use std::path::PathBuf;

use cognicode_core::application::dto::{EditVerification, FileEdit};
use cognicode_core::interface::mcp::file_ops_handlers::{handle_edit_file, handle_write_file};
use cognicode_core::interface::mcp::handlers::HandlerContext;
use cognicode_core::interface::mcp::schemas::{EditFileInput, WriteFileInput};

/// Un contexto con el workspace en un directorio temporal.
///
/// El validador por defecto del contexto es el que acota el workspace, así
/// que una ruta fuera de aquí se rechaza — que es exactamente lo que hace
/// que el defecto sea visible: el rechazo llega *después* del efecto.
fn ctx_for(workspace: &std::path::Path) -> HandlerContext {
    HandlerContext::builder()
        .with_working_dir(workspace.to_path_buf())
        .build()
}

/// Una escritura rechazada no deja ningún directorio detrás.
///
/// El caso: una ruta absoluta fuera del workspace, con `create_dirs`. El
/// producto crea el árbol de directorios y después rechaza la escritura.
/// El efecto que la petición rechazada no debía tener se queda en el disco.
#[tokio::test]
async fn a_rejected_write_leaves_no_directory_outside_the_workspace() {
    let workspace = tempfile::tempdir().expect("workspace");
    let outside = tempfile::tempdir().expect("outside");
    let ctx = ctx_for(workspace.path());

    let target = outside.path().join("deep").join("nested").join("evil.txt");
    let result = handle_write_file(
        &ctx,
        WriteFileInput {
            path: target.to_string_lossy().into_owned(),
            content: "malicious content".to_string(),
            create_dirs: Some(true),

            expected_content_hash: None,
        },
    )
    .await;

    assert!(result.is_err(), "a path outside the workspace is refused");
    assert!(
        !outside.path().join("deep").exists(),
        "a refused write left a directory tree outside the workspace: {:?}",
        outside.path().join("deep")
    );
}

/// Una ruta relativa se resuelve contra el workspace, no contra el
/// directorio del proceso.
///
/// El caso: una ruta relativa con `create_dirs`. El directorio donde el
/// proceso corre y el workspace son sitios distintos, y el producto no
/// puede crear el árbol en uno y validar en otro.
#[tokio::test]
async fn a_relative_write_creates_the_tree_inside_the_workspace() {
    let workspace = tempfile::tempdir().expect("workspace");
    let ctx = ctx_for(workspace.path());

    // El nombre lleva el PID para no chocar con otra corrida de esta misma
    // prueba, que crea el mismo camino relativo.
    let relative = format!("h25-probe-{}/nested/file.txt", std::process::id());
    let stray: PathBuf = std::env::current_dir()
        .expect("the process has a directory")
        .join(&relative);
    let _ = std::fs::remove_dir_all(&stray);

    let result = handle_write_file(
        &ctx,
        WriteFileInput {
            path: relative.clone(),
            content: "dentro del workspace".to_string(),
            create_dirs: Some(true),

            expected_content_hash: None,
        },
    )
    .await;

    let written = workspace.path().join(&relative);
    let stray_exists = stray.exists();
    // Se limpia lo que el producto haya dejado donde no debía, para que la
    // prueba no ensucie el árbol del repositorio.
    let _ = std::fs::remove_dir_all(&stray);

    assert!(
        result.is_ok(),
        "a relative path is resolved against the workspace that the context declares"
    );
    assert!(written.exists(), "the file is written inside the workspace");
    assert!(
        !stray_exists,
        "the directory tree was created at the process directory instead of the workspace: {:?}",
        stray
    );
}

/// Una validación que no ocurrió no se reporta como una validación
/// satisfactoria.
///
/// El caso: un fichero en un lenguaje que el producto no conoce. No hay
/// parser, así que no hay verificación — pero la escritura sí procede, y la
/// respuesta tiene que decir la verdad sobre lo que pasó.
#[tokio::test]
async fn an_edit_that_could_not_be_verified_does_not_report_a_passed_validation() {
    let workspace = tempfile::tempdir().expect("workspace");
    let ctx = ctx_for(workspace.path());

    let file = workspace.path().join("notes.unknownlang");
    std::fs::write(&file, "hola mundo\n").expect("the file is written");

    let output = handle_edit_file(
        &ctx,
        EditFileInput {
            path: file.to_string_lossy().into_owned(),
            edits: vec![FileEdit {
                old_string: "hola".to_string(),
                new_string: "adios".to_string(),
            }],

            expected_content_hash: None,
        },
    )
    .await
    .expect("the edit is answered");

    assert!(
        output.applied,
        "the edit is still applied: there was no reason to refuse it"
    );
    assert!(
        !output.validation.passed,
        "a validation that did not happen must not be reported as a passed validation"
    );
    assert_eq!(
        output.validation.verification,
        EditVerification::Unverified,
        "the response says that the content was not checked"
    );
}

/// Una validación que sí ocurrió lo dice.
///
/// El otro lado del mismo contrato: cuando hay parser y el contenido está
/// bien, la respuesta lo afirma. Un estado que sólo sabe decir «no lo sé»
/// tampoco vale.
#[tokio::test]
async fn a_verified_edit_says_it_was_verified() {
    let workspace = tempfile::tempdir().expect("workspace");
    let ctx = ctx_for(workspace.path());

    let file = workspace.path().join("good.rs");
    std::fs::write(&file, "fn test() {}\n").expect("the file is written");

    let output = handle_edit_file(
        &ctx,
        EditFileInput {
            path: file.to_string_lossy().into_owned(),
            edits: vec![FileEdit {
                old_string: "fn test() {}".to_string(),
                new_string: "fn test() { }".to_string(),
            }],

            expected_content_hash: None,
        },
    )
    .await
    .expect("the edit is answered");

    assert!(output.applied, "the edit is applied");
    assert_eq!(
        output.validation.verification,
        EditVerification::Verified,
        "the response says that the content was checked"
    );
    assert!(
        output.validation.passed,
        "a content that was really verified is a passed validation"
    );
}

/// Un fichero roto se rechaza: el caso que ya funcionaba, y que no puede
/// dejar de funcionar.
///
/// El check va contra el otro lado del mismo contrato. Si el arreglo
/// aparta la escritura del predicado `passed`, este check es el que dice
/// que el rechazo por sintaxis rota sigue funcionando.
#[tokio::test]
async fn a_broken_file_is_rejected() {
    let workspace = tempfile::tempdir().expect("workspace");
    let ctx = ctx_for(workspace.path());

    let file = workspace.path().join("broken.rs");
    std::fs::write(&file, "fn test() {}\n").expect("the file is written");

    let output = handle_edit_file(
        &ctx,
        EditFileInput {
            path: file.to_string_lossy().into_owned(),
            edits: vec![FileEdit {
                old_string: "fn test() {}".to_string(),
                new_string: "fn test() { ".to_string(),
            }],

            expected_content_hash: None,
        },
    )
    .await
    .expect("the edit is answered");

    assert!(!output.applied, "a file that does not parse is not written");
    assert!(
        !output.validation.passed,
        "a rejected content is not a passed validation"
    );
    assert_eq!(
        output.validation.verification,
        EditVerification::Rejected,
        "the response says that a parser rejected the content"
    );
}

/// Una edición que no cambia nada lo dice.
///
/// El caso: la edición se aplica pero el resultado es idéntico al fichero
/// que ya estaba. La salida dice `applied: false`, y su `reason` —que la
/// documentación define como «el motivo del rechazo o de la no
/// aplicación»— dice `None`. La no aplicación no tiene motivo escrito.
#[tokio::test]
async fn an_edit_that_changes_nothing_says_why() {
    let workspace = tempfile::tempdir().expect("workspace");
    let ctx = ctx_for(workspace.path());

    let file = workspace.path().join("same.rs");
    std::fs::write(&file, "fn test() {}\n").expect("the file is written");

    let output = handle_edit_file(
        &ctx,
        EditFileInput {
            path: file.to_string_lossy().into_owned(),
            edits: vec![FileEdit {
                old_string: "fn test() {}".to_string(),
                new_string: "fn test() {}".to_string(),
            }],

            expected_content_hash: None,
        },
    )
    .await
    .expect("the edit is answered");

    assert!(
        !output.applied,
        "an edit that changes nothing is not applied"
    );
    assert_eq!(
        output.reason.as_deref(),
        Some("no_change"),
        "a non-application says why it did not apply"
    );
}

/// Una escritura no puede ser bloqueada por el temporal de otra.
///
/// El caso: el temporal de `write_file` se nombra con el PID del proceso y
/// nada más, así que dos escrituras al mismo camino dentro del mismo
/// proceso comparten el mismo fichero temporal. Aquí se planta en ese
/// sitio un artefacto que ocupa el nombre: si el temporal lleva un
/// discriminante por escritura, la escritura pasa; si el nombre es
/// predecible, choca con lo que había ahí.
#[tokio::test]
async fn a_write_is_not_blocked_by_what_another_write_left_behind() {
    let workspace = tempfile::tempdir().expect("workspace");
    let ctx = ctx_for(workspace.path());

    let target = workspace.path().join("collide.txt");
    // El sitio donde caería el temporal de un nombre derivado sólo del PID.
    let planted = format!("{}.tmp.{}", target.to_string_lossy(), std::process::id());
    std::fs::create_dir_all(&planted).expect("the artefact is planted");

    let result = handle_write_file(
        &ctx,
        WriteFileInput {
            path: target.to_string_lossy().into_owned(),
            content: "contenido".to_string(),
            create_dirs: Some(false),

            expected_content_hash: None,
        },
    )
    .await;

    let _ = std::fs::remove_dir_all(&planted);

    assert!(
        result.is_ok(),
        "a write is not refused because of what a previous write left at a predictable name"
    );
    assert!(target.exists(), "the file is written");
}
