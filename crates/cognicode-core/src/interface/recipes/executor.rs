//! El ejecutor de una receta: qué se corre, en qué orden y qué se reutiliza.
//!
//! ## Las fases
//!
//! Cada goal declara la fase más temprana en la que ya puede responder
//! ([`crate::interface::goals::GoalPhase`]). La fase de un paso es la de su
//! goal, y las fases van en orden: `discover`, `index`, `link`, `analyze`.
//!
//! ## El paralelismo
//!
//! Los pasos de la **misma** fase son independientes: responden preguntas
//! distintas sobre el mismo estado del producto. Corren a la vez, en hilos
//! distintos. Los de fases distintas no: una fase posterior se apoya en lo que
//! la anterior dejó.
//!
//! ## El memo
//!
//! Antes de correr un paso, el ejecutor pregunta al memo si ya se calculó
//! para **este** árbol. Si sí, lo sirve; si no, lo corre y lo guarda. El memo
//! es lo que hace que la segunda corrida de una receta sea barata, y lo que
//! hace que el plan pueda decir qué está en caché.
//!
//! ## Un paso que falla
//!
//! Un paso que falla **no detiene** a los demás: los pasos de una fase son
//! independientes, y el fallo de uno no es el fallo de los otros. La receta
//! queda `partial`, dice qué paso falló y por qué, y el sobre publica el
//! enlace que la vuelve a correr.

use std::collections::BTreeMap;
use std::path::Path;
use std::time::Instant;

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use crate::interface::goals::{GoalEnvironment, GoalPhase};
use crate::interface::recipes::memo::MemoStore;
use crate::interface::recipes::{Recipe, substitute};

// --- Quién corre un goal ---

/// Alguien que puede correr un goal por su nombre.
///
/// La costura existe para que el ejecutor se pueda probar sin el producto:
/// un test mete un doble que cuenta, falla o espera, y el ejecutor no sabe la
/// diferencia. El runner del producto es el registro.
pub trait GoalRunner: Send + Sync {
    /// Corre el goal con ese nombre.
    fn run(
        &self,
        ctx: &GoalEnvironment<'_>,
        goal: &str,
        arguments: &Value,
    ) -> Result<Value, String>;
}

/// El runner del producto: el goal se busca en el registro y se llama.
#[derive(Debug, Clone, Copy, Default)]
pub struct RegistryRunner;

impl GoalRunner for RegistryRunner {
    fn run(
        &self,
        ctx: &GoalEnvironment<'_>,
        goal: &str,
        arguments: &Value,
    ) -> Result<Value, String> {
        let spec = crate::interface::goals::find(goal)
            .ok_or_else(|| format!("there is no goal named `{goal}`"))?;
        (spec.run)(ctx, arguments)
    }
}

// --- La respuesta ---

/// Lo que pasó con un paso.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct RecipeStepReport {
    /// El goal que este paso corre.
    pub goal: String,
    /// La fase en la que el goal responde.
    pub phase: String,
    /// Lo que pasó: `computed`, `cached`, `failed` o `planned`.
    pub state: String,
    /// Si el paso vino de la caché.
    pub cached: bool,
    /// Cuánto tardó el paso, en milisegundos.
    pub duration_ms: u64,
    /// La respuesta del goal, cuando la hubo.
    pub result: Option<Value>,
    /// Por qué falló el paso, cuando falló.
    pub error: Option<String>,
}

/// El paso que falló, con el motivo.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct RecipeFailure {
    /// El goal que falló.
    pub goal: String,
    /// Por qué falló.
    pub reason: String,
}

/// Lo que costó la receta.
#[derive(Debug, Clone, Default, Serialize, Deserialize, schemars::JsonSchema)]
pub struct RecipeBudget {
    /// Lo que costaría correr lo que no está en caché.
    ///
    /// Es la suma de las **estimaciones publicadas** de los goals que
    /// correrían: una estimación, no una medición. Lo que se midió va en
    /// `elapsed_ms`.
    pub estimated_latency_ms: u64,
    /// Cuántos pasos tiene la receta.
    pub steps: usize,
    /// Cuántos vinieron de la caché.
    pub cached: usize,
    /// Cuántos corrieron —o correrían, en un plan—.
    pub computed: usize,
    /// Cuántos fallaron.
    pub failed: usize,
    /// Cuánto tardó la receta, en milisegundos.
    pub elapsed_ms: u64,
    /// Cuántos memos no se pudieron escribir.
    ///
    /// El memo es una ayuda: si no se puede guardar, la respuesta sigue siendo
    /// cierta, y el número dice que la próxima corrida no la reutilizará.
    pub memo_errors: usize,
}

/// La respuesta del goal `recipe`.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct GoalRecipe {
    /// Una línea que resume la respuesta.
    pub summary: String,
    /// La versión del esquema de esta respuesta.
    pub schema_version: String,
    /// El goal que contesta.
    pub goal: String,
    /// La receta que se corrió.
    pub recipe: String,
    /// De dónde salió la receta: `product` o `workspace`.
    pub origin: String,
    /// El veredicto: `complete` o `partial`.
    pub status: String,
    /// Si esto es un plan y no una ejecución.
    pub planned: bool,
    /// Lo que pasó con cada paso.
    pub steps: Vec<RecipeStepReport>,
    /// El paso que falló, si alguno falló.
    pub failure: Option<RecipeFailure>,
    /// Lo que costó.
    pub budget: RecipeBudget,
}

// --- El ejecutor ---

/// Un paso con sus argumentos ya resueltos y su fase ya decidida.
struct ResolvedStep {
    goal: String,
    args: Map<String, Value>,
    phase: GoalPhase,
}

/// Lo que pasó con un paso.
struct StepOutcome {
    value: Option<Value>,
    error: Option<String>,
    duration_ms: u64,
    cached: bool,
    memo_error: bool,
}

/// Corre recetas.
pub struct RecipeExecutor<'a> {
    workspace: &'a Path,
    cache_root: Option<&'a Path>,
    runner: &'a dyn GoalRunner,
}

impl<'a> RecipeExecutor<'a> {
    /// El ejecutor de un workspace.
    pub fn new(
        workspace: &'a Path,
        cache_root: Option<&'a Path>,
        runner: &'a dyn GoalRunner,
    ) -> Self {
        Self {
            workspace,
            cache_root,
            runner,
        }
    }

    /// Dice qué correría, qué está en caché y lo que costaría, **sin correr
    /// nada**.
    pub fn plan(
        &self,
        recipe: &Recipe,
        params: &BTreeMap<String, String>,
    ) -> Result<GoalRecipe, String> {
        self.answer(recipe, params, true)
    }

    /// Corre la receta.
    pub fn execute(
        &self,
        recipe: &Recipe,
        params: &BTreeMap<String, String>,
    ) -> Result<GoalRecipe, String> {
        self.answer(recipe, params, false)
    }

    /// La respuesta: el plan o la ejecución.
    fn answer(
        &self,
        recipe: &Recipe,
        params: &BTreeMap<String, String>,
        planned: bool,
    ) -> Result<GoalRecipe, String> {
        let steps = self.resolved_steps(recipe, params)?;
        let tree = self.tree_digest()?;
        let store = self
            .cache_root
            .map(|root| MemoStore::new(root, self.workspace));

        let started = Instant::now();
        let mut reports: Vec<RecipeStepReport> = Vec::new();
        let mut failure: Option<RecipeFailure> = None;
        let mut budget = RecipeBudget {
            steps: steps.len(),
            ..Default::default()
        };

        for phase in GoalPhase::ALL {
            let in_phase: Vec<&ResolvedStep> =
                steps.iter().filter(|step| step.phase == phase).collect();
            if in_phase.is_empty() {
                continue;
            }

            if planned {
                for step in in_phase {
                    let cached = self.is_cached(step, &store, tree.as_deref());
                    if cached {
                        budget.cached += 1;
                    } else {
                        budget.computed += 1;
                        budget.estimated_latency_ms += estimated_latency(&step.goal);
                    }
                    reports.push(RecipeStepReport {
                        goal: step.goal.clone(),
                        phase: phase.as_str().to_string(),
                        state: if cached { "cached" } else { "planned" }.to_string(),
                        cached,
                        duration_ms: 0,
                        result: None,
                        error: None,
                    });
                }
                continue;
            }

            // Los pasos de la misma fase son independientes: corren a la vez.
            let outcomes: Vec<StepOutcome> = std::thread::scope(|scope| {
                let handles: Vec<_> = in_phase
                    .iter()
                    .map(|step| scope.spawn(|| self.run_step(step, &store, tree.as_deref())))
                    .collect();
                handles
                    .into_iter()
                    .map(|handle| {
                        handle.join().unwrap_or_else(|_| StepOutcome {
                            value: None,
                            error: Some("the step panicked".to_string()),
                            duration_ms: 0,
                            cached: false,
                            memo_error: false,
                        })
                    })
                    .collect()
            });

            for (step, outcome) in in_phase.iter().zip(outcomes.iter()) {
                if outcome.cached {
                    budget.cached += 1;
                } else {
                    budget.computed += 1;
                    budget.estimated_latency_ms += estimated_latency(&step.goal);
                }
                if outcome.memo_error {
                    budget.memo_errors += 1;
                }
                let state = if outcome.error.is_some() {
                    budget.failed += 1;
                    if failure.is_none() {
                        failure = Some(RecipeFailure {
                            goal: step.goal.clone(),
                            reason: outcome.error.clone().unwrap_or_default(),
                        });
                    }
                    "failed"
                } else if outcome.cached {
                    "cached"
                } else {
                    "computed"
                };
                reports.push(RecipeStepReport {
                    goal: step.goal.clone(),
                    phase: phase.as_str().to_string(),
                    state: state.to_string(),
                    cached: outcome.cached,
                    duration_ms: outcome.duration_ms,
                    result: outcome.value.clone(),
                    error: outcome.error.clone(),
                });
            }
        }

        budget.elapsed_ms = started.elapsed().as_millis() as u64;
        let status = if failure.is_some() {
            "partial"
        } else {
            "complete"
        };
        let summary = if planned {
            format!(
                "{} step(s) planned: {} would run, {} already in cache.",
                budget.steps, budget.computed, budget.cached
            )
        } else {
            format!(
                "{} step(s): {} computed, {} cached, {} failed.",
                budget.steps, budget.computed, budget.cached, budget.failed
            )
        };

        Ok(GoalRecipe {
            summary,
            schema_version: "cognicode.goal.recipe/v1".to_string(),
            goal: "recipe".to_string(),
            recipe: recipe.name.clone(),
            origin: recipe.origin.as_str().to_string(),
            status: status.to_string(),
            planned,
            steps: reports,
            failure,
            budget,
        })
    }

    /// Los pasos de la receta, con los parámetros resueltos y la fase de
    /// cada uno.
    ///
    /// Un parámetro declarado que no llega es un error **antes** de correr
    /// nada: una receta a la que le falta una mitad no se corre a medias.
    fn resolved_steps(
        &self,
        recipe: &Recipe,
        params: &BTreeMap<String, String>,
    ) -> Result<Vec<ResolvedStep>, String> {
        for parameter in &recipe.parameters {
            if !params.contains_key(parameter) {
                return Err(format!(
                    "the recipe `{}` needs the parameter `{parameter}`",
                    recipe.name
                ));
            }
        }

        recipe
            .steps
            .iter()
            .map(|step| {
                let spec = crate::interface::goals::find(&step.goal).ok_or_else(|| {
                    format!(
                        "the recipe `{}` invokes the goal `{}`, which this build does not declare",
                        recipe.name, step.goal
                    )
                })?;
                let args = match substitute(&Value::Object(step.args.clone()), params) {
                    Value::Object(args) => args,
                    _ => Map::new(),
                };
                Ok(ResolvedStep {
                    goal: step.goal.clone(),
                    args,
                    phase: spec.minimum_phase,
                })
            })
            .collect()
    }

    /// El hash del árbol sobre el que se corre.
    ///
    /// `None` cuando el producto no tiene dónde guardar lo que deriva: sin
    /// caché no hay memo, y el hash no hace falta para nada más.
    fn tree_digest(&self) -> Result<Option<String>, String> {
        if self.cache_root.is_none() {
            return Ok(None);
        }
        let service = crate::application::services::analysis_service::AnalysisService::new();
        let tree = service.tree_state(self.workspace);
        let manifest =
            crate::application::graph_snapshot::build_manifest(self.workspace, &tree.files);
        Ok(Some(crate::application::graph_snapshot::source_digest(
            &manifest,
        )))
    }

    /// Si el memo tiene lo que este paso necesita.
    fn is_cached(
        &self,
        step: &ResolvedStep,
        store: &Option<MemoStore>,
        tree: Option<&str>,
    ) -> bool {
        let Some(store) = store else { return false };
        let Some(tree) = tree else { return false };
        store.lookup(&step.goal, &step.args, tree).is_some()
    }

    /// Corre un paso: primero el memo, después el goal.
    fn run_step(
        &self,
        step: &ResolvedStep,
        store: &Option<MemoStore>,
        tree: Option<&str>,
    ) -> StepOutcome {
        if let Some(store) = store
            && let Some(tree) = tree
            && let Some(entry) = store.lookup(&step.goal, &step.args, tree)
        {
            return StepOutcome {
                value: Some(entry.value),
                error: None,
                duration_ms: entry.duration_ms,
                cached: true,
                memo_error: false,
            };
        }

        let context = GoalEnvironment::new(self.workspace, self.cache_root);
        let arguments = Value::Object(step.args.clone());
        let started = Instant::now();
        let result = self.runner.run(&context, &step.goal, &arguments);
        let duration_ms = started.elapsed().as_millis() as u64;

        let mut memo_error = false;
        match result {
            Ok(value) => {
                if let Some(store) = store
                    && let Some(tree) = tree
                    && store
                        .store(&step.goal, &step.args, tree, &value, duration_ms)
                        .is_err()
                {
                    memo_error = true;
                }
                StepOutcome {
                    value: Some(value),
                    error: None,
                    duration_ms,
                    cached: false,
                    memo_error,
                }
            }
            Err(error) => StepOutcome {
                value: None,
                error: Some(error),
                duration_ms,
                cached: false,
                memo_error: false,
            },
        }
    }
}

/// Lo que cuesta un goal, según lo que el producto publica.
///
/// Es la **estimación** del goal, no una medición: el número que el catálogo
/// publica para él. La medición de verdad va en `elapsed_ms`.
fn estimated_latency(goal: &str) -> u64 {
    match crate::interface::goals::find(goal) {
        Some(spec) => crate::interface::goals::mcp_descriptor(spec).estimated_latency_ms as u64,
        None => 0,
    }
}
