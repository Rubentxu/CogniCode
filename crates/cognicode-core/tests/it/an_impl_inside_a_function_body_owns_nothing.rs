//! El mapa de dueños de `impl` dice lo que el código escribe.
//!
//! ## El defecto
//!
//! El mapa de dueños se leía recorriendo TODO el árbol del fichero, así que
//! un `impl` escrito dentro del cuerpo de una función concedía la propiedad
//! a todo el fichero: `fn f() { impl Local { fn g() {} } }` dejaba el mapa
//! con `"g" → {"Local"}`, y una llamada `Local::g()` escrita en otro punto
//! del fichero se aceptaba como propiedad del fichero, con una arista hacia
//! un método de ámbito local.
//!
//! ## Qué se mide
//!
//! 1. Un `impl` dentro del cuerpo de una función NO es dueño de nada: la
//!    llamada escrita a través de ese tipo no publica arista.
//! 2. El control: el MISMO `impl` escrito al nivel del módulo sí es dueño, y
//!    la llamada a través de él publica la arista. Sin este control, un
//!    recorrido que no encontrara ningún `impl` también pasaría la primera
//!    medición.
//!
//! ## Por qué el tipo está importado
//!
//! El tipo del `impl` no lo declara el fichero (vive en otro fichero), y el
//! fichero lo importa. Así la llamada no se resuelve por la declaración del
//! fichero sino por el mapa de dueños, que es lo que se está midiendo: con
//! el tipo declarado aquí, la arista saldría igual y la medición no
//! distinguiría una regla de la otra.

use cognicode_core::domain::aggregates::call_graph::CallGraph;
use cognicode_core::infrastructure::graph::per_file_graph::PerFileGraphCache;
use std::path::{Path, PathBuf};

/// El crate de la prueba, con el módulo donde vive el tipo.
const LIB_RS: &str = "\
pub mod local;
pub mod other;
";

/// El tipo, en su propio fichero.
const LOCAL_RS: &str = "\
pub struct Local;
";

/// Un fichero con un `impl` dentro del cuerpo de una función.
const OTHER_INSIDE_A_FUNCTION_RS: &str = "\
use crate::local::Local;

pub fn call_it() -> u64 {
    Local::inner()
}

fn helper() {
    impl Local {
        pub fn inner() -> u64 {
            0
        }
    }
}
";

/// El mismo fichero, con el `impl` al nivel del módulo.
const OTHER_AT_MODULE_LEVEL_RS: &str = "\
use crate::local::Local;

pub fn call_it() -> u64 {
    Local::inner()
}

impl Local {
    pub fn inner() -> u64 {
        0
    }
}
";

/// Un proyecto temporal con los ficheros dados, y sus rutas.
fn project(files: &[(&str, &str)]) -> (tempfile::TempDir, Vec<PathBuf>) {
    let dir = tempfile::tempdir().expect("a temporary project");
    let src = dir.path().join("src");
    std::fs::create_dir_all(&src).expect("the source directory is created");
    let paths = files
        .iter()
        .map(|(name, content)| {
            let path = src.join(name);
            std::fs::write(&path, content).expect("the fixture is written");
            path
        })
        .collect();
    (dir, paths)
}

/// El grafo que la ruta sin índice construye para un fichero.
fn file_route(dir: &Path, file: &Path) -> CallGraph {
    let cache = PerFileGraphCache::with_project_dir(dir.to_string_lossy().into_owned());
    let (graph, _pending) = cache
        .get_or_build(file)
        .expect("the file is read and its graph is built");
    (*graph).clone()
}

/// Los nombres de los símbolos que llaman a la única declaración de `name`.
fn callers_of(graph: &CallGraph, name: &str) -> Vec<String> {
    let declared: Vec<_> = graph
        .symbol_ids()
        .filter(|(_, symbol)| symbol.name() == name)
        .map(|(id, _)| id.clone())
        .collect();
    assert_eq!(
        declared.len(),
        1,
        "the fixture declares `{name}` exactly once, and the graph holds {declared:?}"
    );
    let mut callers: Vec<String> = graph
        .callers(&declared[0])
        .iter()
        .map(|id| {
            graph
                .get_symbol(id)
                .map(|symbol| symbol.name().to_string())
                .unwrap_or_else(|| id.as_str().to_string())
        })
        .collect();
    callers.sort();
    callers
}

/// Un `impl` dentro del cuerpo de una función no concede propiedad a nada.
#[test]
fn an_impl_inside_a_function_body_owns_nothing() {
    let (dir, files) = project(&[
        ("lib.rs", LIB_RS),
        ("local.rs", LOCAL_RS),
        ("other.rs", OTHER_INSIDE_A_FUNCTION_RS),
    ]);
    let graph = file_route(dir.path(), &files[2]);

    assert_eq!(
        callers_of(&graph, "inner"),
        Vec::<String>::new(),
        "the `impl` is written inside a function body: it is at the level of no module, so it \
         owns nothing and the call through it is not the file's own"
    );
}

/// El control: el mismo `impl` al nivel del módulo sí es dueño.
#[test]
fn an_impl_at_the_level_of_a_module_owns_its_calls() {
    let (dir, files) = project(&[
        ("lib.rs", LIB_RS),
        ("local.rs", LOCAL_RS),
        ("other.rs", OTHER_AT_MODULE_LEVEL_RS),
    ]);
    let graph = file_route(dir.path(), &files[2]);

    assert_eq!(
        callers_of(&graph, "inner"),
        vec!["call_it".to_string()],
        "the `impl` is written at the level of the module: the file owns the calls written \
         through that type"
    );
}
