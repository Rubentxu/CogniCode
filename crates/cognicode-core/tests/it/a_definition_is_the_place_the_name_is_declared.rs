//! La definición de un símbolo es el sitio donde su nombre se declara.
//!
//! ## El defecto
//!
//! `find-usages` marcaba `is_definition` con una heurística de texto: la
//! línea de la primera aparición que contuviera `fn `, `def `, `class `,
//! `struct `, `function `, `const ` o `let ` era la definición, y las demás
//! no. Una llamada escrita en la línea de otra función cumplía la condición
//! antes que la declaración: el consumidor que filtra «dónde se define»
//! recibía el sitio equivocado, y el que filtra «dónde se usa» recibía la
//! declaración en el sitio de la llamada.
//!
//! ## Lo que se mide
//!
//! El lugar que se marca como definición: eso es lo que el consumidor
//! filtra.
//!
//! ## Lo que tiene que seguir siendo verdad
//!
//! - una definición escrita antes de sus usos sigue siendo la
//!   definición;
//! - un símbolo que nada declara no marca ninguna definición: no hay
//!   dónde mirar.

#![cfg(unix)]

use cognicode_core::application::services::analysis_service::{AnalysisService, UsageSearchParams};
use std::path::{Path, PathBuf};

/// El corpus del defecto: la llamada y la declaración en líneas distintas,
/// con la llamada escrita en la línea de otra función.
const CALL_BEFORE_DECLARATION: &str = "\
pub fn alpha() -> u32 { beta() }

pub fn beta() -> u32 { 1 }
";

/// El corpus del signo contrario: la declaración primero.
const DECLARATION_BEFORE_CALL: &str = "\
pub fn beta() -> u32 { 1 }

pub fn alpha() -> u32 { beta() }
";

/// Un árbol con un único fichero escrito.
fn tree_with(contents: &str) -> (tempfile::TempDir, PathBuf) {
    let dir = tempfile::tempdir().expect("tempdir");
    let root = dir.path().to_path_buf();
    let src = root.join("src");
    std::fs::create_dir_all(&src).expect("the directory is created");
    std::fs::write(src.join("lib.rs"), contents).expect("the file is written");
    (dir, root)
}

/// Los sitios que la búsqueda marca como definición.
fn definitions_of(root: &Path, symbol: &str) -> Vec<(u32, u32)> {
    let service = AnalysisService::new();
    let search = service
        .find_symbol_usages_with_coverage(UsageSearchParams {
            project_dir: root.to_path_buf(),
            symbol_name: symbol.to_string(),
            include_declaration: true,
            context_lines: None,
            first_only_definition: true,
        })
        .expect("the search answers");

    search
        .usages
        .into_iter()
        .filter(|usage| usage.is_definition)
        .map(|usage| (usage.line, usage.column))
        .collect()
}

/// La definición es la línea que declara el nombre, no la primera línea que
/// habla de `fn`.
#[test]
fn the_definition_is_the_line_that_declares_the_name() {
    let (_tmp, root) = tree_with(CALL_BEFORE_DECLARATION);

    let definitions = definitions_of(&root, "beta");

    assert_eq!(
        definitions,
        vec![(3, 8)],
        "la definición es la línea 3, la que escribe `pub fn beta`: {definitions:?}"
    );
}

/// El signo contrario: la definición antes de sus usos sigue siendo la
/// definición.
#[test]
fn a_definition_written_before_its_uses_is_still_the_definition() {
    let (_tmp, root) = tree_with(DECLARATION_BEFORE_CALL);

    let definitions = definitions_of(&root, "beta");

    assert_eq!(
        definitions,
        vec![(1, 8)],
        "la definición es la línea 1, la que escribe `pub fn beta`: {definitions:?}"
    );
}

/// Un símbolo que nada declara no marca ninguna definición: no hay dónde
/// mirar.
#[test]
fn a_symbol_that_nothing_declares_marks_no_definition() {
    let (_tmp, root) = tree_with(CALL_BEFORE_DECLARATION);

    let definitions = definitions_of(&root, "gamma");

    assert!(
        definitions.is_empty(),
        "un nombre que nada declara no tiene definición: {definitions:?}"
    );
}

/// Una llamada escrita en la línea de la declaración no es la declaración.
#[test]
fn a_call_written_on_the_line_of_a_declaration_is_not_the_declaration() {
    let (_tmp, root) = tree_with("pub fn beta() -> u32 { beta() }\n");

    let definitions = definitions_of(&root, "beta");

    assert_eq!(
        definitions,
        vec![(1, 8)],
        "la línea declara el nombre y lo usa: la definición es la primera \
         aparición: {definitions:?}"
    );
}
