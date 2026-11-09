//! TASK-82 — un goal se declara en un solo sitio.
//!
//! Un goal es lo que un consumidor **pide** (`status`), no lo que el
//! producto **calcula** (`find_usages`). El registro es el sitio donde vive
//! esa declaración, y de ella salen las cuatro superficies por las que un
//! consumidor puede ver al goal:
//!
//!   - el esquema MCP,
//!   - el comando clap,
//!   - la entrada de `product/tools.json`,
//!   - la ayuda.
//!
//! Lo que este fichero pinea es que las cuatro SALEN de la declaración, y
//! que no son cuatro copias: si una de ellas se escribiera a mano, el
//! contrato se partiría en dos y nadie lo notaría hasta que un cliente
//! leyera una cosa y el servidor sirviera otra.
//!
//! ## Lo que NO se comprueba aquí
//!
//! - Qué goals existen, ni qué anuncia cada perfil: eso
//!   lo mide `the_nine_goals_and_the_profiles`.
//! - Que el workspace se mida: el goal `status` publica la cobertura de la
//!   lectura, y el test la mira en la respuesta del goal, no en el catálogo.

use serde_json::Value;

use cognicode_cli::cli::commands::{CommandExecutor, Format};
use cognicode_cli::cli::result::CommandOutput;
use cognicode_core::interface::goals::{self, GoalSpec};
use cognicode_core::interface::mcp::handlers::HandlerContext;
use cognicode_core::interface::mcp::handlers::goal::handle_goal;
use cognicode_mcp::rmcp_adapter::build_all_tools;

/// El catálogo que el operador mantiene a mano.
const CATALOG: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../cognicode-core/product/tools.json"
);

fn catalogue() -> Value {
    let path = std::path::Path::new(CATALOG);
    let text = std::fs::read_to_string(path)
        .unwrap_or_else(|error| panic!("cannot read {}: {error}", path.display()));
    serde_json::from_str(&text).expect("the catalogue is valid JSON")
}

fn entry_of(name: &str) -> Value {
    catalogue()
        .get("tools")
        .and_then(Value::as_array)
        .expect("the catalogue publishes tools")
        .iter()
        .find(|tool| tool.get("name").and_then(Value::as_str) == Some(name))
        .cloned()
        .unwrap_or_else(|| panic!("the catalogue does not publish `{name}`"))
}

fn the_goal() -> &'static GoalSpec {
    goals::find("status").expect("the registry declares the goal `status`")
}

/// La declaración del goal está entera: lo que el consumidor ve, lo que
/// puede hacer, cuándo puede hacerlo y quién responde.
///
/// Una declaración a medias es una promesa a medias: un goal sin dueño no
/// tiene a quién perguntarle cuando algo va mal.
#[test]
fn the_registry_declares_the_goal_with_its_whole_declaration() {
    let goal = the_goal();

    assert_eq!(goal.name, "status");
    assert!(!goal.title.is_empty(), "the goal has no title");
    assert!(!goal.description.is_empty(), "the goal has no description");
    assert!(
        goal.examples
            .iter()
            .any(|example| example.contains(goal.name)),
        "the goal declares no example that uses its own name: {:?}",
        goal.examples
    );
    assert!(!goal.owner.is_empty(), "the goal has no owner");
    assert_eq!(
        goal.authority.as_str(),
        "read",
        "the goal reads the workspace and does not write it"
    );
    assert!(
        !goal.minimum_phase.as_str().is_empty(),
        "the goal declares no minimum phase"
    );
}

/// El catálogo publica el goal, y publica lo que el registro declara.
///
/// La entrada del maestro se escribe a mano —el maestro es curado— pero su
/// título y su descripción tienen que ser los de la declaración: si el
/// maestro dijera otra cosa, un cliente leería una promesa y el registro
/// cumpliría otra.
#[test]
fn the_catalogue_publishes_what_the_registry_declares() {
    let goal = the_goal();
    let entry = entry_of(goal.name);

    assert_eq!(
        entry.get("title").and_then(Value::as_str),
        Some(goal.title),
        "the catalogue publishes another title than the declaration"
    );
    assert_eq!(
        entry.get("description").and_then(Value::as_str),
        Some(goal.description),
        "the catalogue publishes another description than the declaration"
    );
    assert_eq!(
        entry.get("authority").and_then(Value::as_str),
        Some(goal.authority.as_str()),
        "the catalogue publishes another authority than the declaration"
    );
}

/// El `inputSchema` publicado es el derivado del struct que lee los
/// argumentos.
///
/// Es el mismo check que el del resto de las tools (AC3), y por el mismo
/// motivo: el maestro guarda una copia, y la copia se comprueba contra la
/// derivación para que no pueda pudrirse en silencio.
#[test]
fn the_published_input_schema_is_the_derived_one() {
    let goal = the_goal();
    let published = entry_of(goal.name)
        .get("input_schema")
        .cloned()
        .expect("the goal publishes an input schema");
    let derived = Value::Object(goals::input_schema(goal));

    assert_eq!(
        published, derived,
        "the catalogue publishes a shape the argument struct does not describe"
    );
}

/// El goal **no** publica `outputSchema`, y el maestro tampoco.
///
/// TASK-84: medido el 2026-11-07 sobre este árbol, la lista del perfil por
/// defecto —los cinco goals— pesa 4.290 bytes sin esquema (4.350 por el
/// cable); con el esquema del sobre bajo cada goal pesaría entre 26.420 y
/// 65.890 bytes, y el criterio de aceptación pide 6 KB o menos. Un goal
/// anuncia **qué pregunta responde** (`inputSchema`) y **cómo se llama**: el
/// sobre es el mismo para todos, y por eso no se anuncia por goal.
///
/// La ausencia es también un contrato: el maestro y el runtime tienen que
/// decir lo mismo, y el maestro se escribe a mano.
#[test]
fn the_goal_publishes_no_output_schema() {
    let goal = the_goal();
    let entry = entry_of(goal.name);

    // El maestro declara los trece campos de cada entrada: el campo está, y
    // vale `null` —la respuesta honesta para «no hay contrato», la misma que
    // dan las tools sin esquema. Un goal que publicara un esquema en el
    // maestro, o que lo omitiera, rompería el contrato de trece campos.
    assert_eq!(
        entry.get("output_schema"),
        Some(&Value::Null),
        "the catalogue publishes an output schema for the goal `{}`: {:?}",
        goal.name,
        entry.get("output_schema")
    );
    assert!(
        goals::output_schema(goal).is_empty(),
        "the runtime announces an output schema for the goal `{}`",
        goal.name
    );
}

/// El servidor anuncia el goal: un goal que no está en `tools/list` es un
/// goal que nadie puede llamar.
#[test]
fn the_server_announces_the_goal() {
    let goal = the_goal();
    let listed: Vec<String> = build_all_tools()
        .iter()
        .map(|tool| tool.name.to_string())
        .collect();

    assert!(
        listed.iter().any(|name| name == goal.name),
        "`tools/list` does not announce the goal `{}`",
        goal.name
    );
}

/// El comando clap sale de la declaración.
///
/// El nombre, el título y los ejemplos son los de la declaración, y el
/// comando no es una variante de un enum: el enum lo escribiría una persona
/// y el goal viviría en dos sitios.
#[test]
fn the_clap_command_comes_from_the_declaration() {
    let goal = the_goal();
    let command = cognicode_cli::cli::goals_clap::clap_command(goal);

    assert_eq!(command.get_name(), goal.name);
    assert_eq!(
        command.get_about().map(|about| about.to_string()),
        Some(goal.title.to_string()),
        "the command says another thing than the declaration"
    );

    let long_about = command
        .get_long_about()
        .map(|about| about.to_string())
        .unwrap_or_default();
    for example in goal.examples {
        assert!(
            long_about.contains(example),
            "the help does not carry the example `{example}`: {long_about}"
        );
    }
}

/// La ayuda de la línea de órdenes lista el goal.
///
/// La ayuda sale del árbol de comandos, y el árbol lleva los goals: un goal
/// que no aparece en `--help` es un goal que hay que conocer por otro
/// camino.
#[test]
fn the_help_lists_the_goal() {
    let goal = the_goal();
    let help = cognicode_cli::cli::cli_command().render_help().to_string();

    assert!(
        help.contains(goal.name),
        "`cognicode --help` does not list the goal `{}`: {help}",
        goal.name
    );
}

/// El valor por defecto del flag es el valor por defecto del cable.
///
/// El flag de la terminal y el campo del cable son el mismo argumento: si el
/// uno valiera `true` y el otro `false`, la misma pregunta con el argumento
/// omitido tendría dos respuestas.
#[test]
fn the_default_of_the_flag_is_the_default_of_the_wire() {
    let goal = the_goal();
    let schema = goals::input_schema(goal);
    let published = schema
        .get("properties")
        .and_then(Value::as_object)
        .and_then(|properties| properties.get("include_goals"))
        .and_then(|property| property.get("default"))
        .cloned()
        .expect("the argument publishes its default");
    let declared = schema
        .get("properties")
        .and_then(Value::as_object)
        .map(|properties| properties.len())
        .unwrap_or_default();

    assert_eq!(declared, 1, "the goal declares one argument");
    assert_eq!(
        published,
        Value::Bool(true),
        "the argument's default is not the one the struct declares"
    );

    let command = cognicode_cli::cli::goals_clap::clap_command(goal);
    let matches = command
        .try_get_matches_from(["status"])
        .expect("the command reads its arguments");
    let values = cognicode_cli::cli::goals_clap::arguments_from_matches(goal, &matches);
    assert_eq!(
        values.get("include_goals"),
        Some(&Value::Bool(true)),
        "the flag's default is not the wire's default: {values}"
    );
}

/// El CLI y el MCP devuelven el mismo sobre.
///
/// El goal se implementa una vez; el CLI y el MCP son dos proyecciones de la
/// misma implementación. Si los dos compiten, el agente y la persona reciben
/// respuestas distintas de la misma pregunta.
///
/// TASK-84: el CLI imprime el **sobre** —el mismo que el MCP publica en su
/// `structuredContent`—, así que lo que se compara es el `result` del sobre
/// con la respuesta del goal. El sobre entero se compara en
/// `the_nine_goals_and_the_profiles`, que lo construye por las dos vías.
#[tokio::test]
async fn the_cli_and_the_mcp_return_the_same_answer() {
    let goal = the_goal();
    let workspace = tempfile::tempdir().expect("tempdir");
    std::fs::write(workspace.path().join("Cargo.toml"), "[package]\n").expect("write");

    let matches = cognicode_cli::cli::goals_clap::clap_command(goal)
        .try_get_matches_from(["status"])
        .expect("the command reads its arguments");
    let output = CommandExecutor::execute_goal(goal, &matches, workspace.path(), Format::Json)
        .await
        .expect("the goal answers");
    let CommandOutput::Rendered { text, .. } = output else {
        panic!("the goal returns a rendered answer");
    };
    let cli_envelope: Value = serde_json::from_str(&text).expect("the envelope is JSON");

    let ctx = HandlerContext::builder()
        .with_working_dir(workspace.path().to_path_buf())
        .build();
    let mcp_answer = handle_goal(&ctx, goal, serde_json::json!({}))
        .await
        .expect("the goal answers");

    assert_eq!(
        cli_envelope.get("result"),
        Some(&mcp_answer),
        "the CLI and the MCP answer the same question differently"
    );
}

/// El goal se llama por su nombre en el cable, y el nombre del goal es el
/// que el registro declara.
#[tokio::test]
async fn the_goal_answers_by_its_name() {
    let goal = the_goal();
    let workspace = tempfile::tempdir().expect("tempdir");
    let ctx = HandlerContext::builder()
        .with_working_dir(workspace.path().to_path_buf())
        .build();

    let answer = handle_goal(&ctx, goal, serde_json::json!({}))
        .await
        .expect("the goal answers");

    assert_eq!(
        answer.get("goal").and_then(Value::as_str),
        Some(goal.name),
        "the answer does not name the goal that answered it: {answer}"
    );
    assert!(
        answer
            .get("summary")
            .and_then(Value::as_str)
            .is_some_and(|summary| !summary.is_empty()),
        "the answer arrives with no summary: {answer}"
    );
}
