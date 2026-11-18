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
use cognicode_core::application::graph_cache::{cache_dir, default_cache_root, workspace_key};
use cognicode_core::application::revision_snapshots::RevisionSnapshotStore;
use cognicode_core::interface::goals::{self, GoalSpec};
use cognicode_core::interface::mcp::handlers::HandlerContext;
use cognicode_core::interface::mcp::handlers::goal::handle_goal;
use cognicode_mcp::rmcp_adapter::build_all_tools;

/// The canonical form of the workspace, the way the product keys it.
fn canonical(workspace: &std::path::Path) -> std::path::PathBuf {
    workspace.canonicalize().expect("canonical workspace")
}

/// The `.snap` files directly under `dir`: the revision snapshots, apart
/// from the per-file fact objects that live in a subdirectory.
fn snapshot_files(dir: &std::path::Path) -> usize {
    std::fs::read_dir(dir)
        .map(|entries| {
            entries
                .flatten()
                .filter(|entry| entry.path().extension().is_some_and(|ext| ext == "snap"))
                .count()
        })
        .unwrap_or(0)
}

/// Runs `goal` on the CLI with its cache at `cache` and returns the envelope
/// it prints.
async fn run_in_cache(
    goal: &GoalSpec,
    matches: &clap::ArgMatches,
    workspace: &std::path::Path,
    cache: &std::path::Path,
) -> Value {
    let output =
        CommandExecutor::execute_goal_in_cache(goal, matches, workspace, Some(cache), Format::Json)
            .await
            .expect("the CLI goal answers");
    let CommandOutput::Rendered { text, .. } = output else {
        panic!("a goal returns its shared envelope");
    };
    serde_json::from_str(&text).expect("the CLI prints only JSON")
}

/// The workspace of a test is a fresh temporary directory, so no entry keyed
/// by it can exist in the cache of the environment unless the test wrote it
/// there: this is the leak, made checkable without touching the environment.
/// Every kind of entry the product keeps (snapshots, revision snapshots,
/// detector read-sets, sockets) is named by the key of its workspace, so the
/// key is looked for under the whole cache directory.
fn assert_not_in_the_environment_cache(workspace: &std::path::Path) {
    let Some(environment) = default_cache_root() else {
        return;
    };
    let key = workspace_key(&canonical(workspace));
    let mut pending = vec![cache_dir(&environment)];
    while let Some(dir) = pending.pop() {
        for entry in std::fs::read_dir(&dir).into_iter().flatten().flatten() {
            let path = entry.path();
            assert!(
                !path
                    .file_name()
                    .is_some_and(|name| name.to_string_lossy().starts_with(key.as_str())),
                "the test wrote into the cache of the environment: {}",
                path.display()
            );
            if path.is_dir() {
                pending.push(path);
            }
        }
    }
}

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

/// Las limitaciones del maestro son las que deriva la declaración.
///
/// Las limitaciones son lo que un cliente lee **antes** de llamar: si el
/// maestro dice una cosa y la declaración otra, el cliente recibe una
/// promesa que el runtime no cumple. El texto tiene un dueño —la
/// declaración— y el maestro es su espejo.
///
/// El check es para todos los goals, no para uno: un goal que necesita el
/// grafo lo dice en sus limitaciones —y el maestro tiene que decirlo
/// también—.
#[test]
fn the_published_limitations_are_the_derived_ones() {
    for goal in goals::registry() {
        let published = entry_of(goal.name)
            .get("limitations")
            .cloned()
            .expect("the goal publishes its limitations");
        let derived = goals::tools_json_entry(goal)
            .get("limitations")
            .cloned()
            .expect("the derived entry declares its limitations");

        assert_eq!(
            published, derived,
            "the catalogue publishes other limitations than the declaration derives for the goal \
             `{}`",
            goal.name
        );
    }
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

#[tokio::test]
async fn compare_is_the_same_cli_and_mcp_contract_and_states_what_it_evaluated() {
    let goal = goals::find("compare").expect("the registry declares compare");
    let workspace = tempfile::tempdir().expect("temporary Git repository");
    let git = |args: &[&str]| {
        let no_configuration = workspace.path().join("no-git-configuration");
        let output = std::process::Command::new("git")
            .env("PATH", "/usr/bin:/bin")
            .arg("-C")
            .arg(workspace.path())
            .args(args)
            .env("GIT_CONFIG_GLOBAL", &no_configuration)
            .env("GIT_CONFIG_SYSTEM", &no_configuration)
            .output()
            .expect("git is installed");
        assert!(
            output.status.success(),
            "git {args:?}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        output.stdout
    };

    git(&["init", "--quiet"]);
    git(&["config", "user.name", "CogniCode tests"]);
    git(&["config", "user.email", "tests@example.invalid"]);
    git(&["symbolic-ref", "HEAD", "refs/heads/main"]);
    std::fs::write(workspace.path().join("src.rs"), "fn base() {}\n").expect("write base");
    git(&["add", "src.rs"]);
    let base_tree = String::from_utf8(git(&["write-tree"])).expect("base tree is UTF-8");
    let base = String::from_utf8(git(&["commit-tree", base_tree.trim(), "-m", "base"]))
        .expect("base revision is UTF-8");
    git(&["update-ref", "refs/heads/main", base.trim()]);
    std::fs::write(workspace.path().join("src.rs"), "fn candidate() {}\n")
        .expect("write candidate");
    git(&["add", "src.rs"]);
    let candidate_tree = String::from_utf8(git(&["write-tree"])).expect("candidate tree is UTF-8");
    let candidate = String::from_utf8(git(&[
        "commit-tree",
        candidate_tree.trim(),
        "-p",
        base.trim(),
        "-m",
        "candidate",
    ]))
    .expect("candidate revision is UTF-8");
    git(&["update-ref", "refs/heads/main", candidate.trim()]);
    std::fs::write(workspace.path().join("staged.rs"), "fn staged() {}\n").expect("write staged");
    git(&["add", "staged.rs"]);
    std::fs::write(workspace.path().join("src.rs"), "fn dirty() {}\n").expect("write dirty");
    std::fs::write(workspace.path().join("untracked.rs"), "fn untracked() {}\n")
        .expect("write untracked");

    // Each surface keeps what it derives in a cache of its own, fresh for
    // this test: nothing the suite did before, or on the machine, can reach
    // the answer, and nothing the answer persists leaves the temporary
    // directories.
    let cli_cache = tempfile::tempdir().expect("CLI cache");
    let mcp_cache = tempfile::tempdir().expect("MCP cache");

    let matches = cognicode_cli::cli::goals_clap::clap_command(goal)
        .try_get_matches_from(["compare", "--base-ref", base.trim(), "--head-ref", "HEAD"])
        .expect("the CLI parses both revisions");
    let output = CommandExecutor::execute_goal_in_cache(
        goal,
        &matches,
        workspace.path(),
        Some(cli_cache.path()),
        Format::Json,
    )
    .await
    .expect("the CLI goal answers");
    let CommandOutput::Rendered { text, .. } = output else {
        panic!("compare returns its shared goal envelope");
    };
    let cli_envelope: Value = serde_json::from_str(&text).expect("CLI envelope is JSON");

    let ctx = HandlerContext::builder()
        .with_working_dir(workspace.path().to_path_buf())
        .with_cache_root(mcp_cache.path().to_path_buf())
        .build();
    let mcp_answer = handle_goal(
        &ctx,
        goal,
        serde_json::json!({"base_ref": base.trim(), "head_ref": "HEAD"}),
    )
    .await
    .expect("the MCP goal answers");

    // The whole answer is the shared contract, snapshot persistence and
    // work reuse included: nothing is masked.
    let cli_answer = cli_envelope["result"].clone();
    assert_eq!(cli_answer, mcp_answer);
    for surface in [&cli_answer, &mcp_answer] {
        assert_eq!(surface["revision_facts"]["persisted"], true, "{surface}");
    }
    for cache in [&cli_cache, &mcp_cache] {
        let store = RevisionSnapshotStore::for_workspace(
            cache.path(),
            &workspace.path().canonicalize().expect("canonical root"),
        );
        assert_eq!(
            snapshot_files(store.root()),
            2,
            "the snapshot of each compared revision is on disk, in the cache the surface was given"
        );
    }
    assert_not_in_the_environment_cache(workspace.path());

    // Without a cache there is no read-set and nothing is persisted, so the
    // answer says so instead of guessing.
    let uncached = HandlerContext::builder()
        .with_working_dir(workspace.path().to_path_buf())
        .build();
    let uncached_answer = handle_goal(
        &uncached,
        goal,
        serde_json::json!({"base_ref": base.trim(), "head_ref": "HEAD"}),
    )
    .await
    .expect("the MCP goal answers without a cache");
    assert_eq!(uncached_answer["revision_facts"]["persisted"], false);
    assert_eq!(uncached_answer["work_reuse_status"], "not_evaluated");
    assert!(
        uncached_answer["work_reuse_reason"]
            .as_str()
            .is_some_and(|reason| reason.contains("read-set"))
    );
    // The head findings are still published, and without a prior read-set
    // they say they were produced again rather than reused.
    let uncached_heads = uncached_answer["head_findings"]
        .as_array()
        .expect("one entry per detector that writes a read-set");
    assert_eq!(uncached_heads.len(), 3);
    for head in uncached_heads {
        assert_eq!(head["reuse"], "rerun", "{head}");
        assert!(
            head["reason"]
                .as_str()
                .is_some_and(|reason| reason.contains("no prior read-set"))
        );
    }

    let answer = cli_answer;
    assert_eq!(answer["coverage"]["changed_paths"], 1);
    assert_eq!(answer["coverage"]["analysis"], "evaluated");
    assert_eq!(answer["impact_status"], "evaluated");
    assert_eq!(answer["impact"]["facts_added"], 2);
    assert_eq!(answer["impact"]["facts_removed"], 2);
    // With a cache, the production detector run wrote the base read-set and
    // the renamed function it read makes that work affected.
    // The renamed function is affected for the function detector and the new
    // definition cannot be proven unread by the type and the bucket detectors,
    // so the verdict is partial: not every prior execution has a proven
    // answer.
    assert_eq!(answer["work_reuse_status"], "partial");
    assert_eq!(answer["work_reuse"]["affected"], 1);
    assert_eq!(answer["work_reuse"]["unknown"], 2);
    // The affected detector ran again on the candidate: its one finding is
    // the renamed function, backed by the candidate revision's facts, and the
    // same answer reached both surfaces. The type detector read no change but
    // the rename adds a definition it could not have read, so it runs again
    // too, saying that reuse is not proven, and so does the bucket detector.
    let heads = answer["head_findings"]
        .as_array()
        .expect("one per detector");
    let detectors: Vec<&str> = heads
        .iter()
        .map(|head| head["detector"].as_str().expect("detector"))
        .collect();
    assert_eq!(
        detectors,
        [
            "structure.function_definition",
            "structure.type_definition",
            "infrastructure.unprotected_s3_bucket"
        ]
    );
    let functions = &heads[0];
    assert_eq!(functions["reuse"], "rerun");
    assert_eq!(functions["coverage"], "full");
    assert_eq!(functions["findings"][0]["file"], "src.rs");
    assert_eq!(
        functions["findings"][0]["evidence_revision"],
        answer["candidate"]["revision"]
    );
    assert_eq!(heads[1]["reuse"], "rerun");
    assert!(
        heads[1]["reason"]
            .as_str()
            .is_some_and(|reason| reason.contains("not proven")),
        "{}",
        heads[1]
    );
    assert_eq!(answer["changes"][0]["path"], "src.rs");
    assert_eq!(
        answer["working_copy"]["staged_changes"][0]["path"],
        "staged.rs"
    );
    let dirty_paths = answer["working_copy"]["dirty_paths"]
        .as_array()
        .expect("working-tree paths are published");
    assert!(dirty_paths.iter().any(|path| path == "src.rs"));
    assert!(dirty_paths.iter().any(|path| path == "untracked.rs"));
}

/// `verify` is one contract on both surfaces: the CLI prints the shared
/// envelope as JSON and the MCP returns the same answer, and a revision with
/// no stored snapshot is `not_evaluated` on both, never `verified`.
#[tokio::test]
async fn verify_is_the_same_cli_and_mcp_contract_and_never_claims_an_unstored_snapshot() {
    let goal = goals::find("verify").expect("the registry declares verify");
    let workspace = tempfile::tempdir().expect("temporary Git repository");
    let git = |args: &[&str]| {
        let no_configuration = workspace.path().join("no-git-configuration");
        let output = std::process::Command::new("git")
            .arg("-C")
            .arg(workspace.path())
            .args(args)
            .env("GIT_CONFIG_GLOBAL", &no_configuration)
            .env("GIT_CONFIG_SYSTEM", &no_configuration)
            .output()
            .expect("git is installed");
        assert!(
            output.status.success(),
            "git {args:?}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    };
    git(&["init", "--quiet"]);
    git(&["config", "user.name", "CogniCode tests"]);
    git(&["config", "user.email", "tests@example.invalid"]);
    std::fs::write(workspace.path().join("src.rs"), "fn only() {}\n").expect("write source");
    git(&["add", "src.rs"]);
    git(&["commit", "--quiet", "-m", "base"]);

    let cli_cache = tempfile::tempdir().expect("CLI cache");
    let mcp_cache = tempfile::tempdir().expect("MCP cache");

    let matches = cognicode_cli::cli::goals_clap::clap_command(goal)
        .try_get_matches_from(["verify", "--revision", "HEAD"])
        .expect("the CLI parses the revision");
    let cli_envelope = run_in_cache(goal, &matches, workspace.path(), cli_cache.path()).await;

    let ctx = HandlerContext::builder()
        .with_working_dir(workspace.path().to_path_buf())
        .with_cache_root(mcp_cache.path().to_path_buf())
        .build();
    let mcp_answer = handle_goal(&ctx, goal, serde_json::json!({"revision": "HEAD"}))
        .await
        .expect("the MCP goal answers");

    let cli_answer = &cli_envelope["result"];
    assert_eq!(cli_answer["status"], "not_evaluated");
    assert_eq!(mcp_answer["status"], "not_evaluated");
    assert_eq!(cli_answer["revision"], mcp_answer["revision"]);
    assert_eq!(cli_answer["coverage"], mcp_answer["coverage"]);
    assert_eq!(cli_answer["snapshot"]["found"], false);
    assert_eq!(mcp_answer["snapshot"]["found"], false);

    // `verify` never writes: it leaves nothing behind in either cache.
    for cache in [&cli_cache, &mcp_cache] {
        assert!(
            !RevisionSnapshotStore::for_workspace(cache.path(), &canonical(workspace.path()))
                .root()
                .exists(),
            "verify persisted a snapshot instead of only reading one"
        );
    }
    assert_not_in_the_environment_cache(workspace.path());
}

/// A snapshot `inspect` persists on the CLI is the one `verify` confirms on
/// the CLI and on the MCP that share its cache, and it is on disk in that
/// cache and nowhere else.
#[tokio::test]
async fn the_cli_persists_the_snapshot_that_verify_then_confirms_on_both_surfaces() {
    let inspect = goals::find("inspect").expect("the registry declares inspect");
    let verify = goals::find("verify").expect("the registry declares verify");
    let workspace = tempfile::tempdir().expect("temporary Git repository");
    let git = |args: &[&str]| {
        let no_configuration = workspace.path().join("no-git-configuration");
        let output = std::process::Command::new("git")
            .arg("-C")
            .arg(workspace.path())
            .args(args)
            .env("GIT_CONFIG_GLOBAL", &no_configuration)
            .env("GIT_CONFIG_SYSTEM", &no_configuration)
            .output()
            .expect("git is installed");
        assert!(
            output.status.success(),
            "git {args:?}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    };
    git(&["init", "--quiet"]);
    git(&["config", "user.name", "CogniCode tests"]);
    git(&["config", "user.email", "tests@example.invalid"]);
    std::fs::write(workspace.path().join("src.rs"), "fn only() {}\n").expect("write source");
    git(&["add", "src.rs"]);
    git(&["commit", "--quiet", "-m", "base"]);
    let cache = tempfile::tempdir().expect("cache");

    let inspected = run_in_cache(
        inspect,
        &cognicode_cli::cli::goals_clap::clap_command(inspect)
            .try_get_matches_from(["inspect", "--revision", "HEAD"])
            .expect("the CLI parses the revision"),
        workspace.path(),
        cache.path(),
    )
    .await;
    assert_eq!(
        inspected["result"]["revision_facts"]["persisted"], true,
        "{inspected}"
    );
    let store = RevisionSnapshotStore::for_workspace(cache.path(), &canonical(workspace.path()));
    assert_eq!(snapshot_files(store.root()), 1, "the snapshot is on disk");

    let verified = run_in_cache(
        verify,
        &cognicode_cli::cli::goals_clap::clap_command(verify)
            .try_get_matches_from(["verify", "--revision", "HEAD"])
            .expect("the CLI parses the revision"),
        workspace.path(),
        cache.path(),
    )
    .await;
    assert_eq!(verified["result"]["status"], "verified", "{verified}");
    assert_eq!(verified["result"]["snapshot"]["found"], true);

    let ctx = HandlerContext::builder()
        .with_working_dir(workspace.path().to_path_buf())
        .with_cache_root(cache.path().to_path_buf())
        .build();
    let mcp_verified = handle_goal(&ctx, verify, serde_json::json!({"revision": "HEAD"}))
        .await
        .expect("the MCP goal answers");
    assert_eq!(
        mcp_verified, verified["result"],
        "the MCP confirms what the CLI persisted, in the same words"
    );
    assert_not_in_the_environment_cache(workspace.path());
}

/// The isolation itself: a goal run in an injected cache writes there, and
/// the cache of the environment gains nothing for that workspace, whatever
/// `HOME` and `XDG_CACHE_HOME` say where the suite runs.
#[tokio::test]
async fn a_goal_run_in_an_injected_cache_leaves_the_environment_cache_untouched() {
    let inspect = goals::find("inspect").expect("the registry declares inspect");
    let workspace = tempfile::tempdir().expect("temporary Git repository");
    let git = |args: &[&str]| {
        let no_configuration = workspace.path().join("no-git-configuration");
        let output = std::process::Command::new("git")
            .arg("-C")
            .arg(workspace.path())
            .args(args)
            .env("GIT_CONFIG_GLOBAL", &no_configuration)
            .env("GIT_CONFIG_SYSTEM", &no_configuration)
            .output()
            .expect("git is installed");
        assert!(output.status.success(), "git {args:?}");
    };
    git(&["init", "--quiet"]);
    git(&["config", "user.name", "CogniCode tests"]);
    git(&["config", "user.email", "tests@example.invalid"]);
    std::fs::write(workspace.path().join("src.rs"), "fn only() {}\n").expect("write source");
    git(&["add", "src.rs"]);
    git(&["commit", "--quiet", "-m", "base"]);
    let cache = tempfile::tempdir().expect("cache");

    run_in_cache(
        inspect,
        &cognicode_cli::cli::goals_clap::clap_command(inspect)
            .try_get_matches_from(["inspect", "--revision", "HEAD"])
            .expect("the CLI parses the revision"),
        workspace.path(),
        cache.path(),
    )
    .await;

    let store = RevisionSnapshotStore::for_workspace(cache.path(), &canonical(workspace.path()));
    assert!(
        store.root().exists(),
        "the injected cache received the snapshot"
    );
    assert_not_in_the_environment_cache(workspace.path());
}

/// `inspect` takes a file or a revision, exactly one of them, and says so
/// on both surfaces.
#[tokio::test]
async fn inspect_takes_a_file_or_a_revision_and_not_both_nor_neither() {
    let goal = goals::find("inspect").expect("the registry declares inspect");
    let workspace = tempfile::tempdir().expect("workspace");
    let ctx = HandlerContext::builder()
        .with_working_dir(workspace.path().to_path_buf())
        .build();

    for arguments in [
        serde_json::json!({}),
        serde_json::json!({"file": "a.rs", "revision": "HEAD"}),
    ] {
        let error = handle_goal(&ctx, goal, arguments.clone())
            .await
            .expect_err("an ambiguous or empty question is a failure");
        assert!(
            format!("{error}").contains("file"),
            "{arguments}: the failure names the arguments: {error}"
        );
    }

    // An optional flag nobody wrote reaches the goal as absent, not as text.
    let revision_only = cognicode_cli::cli::goals_clap::clap_command(goal)
        .try_get_matches_from(["inspect", "--revision", "HEAD"])
        .expect("clap parses the revision alone");
    assert_eq!(
        cognicode_cli::cli::goals_clap::arguments_from_matches(goal, &revision_only),
        serde_json::json!({"revision": "HEAD"})
    );

    let both = cognicode_cli::cli::goals_clap::clap_command(goal).try_get_matches_from([
        "inspect",
        "--file",
        "a.rs",
        "--revision",
        "HEAD",
    ]);
    let matches = both.expect("clap parses both; the goal refuses them");
    let refused =
        CommandExecutor::execute_goal(goal, &matches, workspace.path(), Format::Json).await;
    assert!(
        refused.is_err(),
        "the CLI refuses file and revision together"
    );
}
