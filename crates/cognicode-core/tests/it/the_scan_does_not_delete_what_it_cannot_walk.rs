//! El recorrido de la ingesta no borra lo que no pudo ver.
//!
//! `walk_files` entregaba el recorrido con `filter_map(|e| e.ok())`:
//! un directorio que el proceso no puede leer desaparecía del
//! inventario sin dejar rastro, y el paso 3 de `scan_for_changes`
//! publicaba como `Deleted` todo lo que el índice tenía y el
//! inventario ya no. Un directorio con el DAC cerrado borraba su
//! contenido del índice sin que nadie lo borrase del disco: bajas
//! fantasma, del mismo género que el `Complete` sobre un árbol que no
//! se recorrió entero (`walk_errors_are_errors.rs`).
//!
//! La garantía aquí es de integridad del índice: un archivo que no se
//! pudo inspeccionar no es un archivo eliminado. Lo que hay detrás de
//! un directorio ilegible queda fuera de la decisión, y la omisión se
//! declara — el silencio del scan significa «nada cambió», y un
//! subárbol que nadie recorrió no es silencio.

use std::collections::HashMap;

use cognicode_core::application::ingest::{
    ChangeKind, ScanEntry, scan_for_changes, scan_for_changes_with_coverage, walk_files,
    walk_inventory,
};

/// Deja un directorio que este proceso no puede leer y devuelve su ruta.
///
/// `mode 0o000` cierra el DAC al proceso que no es root. Si el proceso
/// es root, root salta los permisos y el recorrido SÍ leería el árbol:
/// la sonda lo detecta y devuelve `None`; ese `None` no es un fallo del
/// test, es la razón explícita de por qué este caso no se puede medir
/// en esta máquina. Mismo patrón que `walk_errors_are_errors.rs`.
#[cfg(unix)]
fn lock_dir(root: &std::path::Path, name: &str) -> Option<std::path::PathBuf> {
    use std::os::unix::fs::PermissionsExt as _;

    let path = root.join(name);
    std::fs::create_dir_all(&path).expect("locked dir");
    std::fs::write(path.join("inside.rs"), "pub fn inside() {}\n").expect("inside.rs");
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o000)).expect("chmod 0o000");

    // Sonda: ¿el DAC está realmente cerrado para este proceso?
    if std::fs::read_dir(&path).is_ok() {
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755))
            .expect("restore perms");
        return None;
    }
    Some(path)
}

/// Devuelve el directorio a `0o755` para que el `TempDir` se pueda
/// borrar al final del test.
#[cfg(unix)]
fn unlock_dir(path: &std::path::Path) {
    use std::os::unix::fs::PermissionsExt as _;
    let _ = std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755));
}

/// Repo con un fichero legible y el índice que dice que `locked/inside.rs`
/// existía en la pasada anterior.
#[cfg(unix)]
fn repo_with_indexed_locked_file() -> (tempfile::TempDir, HashMap<String, ScanEntry>) {
    let dir = tempfile::tempdir().expect("tempdir");
    std::fs::write(dir.path().join("main.rs"), "fn main() {}\n").expect("main.rs");

    let mut previous = HashMap::new();
    previous.insert(
        "locked/inside.rs".to_string(),
        ScanEntry {
            content_hash: Some("previous-hash".to_string()),
            mtime_millis: Some(1_000),
        },
    );
    previous.insert(
        "main.rs".to_string(),
        ScanEntry {
            content_hash: Some("previous-hash".to_string()),
            mtime_millis: Some(1_000),
        },
    );
    (dir, previous)
}

#[cfg(unix)]
#[test]
fn la_baja_fantasma_no_existe_lo_que_el_recorrido_no_pudo_ver_no_se_borra() {
    let (dir, previous) = repo_with_indexed_locked_file();
    let Some(locked) = lock_dir(dir.path(), "locked") else {
        eprintln!(
            "SKIP: el DAC no se puede cerrar para este proceso (root lo salta); \
             no hay omisión de recorrido que exigir en esta máquina"
        );
        return;
    };

    let changes = scan_for_changes(dir.path(), &previous);
    let phantom = changes
        .iter()
        .find(|c| c.kind == ChangeKind::Deleted && c.path.ends_with("inside.rs"));
    unlock_dir(&locked);

    assert!(
        phantom.is_none(),
        "el recorrido no pudo entrar en `locked/` y el scan publica la baja de \
         `locked/inside.rs`: un archivo que nadie pudo inspeccionar no es un \
         archivo eliminado. Cambios publicados: {:?}",
        changes
    );
}

#[cfg(unix)]
#[test]
fn la_baja_real_sigue_publicandose() {
    // Control del caso sano: sin omisión de recorrido, una baja real
    // tiene que seguir publicándose. Si el arreglo de arriba apagase
    // también esta, el índice dejaría de morir nunca.
    let dir = tempfile::tempdir().expect("tempdir");
    std::fs::write(dir.path().join("existing.rs"), "// old\n").expect("existing.rs");

    let mut previous = HashMap::new();
    previous.insert(
        "existing.rs".to_string(),
        ScanEntry {
            content_hash: Some("previous-hash".to_string()),
            mtime_millis: Some(1_000),
        },
    );

    std::fs::remove_file(dir.path().join("existing.rs")).expect("remove file");
    let changes = scan_for_changes(dir.path(), &previous);

    let deleted = changes
        .iter()
        .find(|c| c.kind == ChangeKind::Deleted && c.path.ends_with("existing.rs"));
    assert!(
        deleted.is_some(),
        "una baja real se sigue publicando: {:?}",
        changes
    );
}

#[cfg(unix)]
#[test]
fn el_inventario_nombra_lo_que_el_recorrido_no_pudo_leer() {
    let (dir, _previous) = repo_with_indexed_locked_file();
    let Some(locked) = lock_dir(dir.path(), "locked") else {
        eprintln!("SKIP: el DAC no se puede cerrar para este proceso");
        return;
    };

    let inventory = walk_inventory(dir.path());
    let files = walk_files(dir.path());
    unlock_dir(&locked);

    let omission = inventory
        .omissions
        .iter()
        .find(|o| o.path.ends_with("locked"))
        .expect("el inventario nombra el directorio que el recorrido no pudo leer");
    assert!(
        !omission.reason.is_empty(),
        "la omisión dice por qué: {:?}",
        omission
    );

    assert!(
        inventory.files.iter().any(|p| p.ends_with("main.rs")),
        "los ficheros legibles siguen en el inventario: {:?}",
        inventory.files
    );
    assert_eq!(
        files, inventory.files,
        "walk_files es la proyección de ficheros del mismo inventario, no otro recorrido"
    );
}

#[cfg(unix)]
#[test]
fn la_cobertura_con_omisiones_se_publica_junto_a_los_cambios() {
    let (dir, previous) = repo_with_indexed_locked_file();
    let Some(locked) = lock_dir(dir.path(), "locked") else {
        eprintln!("SKIP: el DAC no se puede cerrar para este proceso");
        return;
    };

    let (changes, omissions) = scan_for_changes_with_coverage(dir.path(), &previous);
    unlock_dir(&locked);

    assert!(
        omissions.iter().any(|o| o.path.ends_with("locked")),
        "el scan declara la omisión junto a los cambios: {:?}",
        omissions
    );
    assert!(
        !changes
            .iter()
            .any(|c| c.kind == ChangeKind::Deleted && c.path.ends_with("inside.rs")),
        "sin bajas fantasma tampoco en la puerta con cobertura: {:?}",
        changes
    );
    assert!(
        changes
            .iter()
            .any(|c| c.path.ends_with("main.rs") && c.kind == ChangeKind::Changed),
        "los ficheros que sí se pudieron leer siguen escaneados: {:?}",
        changes
    );
}
