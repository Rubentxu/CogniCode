//! El escaneo declara los ficheros que no puede fechar, y no los borra.
//!
//! ## El hueco
//!
//! El escaneo de la ingestion media la marca de tiempo de cada fichero que
//! encuentra y, con esa medida, decidia si el fichero habia cambiado. El
//! filtro que hacia las dos cosas a la vez —medir y decidir— era una
//! `filter_map`: cuando la fecha no se podia leer, el fichero entero se
//! perdia. No era un fichero sin fecha: era un fichero que el escaneo ya no
//! conocia, y por eso tampoco llegaba a su registro.
//!
//! ## Lo que se mide aqui
//!
//! - Un fichero cuya fecha no se puede leer tiene que seguir en el escaneo.
//! - Su hueco se declara: la fecha es "ninguna" y no un numero que se
//!   parece a la epoca.
//! - Una fecha anterior a la epoca se publica con su signo.
//! - Un fichero cuya fecha no se pudo leer la vez anterior no se da por
//!   igual: se vuelve a mirar, y decide el hash.
//!
//! ## Como se reproduce un fichero que no se puede fechar
//!
//! Con un directorio que se puede listar pero no atravesar (modo `0444`).
//! Sus entradas se ven, pero `stat` sobre ellas falla: el sistema de
//! ficheros no publica la metadata, y con ella la fecha. Es un caso real —
//! un directorio que el usuario puede ver pero no recorrer — y no hace falta
//! falsificar el reloj para reproducirlo.

#![cfg(unix)]

use cognicode_core::application::ingest::{ChangeKind, ScanEntry, scan_for_changes};
use std::collections::HashMap;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// Un directorio que se puede listar pero no atravesar.
///
/// While it lives, the directory is closed to the traversal: `readdir`
/// works, `stat` over its entries does not.
struct LockedDir {
    path: PathBuf,
}

impl LockedDir {
    /// Crea el directorio cerrado y devuelve el fichero que hay dentro.
    fn create(root: &Path) -> (LockedDir, PathBuf) {
        let path = root.join("locked");
        fs::create_dir(&path).expect("se crea el directorio");
        let file = path.join("sin_fecha.rs");
        fs::write(&file, "fn main() {}\n").expect("se escribe el fichero");
        fs::set_permissions(&path, fs::Permissions::from_mode(0o444))
            .expect("se cierra el directorio a la lectura");
        (LockedDir { path }, file)
    }
}

impl Drop for LockedDir {
    fn drop(&mut self) {
        // The directory has to go back to being walkable, or the temporary
        // workspace could not be cleaned up.
        let _ = fs::set_permissions(&self.path, fs::Permissions::from_mode(0o755));
    }
}

/// Escribe un fichero y le pone una fecha concreta.
fn write_with_mtime(path: &Path, body: &str, mtime: SystemTime) {
    fs::write(path, body).expect("se escribe el fichero");
    let file = fs::OpenOptions::new()
        .write(true)
        .open(path)
        .expect("se abre para la fecha");
    file.set_modified(mtime).expect("se pone la fecha");
}

/// El cambio que el escaneo dice de un fichero concreto.
fn change_of<'a>(
    changes: &'a [cognicode_core::application::ingest::FileChange],
    path: &Path,
) -> &'a cognicode_core::application::ingest::FileChange {
    changes
        .iter()
        .find(|change| change.path == path)
        .unwrap_or_else(|| panic!("el escaneo tiene que traer {}", path.display()))
}

#[test]
fn un_fichero_que_no_se_puede_fechar_no_desaparece_del_escaneo() {
    let workspace = tempfile::tempdir().expect("workspace");
    fs::write(workspace.path().join("visible.rs"), "fn main() {}\n").expect("se escribe");
    let (_locked, undatable) = LockedDir::create(workspace.path());

    let changes = scan_for_changes(workspace.path(), &HashMap::new());

    let paths: Vec<PathBuf> = changes.iter().map(|change| change.path.clone()).collect();
    assert!(
        paths.contains(&undatable),
        "el fichero cuya fecha no se puede leer tiene que seguir en el escaneo; \
         el escaneo lo borro y con el se borro su hueco: {:?}",
        paths
    );
}

#[test]
fn un_fichero_sin_fecha_declara_su_hueco() {
    let workspace = tempfile::tempdir().expect("workspace");
    let (_locked, undatable) = LockedDir::create(workspace.path());

    let changes = scan_for_changes(workspace.path(), &HashMap::new());
    let change = change_of(&changes, &undatable);

    assert_eq!(
        change.mtime_millis, None,
        "un fichero cuya fecha no se puede leer declara que no tiene fecha: \
         la ausencia no es un numero, y el numero mas parecido es la epoca"
    );
}

#[test]
fn una_fecha_anterior_a_la_epoca_se_publica_con_su_signo() {
    let workspace = tempfile::tempdir().expect("workspace");
    let path = workspace.path().join("notas.txt");
    // Un minuto antes de la epoca: una fecha real, y anterior a ella.
    write_with_mtime(&path, "contenido\n", UNIX_EPOCH - Duration::from_secs(60));

    let changes = scan_for_changes(workspace.path(), &HashMap::new());
    let change = change_of(&changes, &path);

    assert_eq!(
        change.mtime_millis,
        Some(-60_000),
        "el fichero lleva fecha de un minuto antes de la epoca: el escaneo \
         tiene que publicar esa fecha, no la epoca"
    );
}

#[test]
fn una_fecha_medida_se_publica_como_el_numero_que_es() {
    // La contraprueba: si el escaneo publicara siempre `None`, los tests
    // anteriores pasarian igual. Aqui se pinna el valor medido.
    let workspace = tempfile::tempdir().expect("workspace");
    let path = workspace.path().join("notas.txt");
    write_with_mtime(
        &path,
        "contenido\n",
        UNIX_EPOCH + Duration::from_secs(1_700_000_000),
    );

    let changes = scan_for_changes(workspace.path(), &HashMap::new());
    let change = change_of(&changes, &path);

    assert_eq!(
        change.mtime_millis,
        Some(1_700_000_000_000),
        "una fecha medida se publica tal cual se midio"
    );
}

#[test]
fn un_fichero_que_no_se_pudo_fechar_la_vez_anterior_no_se_da_por_igual() {
    // Sin fecha no se puede afirmar que nada cambio. El escaneo tiene que
    // volver a mirar el fichero, y dejar que decida el hash.
    let workspace = tempfile::tempdir().expect("workspace");
    let path = workspace.path().join("main.rs");
    fs::write(&path, "fn main() { println!(\"cambiado\"); }\n").expect("se escribe");

    let previous = HashMap::from([(
        "main.rs".to_string(),
        ScanEntry {
            content_hash: Some("el-hash-anterior".to_string()),
            mtime_millis: None,
        },
    )]);

    let changes = scan_for_changes(workspace.path(), &previous);
    let change = change_of(&changes, &path);

    assert_eq!(
        change.kind,
        ChangeKind::Changed,
        "un fichero sin fecha no puede darse por igual: su contenido cambio"
    );
}
