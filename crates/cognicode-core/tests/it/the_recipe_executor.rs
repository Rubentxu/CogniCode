//! TASK-85 — el ejecutor de recetas: memo, paralelismo, presupuesto y `--plan`.
//!
//! Una receta es una pregunta compuesta: **qué goals** hay que correr, **en
//! qué orden** y **qué se puede reutilizar** de la corrida anterior. Este
//! fichero mide las cuatro cosas que la receta promete:
//!
//!   - **AC1**: cada paso se memoiza por `(goal, args, hash del árbol)` y lo
//!     independiente corre en paralelo.
//!   - **AC2**: un paso que falla deja la receta en `partial`, con el motivo
//!     y el enlace de reintento.
//!   - **AC3**: `plan: true` y `--plan` dicen qué correría, qué está en caché
//!     y el coste estimado, **sin ejecutar**.
//!   - **AC4**: una receta del workspace no puede invocar `change`.
//!
//! ## Lo que NO se comprueba aquí
//!
//! - Que el goal `recipe` esté en el catálogo: eso lo mide el maestro
//!   (`a_goal_is_declared_once`) y el perfil (`the_nine_goals_and_the_profiles`).
//! - Que el grafo se construya: aquí los goals los responde un doble, y el
//!   producto de verdad se mide en los tests de cada goal.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Barrier, Mutex};
use std::time::Duration;

use serde_json::{Map, Value, json};

use cognicode_core::interface::goals::{self, GoalEnvironment};
use cognicode_core::interface::hateoas;
use cognicode_core::interface::recipes::executor::{GoalRunner, RecipeExecutor};
use cognicode_core::interface::recipes::memo::MemoStore;
use cognicode_core::interface::recipes::{Recipe, RecipeBook, RecipeOrigin, RecipeStep};

// --- Los dobles ---

/// Un goal que responde siempre lo mismo, y cuenta cuántas veces se llamó.
struct CountingRunner {
    calls: AtomicUsize,
    seen: Mutex<Vec<String>>,
}

impl CountingRunner {
    fn new() -> Self {
        Self {
            calls: AtomicUsize::new(0),
            seen: Mutex::new(Vec::new()),
        }
    }

    fn calls(&self) -> usize {
        self.calls.load(Ordering::SeqCst)
    }
}

impl GoalRunner for CountingRunner {
    fn run(
        &self,
        _ctx: &GoalEnvironment<'_>,
        goal: &str,
        _arguments: &Value,
    ) -> Result<Value, String> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        self.seen
            .lock()
            .expect("the lock is not poisoned")
            .push(goal.to_string());
        Ok(json!({ "goal": goal }))
    }
}

/// Un goal que falla, y dice por qué.
struct FailingRunner;

impl GoalRunner for FailingRunner {
    fn run(
        &self,
        _ctx: &GoalEnvironment<'_>,
        goal: &str,
        _arguments: &Value,
    ) -> Result<Value, String> {
        Err(format!(
            "the goal `{goal}` cannot answer: the symbol is not declared"
        ))
    }
}

/// Un goal que espera a su compañero: dos pasos de la misma fase sólo
/// llegan los dos si corren en paralelo.
///
/// La espera es una barrera, no un `sleep`: si el ejecutor los corre en
/// serie, el primero se queda esperando a un segundo que no llega, y el
/// test lo ve en su plazo.
struct BarrierRunner {
    barrier: Barrier,
}

impl GoalRunner for BarrierRunner {
    fn run(
        &self,
        _ctx: &GoalEnvironment<'_>,
        goal: &str,
        _arguments: &Value,
    ) -> Result<Value, String> {
        self.barrier.wait();
        Ok(json!({ "goal": goal }))
    }
}

// --- Los helpers ---

/// Un workspace con un fichero que el producto puede leer.
fn workspace() -> tempfile::TempDir {
    let dir = tempfile::tempdir().expect("tempdir");
    std::fs::write(
        dir.path().join("main.rs"),
        "fn helper() {}\nfn main() { helper() }\n",
    )
    .expect("the source is written");
    dir
}

/// Una receta de un solo paso.
fn recipe_with(goal: &str) -> Recipe {
    Recipe {
        name: "prueba".to_string(),
        description: "Una receta de prueba.".to_string(),
        parameters: Vec::new(),
        steps: vec![RecipeStep {
            goal: goal.to_string(),
            args: Map::new(),
        }],
        origin: RecipeOrigin::Product,
    }
}

/// Los parámetros de una receta.
fn params(pairs: &[(&str, &str)]) -> BTreeMap<String, String> {
    pairs
        .iter()
        .map(|(name, value)| (name.to_string(), value.to_string()))
        .collect()
}

// --- AC4: una receta del workspace no puede invocar `change` ---

#[test]
fn a_workspace_recipe_that_invokes_change_is_refused_with_its_reason() {
    let dir = tempfile::tempdir().expect("tempdir");
    std::fs::create_dir_all(dir.path().join(".cognicode")).expect("the folder is created");
    std::fs::write(
        dir.path().join(".cognicode/goals.toml"),
        "[[recipe]]\n\
         name = \"peligrosa\"\n\
         description = \"Una receta que escribe.\"\n\
         steps = [ { goal = \"change\", args = { symbol = \"x\", new_name = \"y\" } } ]\n",
    )
    .expect("the recipe is written");

    let book = RecipeBook::load(dir.path());

    assert!(
        book.get("peligrosa").is_none(),
        "a recipe that invokes `change` is not published: it is refused whole, \
         not in part"
    );
    let refusal = book
        .refused()
        .iter()
        .find(|refusal| refusal.name == "peligrosa")
        .expect("the refused recipe is there, with its reason");
    assert!(
        refusal.reason.contains("change"),
        "the reason names the goal that cannot be invoked: {}",
        refusal.reason
    );
}

#[test]
fn a_workspace_recipe_that_only_reads_is_published() {
    let dir = tempfile::tempdir().expect("tempdir");
    std::fs::create_dir_all(dir.path().join(".cognicode")).expect("the folder is created");
    std::fs::write(
        dir.path().join(".cognicode/goals.toml"),
        "[[recipe]]\n\
         name = \"del-equipo\"\n\
         description = \"Una receta de sólo lectura.\"\n\
         steps = [ { goal = \"status\" }, { goal = \"context\" } ]\n",
    )
    .expect("the recipe is written");

    let book = RecipeBook::load(dir.path());

    let recipe = book
        .get("del-equipo")
        .expect("a recipe that only reads is published");
    assert_eq!(recipe.origin, RecipeOrigin::Workspace);
    assert_eq!(recipe.steps.len(), 2);
}

// --- AC1: el memo ---

#[test]
fn a_step_runs_once_and_is_served_from_the_memo_while_the_tree_does_not_change() {
    let dir = workspace();
    let cache = tempfile::tempdir().expect("tempdir");
    let cache_root = cache.path().join("cache");
    let runner = CountingRunner::new();
    let recipe = recipe_with("status");
    let parameters = params(&[]);

    let first = {
        let executor = RecipeExecutor::new(dir.path(), Some(cache_root.as_path()), &runner);
        executor
            .execute(&recipe, &parameters)
            .expect("the recipe runs")
    };

    assert_eq!(runner.calls(), 1, "the step ran once");
    assert_eq!(first.steps[0].state, "computed");
    assert!(!first.steps[0].cached);

    let second = {
        let executor = RecipeExecutor::new(dir.path(), Some(cache_root.as_path()), &runner);
        executor
            .execute(&recipe, &parameters)
            .expect("the recipe runs")
    };

    assert_eq!(
        runner.calls(),
        1,
        "the second run did not run the step again"
    );
    assert_eq!(second.steps[0].state, "cached");
    assert!(second.steps[0].cached);
}

#[test]
fn a_step_runs_again_when_the_tree_changes() {
    let dir = workspace();
    let cache = tempfile::tempdir().expect("tempdir");
    let cache_root = cache.path().join("cache");
    let runner = CountingRunner::new();
    let recipe = recipe_with("status");
    let parameters = params(&[]);

    {
        let executor = RecipeExecutor::new(dir.path(), Some(cache_root.as_path()), &runner);
        executor
            .execute(&recipe, &parameters)
            .expect("the recipe runs");
    }
    assert_eq!(runner.calls(), 1);

    // El fichero cambia de contenido: el hash del árbol cambia, y el memo
    // deja de servir.
    std::fs::write(
        dir.path().join("main.rs"),
        "fn helper() {}\nfn main() { helper() }\nfn extra() {}\n",
    )
    .expect("the source is written");

    let third = {
        let executor = RecipeExecutor::new(dir.path(), Some(cache_root.as_path()), &runner);
        executor
            .execute(&recipe, &parameters)
            .expect("the recipe runs")
    };

    assert_eq!(runner.calls(), 2, "the step ran again: the tree changed");
    assert_eq!(third.steps[0].state, "computed");
}

#[test]
fn the_memo_is_keyed_by_the_goal_the_args_and_the_tree() {
    let cache = tempfile::tempdir().expect("tempdir");
    let workspace = cache.path().join("ws");
    std::fs::create_dir_all(&workspace).expect("the workspace is created");
    let store = MemoStore::new(&cache.path().join("cache"), &workspace);

    let args = json!({ "symbol": "helper" }).as_object().cloned().unwrap();
    store
        .store("find", &args, "tree-a", &json!({ "found": true }), 7)
        .expect("the memo is written");

    let hit = store
        .lookup("find", &args, "tree-a")
        .expect("the memo answers for the same goal, the same args and the same tree");
    assert_eq!(hit.value, json!({ "found": true }));
    assert_eq!(hit.duration_ms, 7);

    assert!(
        store.lookup("find", &args, "tree-b").is_none(),
        "another tree is another memo"
    );
    assert!(
        store.lookup("trace", &args, "tree-a").is_none(),
        "another goal is another memo"
    );
    let other_args = json!({ "symbol": "otro" }).as_object().cloned().unwrap();
    assert!(
        store.lookup("find", &other_args, "tree-a").is_none(),
        "other arguments are another memo"
    );
}

#[test]
fn the_steps_of_the_same_phase_run_in_parallel() {
    let dir = workspace();
    let cache = tempfile::tempdir().expect("tempdir");
    // Los dos goals son de la fase `discover`: el ejecutor los corre juntos.
    let recipe = Recipe {
        name: "paralela".to_string(),
        description: "Dos pasos de la misma fase.".to_string(),
        parameters: Vec::new(),
        steps: vec![
            RecipeStep {
                goal: "status".to_string(),
                args: Map::new(),
            },
            RecipeStep {
                goal: "context".to_string(),
                args: Map::new(),
            },
        ],
        origin: RecipeOrigin::Product,
    };
    let parameters = params(&[]);

    let (sender, receiver) = std::sync::mpsc::channel();
    let workspace_path: PathBuf = dir.path().to_path_buf();
    let cache_path: PathBuf = cache.path().join("cache");
    let handle = std::thread::spawn(move || {
        let runner = BarrierRunner {
            barrier: Barrier::new(2),
        };
        let executor = RecipeExecutor::new(&workspace_path, Some(&cache_path), &runner);
        let outcome = executor.execute(&recipe, &parameters);
        let _ = sender.send(outcome);
    });

    let outcome = receiver
        .recv_timeout(Duration::from_secs(30))
        .expect(
            "the two steps of the same phase ran in parallel: the barrier \
                  was crossed, which only happens when they run at the same time",
        )
        .expect("the recipe runs");
    let _ = handle.join();

    assert_eq!(outcome.status, "complete");
    assert_eq!(outcome.steps.len(), 2);
}

// --- AC2: un paso que falla ---

#[test]
fn a_failing_step_leaves_the_recipe_partial_with_the_reason() {
    let dir = workspace();
    let cache = tempfile::tempdir().expect("tempdir");
    let cache_root = cache.path().join("cache");
    let runner = FailingRunner;
    let recipe = recipe_with("impact");
    let parameters = params(&[]);

    let outcome = {
        let executor = RecipeExecutor::new(dir.path(), Some(cache_root.as_path()), &runner);
        executor
            .execute(&recipe, &parameters)
            .expect("the recipe answers even when a step fails")
    };

    assert_eq!(outcome.status, "partial");
    let failure = outcome
        .failure
        .as_ref()
        .expect("the recipe says which step failed");
    assert_eq!(failure.goal, "impact");
    assert!(
        failure.reason.contains("the symbol is not declared"),
        "the reason is the one the goal said: {}",
        failure.reason
    );
    assert_eq!(outcome.budget.failed, 1);
}

#[test]
fn the_envelope_publishes_the_link_that_retries_the_failed_step() {
    let dir = workspace();
    let cache = tempfile::tempdir().expect("tempdir");
    let cache_root = cache.path().join("cache");
    let runner = FailingRunner;
    let recipe = recipe_with("impact");
    let parameters = params(&[]);

    let outcome = {
        let executor = RecipeExecutor::new(dir.path(), Some(cache_root.as_path()), &runner);
        executor
            .execute(&recipe, &parameters)
            .expect("the recipe answers")
    };
    let result = serde_json::to_value(&outcome).expect("the answer is serialisable");
    let mut arguments = Map::new();
    arguments.insert("name".to_string(), json!("prueba"));
    arguments.insert("plan".to_string(), json!(false));
    let context = hateoas::HandleContext {
        workspace: dir.path().to_string_lossy().to_string(),
        snapshot: "none".to_string(),
    };

    let envelope = hateoas::envelope_for("recipe", &arguments, &result, &context);

    assert_eq!(envelope.state, "partial");
    let retry = envelope
        .links
        .iter()
        .find(|link| link.rel == "retry")
        .expect("the envelope publishes the link that retries the recipe");
    assert_eq!(retry.tool, "recipe");
    assert_eq!(
        retry.required_args.get("name").and_then(Value::as_str),
        Some("prueba"),
        "the link carries the arguments the retry needs"
    );
}

// --- AC3: el plan ---

#[test]
fn the_plan_says_what_would_run_without_running_it() {
    let dir = workspace();
    let cache = tempfile::tempdir().expect("tempdir");
    let cache_root = cache.path().join("cache");
    let runner = CountingRunner::new();
    let recipe = recipe_with("status");
    let parameters = params(&[]);

    let plan = {
        let executor = RecipeExecutor::new(dir.path(), Some(cache_root.as_path()), &runner);
        executor
            .plan(&recipe, &parameters)
            .expect("the plan is made")
    };

    assert_eq!(runner.calls(), 0, "the plan ran nothing");
    assert!(plan.planned);
    assert_eq!(plan.steps[0].state, "planned");
    assert!(!plan.steps[0].cached);
    assert!(
        plan.budget.estimated_latency_ms > 0,
        "the plan says what it would cost: {:?}",
        plan.budget
    );
    assert_eq!(
        plan.budget.computed, 1,
        "the plan says how many steps would run"
    );
}

#[test]
fn the_plan_says_what_is_already_in_the_cache() {
    let dir = workspace();
    let cache = tempfile::tempdir().expect("tempdir");
    let cache_root = cache.path().join("cache");
    let runner = CountingRunner::new();
    let recipe = recipe_with("status");
    let parameters = params(&[]);

    {
        let executor = RecipeExecutor::new(dir.path(), Some(cache_root.as_path()), &runner);
        executor
            .execute(&recipe, &parameters)
            .expect("the recipe runs");
    }

    let plan = {
        let executor = RecipeExecutor::new(dir.path(), Some(cache_root.as_path()), &runner);
        executor
            .plan(&recipe, &parameters)
            .expect("the plan is made")
    };

    assert_eq!(runner.calls(), 1, "the plan ran nothing");
    assert!(plan.steps[0].cached);
    assert_eq!(plan.steps[0].state, "cached");
    assert_eq!(plan.budget.cached, 1);
    assert_eq!(
        plan.budget.estimated_latency_ms, 0,
        "a step that is in the cache costs nothing"
    );
}

// --- El producto: las recetas que trae ---

#[test]
fn the_product_publishes_its_five_recipes() {
    let book = RecipeBook::load(Path::new("."));

    for name in ["context", "onboard", "pre-change", "post-change", "review"] {
        let recipe = book
            .get(name)
            .unwrap_or_else(|| panic!("the product publishes the recipe `{name}`"));
        assert_eq!(recipe.origin, RecipeOrigin::Product);
        assert!(!recipe.steps.is_empty(), "the recipe `{name}` has no steps");
    }
}

#[test]
fn the_recipes_of_the_product_only_invoke_goals_that_read() {
    let book = RecipeBook::load(Path::new("."));

    for name in book.names() {
        let recipe = book.get(name).expect("the name is a recipe");
        for step in &recipe.steps {
            let goal = goals::find(&step.goal)
                .unwrap_or_else(|| panic!("the recipe `{name}` invokes an unknown goal"));
            assert_ne!(
                goal.authority,
                goals::GoalAuthority::Mutating,
                "the recipe `{name}` invokes the goal `{}`, which writes",
                step.goal
            );
        }
    }
}

#[test]
fn a_recipe_with_a_parameter_needs_that_parameter() {
    let dir = workspace();
    let cache = tempfile::tempdir().expect("tempdir");
    let cache_root = cache.path().join("cache");
    let runner = CountingRunner::new();
    let book = RecipeBook::load(dir.path());
    let recipe = book
        .get("pre-change")
        .expect("the product publishes the recipe `pre-change`");

    let outcome = {
        let executor = RecipeExecutor::new(dir.path(), Some(cache_root.as_path()), &runner);
        executor.execute(recipe, &params(&[]))
    };

    assert!(
        outcome.is_err(),
        "a recipe that needs `symbol` and does not get it says so: {outcome:?}"
    );
    let error = outcome.expect_err("the recipe cannot run without its parameter");
    assert!(
        error.contains("symbol"),
        "the error names the parameter that is missing: {error}"
    );
}

// --- La terminal ---

#[test]
fn the_plan_flag_is_written_without_its_value() {
    let goal = goals::find("recipe").expect("the registry declares the goal `recipe`");
    let matches = goals::clap_command(goal)
        .try_get_matches_from([goal.name, "--name", "onboard", "--plan"])
        .expect("the command reads its arguments");

    let values = goals::arguments_from_matches(goal, &matches);

    assert_eq!(
        values["plan"],
        json!(true),
        "the flag says what it says without a value: {values}"
    );
}

#[test]
fn the_terminal_publishes_the_parameters_as_an_object_like_the_wire() {
    let goal = goals::find("recipe").expect("the registry declares the goal `recipe`");
    let matches = goals::clap_command(goal)
        .try_get_matches_from([
            goal.name,
            "--name",
            "pre-change",
            "--params",
            "{\"symbol\":\"helper\"}",
        ])
        .expect("the command reads its arguments");

    let values = goals::arguments_from_matches(goal, &matches);

    assert_eq!(
        values["params"],
        json!({ "symbol": "helper" }),
        "the flag carries text, but what the goal reads is an object: {values}"
    );
}
