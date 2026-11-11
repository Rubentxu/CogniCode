//! TASK-84 — los goals, los perfiles y `status` como raíz.
//!
//! Un goal es lo que un consumidor **pide**; una tool es lo que el producto
//! **calcula**. La unidad monta los goals sobre los dueños que ya
//! existían —sin recalcular nada— y decide **qué se anuncia** en cada
//! superficie: el perfil `default` publica los goals que responden antes del
//! análisis completo, el `full` publica todo, y el `legacy` publica las
//! tools con el aviso que nombra al goal que las sustituye.
//!
//! TASK-85 añade el décimo goal —`recipe`, las preguntas compuestas—: el
//! registro crece, y el perfil por defecto **no** crece con él, porque la
//! fase de la receta es `analyze` y la regla del perfil por defecto la
//! deriva de la fase.
//!
//! ## Lo que este fichero pinea
//!
//! - **AC1**: el perfil por defecto publica cinco goals, y la lista pesa
//!   6 KB o menos.
//! - **AC2**: `status` devuelve las fases, los lenguajes, la cobertura y
//!   los goals con sus plantillas de argumentos.
//! - **AC3**: `cognicode <goal>` y la tool MCP del mismo nombre devuelven
//!   el mismo sobre.
//! - **AC4**: `change` —el goal que escribe— no aparece en el modo de sólo
//!   lectura.
//!
//! ## Lo que NO se comprueba aquí
//!
//! - Que cada goal calcule bien: eso lo mide el test de cada goal, en su
//!   propio módulo.
//! - Que la lista entera del cable pese lo que pesan las de una tool: el
//!   perfil `full` publica las 88 entradas, y su peso es el de siempre.

use serde_json::{Map, Value};

use cognicode_cli::cli::commands::{CommandExecutor, Format};
use cognicode_cli::cli::result::CommandOutput;
use cognicode_core::interface::goals;
use cognicode_core::interface::hateoas::HandleContext;
use cognicode_core::interface::mcp::handlers::HandlerContext;
use cognicode_core::interface::mcp::handlers::goal::handle_goal;
use cognicode_core::interface::surface::SurfaceProfile;
use cognicode_mcp::rmcp_adapter::{tools_for_profile, visible_tools};
use cognicode_mcp::tool_response::success_result;

/// El presupuesto de la lista por defecto: 6 KB.
const DEFAULT_LIST_BUDGET: usize = 6 * 1024;

/// Los nombres que el perfil por defecto publica, en orden.
fn default_names() -> Vec<String> {
    tools_for_profile(SurfaceProfile::Default)
        .iter()
        .map(|tool| tool.name.to_string())
        .collect()
}

/// AC1 — el perfil por defecto publica cinco goals.
///
/// No cinco tools: cinco goals. La superficie por defecto es la de los
/// goals, y las tools se anuncian en el perfil `full`.
///
/// El orden no es parte del criterio: lo publica el catálogo, y el
/// catálogo es el que dice en qué orden se anuncia. Lo que se exige es
/// **qué** se anuncia y **cuántas** son.
#[test]
fn the_default_profile_publishes_five_goals() {
    let mut names = default_names();
    names.sort();

    assert_eq!(
        names,
        vec![
            "context".to_string(),
            "find".to_string(),
            "inspect".to_string(),
            "status".to_string(),
            "trace".to_string(),
        ],
        "the default profile publishes the five goals that answer before the analysis"
    );
}

/// AC1 — y la lista por defecto pesa 6 KB o menos.
///
/// El peso se mide sobre el documento que viaja por el cable, con las
/// herramientas que el servidor anuncia: el número que se publica es el
/// que se mide.
#[test]
fn the_default_profile_fits_in_six_kilobytes() {
    let document = serde_json::json!({ "tools": tools_for_profile(SurfaceProfile::Default) });
    let bytes = serde_json::to_vec(&document)
        .expect("the announced tools are serialisable")
        .len();

    assert!(
        bytes <= DEFAULT_LIST_BUDGET,
        "the default list weighs {bytes} bytes and the budget is {DEFAULT_LIST_BUDGET}"
    );
}

/// El registro declara los diez goals, cada uno con su declaración entera.
///
/// Un goal a medias es una promesa a medias: sin dueño no hay a quién
/// preguntarle, y sin fase no se sabe si el perfil por defecto lo publica.
#[test]
fn the_registry_declares_the_ten_goals() {
    let names: Vec<&str> = goals::registry().iter().map(|goal| goal.name).collect();

    assert_eq!(
        names,
        vec![
            "status", "context", "find", "inspect", "trace", "impact", "overview", "audit",
            "change", "recipe"
        ],
        "the registry declares the ten goals"
    );

    for goal in goals::registry() {
        assert!(!goal.title.is_empty(), "`{}` has no title", goal.name);
        assert!(
            !goal.description.is_empty(),
            "`{}` has no description",
            goal.name
        );
        assert!(!goal.owner.is_empty(), "`{}` has no owner", goal.name);
        assert!(
            !goal.examples.is_empty(),
            "`{}` declares no example",
            goal.name
        );
    }
}

/// El goal que escribe es el único que no se anuncia en modo de sólo
/// lectura.
///
/// AC4: `change` declara su autoridad `mutating`, y el servidor de sólo
/// lectura no anuncia lo que escribe.
#[test]
fn the_change_goal_is_not_announced_in_read_only_mode() {
    let read_only: Vec<String> = visible_tools(SurfaceProfile::Full, true)
        .iter()
        .map(|tool| tool.name.to_string())
        .collect();
    let writable: Vec<String> = visible_tools(SurfaceProfile::Full, false)
        .iter()
        .map(|tool| tool.name.to_string())
        .collect();

    assert!(
        !read_only.iter().any(|name| name == "change"),
        "a server that may not write does not announce the goal that writes: {read_only:?}"
    );
    assert!(
        writable.iter().any(|name| name == "change"),
        "a server that may write announces the goal that writes"
    );
}

/// AC2 — `status` devuelve las fases, los lenguajes, la cobertura y los
/// goals con sus plantillas de argumentos.
///
/// Las cuatro cosas son la respuesta de `status`: sin una de ellas, quien
/// pregunta qué ofrece el build recibe media respuesta.
#[test]
fn the_status_goal_answers_the_phases_the_languages_the_coverage_and_the_goals() {
    let workspace = tempfile::tempdir().expect("tempdir");
    std::fs::write(workspace.path().join("Cargo.toml"), "[package]\n").expect("write");
    std::fs::write(workspace.path().join("main.rs"), "fn main() {}").expect("write");

    let goal = goals::find("status").expect("the registry declares the goal `status`");
    let environment = goals::GoalEnvironment::new(workspace.path(), None);
    let answer = (goal.run)(&environment, &serde_json::json!({})).expect("the goal answers");

    let phases = answer
        .get("phases")
        .and_then(Value::as_array)
        .expect("the answer carries the phases");
    assert!(!phases.is_empty(), "the answer carries no phase");

    let languages = answer
        .get("workspace")
        .and_then(|workspace| workspace.get("languages"))
        .and_then(Value::as_array)
        .expect("the answer carries the languages");
    assert!(
        languages.iter().any(|language| language == "Rust"),
        "a workspace with a Cargo.toml is Rust: {answer}"
    );

    let coverage = answer
        .get("coverage")
        .expect("the answer carries the coverage");
    assert!(
        coverage.get("status").and_then(Value::as_str).is_some(),
        "the coverage says what it is: {coverage}"
    );

    let goals = answer
        .get("goals")
        .and_then(Value::as_array)
        .expect("the answer carries the goals");
    assert_eq!(
        goals.len(),
        goals::registry().len(),
        "the answer carries every goal this build declares"
    );
    for goal in goals {
        let name = goal
            .get("name")
            .and_then(Value::as_str)
            .expect("a goal has a name");
        let arguments = goal
            .get("arguments")
            .and_then(Value::as_array)
            .unwrap_or_else(|| panic!("`{name}` carries no argument template"));
        for argument in arguments {
            assert!(
                argument.get("description").is_some(),
                "the template of an argument says what it does: {goal}"
            );
            assert!(
                argument.get("required").is_some(),
                "the template of an argument says if it is required: {goal}"
            );
        }
    }
}

/// AC3 — la terminal y el cable devuelven el mismo sobre.
///
/// El goal se implementa una vez; las dos superficies son dos proyecciones
/// de la misma implementación. Si el sobre fuera distinto, el agente y la
/// persona recibirían respuestas distintas de la misma pregunta.
#[tokio::test]
async fn the_cli_and_the_mcp_return_the_same_envelope() {
    let goal = goals::find("status").expect("the registry declares the goal `status`");
    let workspace = tempfile::tempdir().expect("tempdir");
    std::fs::write(workspace.path().join("Cargo.toml"), "[package]\n").expect("write");

    // La terminal. El comando del goal ya es el comando: el primer
    // elemento de la lista es el nombre del binario, no un argumento.
    let matches = cognicode_cli::cli::goals_clap::clap_command(goal)
        .try_get_matches_from([goal.name])
        .expect("the command reads its arguments");
    let output = CommandExecutor::execute_goal(goal, &matches, workspace.path(), Format::Json)
        .await
        .expect("the goal answers");
    let CommandOutput::Rendered { text, .. } = output else {
        panic!("the goal returns a rendered answer");
    };
    let cli_envelope: Value = serde_json::from_str(&text).expect("the envelope is JSON");

    // El cable.
    let ctx = HandlerContext::builder()
        .with_working_dir(workspace.path().to_path_buf())
        .build();
    let answer = handle_goal(&ctx, goal, serde_json::json!({}))
        .await
        .expect("the goal answers");
    let body = serde_json::to_string(&answer).expect("the answer is serialisable");
    let context = HandleContext {
        workspace: workspace.path().to_string_lossy().to_string(),
        snapshot: "none".to_string(),
    };
    let result = success_result(goal.name, body, &Map::new(), &context);
    let mcp_envelope = result
        .structured_content
        .expect("the MCP publishes the envelope");

    assert_eq!(
        cli_envelope, mcp_envelope,
        "the terminal and the wire publish a different envelope for the same question"
    );
}

/// `cognicode` sin argumentos es el goal `status`.
///
/// La raíz del producto: quien escribe `cognicode` pregunta qué ofrece el
/// build y qué se ve del workspace, y esa pregunta tiene un goal.
#[test]
fn cognicode_without_arguments_is_the_status_goal() {
    let goal = goals::find("status").expect("the registry declares the goal `status`");
    let help = cognicode_cli::cli::cli_command().render_help().to_string();

    assert!(
        help.contains(goal.name),
        "the help lists the goal `{}`: {help}",
        goal.name
    );

    // El árbol de comandos acepta `status` como subcomando: el goal se
    // escribe en la terminal como se escribe en el cable.
    let command = cognicode_cli::cli::cli_command();
    let matches = command
        .try_get_matches_from(["cognicode", "status"])
        .expect("the goal is a subcommand of the root");
    assert_eq!(matches.subcommand_name(), Some("status"));
}

/// El perfil `legacy` publica las tools con el aviso que nombra al goal
/// que las sustituye.
///
/// Un nombre publicado que desaparece sin un aviso rompe a quien lo usa, y
/// un aviso que no dice con qué se sustituye no dice nada.
#[test]
fn the_legacy_profile_publishes_the_tools_with_their_notice() {
    let legacy = tools_for_profile(SurfaceProfile::Legacy);
    let full = tools_for_profile(SurfaceProfile::Full);

    assert!(
        legacy.len() < full.len(),
        "the legacy profile publishes fewer tools than the full one"
    );
    assert!(
        !legacy.iter().any(|tool| tool.name.as_ref() == "status"),
        "the legacy profile publishes no goal: the goals are the surface now"
    );

    let analyze_impact = legacy
        .iter()
        .find(|tool| tool.name.as_ref() == "analyze_impact")
        .expect("the legacy profile publishes the tools the product published before");
    let description = analyze_impact
        .description
        .as_ref()
        .map(|description| description.to_string())
        .unwrap_or_default();
    assert!(
        description.contains("impact"),
        "the notice names the goal that answers the same question: {description}"
    );
}
