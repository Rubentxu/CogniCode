//! Un enlace que se queda dentro del workspace no es un agujero.
//!
//! ## El defecto
//!
//! El walk nunca sigue un enlace —esa es la ley y no cambia—, pero cada
//! declinación contaba como un agujero de cobertura. Este repositorio tiene
//! ocho ficheros `LICENSE-*` que apuntan al `LICENSE-MIT` de la raíz, y con
//! ellos el `overview` y el `status` de este repositorio salían `partial`
//! para siempre, con el motivo «symlink to ../../LICENSE-MIT, not followed».
//! Un estado `partial` permanente entrena al lector a ignorar `partial`, y
//! eso destruye la señal que el contrato de honestidad quiere dar.
//!
//! ## La regla
//!
//! Un declinado es un agujero cuando lo que hay detrás **no** está cubierto
//! por el walk:
//!
//! - un enlace que apunta fuera del workspace describe contenido que el
//!   walk no puede alcanzar: eso sí es un agujero, y se sigue declarando
//!   con su motivo;
//! - un enlace que apunta dentro describe contenido que el walk lee bajo su
//!   nombre real (un fuente) o contenido que el grafo nunca reclamó (una
//!   licencia): declinarlo no deja nada fuera.
//!
//! ## Lo que no cambia
//!
//! El walk sigue sin seguir el enlace: la ley es la ley. Lo que cambia es si
//! la declinación cuesta la cobertura.

#![cfg(unix)]

use cognicode_core::infrastructure::graph::source_files::inventory;
use cognicode_core::interface::goals::common::coverage_of;

/// Escribe un fichero, creando su directorio.
fn write(path: &std::path::Path, contents: &str) {
    std::fs::create_dir_all(path.parent().expect("a file has a parent")).expect("directory");
    std::fs::write(path, contents).expect("the file is written");
}

/// Un árbol con un fuente y una licencia, y un enlace relativo a la
/// licencia desde un subdirectorio: la forma que tiene este repositorio.
fn tree_with_a_link_to_the_licence() -> (tempfile::TempDir, std::path::PathBuf) {
    let dir = tempfile::tempdir().expect("tempdir");
    let root = dir.path().to_path_buf();
    write(&root.join("main.rs"), "fn main() {}\n");
    write(&root.join("LICENSE-MIT"), "the licence\n");
    write(&root.join("pkg/lib.rs"), "pub fn kept() {}\n");
    std::fs::create_dir_all(root.join("pkg")).expect("directory");
    std::os::unix::fs::symlink("../LICENSE-MIT", root.join("pkg/LICENSE-MIT")).expect("symlink");
    (dir, root)
}

/// Un enlace que apunta dentro del workspace no es un agujero.
///
/// El destino es una licencia: el grafo nunca la reclamó, así que declinar
/// el enlace no deja nada fuera de la cobertura.
#[test]
fn a_link_to_a_licence_of_the_workspace_is_not_a_hole() {
    let (_tmp, root) = tree_with_a_link_to_the_licence();

    let found = inventory(&root);
    let holes = found.coverage_holes();

    assert!(
        holes.is_empty(),
        "un enlace que apunta dentro del workspace no deja la cobertura incompleta: {holes:?}"
    );
    assert_eq!(
        coverage_of(&root).status,
        "complete",
        "la cobertura que publica el goal dice lo mismo que el inventario"
    );
}

/// El walk sigue declinando el enlace: lo que cambia es si la declinación
/// cuesta la cobertura.
///
/// Si el enlace desapareciera de la lista de declinados, el walk estaría
/// siguiendo enlaces —o guardando la decisión en otro sitio— y la ley se
/// habría movido sin que nadie lo pidiera.
#[test]
fn the_walk_still_declines_the_link() {
    let (_tmp, root) = tree_with_a_link_to_the_licence();

    let found = inventory(&root);

    let declined: Vec<&str> = found
        .declined
        .iter()
        .map(|entry| entry.path.as_str())
        .collect();
    assert_eq!(
        declined.len(),
        1,
        "el walk sigue declinando el enlace: {declined:?}"
    );
    assert!(
        declined[0].ends_with("pkg/LICENSE-MIT"),
        "el declinado es el enlace: {declined:?}"
    );
}

/// Un enlace a un fuente del workspace tampoco es un agujero.
///
/// El fuente está cubierto: el walk lo lee bajo su nombre real. El enlace es
/// otro nombre para lo mismo.
#[test]
fn a_link_to_a_source_of_the_workspace_is_not_a_hole() {
    let dir = tempfile::tempdir().expect("tempdir");
    let root = dir.path().to_path_buf();
    write(&root.join("src/lib.rs"), "pub fn kept() {}\n");
    std::fs::create_dir_all(root.join("alias")).expect("directory");
    std::os::unix::fs::symlink("../src/lib.rs", root.join("alias/lib.rs")).expect("symlink");

    let found = inventory(&root);

    assert!(
        found.coverage_holes().is_empty(),
        "el fuente está leído bajo su nombre real: {:?}",
        found.coverage_holes()
    );
    assert!(
        found
            .files
            .iter()
            .any(|file| file.path.ends_with("src/lib.rs")),
        "el walk lee el fuente por su nombre real"
    );
    assert!(
        !found
            .files
            .iter()
            .any(|file| file.path.ends_with("alias/lib.rs")),
        "el walk no lee a través del enlace"
    );
}

/// Un enlace que apunta fuera del workspace sigue siendo un agujero.
///
/// El contenido de fuera no lo describe nadie: la cobertura tiene que decirlo
/// y nombrar el fichero.
#[test]
fn a_link_outside_the_workspace_is_still_a_hole() {
    let dir = tempfile::tempdir().expect("tempdir");
    let outside = tempfile::tempdir().expect("tempdir");
    let root = dir.path().to_path_buf();
    write(&root.join("main.rs"), "fn main() {}\n");
    let target = outside.path().join("elsewhere.rs");
    write(&target, "pub fn elsewhere() {}\n");
    std::os::unix::fs::symlink(&target, root.join("linked.rs")).expect("symlink");

    let found = inventory(&root);
    let holes = found.coverage_holes();

    assert_eq!(
        holes.len(),
        1,
        "un enlace que sale del workspace es un agujero: {holes:?}"
    );
    assert!(
        holes[0].path.ends_with("linked.rs"),
        "el agujero nombra el fichero: {holes:?}"
    );
    assert_eq!(
        coverage_of(&root).status,
        "partial",
        "la cobertura que publica el goal dice lo mismo que el inventario"
    );
}
