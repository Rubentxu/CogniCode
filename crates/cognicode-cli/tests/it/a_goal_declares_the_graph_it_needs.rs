//! Cada goal declara el grafo que necesita, y la declaración dice la verdad.
//!
//! ## El defecto
//!
//! Los diez goals del registro declaraban `requires_graph: false` en el
//! catálogo, y siete de ellos CONSTRUYEN el grafo del workspace: `overview`,
//! `audit`, `find`, `trace`, `impact`, `change` y `recipe`. La declaración
//! decide la ventana de despacho —una herramienta que declara
//! `requires_graph` recibe la ventana del grafo— así que un goal que
//! construye el grafo con la ventana por defecto de 30 s es un goal cuyo
//! plazo depende del tamaño del árbol: en este repositorio el grafo tarda
//! 28 s en construirse, y 30 s es el margen entero.
//!
//! ## Lo que mide este fichero
//!
//! Una llamada EN FRÍO por goal —un proceso por goal, con una caché propia—
//! y una sola pregunta: ¿dejó el goal un grafo construido? La respuesta
//! observable es el instantáneo que el producto guarda
//! (`<cache>/cognicode/<clave>`), no un cronómetro: el reloj no es una
//! medida, y un test que espera a que el grafo esté "listo" mide la máquina
//! en la que corre, no el producto.
//!
//! Y después, la comparación: lo que el goal HIZO y lo que el catálogo
//! DECLARA. Las dos cosas tienen que decir lo mismo, porque la ventana sale
//! de la declaración: un goal que construye el grafo y dice que no lo
//! necesita recibe un plazo que no cubre su trabajo.
//!
//! ## La medición no es una tabla
//!
//! Los argumentos de cada goal salen del esquema derivado (los mismos flags
//! que el cable publica); sólo el valor es del fixture. Y la lista de goals
//! sale del registro, no de aquí: un goal nuevo que se registrara sin
//! medirse no entraría en la comparación, así que la comparación recorre el
//! registro entero.

use cognicode_core::application::graph_cache::snapshot_path;
use cognicode_core::interface::goals::{self, GoalSpec};
use cognicode_core::interface::recipes::RecipeBook;
use serde_json::Value;
use std::path::{Path, PathBuf};
use std::process::Command;

/// El catálogo que el operador mantiene a mano.
const CATALOG: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../cognicode-core/product/tools.json"
);

/// La fuente del fixture: un símbolo y la llamada que lo alcanza.
const SOURCE: &str = "pub fn alpha() -> u32 { beta() + 1 }\npub fn beta() -> u32 { 2 }\n";

/// Las ediciones candidatas de `preflight`: `lib.rs` con otro cuerpo.
const EDITS: &str =
    r#"{"lib.rs":"pub fn alpha() -> u32 { beta() + 2 }\npub fn beta() -> u32 { 2 }\n"}"#;

// ---------------------------------------------------------------------------
// Fixture: un workspace y una caché que no son de nadie más.
// ---------------------------------------------------------------------------

/// Un workspace con una fuente y una caché propia.
///
/// La caché es la que hace la llamada EN FRÍO: sin ella, el goal encontraría
/// el instantáneo de otra corrida y la medición no mediría al goal.
fn workspace() -> (tempfile::TempDir, PathBuf, PathBuf) {
    let root = tempfile::tempdir().expect("tempdir");
    let workspace = root.path().join("ws");
    std::fs::create_dir_all(&workspace).expect("the workspace is created");
    std::fs::write(workspace.join("lib.rs"), SOURCE).expect("the source is written");
    let cache = root.path().join("cache");
    (root, workspace, cache)
}

/// Un historial de dos commits en el workspace.
///
/// `compare`, `verify` y `preflight` leen revisiones de Git: sobre un
/// directorio sin historial no hay nada que comparar, y su fallo no diría
/// nada de si construye el grafo.
fn commit_two_revisions(workspace: &Path) {
    let git = |args: &[&str]| {
        let no_configuration = workspace.join("no-git-configuration");
        let output = Command::new("git")
            .arg("-C")
            .arg(workspace)
            .args(args)
            .env("GIT_CONFIG_GLOBAL", &no_configuration)
            .env("GIT_CONFIG_SYSTEM", &no_configuration)
            .output()
            .expect("git is installed");
        assert!(
            output.status.success(),
            "git {args:?} failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    };
    git(&["init", "--quiet"]);
    git(&["config", "user.name", "CogniCode tests"]);
    git(&["config", "user.email", "tests@example.invalid"]);
    git(&["add", "lib.rs"]);
    git(&["commit", "--quiet", "-m", "base"]);
    std::fs::write(workspace.join("lib.rs"), format!("{SOURCE}// changed\n"))
        .expect("the candidate source is written");
    git(&["add", "lib.rs"]);
    git(&["commit", "--quiet", "-m", "candidate"]);
}

// ---------------------------------------------------------------------------
// La llamada en frío.
// ---------------------------------------------------------------------------

/// Los argumentos que el goal necesita, con los valores del fixture.
///
/// Los flags salen del esquema derivado (los mismos que el cable publica);
/// el valor es del fixture. Un goal con un argumento obligatorio que el
/// fixture no sabe responder se dice aquí, en vez de que la llamada falle
/// con un error que parece del producto.
fn arguments_of(goal: &GoalSpec) -> Vec<String> {
    let schema = goals::input_schema(goal);
    let mut arguments = Vec::new();
    if goal.name == "inspect" {
        // `inspect` takes a file or a revision, so neither is required by the
        // schema alone; the fixture asks about its file.
        arguments.push("--file".to_string());
        arguments.push("lib.rs".to_string());
    }
    for name in goals::required_names(&schema) {
        let value = match name {
            "symbol" => "alpha",
            "file" => "lib.rs",
            "new_name" => "gamma",
            "base_ref" => "HEAD~1",
            "head_ref" => "HEAD",
            "revision" => "HEAD",
            // `preflight` asks about a candidate: a map from path to the new
            // content of that file.
            "edits" => EDITS,
            other => panic!(
                "the fixture does not know what to answer to the argument `{other}` of the goal \
                 `{}`",
                goal.name
            ),
        };
        arguments.push(format!("--{}", name.replace('_', "-")));
        arguments.push(value.to_string());
    }
    arguments
}

/// Las llamadas que miden lo que hace un goal.
///
/// Un goal se mide con la llamada que el consumidor le hace. La receta se
/// mide con TODAS: su trabajo son los pasos de la receta, y la declaración
/// tiene que cubrir la peor —una receta puede no construir el grafo y otra
/// sí, y la ventana es una sola para todas—. Las recetas son las que el
/// producto publica, más las que traiga el workspace: el libro las nombra,
/// así que una receta nueva entra en la medida sin que nadie la escriba
/// aquí.
fn calls_of(goal: &GoalSpec, workspace: &Path) -> Vec<Vec<String>> {
    if goal.name == "recipe" {
        let book = RecipeBook::load(workspace);
        assert!(
            !book.names().is_empty(),
            "the book of recipes is empty: there is nothing to measure"
        );
        return book
            .names()
            .iter()
            .map(|name| {
                let recipe = book.get(name).unwrap_or_else(|| {
                    panic!("the book names a recipe it does not publish: {name}")
                });
                let mut arguments = vec!["--name".to_string(), recipe.name.clone()];
                if !recipe.parameters.is_empty() {
                    // Los parámetros que la receta declara, con el valor del
                    // fixture: una receta con un parámetro sin valor no
                    // corre, y su fallo no dice nada del goal.
                    let values: Vec<String> = recipe
                        .parameters
                        .iter()
                        .map(|parameter| format!("\"{parameter}\":\"{}\"", value_of(parameter)))
                        .collect();
                    arguments.push("--params".to_string());
                    arguments.push(format!("{{{}}}", values.join(",")));
                }
                arguments
            })
            .collect();
    }
    vec![arguments_of(goal)]
}

/// El valor que el fixture da a un parámetro.
fn value_of(parameter: &str) -> &'static str {
    match parameter {
        "symbol" => "alpha",
        other => panic!("the fixture does not know what to answer to the parameter `{other}`"),
    }
}

/// Corre el goal como lo corre quien lo usa: el binario, en el workspace.
///
/// El goal se llama por su nombre, que es como lo llama el cable; su
/// respuesta se pide en JSON, que es como la pide un cliente.
fn run_goal(goal: &GoalSpec, workspace: &Path, cache: &Path, arguments: &[String]) {
    let binary = crate::common::binary_path("cognicode");
    let output = Command::new(&binary)
        .current_dir(workspace)
        .arg(goal.name)
        .arg("--format")
        .arg("json")
        .args(arguments)
        .env("XDG_CACHE_HOME", cache)
        .output()
        .unwrap_or_else(|error| panic!("failed to spawn {}: {error}", binary.display()));
    assert!(
        output.status.success(),
        "the goal `{}` exited with {}: {}",
        goal.name,
        output.status,
        String::from_utf8_lossy(&output.stderr)
    );
}

/// Si el goal dejó un grafo construido.
///
/// El instantáneo es lo que el producto guarda de un grafo que construyó: si
/// el goal no construye el grafo, no hay instantáneo, y no hay nada que
/// servir en una llamada siguiente.
fn built_the_graph(cache: &Path, workspace: &Path) -> bool {
    snapshot_path(cache, workspace).exists()
}

// ---------------------------------------------------------------------------
// Lo que el catálogo declara.
// ---------------------------------------------------------------------------

/// Si el catálogo dice que este goal necesita el grafo.
fn catalogue_requires_graph(name: &str) -> bool {
    let text = std::fs::read_to_string(CATALOG)
        .unwrap_or_else(|error| panic!("cannot read {CATALOG}: {error}"));
    let document: Value = serde_json::from_str(&text)
        .unwrap_or_else(|error| panic!("the catalogue is JSON: {error}"));
    let tools = document
        .get("tools")
        .and_then(Value::as_array)
        .expect("the catalogue publishes tools");
    let entry = tools
        .iter()
        .find(|tool| tool.get("name").and_then(Value::as_str) == Some(name))
        .unwrap_or_else(|| panic!("the catalogue does not publish `{name}`"));
    entry
        .get("runtime_metadata")
        .and_then(|metadata| metadata.get("requires_graph"))
        .and_then(Value::as_bool)
        .unwrap_or_else(|| panic!("the catalogue does not say what `{name}` needs"))
}

// ---------------------------------------------------------------------------
// La comparación.
// ---------------------------------------------------------------------------

/// Cada goal dice lo mismo de sí mismo que lo que hace.
///
/// La declaración decide la ventana de despacho, así que la diferencia entre
/// las dos cosas es un plazo que no cubre el trabajo: un goal que construye
/// el grafo del workspace y dice que no lo necesita recibe 30 s para un
/// trabajo que, en un árbol grande, tarda más.
#[test]
fn every_goal_declares_the_graph_it_builds() {
    for goal in goals::registry() {
        let (root, workspace, _cache) = workspace();
        if matches!(goal.name, "compare" | "verify" | "preflight") {
            commit_two_revisions(&workspace);
        }
        let calls = calls_of(goal, &workspace);

        let mut built = false;
        for (index, arguments) in calls.iter().enumerate() {
            // Cada llamada es EN FRÍO: su propia caché, para que no encuentre
            // el instantáneo de otra llamada y la medición no mida al goal.
            let cache = root.path().join(format!("cache-{index}"));
            run_goal(goal, &workspace, &cache, arguments);
            built |= built_the_graph(&cache, &workspace);
        }

        let declared = catalogue_requires_graph(goal.name);

        assert_eq!(
            declared,
            built,
            "the goal `{}` {} the graph of the workspace and the catalogue says it {}: the \
             declaration decides the window of the dispatch, so a goal that builds the graph \
             and does not say so gets a deadline that does not cover its work",
            goal.name,
            if built { "builds" } else { "does not build" },
            if declared {
                "needs it"
            } else {
                "does not need it"
            },
        );
    }
}

/// El registro y el catálogo dicen lo mismo del grafo.
///
/// El registro es el dueño de la declaración y el catálogo es su espejo: la
/// ventana del despacho sale del catálogo, así que si los dos se separan, la
/// verdad y el plazo son de dos sitios distintos.
#[test]
fn the_catalogue_publishes_the_graph_the_registry_declares() {
    for goal in goals::registry() {
        assert_eq!(
            catalogue_requires_graph(goal.name),
            goal.requires_graph,
            "the catalogue publishes another need for the goal `{}` than the registry declares",
            goal.name
        );
    }
}
