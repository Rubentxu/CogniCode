//! Lo que se rechaza no deja rastro, y las ediciones se aplican al original.
//!
//! Estos checks son de caracterización: **miden el defecto antes de que exista
//! el arreglo**, y por eso están escritos contra la API que había, no contra la
//! que hay después. Los discriminantes fallan antes del arreglo; los que
//! guardan pasan antes y después, y son los que dicen que el arreglo no rompe
//! lo que ya funcionaba.
//!
//! Lo que miden:
//!
//! 1. **Una escritura rechazada por su tamaño no deja directorios.** El
//!    límite se comprueba después de crear el árbol, así que la petición
//!    rechazada deja en el disco lo que pidió.
//! 2. **Las ediciones se aplican sobre el contenido original.** Se comprueban
//!    sobre el original y se aplican en cadena sobre el resultado de la
//!    anterior: sobre `A B`, `A→B` y `B→C` producen `C C` y no `B C`.
//! 3. **Dos ediciones que se solapan se rechazan.** La segunda pierde su
//!    coincidencia porque la primera se la llevó, y la respuesta dice que se
//!    aplicó sin decir que la mitad de lo pedido no se hizo.
//!
//! El cuarto y los dos siguientes guardan: una sola edición se comporta como
//! antes, y los motivos `no_match` y `multiple_matches` siguen siendo los
//! mismos.

use cognicode_core::application::dto::FileEdit;
use cognicode_core::interface::mcp::file_ops_handlers::{handle_edit_file, handle_write_file};
use cognicode_core::interface::mcp::handlers::HandlerContext;
use cognicode_core::interface::mcp::schemas::{EditFileInput, WriteFileInput};

/// Un contexto con el workspace en un directorio temporal.
fn ctx_for(workspace: &std::path::Path) -> HandlerContext {
    HandlerContext::builder()
        .with_working_dir(workspace.to_path_buf())
        .build()
}

/// Una escritura rechazada por su tamaño no deja el árbol de directorios.
///
/// El caso: una ruta dentro del workspace, con `create_dirs`, y un contenido
/// que pasa del límite de 10 MB. El producto crea el árbol y después rechaza
/// la escritura: lo que la petición rechazada no debía tener se queda en el
/// disco.
#[tokio::test]
async fn a_write_refused_for_being_too_big_leaves_no_directory() {
    let workspace = tempfile::tempdir().expect("workspace");
    let ctx = ctx_for(workspace.path());

    let target = workspace.path().join("deep").join("nested").join("big.txt");
    let result = handle_write_file(
        &ctx,
        WriteFileInput {
            path: target.to_string_lossy().into_owned(),
            content: "x".repeat(10 * 1024 * 1024 + 1),
            create_dirs: Some(true),

            expected_content_hash: None,
        },
    )
    .await;

    assert!(result.is_err(), "a content over the limit is refused");
    assert!(
        !workspace.path().join("deep").exists(),
        "a refused write left a directory tree behind: {:?}",
        workspace.path().join("deep")
    );
}

/// Las ediciones se aplican sobre el contenido original.
///
/// El caso: el contenido es `A B` y las ediciones son `A→B` y `B→C`. Las dos
/// entradas existen exactamente una vez en el original, así que las dos
/// ediciones son válidas. Aplicadas a la vez, el resultado es `B C`: la
/// primera cambia su `A` y la segunda cambia el `B` que ya estaba. Aplicadas
/// en cadena, la primera convierte el contenido en `B B` y la segunda cambia
/// los dos `B` de golpe: el resultado es `C C`.
#[tokio::test]
async fn two_edits_are_applied_to_the_original_content() {
    let workspace = tempfile::tempdir().expect("workspace");
    let ctx = ctx_for(workspace.path());

    let file = workspace.path().join("notas.notalang");
    std::fs::write(&file, "A B").expect("the file is written");

    let output = handle_edit_file(
        &ctx,
        EditFileInput {
            path: file.to_string_lossy().into_owned(),
            edits: vec![
                FileEdit {
                    old_string: "A".to_string(),
                    new_string: "B".to_string(),
                },
                FileEdit {
                    old_string: "B".to_string(),
                    new_string: "C".to_string(),
                },
            ],

            expected_content_hash: None,
        },
    )
    .await
    .expect("the edit is answered");

    let content = std::fs::read_to_string(&file).expect("the file is readable");
    assert!(
        output.applied,
        "the edit is applied: both old strings occur exactly once in the original"
    );
    assert_eq!(
        content, "B C",
        "the edits are applied to the original content, not to the result of the previous edit"
    );
}

/// Dos ediciones que se solapan se rechazan.
///
/// El caso: el contenido es `abc` y las ediciones son `ab→X` y `bc→Y`. Las
/// dos entradas existen exactamente una vez, pero las dos cubren la misma
/// región: aplicarlas a la vez no tiene un único resultado. Aplicadas en
/// cadena, la segunda ya no encuentra su coincidencia —la primera se la
/// llevó— y la petición se declara aplicada sin haber hecho la mitad de lo
/// que pedía.
#[tokio::test]
async fn overlapping_edits_are_refused_instead_of_losing_one() {
    let workspace = tempfile::tempdir().expect("workspace");
    let ctx = ctx_for(workspace.path());

    let file = workspace.path().join("solape.notalang");
    std::fs::write(&file, "abc").expect("the file is written");

    let output = handle_edit_file(
        &ctx,
        EditFileInput {
            path: file.to_string_lossy().into_owned(),
            edits: vec![
                FileEdit {
                    old_string: "ab".to_string(),
                    new_string: "X".to_string(),
                },
                FileEdit {
                    old_string: "bc".to_string(),
                    new_string: "Y".to_string(),
                },
            ],

            expected_content_hash: None,
        },
    )
    .await
    .expect("the edit is answered");

    let content = std::fs::read_to_string(&file).expect("the file is readable");
    assert!(
        !output.applied,
        "an ambiguous request is not applied: the two edits cover the same region"
    );
    assert_eq!(
        output.reason.as_deref(),
        Some("overlapping_edits"),
        "the refusal says why it did not apply"
    );
    assert_eq!(
        content, "abc",
        "a refused edit leaves the file exactly as it was"
    );
}

/// Una sola edición se comporta como antes.
///
/// La guarda del camino corriente: el arreglo de las ediciones múltiples no
/// puede tocar el caso de una.
#[tokio::test]
async fn a_single_edit_still_applies() {
    let workspace = tempfile::tempdir().expect("workspace");
    let ctx = ctx_for(workspace.path());

    let file = workspace.path().join("una.rs");
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

    let content = std::fs::read_to_string(&file).expect("the file is readable");
    assert!(output.applied, "a single edit is applied");
    assert_eq!(content, "fn test() { }\n", "the file holds the edit");
}

/// Una edición que no encuentra su coincidencia lo dice.
///
/// La guarda del motivo que ya existía: el arreglo no puede cambiarlo.
#[tokio::test]
async fn an_edit_without_a_match_still_says_no_match() {
    let workspace = tempfile::tempdir().expect("workspace");
    let ctx = ctx_for(workspace.path());

    let file = workspace.path().join("nada.notalang");
    std::fs::write(&file, "contenido").expect("the file is written");

    let output = handle_edit_file(
        &ctx,
        EditFileInput {
            path: file.to_string_lossy().into_owned(),
            edits: vec![FileEdit {
                old_string: "inexistente".to_string(),
                new_string: "x".to_string(),
            }],

            expected_content_hash: None,
        },
    )
    .await
    .expect("the edit is answered");

    assert!(!output.applied, "an edit without a match is not applied");
    assert_eq!(
        output.reason.as_deref(),
        Some("no_match"),
        "the refusal says the old string was not found"
    );
}

/// Una edición que encuentra varias coincidencias lo dice.
///
/// La guarda del otro motivo que ya existía: el arreglo no puede cambiarlo.
#[tokio::test]
async fn an_edit_with_several_matches_still_says_multiple_matches() {
    let workspace = tempfile::tempdir().expect("workspace");
    let ctx = ctx_for(workspace.path());

    let file = workspace.path().join("varias.notalang");
    std::fs::write(&file, "x x x").expect("the file is written");

    let output = handle_edit_file(
        &ctx,
        EditFileInput {
            path: file.to_string_lossy().into_owned(),
            edits: vec![FileEdit {
                old_string: "x".to_string(),
                new_string: "y".to_string(),
            }],

            expected_content_hash: None,
        },
    )
    .await
    .expect("the edit is answered");

    assert!(
        !output.applied,
        "an edit that matches more than once is not applied"
    );
    assert_eq!(
        output.reason.as_deref(),
        Some("multiple_matches"),
        "the refusal says the old string was not unique"
    );
}
