//! Una escritura que declara la versión que leyó no pisa la versión que hay.
//!
//! El llamante puede decir con qué versión del fichero está trabajando. Si
//! el destino ya no tiene esa versión —porque otro editor escribió entre
//! medias—, la escritura se niega en vez de sustituir el trabajo ajeno.
//!
//! El caso que este fichero cubre es el que no se puede ver en un test
//! concurrente: no hay ninguna carrera, sólo una versión que ya no es la
//! que el llamante leyó.

use std::fs;
use std::path::Path;
use std::sync::Arc;

use cognicode_core::application::dto::{EditFileRequest, FileEdit, WriteFileRequest};
use cognicode_core::application::services::file_operations::FileOperationsService;
use cognicode_core::domain::findings::sha256_hex as digest;
use cognicode_core::infrastructure::parser::syntax_analysis::TreeSitterSyntaxAnalysis;
use cognicode_core::infrastructure::verification::RustVerifier;
use cognicode_core::interface::mcp::security::InputValidator;
use tempfile::TempDir;

fn service(root: &Path) -> FileOperationsService {
    FileOperationsService::new(
        root.to_string_lossy().to_string(),
        Arc::new(InputValidator::new().with_workspace(vec![root.to_path_buf()])),
        Arc::new(RustVerifier::new()),
        Arc::new(TreeSitterSyntaxAnalysis::new()),
    )
}

fn file_in(dir: &TempDir) -> std::path::PathBuf {
    dir.path().join("codigo.rs")
}

/// A write that declares a version the destination no longer has is refused,
/// and the destination keeps what it had.
#[test]
fn a_write_that_declares_a_stale_version_is_refused() {
    let dir = TempDir::new().expect("a temporary directory");
    let file = file_in(&dir);
    fs::write(&file, "fn original() {}\n").expect("the file is written");

    // The caller read this version...
    let read_version = digest(&fs::read(&file).expect("the file is readable"));

    // ...and somebody else wrote this one.
    fs::write(&file, "fn ajeno() {}\n").expect("the other writer is written");

    let mut request = WriteFileRequest::new(file.to_string_lossy().to_string(), "fn mio() {}\n");
    request.expected_content_hash = Some(read_version);

    let outcome = service(dir.path()).write_file(request);

    assert!(
        outcome.is_err(),
        "a write that declares a version the file no longer has must be refused"
    );
    assert_eq!(
        fs::read_to_string(&file).expect("the file is readable"),
        "fn ajeno() {}\n",
        "the other writer's work must still be there"
    );
}

/// A write that declares the version the destination has is applied.
#[test]
fn a_write_that_declares_the_current_version_is_applied() {
    let dir = TempDir::new().expect("a temporary directory");
    let file = file_in(&dir);
    fs::write(&file, "fn original() {}\n").expect("the file is written");

    let version = digest(&fs::read(&file).expect("the file is readable"));

    let mut request = WriteFileRequest::new(file.to_string_lossy().to_string(), "fn mio() {}\n");
    request.expected_content_hash = Some(version);

    service(dir.path())
        .write_file(request)
        .expect("a write that declares the current version is applied");

    assert_eq!(
        fs::read_to_string(&file).expect("the file is readable"),
        "fn mio() {}\n"
    );
}

/// A write that declares a version for a file that does not exist is
/// refused: there is no version that could match.
#[test]
fn a_write_that_declares_a_version_for_a_missing_file_is_refused() {
    let dir = TempDir::new().expect("a temporary directory");
    let file = file_in(&dir);

    let mut request = WriteFileRequest::new(file.to_string_lossy().to_string(), "fn mio() {}\n");
    request.expected_content_hash = Some(digest(b"lo que el llamante cree que hay"));

    let outcome = service(dir.path()).write_file(request);

    assert!(
        outcome.is_err(),
        "a declared version cannot match a file that does not exist"
    );
    assert!(!file.exists(), "nothing must be written");
}

/// An edit that declares a version the destination no longer has is refused,
/// and the destination keeps what it had.
#[test]
fn an_edit_that_declares_a_stale_version_is_refused() {
    let dir = TempDir::new().expect("a temporary directory");
    let file = file_in(&dir);
    fs::write(&file, "fn original() {}\n").expect("the file is written");

    let read_version = digest(&fs::read(&file).expect("the file is readable"));

    // The other writer adds a line and leaves the original one in place, so
    // the edit still finds its text: the only reason to refuse it is that
    // the version is not the one the caller read.
    fs::write(&file, "fn original() {}\nfn ajeno() {}\n").expect("the other writer is written");

    let mut request = EditFileRequest::new(
        file.to_string_lossy().to_string(),
        vec![FileEdit {
            old_string: "fn original() {}".to_string(),
            new_string: "fn mio() {}".to_string(),
        }],
    );
    request.expected_content_hash = Some(read_version);

    let outcome = service(dir.path()).edit_file(request);

    assert!(
        outcome.is_err(),
        "an edit that declares a version the file no longer has must be refused"
    );
    assert_eq!(
        fs::read_to_string(&file).expect("the file is readable"),
        "fn original() {}\nfn ajeno() {}\n",
        "the other writer's work must still be there"
    );
}

/// An edit that declares the version the destination has is applied.
#[test]
fn an_edit_that_declares_the_current_version_is_applied() {
    let dir = TempDir::new().expect("a temporary directory");
    let file = file_in(&dir);
    fs::write(&file, "fn original() {}\n").expect("the file is written");

    let version = digest(&fs::read(&file).expect("the file is readable"));

    let mut request = EditFileRequest::new(
        file.to_string_lossy().to_string(),
        vec![FileEdit {
            old_string: "fn original() {}".to_string(),
            new_string: "fn mio() {}".to_string(),
        }],
    );
    request.expected_content_hash = Some(version);

    let result = service(dir.path())
        .edit_file(request)
        .expect("an edit that declares the current version is applied");

    assert!(result.applied, "the edit must be applied: {result:?}");
    assert_eq!(
        fs::read_to_string(&file).expect("the file is readable"),
        "fn mio() {}\n"
    );
}
