//! El inventario dice cuál de las dos es cada cosa.
//!
//! El hermano de `walk_errors_are_errors.rs`. Ese mira lo que el
//! consumidor ve; este mira la declaración que hace el inventario,
//! que es donde la decisión se toma. Los dos hacen falta: un
//! inventario que clasifica bien pero no publica el error no cuenta
//! nada, y un reporte que cuenta el error pero no sabe de dónde sale
//! se lo inventa.
//!
//! ## Lo que hay hoy
//!
//! El inventario entrega `files` y `declined`. En `files` caben tres
//! cosas distintas y el consumidor tiene que adivinarlas:
//!
//!   - la fuente que el gráfico analiza (`.rs`, `.kt`, ...);
//!   - el hueco real: una fuente en un lenguaje que conocemos y todavía
//!     no soportamos;
//!   - la exclusión legítima: un asset que no analizamos A PROPOSITO
//!     (README, lockfile, imagen).
//!
//! Los dos últimos llegan igual: `language: None`. La clasificación
//! que los separa existe (`WalkClassification`: `Analyze |
//! KnownUnsupportedGap | LegitExclusion`) y tiene un solo dueño
//! (`classify_walk_extension`), pero el inventario no la aplica: la
//! aplican las rutas de construcción, cada una con su copia. Un
//! `README` y un `.kt` son la misma cosa en el inventario, y no lo son.
//!
//! MEDIDO. Kotlin tiene gramática, así que el `.kt` de
//! este corpus dejó de ser hueco y pasó a ser fuente. El hueco sigue
//! existiendo como categoría —con su razón y su sitio en el
//! inventario— pero hoy no tiene sujeto: los 31 lenguajes del
//! catálogo tienen gramática. Este fichero mide la diferencia que
//! SIGUE viva (fuente contra exclusión) y deja constancia de la que
//! se perdió (fuente contra hueco), que ahora se mide en el
//! clasificador, en `h11_unified_classification_contract.rs`.
//!
//! ## Lo que se exige
//!
//!   - `errors`: lo que el recorrido no pudo recorrer. No es un
//!     fichero que dejamos fuera: es un fallo, y se cuenta.
//!   - `known_gaps()`: el hueco real, con su razón.
//!   - `legit_exclusions()`: la decisión, con el fichero que se dejó
//!     fuera y nada más.
//!
//! Y que los tres verederos sean disjuntos: un hueco no es una
//! exclusión, y un error no es ninguno de los dos.

use cognicode_core::infrastructure::graph::per_file_graph::SkipReason;
use cognicode_core::infrastructure::graph::source_files::inventory;

/// Repo normal con las clases que hoy existen: la fuente que se
/// analiza (Rust y Kotlin) y la exclusión legítima (el README).
fn cargo_repo() -> tempfile::TempDir {
    let dir = tempfile::tempdir().expect("tempdir");
    let root = dir.path();
    std::fs::create_dir_all(root.join("src")).expect("src");
    std::fs::write(root.join("src").join("main.rs"), "fn main() { run(); }\n").expect("main.rs");
    std::fs::write(root.join("README.md"), "# Fixture\n\nUn proyecto normal.\n")
        .expect("README.md");
    std::fs::write(
        root.join("src").join("Other.kt"),
        "class Other { fun x() = 1 }\n",
    )
    .expect("Other.kt");
    dir
}

/// Nombre de fichero, sin el camino absoluto del tempdir.
fn name_of(path: &std::path::Path) -> String {
    path.file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.to_string_lossy().into_owned())
}

#[test]
fn una_fuente_analizada_y_una_exclusion_legitima_no_se_confunden() {
    let dir = cargo_repo();
    let inv = inventory(dir.path());

    // El hueco: hoy no hay ninguno. Kotlin tiene gramática, así que un
    // `.kt` es fuente. El hueco SIGUE existiendo como categoría —con
    // su razón y su sitio en el inventario—, pero sin sujeto: los 31
    // lenguajes del catálogo tienen gramática. Lo que se mide aquí es
    // que el hueco vacío no se llene con lo que no es un hueco.
    let gaps = inv.known_gaps();
    assert!(
        gaps.is_empty(),
        "ninguna fuente de este repo es un hueco: las dos que hay tienen \
         gramática y el README no es una fuente. Un hueco aquí sería un \
         hueco inventado: {gaps:?}"
    );

    // La exclusión: un asset que no analizamos a propósito.
    let exclusions = inv.legit_exclusions();
    let names: Vec<String> = exclusions.iter().map(|f| name_of(&f.path)).collect();
    assert!(
        names.contains(&"README.md".to_string()),
        "un README está fuera del alcance por política y el inventario tiene que \
         poder decirlo: {names:?}"
    );
    assert!(
        !names.contains(&"Other.kt".to_string()),
        "el `.kt` NO es una exclusión legítima: es una fuente que el producto \
         lee, y llamarlo exclusión escondería cobertura: {names:?}"
    );

    // Y el gráfico se lleva las fuentes, y solo las fuentes.
    let mut sources: Vec<String> = inv.source_files().map(|f| name_of(&f.path)).collect();
    sources.sort();
    assert_eq!(
        sources,
        vec!["Other.kt".to_string(), "main.rs".to_string()],
        "al grafo van las fuentes y solo las fuentes: un `.kt` que se cuela \
         como exclusión pierde código, y un README que se cuela como fuente \
         inventa análisis"
    );
}

#[test]
fn los_tres_verederos_del_inventario_no_se_pisan() {
    // La aserción discriminante, enunciada sola: los tres conjuntos son
    // disjuntos. Si alguien mete el hueco en las exclusiones, o la
    // exclusión en los errores, este test cae aunque los otros sigan
    // verdes.
    let dir = cargo_repo();
    let inv = inventory(dir.path());

    let gap_names: Vec<String> = inv
        .known_gaps()
        .iter()
        .map(|g| name_of(std::path::Path::new(&g.path)))
        .collect();
    let exclusion_names: Vec<String> = inv
        .legit_exclusions()
        .iter()
        .map(|f| name_of(&f.path))
        .collect();
    let error_names: Vec<String> = inv
        .errors
        .iter()
        .map(|e| name_of(std::path::Path::new(&e.path)))
        .collect();

    for name in &gap_names {
        assert!(
            !exclusion_names.contains(name),
            "`{name}` no puede ser hueco y exclusión a la vez: {gap_names:?} / \
             {exclusion_names:?}"
        );
        assert!(
            !error_names.contains(name),
            "`{name}` no puede ser hueco y error a la vez: {gap_names:?} / {error_names:?}"
        );
    }
    for name in &exclusion_names {
        assert!(
            !error_names.contains(name),
            "`{name}` no puede ser exclusión y error a la vez: {exclusion_names:?} \
             / {error_names:?}"
        );
    }

    // Un repo que se recorre entero no tiene errores de recorrido.
    assert!(
        error_names.is_empty(),
        "un repo legible no tiene errores de recorrido: {error_names:?}"
    );
}

#[cfg(unix)]
#[test]
fn un_error_del_recorrido_es_un_error_y_no_una_exclusion() {
    use std::os::unix::fs::PermissionsExt as _;

    let dir = cargo_repo();
    let locked = dir.path().join("locked");
    std::fs::create_dir_all(&locked).expect("locked dir");
    std::fs::write(locked.join("inside.rs"), "pub fn inside() {}\n").expect("inside.rs");
    std::fs::set_permissions(&locked, std::fs::Permissions::from_mode(0o000)).expect("chmod 0o000");

    // Sonda: si el DAC no está cerrado (root lo salta) el recorrido no
    // falla y no hay nada que exigir.
    if std::fs::read_dir(&locked).is_ok() {
        std::fs::set_permissions(&locked, std::fs::Permissions::from_mode(0o755))
            .expect("restore perms");
        eprintln!(
            "SKIP C20: el DAC no se puede cerrar para este proceso (root lo salta); \
             no hay error de recorrido que exigir en esta máquina"
        );
        return;
    }

    let inv = inventory(dir.path());

    assert!(
        !inv.errors.is_empty(),
        "el recorrido no pudo leer `locked/` y el inventario tiene que decirlo: \
         un error del recorrido que no se cuenta es un hueco invisible"
    );
    assert!(
        inv.errors
            .iter()
            .any(|e| e.path.ends_with("locked") && matches!(e.reason, SkipReason::Walk(_))),
        "el error se nombra con su camino y con su razón de recorrido: {:?}",
        inv.errors
    );

    // Lo que hay detrás no aparece en ninguna parte como si se hubiera
    // visto, y lo que sí se pudo ver sigue ahí.
    let sources: Vec<String> = inv.source_files().map(|f| name_of(&f.path)).collect();
    assert!(
        !sources.contains(&"inside.rs".to_string()),
        "lo que hay detrás de un directorio ilegible no se publica: {sources:?}"
    );
    assert!(
        sources.contains(&"main.rs".to_string()),
        "el recorrido sigue después del error: un directorio ilegible no es un \
         árbol entero. Vistos: {sources:?}"
    );

    let _ = std::fs::set_permissions(&locked, std::fs::Permissions::from_mode(0o755));
}
