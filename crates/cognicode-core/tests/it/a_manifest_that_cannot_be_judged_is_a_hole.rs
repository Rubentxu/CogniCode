//! Un manifiesto que no se pudo juzgar es un agujero de la cobertura.
//!
//! ## El defecto
//!
//! Un manifiesto que no se puede leer —o cuyo texto no es un manifiesto—
//! responde exactamente lo que responde un manifiesto que no declara nada:
//! ninguna crate de fuera. El juicio de la regla de la ruta de crate, que es
//! lo que consulta el manifiesto, no se llega a hacer para ese paquete, la
//! arista falsa se queda, y nada de lo que se publica dice que el juicio se
//! saltó: el resto del análisis se presenta como completo.
//!
//! ## La regla
//!
//! Un resultado que no es completo nunca se presenta como completo. La
//! cobertura ya cuenta sus agujeros —un fichero que no se pudo leer es un
//! agujero, una referencia que no está declarada es un agujero—, así que un
//! manifiesto que no se pudo juzgar es un agujero más, y nombra el paquete al
//! que pertenece.
//!
//! ## Lo que no cambia
//!
//! El manifiesto que se puede leer sigue decidiendo lo que decide, y el
//! paquete que no declara ninguna crate de fuera sigue sin declarar ninguna.

use cognicode_core::infrastructure::graph::per_file_graph::{
    BuildStatus, PerFileGraphCache, SkippedFile,
};
use cognicode_core::interface::mcp::schemas::SkippedFileInfo;
use std::path::{Path, PathBuf};

/// Una llamada escrita a través de una ruta de crate.
///
/// Es la llamada la que hace falta el manifiesto: es el juicio de la regla
/// de la ruta de crate el que lo consulta, y sin manifiesto ese juicio no se
/// llega a hacer.
const CALL_THROUGH_A_CRATE_PATH: &str = "\
pub fn from_the_crate() -> String {
    serde_json::to_value(1).to_string()
}
";

/// Un paquete con un manifiesto cuyo texto no es un manifiesto, y el fuente
/// que llama a través de una ruta de crate.
fn package_with_an_unparseable_manifest() -> (tempfile::TempDir, PathBuf, Vec<PathBuf>) {
    let tmp = tempfile::tempdir().expect("a temporary project");
    let root = tmp.path().join("project");
    let package = root.join("pkg");
    let src = package.join("src");
    std::fs::create_dir_all(&src).expect("the source directory is created");
    let file = src.join("lib.rs");
    std::fs::write(&file, CALL_THROUGH_A_CRATE_PATH).expect("the source is written");
    std::fs::write(package.join("Cargo.toml"), "[package\nname = \"fixture\"\n")
        .expect("the manifest is written");
    (tmp, root, vec![file])
}

/// Un paquete con un manifiesto que se puede leer, y el mismo fuente.
fn package_with_a_readable_manifest() -> (tempfile::TempDir, PathBuf, Vec<PathBuf>) {
    let tmp = tempfile::tempdir().expect("a temporary project");
    let root = tmp.path().join("project");
    let package = root.join("pkg");
    let src = package.join("src");
    std::fs::create_dir_all(&src).expect("the source directory is created");
    let file = src.join("lib.rs");
    std::fs::write(&file, CALL_THROUGH_A_CRATE_PATH).expect("the source is written");
    std::fs::write(
        package.join("Cargo.toml"),
        "[package]\nname = \"fixture\"\n\n[dependencies]\nserde_json = \"1\"\n",
    )
    .expect("the manifest is written");
    (tmp, root, vec![file])
}

/// Los agujeros que la cobertura del merge publica.
fn holes_of(dir: &Path, files: &[PathBuf]) -> Vec<SkippedFile> {
    let cache = PerFileGraphCache::with_project_dir(dir.to_string_lossy().into_owned());
    let paths: Vec<&Path> = files.iter().map(PathBuf::as_path).collect();
    match cache.merge_with_report(&paths).status {
        BuildStatus::Complete => Vec::new(),
        BuildStatus::Partial { skipped, .. } => skipped,
    }
}

/// Un manifiesto cuyo texto no es un manifiesto es un agujero de la cobertura.
///
/// El paquete al que pertenece el manifiesto es el que se nombra: sin el
/// manifiesto no se sabe el nombre del crate, pero se sabe el directorio que
/// lo guarda, y eso es lo que la cobertura dice.
#[test]
fn a_manifest_that_cannot_be_parsed_is_a_hole_of_the_coverage() {
    let (_tmp, root, files) = package_with_an_unparseable_manifest();

    let holes = holes_of(&root, &files);

    assert_eq!(
        holes.len(),
        1,
        "el manifiesto no se pudo interpretar y la cobertura no lo cuenta: {holes:?}"
    );
    let published = SkippedFileInfo::from_skipped(holes[0].clone());
    assert!(
        published.path.ends_with("pkg/Cargo.toml"),
        "el agujero nombra el manifiesto: {published:?}"
    );
    assert!(
        published.detail.contains("pkg"),
        "el agujero nombra el paquete: {published:?}"
    );
}

/// El manifiesto que se puede leer no es un agujero.
///
/// El control: con el manifiesto en su sitio, el paquete dice lo que dice y
/// la cobertura no tiene nada que contar.
#[test]
fn a_manifest_that_can_be_read_is_not_a_hole() {
    let (_tmp, root, files) = package_with_a_readable_manifest();

    let holes = holes_of(&root, &files);

    assert!(
        holes.is_empty(),
        "el manifiesto se pudo leer y la cobertura cuenta un agujero: {holes:?}"
    );
}

/// El agujero tiene un motivo propio.
///
/// `Read` y `Parse` nombran un fichero fuente que no se pudo leer o
/// interpretar; éste nombra un manifiesto, y el consumidor tiene que poder
/// distinguirlos sin leer la prosa.
#[test]
fn the_manifest_hole_has_a_reason_of_its_own() {
    let (_tmp, root, files) = package_with_an_unparseable_manifest();

    let holes = holes_of(&root, &files);
    let published = SkippedFileInfo::from_skipped(holes[0].clone());
    let reason = serde_json::to_value(&published.reason).expect("the reason is published");

    assert_eq!(
        reason,
        serde_json::json!("manifest"),
        "el motivo del agujero es el del manifiesto: {published:?}"
    );
}

/// Un manifiesto que no se puede leer es un agujero de la cobertura.
///
/// El mismo agujero que el manifiesto ilegible, con la diferencia de que
/// aquí el fichero existe y no se puede abrir.
#[cfg(unix)]
mod unreadable {
    use super::{CALL_THROUGH_A_CRATE_PATH, holes_of};
    use cognicode_core::interface::mcp::schemas::SkippedFileInfo;
    use std::os::unix::fs::PermissionsExt;
    use std::path::PathBuf;

    /// Da los permisos al manifiesto cuando el test acaba, como haga falta.
    struct Unlock(PathBuf, u32);

    impl Drop for Unlock {
        fn drop(&mut self) {
            let _ = std::fs::set_permissions(&self.0, std::fs::Permissions::from_mode(self.1));
        }
    }

    #[test]
    fn a_manifest_that_cannot_be_read_is_a_hole_of_the_coverage() {
        let tmp = tempfile::tempdir().expect("a temporary project");
        let root = tmp.path().join("project");
        let package = root.join("pkg");
        let src = package.join("src");
        std::fs::create_dir_all(&src).expect("the source directory is created");
        let file = src.join("lib.rs");
        std::fs::write(&file, CALL_THROUGH_A_CRATE_PATH).expect("the source is written");
        let manifest = package.join("Cargo.toml");
        std::fs::write(
            &manifest,
            "[package]\nname = \"fixture\"\n\n[dependencies]\nserde_json = \"1\"\n",
        )
        .expect("the manifest is written");
        std::fs::set_permissions(&manifest, std::fs::Permissions::from_mode(0o000))
            .expect("the manifest is locked");
        assert!(
            std::fs::read_to_string(&manifest).is_err(),
            "la premisa: el manifiesto no se puede leer"
        );
        let _unlock = Unlock(manifest.clone(), 0o644);

        let holes = holes_of(&root, &[file]);

        assert_eq!(
            holes.len(),
            1,
            "el manifiesto no se pudo leer y la cobertura no lo cuenta: {holes:?}"
        );
        let published = SkippedFileInfo::from_skipped(holes[0].clone());
        assert!(
            published.path.ends_with("pkg/Cargo.toml"),
            "el agujero nombra el manifiesto: {published:?}"
        );
        assert!(
            published.detail.contains("pkg"),
            "el agujero nombra el paquete: {published:?}"
        );
    }
}
