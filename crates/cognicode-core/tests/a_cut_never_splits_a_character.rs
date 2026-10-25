//! Un corte por indice de byte no parte un caracter.
//!
//! Los tres caminos que este fichero ejercita eran alcanzables con valores
//! que el llamante controla: el tamano del trozo, el desplazamiento del
//! token de continuacion y el texto que se edita. Un corte por indice de
//! byte sobre texto multibyte aborta el proceso que lo intenta, y en el
//! servidor de protocolo ese proceso es el que atiende las peticiones de
//! todos los agentes conectados a la vez.

use std::fs;
use std::path::Path;
use std::sync::Arc;

use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
use cognicode_core::application::dto::{EditFileRequest, FileEdit, ReadFileRequest, ReadMode};
use cognicode_core::application::services::file_operations::FileOperationsService;
use cognicode_core::infrastructure::parser::syntax_analysis::TreeSitterSyntaxAnalysis;
use cognicode_core::infrastructure::verification::RustVerifier;
use cognicode_core::interface::mcp::security::InputValidator;
use serde_json::json;
use tempfile::TempDir;

fn service(root: &Path) -> FileOperationsService {
    FileOperationsService::new(
        root.to_string_lossy().to_string(),
        Arc::new(InputValidator::new().with_workspace(vec![root.to_path_buf()])),
        Arc::new(RustVerifier::new()),
        Arc::new(TreeSitterSyntaxAnalysis::new()),
    )
}

/// The continuation token the service itself issues, with the offset the
/// caller asks for. The token is not signed, so the caller can put any offset
/// in it; that is exactly the surface these tests cover.
fn continuation_token(path: &Path, offset: usize, chunk_size: usize) -> String {
    let canonical = fs::canonicalize(path).expect("the file exists");
    let payload = json!({
        "path": canonical.to_string_lossy(),
        "offset": offset,
        "chunk_size": chunk_size,
    });
    BASE64.encode(serde_json::to_vec(&payload).expect("the payload serializes"))
}

/// A chunk that ends inside a character is refused with an error, and the
/// process keeps running.
#[test]
fn a_chunk_that_ends_inside_a_character_is_refused() {
    let dir = TempDir::new().expect("a temporary directory");
    let file = dir.path().join("multibyte.txt");
    // One character, two bytes.
    fs::write(&file, "é").expect("the file is written");

    let mut request = ReadFileRequest::new(file.to_string_lossy().to_string());
    request.mode = Some(ReadMode::Raw);
    // One byte of a two-byte character.
    request.chunk_size = Some(1);

    let outcome = service(dir.path()).read_file(request);

    assert!(
        outcome.is_err(),
        "a chunk that ends inside a character must be refused, not served half a character"
    );
}

/// A token that starts inside a character is refused with an error.
#[test]
fn a_token_that_starts_inside_a_character_is_refused() {
    let dir = TempDir::new().expect("a temporary directory");
    let file = dir.path().join("multibyte.txt");
    fs::write(&file, "éabc").expect("the file is written");

    let mut request = ReadFileRequest::new(file.to_string_lossy().to_string());
    request.mode = Some(ReadMode::Raw);
    // Byte 1 is inside the two-byte character.
    request.continuation_token = Some(continuation_token(&file, 1, 16));

    let outcome = service(dir.path()).read_file(request);

    assert!(
        outcome.is_err(),
        "a token that starts inside a character must be refused, not served half a character"
    );
}

/// The chunks of a read still reassemble into the file, byte for byte, when
/// the chunk size lands inside a character.
#[test]
fn chunks_that_stop_on_a_character_boundary_still_reassemble_the_file() {
    let dir = TempDir::new().expect("a temporary directory");
    let file = dir.path().join("multibyte.txt");
    let content = "aé\nb";
    fs::write(&file, content).expect("the file is written");

    let service = service(dir.path());
    let mut assembled = String::new();
    let mut token: Option<String> = None;

    for _ in 0..10 {
        let mut request = ReadFileRequest::new(file.to_string_lossy().to_string());
        request.mode = Some(ReadMode::Raw);
        request.chunk_size = Some(2);
        request.continuation_token = token;

        let result = service.read_file(request).expect("the read succeeds");
        assembled.push_str(&result.content);
        token = result.next_token;
        if token.is_none() {
            break;
        }
    }

    assert_eq!(
        assembled, content,
        "the chunks must reassemble into the file without losing or splitting a character"
    );
}

/// A refused edit reports a message that does not split the character it
/// truncates.
#[test]
fn a_refused_edit_reports_a_message_that_does_not_split_a_character() {
    let dir = TempDir::new().expect("a temporary directory");
    let file = dir.path().join("multibyte.txt");
    fs::write(&file, "hola\n").expect("the file is written");

    // Fifty-one bytes of three-byte characters, absent from the file. The
    // message keeps fifty bytes, and byte fifty lands inside the last
    // character.
    let old_string = "€".repeat(17);
    let request = EditFileRequest::new(
        file.to_string_lossy().to_string(),
        vec![FileEdit {
            old_string,
            new_string: "x".to_string(),
        }],
    );

    let result = service(dir.path())
        .edit_file(request)
        .expect("the edit is refused, not fatal");

    assert!(!result.applied, "there is no match, so nothing is applied");
    let preview = result.preview.unwrap_or_default();
    assert!(
        preview.contains(&"€".repeat(16)),
        "the message must show the whole characters it kept, got: {preview}"
    );
}
