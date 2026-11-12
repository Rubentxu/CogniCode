//! TASK-222 — solo el veredicto de cobertura publica Complete (H8).
//!
//! AGENTS.md: un Partial/Unknown nunca se presenta como Complete, y la
//! palabra y la decisión tienen un dueño cada una. Tres sitios decidían
//! «Complete» por su cuenta: el early-return de `merge_graphs` sin
//! ficheros, el default de `state_of` en el sobre HATEOAS, y los
//! handlers que traducen `BuildStatus` sin pasar por `coverage_verdict`
//! ni por `no_material_outcome`.
//!
//! Este contrato cierra el de los enums publicados: fuera del dueño no
//! se nombra `MergeCoverage::Complete`. El dueño es `schemas.rs`, que
//! lo deriva del veredicto (`coverage_verdict`); el resto pide la
//! palabra, no la fabrica.

use std::fs;
use std::path::{Path, PathBuf};

/// Ficheros .rs de producción bajo `src/`, parando en el primer
/// `#[cfg(test)]` de cada uno.
fn production_sources(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            production_sources(&path, out);
        } else if path.extension().and_then(|e| e.to_str()) == Some("rs") {
            out.push(path);
        }
    }
}

/// AC #4 — fuera del dueño no se construye `Complete`.
#[test]
fn only_the_verdict_owner_names_merge_coverage_complete() {
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut files = Vec::new();
    production_sources(&src, &mut files);

    let mut offenders = Vec::new();
    for file in files {
        // El dueño vive en schemas.rs: define el enum y deriva la
        // palabra del veredicto.
        if file.ends_with("schemas.rs") {
            continue;
        }
        let Ok(text) = fs::read_to_string(&file) else {
            continue;
        };
        let production = text
            .lines()
            .take_while(|line| !line.trim_start().starts_with("#[cfg(test)]"))
            .collect::<Vec<_>>()
            .join("\n");
        if production.contains("MergeCoverage::Complete") {
            offenders.push(file.strip_prefix(&src).unwrap().display().to_string());
        }
    }
    assert!(
        offenders.is_empty(),
        "`MergeCoverage::Complete` se construye fuera del dueño del \
         veredicto (schemas.rs): {offenders:#?} — pedid la palabra al \
         dueño (MergeCoverage::from_verdict / no_material), no la \
         fabricéis"
    );
}
