//! El escaneo declara los ficheros que no puede leer, y no los borra.
//!
//! ## El hueco
//!
//! El escaneo decide qué se vuelve a leer comparando el hash del fichero con
//! el que el índice guardó la vez anterior. Para compararlo hace falta leer
//! el fichero. Cuando la lectura falla, el escaneo no lo decía: el fichero
//! desaparecía del mapa de hashes, y a partir de ahí una lectura fallida y
//! un fichero intacto eran la misma cosa. Un fichero nuevo ilegible entraba
//! en el índice con un hash vacío —una afirmación sobre un contenido que
//! nadie leyó— y un fichero existente ilegible se publicaba como «sin
//! cambios».
//!
//! ## Lo que se mide aquí
//!
//! - Un fichero nuevo que no se puede leer no entra con un hash vacío.
//! - Un fichero que no se puede leer se publica, porque el silencio del
//!   escaneo significa «no ha cambiado».
//! - El fallo se publica con su causa, no como un silencio.
//! - El índice no guarda el hash viejo de un fichero que nadie pudo leer, y
//!   el fichero se vuelve a mirar en la siguiente pasada.
//! - Un fichero que no ha cambiado no se publica: la contraprueba, para que
//!   publicar de más no pueda pasar por publicar la verdad.
//!
//! ## Cómo se reproduce un fichero que no se puede leer
//!
//! Con un fichero en modo `0000` dentro de un directorio que se puede
//! recorrer. El directorio se puede listar y el fichero se puede fechar —el
//! sistema de ficheros publica su metadata—, pero su contenido no se puede
//! leer. Es un caso real: un fichero que el usuario no tiene permiso de
//! leer, y el escaneo tiene que seguir viéndolo.

#![cfg(unix)]

use cognicode_core::application::ingest::{ChangeKind, ScanEntry, hash_file, scan_for_changes};
use cognicode_core::domain::value_objects::file_time::measured_millis;
use std::collections::HashMap;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

/// Un fichero que se puede listar y fechar, pero no leer.
struct UnreadableFile {
    path: PathBuf,
}

impl UnreadableFile {
    /// Crea el fichero cerrado a la lectura y devuelve su ruta.
    fn create(root: &Path, name: &str) -> (UnreadableFile, PathBuf) {
        let path = root.join(name);
        fs::write(&path, "fn main() {}\n").expect("se escribe el fichero");
        fs::set_permissions(&path, fs::Permissions::from_mode(0o000))
            .expect("se cierra el fichero a la lectura");
        (UnreadableFile { path: path.clone() }, path)
    }
}

impl Drop for UnreadableFile {
    fn drop(&mut self) {
        // The file has to become readable again, or the temporary workspace
        // could not be cleaned up.
        let _ = fs::set_permissions(&self.path, fs::Permissions::from_mode(0o644));
    }
}

/// El cambio que el escaneo dice de un fichero concreto.
fn change_of<'a>(
    changes: &'a [cognicode_core::application::ingest::FileChange],
    path: &Path,
) -> Option<&'a cognicode_core::application::ingest::FileChange> {
    changes.iter().find(|change| change.path == path)
}

/// La fecha que el escaneo va a medir de un fichero.
fn mtime_of(path: &Path) -> Option<i64> {
    measured_millis(
        fs::metadata(path)
            .ok()
            .and_then(|meta| meta.modified().ok()),
    )
}

#[test]
fn un_fichero_nuevo_que_no_se_puede_leer_no_entra_con_un_hash_vacio() {
    let workspace = tempfile::tempdir().expect("workspace");
    let (_unreadable, path) = UnreadableFile::create(workspace.path(), "ilegible.rs");

    let changes = scan_for_changes(workspace.path(), &HashMap::new());
    let published = change_of(&changes, &path).expect("el escaneo tiene que publicar el fichero");

    assert_eq!(
        published.content_hash, None,
        "un fichero que nadie pudo leer no entra en el indice con un hash \
         vacio: el hash vacio es una afirmacion sobre un contenido que nadie leyo"
    );
}

#[test]
fn un_fichero_que_no_se_puede_leer_se_publica_como_fallo() {
    let workspace = tempfile::tempdir().expect("workspace");
    let (_unreadable, path) = UnreadableFile::create(workspace.path(), "ilegible.rs");

    let changes = scan_for_changes(workspace.path(), &HashMap::new());
    let published = change_of(&changes, &path).expect("el escaneo tiene que publicar el fichero");

    assert_eq!(
        published.kind,
        ChangeKind::Unreadable,
        "un fichero que nadie pudo leer se publica como lo que es: un fallo \
         de lectura, no un fichero nuevo y no un fichero sin cambios"
    );
}

#[test]
fn un_fichero_que_no_se_puede_leer_declara_la_causa_del_fallo() {
    let workspace = tempfile::tempdir().expect("workspace");
    let (_unreadable, path) = UnreadableFile::create(workspace.path(), "ilegible.rs");

    let changes = scan_for_changes(workspace.path(), &HashMap::new());
    let published = change_of(&changes, &path).expect("el escaneo tiene que publicar el fichero");

    assert!(
        published
            .read_error
            .as_deref()
            .is_some_and(|reason| !reason.trim().is_empty()),
        "un fallo de lectura tiene una causa: un fallo sin causa es medio fallo"
    );
}

#[test]
fn un_fichero_que_no_se_puede_leer_no_se_da_por_igual() {
    // El fichero estaba en el indice con un hash viejo, y ahora su contenido
    // no se puede leer. El escaneo no puede decir que no cambio: no lo leyo,
    // y el silencio del escaneo significa «no ha cambiado».
    let workspace = tempfile::tempdir().expect("workspace");
    let (_unreadable, path) = UnreadableFile::create(workspace.path(), "ilegible.rs");

    let previous = HashMap::from([(
        "ilegible.rs".to_string(),
        ScanEntry {
            content_hash: Some("el-hash-anterior".to_string()),
            mtime_millis: Some(0),
        },
    )]);

    let changes = scan_for_changes(workspace.path(), &previous);

    assert!(
        change_of(&changes, &path).is_some(),
        "un fichero que no se pudo leer tiene que salir en el escaneo: \
         el escaneo lo borro, y con el se borro el hecho de que nadie lo leyo"
    );
}

#[test]
fn un_fichero_que_no_se_puede_leer_no_publica_el_hash_viejo() {
    let workspace = tempfile::tempdir().expect("workspace");
    let (_unreadable, path) = UnreadableFile::create(workspace.path(), "ilegible.rs");

    let previous = HashMap::from([(
        "ilegible.rs".to_string(),
        ScanEntry {
            content_hash: Some("el-hash-anterior".to_string()),
            mtime_millis: Some(0),
        },
    )]);

    let changes = scan_for_changes(workspace.path(), &previous);
    let change = change_of(&changes, &path).expect("el escaneo tiene que publicar el fichero");

    assert_eq!(
        change.content_hash, None,
        "el indice no guarda el hash viejo de un fichero que nadie pudo leer: \
         ese hash dice el contenido de una lectura que no ocurrio"
    );
}

#[test]
fn el_indice_no_guarda_el_hash_de_un_fichero_que_nadie_pudo_leer() {
    // Lo que el escaneo deja en el indice es lo que la siguiente pasada
    // va a comparar. Si ahi queda un hash, la siguiente pasada va a creer
    // que el contenido de esa lectura existe.
    let workspace = tempfile::tempdir().expect("workspace");
    let (_unreadable, path) = UnreadableFile::create(workspace.path(), "ilegible.rs");

    let changes = scan_for_changes(workspace.path(), &HashMap::new());
    let published = change_of(&changes, &path).expect("el escaneo tiene que publicar el fichero");
    let entry = published.scan_entry();

    assert_eq!(
        entry.content_hash, None,
        "el indice guarda que nadie leyo el contenido: un hash ahi seria \
         una afirmacion sobre una lectura que no ocurrio"
    );
}

#[test]
fn un_fichero_que_no_se_pudo_leer_se_vuelve_a_mirar_en_la_siguiente_pasada() {
    // El indice dice que nadie leyo el contenido de este fichero. La fecha
    // no ha cambiado, asi que la via rapida no lo miraria: el escaneo tiene
    // que volver a mirarlo, porque no hay hash con el que comparar.
    let workspace = tempfile::tempdir().expect("workspace");
    let path = workspace.path().join("main.rs");
    fs::write(&path, "fn main() {}\n").expect("se escribe");
    let hash = hash_file(&path).expect("se lee");

    let previous = HashMap::from([(
        "main.rs".to_string(),
        ScanEntry {
            content_hash: None,
            mtime_millis: mtime_of(&path),
        },
    )]);

    let changes = scan_for_changes(workspace.path(), &previous);
    let published = change_of(&changes, &path).expect("el escaneo tiene que volver a mirarlo");

    assert_eq!(
        published.content_hash.as_deref(),
        Some(hash.as_str()),
        "un fichero que la vez anterior nadie pudo leer se vuelve a mirar: \
         sin hash no hay nada con lo que comparar"
    );
}

#[test]
fn un_fichero_que_no_cambio_no_se_publica() {
    // La contraprueba: si el escaneo publicara todo, los tests anteriores
    // pasarian igual. Un fichero que se puede leer y que no ha cambiado no
    // es un cambio, y no se publica.
    let workspace = tempfile::tempdir().expect("workspace");
    let path = workspace.path().join("main.rs");
    fs::write(&path, "fn main() {}\n").expect("se escribe");
    let hash = hash_file(&path).expect("se lee");

    let previous = HashMap::from([(
        "main.rs".to_string(),
        ScanEntry {
            content_hash: Some(hash),
            mtime_millis: Some(0),
        },
    )]);

    let changes = scan_for_changes(workspace.path(), &previous);
    assert!(
        change_of(&changes, &path).is_none(),
        "un fichero que no cambio no se publica: el escaneo publica cambios"
    );
}

#[test]
fn un_fichero_que_cambio_se_publica_como_cambiado() {
    // La contraprueba del camino de los cambios: el fallo no se ha tragado
    // al camino que si funciona.
    let workspace = tempfile::tempdir().expect("workspace");
    let path = workspace.path().join("main.rs");
    fs::write(&path, "fn main() {}\n").expect("se escribe");
    let hash = hash_file(&path).expect("se lee");

    let previous = HashMap::from([(
        "main.rs".to_string(),
        ScanEntry {
            content_hash: Some("el-hash-anterior".to_string()),
            mtime_millis: Some(0),
        },
    )]);

    let changes = scan_for_changes(workspace.path(), &previous);
    let published = change_of(&changes, &path).expect("el escaneo tiene que publicar el fichero");

    assert_eq!(
        published.kind,
        ChangeKind::Changed,
        "un fichero cuyo contenido cambio se publica como cambiado"
    );
    assert_eq!(published.content_hash.as_deref(), Some(hash.as_str()));
}
