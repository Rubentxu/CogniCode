//! Las recetas: preguntas compuestas que el producto sabe responder.
//!
//! Un goal es **una** pregunta. Una receta son **varias**: los goals que hay
//! que correr, en el orden de sus fases, con lo que se puede reutilizar de la
//! corrida anterior. `onboard` no es un goal nuevo: es `status`, `context` y
//! `overview` en un orden que el producto conoce.
//!
//! ## De dónde salen las recetas
//!
//! | Origen | Dónde vive | Quién la escribe |
//! |---|---|---|
//! | el producto | `product/goals.toml`, compilado en el binario | el producto |
//! | el equipo | `<workspace>/.cognicode/goals.toml` | quien tiene el workspace |
//!
//! Las dos se leen con el mismo formato y se validan con el **mismo** validador
//! ([`validate`]). La receta del equipo con el mismo nombre sustituye a la del
//! producto, y la respuesta dice de dónde salió (`origin`): una respuesta que
//! no dice de dónde salió no dice qué se está ejecutando.
//!
//! ## Lo que una receta no puede hacer
//!
//! **Escribir.** Una receta sólo puede invocar goals de lectura, y el validador
//! lo dice con el nombre del goal que no puede invocar. La receta del equipo
//! es un fichero que trae el workspace: si pudiera escribir, un repositorio
//! cualquiera podría hacer que el producto escribiera en él al abrirlo.
//!
//! ## Los parámetros
//!
//! Una receta declara los parámetros que necesita (`symbol`, por ejemplo) y
//! sus pasos los usan con `{{symbol}}`. El valor lo da quien llama. Un
//! parámetro declarado que ningún paso usa es un parámetro muerto, y el
//! validador lo dice: una receta con un parámetro que nadie lee es una receta
//! que pide trabajo que no va a usar.

pub mod executor;
pub mod memo;

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use crate::interface::goals::GoalAuthority;

/// Las recetas del producto, compiladas en el binario.
///
/// El fichero vive en `product/`, junto al resto del catálogo del producto, y
/// se compila aquí: una receta publicada existe en el mismo binario que la
/// sirve, así que no hay una receta que el catálogo prometa y el binario no
/// sepa correr.
const PRODUCT_RECIPES: &str = include_str!("../../../../../product/goals.toml");

/// El fichero del workspace donde el equipo declara sus recetas.
const WORKSPACE_RECIPES: &str = ".cognicode/goals.toml";

// --- El modelo ---

/// De dónde sale una receta.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub enum RecipeOrigin {
    /// La receta la trae el producto, compilada en el binario.
    Product,
    /// La receta la trae el workspace, en su `.cognicode/goals.toml`.
    Workspace,
}

impl RecipeOrigin {
    /// La palabra que lo dice en las respuestas.
    pub fn as_str(self) -> &'static str {
        match self {
            RecipeOrigin::Product => "product",
            RecipeOrigin::Workspace => "workspace",
        }
    }
}

/// Un paso de una receta: un goal y los argumentos con los que se llama.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct RecipeStep {
    /// El goal que este paso corre.
    pub goal: String,
    /// Los argumentos del goal, con los `{{parámetro}}` ya resueltos.
    pub args: Map<String, Value>,
}

/// Una receta: un nombre, unos parámetros y unos pasos.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct Recipe {
    /// El nombre de la receta: el que se pide.
    pub name: String,
    /// Lo que la receta hace, en una frase.
    pub description: String,
    /// Los parámetros que la receta necesita.
    pub parameters: Vec<String>,
    /// Los pasos, en el orden en el que se declaran.
    pub steps: Vec<RecipeStep>,
    /// De dónde salió la receta.
    pub origin: RecipeOrigin,
}

/// Una receta que no se publica, con el motivo.
///
/// El motivo es la parte que importa: una receta que el producto no publica
/// sin decir por qué es una receta que parece no existir.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RefusedRecipe {
    /// El nombre de la receta recusada.
    pub name: String,
    /// Por qué no se publica.
    pub reason: String,
}

// --- El libro de recetas ---

/// Las recetas que este build conoce.
///
/// El libro tiene dos mitades: la del producto —compilada en el binario— y la
/// del workspace —el fichero que trae el repositorio—. Las dos se validan
/// igual, y la del workspace sustituye a la del producto cuando el nombre
/// coincide.
#[derive(Debug, Clone, Default)]
pub struct RecipeBook {
    recipes: Vec<Recipe>,
    refused: Vec<RefusedRecipe>,
}

impl RecipeBook {
    /// El libro de recetas de un workspace.
    ///
    /// No falla: una receta que no se puede leer se **recusa** con su motivo,
    /// y las demás siguen en el libro. Un fichero de recetas roto en el
    /// workspace no puede dejar al producto sin sus recetas.
    pub fn load(workspace: &Path) -> Self {
        let mut book = Self::default();
        book.absorb(parse_catalogue(
            PRODUCT_RECIPES,
            RecipeOrigin::Product,
            "product/goals.toml",
        ));
        let path = workspace.join(WORKSPACE_RECIPES);
        if path.is_file()
            && let Ok(text) = std::fs::read_to_string(&path)
        {
            book.absorb(parse_catalogue(
                &text,
                RecipeOrigin::Workspace,
                &path.to_string_lossy(),
            ));
        }
        book
    }

    /// La receta con ese nombre, si el libro la publica.
    pub fn get(&self, name: &str) -> Option<&Recipe> {
        self.recipes.iter().find(|recipe| recipe.name == name)
    }

    /// Los nombres de las recetas que el libro publica.
    pub fn names(&self) -> Vec<&str> {
        self.recipes
            .iter()
            .map(|recipe| recipe.name.as_str())
            .collect()
    }

    /// Las recetas que el libro no publica, con sus motivos.
    pub fn refused(&self) -> &[RefusedRecipe] {
        &self.refused
    }

    /// Añade las recetas de un origen: la del workspace sustituye a la del
    /// producto cuando el nombre coincide.
    fn absorb(&mut self, (recipes, refused): (Vec<Recipe>, Vec<RefusedRecipe>)) {
        for recipe in recipes {
            match self
                .recipes
                .iter_mut()
                .find(|existing| existing.name == recipe.name)
            {
                Some(existing) => *existing = recipe,
                None => self.recipes.push(recipe),
            }
        }
        self.refused.extend(refused);
    }
}

// --- El formato ---

/// El fichero de recetas, tal y como se lee.
#[derive(Debug, Default, Deserialize)]
struct Catalogue {
    #[serde(default)]
    recipe: Vec<CatalogueRecipe>,
}

/// Una receta del fichero.
#[derive(Debug, Deserialize)]
struct CatalogueRecipe {
    name: String,
    #[serde(default)]
    description: String,
    #[serde(default)]
    parameters: Vec<String>,
    #[serde(default)]
    steps: Vec<CatalogueStep>,
}

/// Un paso del fichero.
#[derive(Debug, Deserialize)]
struct CatalogueStep {
    goal: String,
    #[serde(default)]
    args: Map<String, Value>,
}

/// Lee un fichero de recetas y lo separa en las que se publican y las que se
/// recusan.
///
/// Un fichero que no se puede leer no es una receta recusada: es un fichero
/// recusado, y el motivo dice por qué. Las recetas que se pueden leer se
/// validan una a una, y cada una se publica o se recusa por su cuenta.
fn parse_catalogue(
    text: &str,
    origin: RecipeOrigin,
    source: &str,
) -> (Vec<Recipe>, Vec<RefusedRecipe>) {
    let catalogue: Catalogue = match toml::from_str(text) {
        Ok(catalogue) => catalogue,
        Err(error) => {
            return (
                Vec::new(),
                vec![RefusedRecipe {
                    name: source.to_string(),
                    reason: format!("the recipes of `{source}` cannot be read: {error}"),
                }],
            );
        }
    };

    let mut recipes = Vec::new();
    let mut refused = Vec::new();
    for entry in catalogue.recipe {
        let recipe = Recipe {
            name: entry.name.clone(),
            description: entry.description,
            parameters: entry.parameters,
            steps: entry
                .steps
                .into_iter()
                .map(|step| RecipeStep {
                    goal: step.goal,
                    args: step.args,
                })
                .collect(),
            origin,
        };
        match validate(&recipe) {
            Ok(()) => recipes.push(recipe),
            Err(reason) => refused.push(RefusedRecipe {
                name: recipe.name,
                reason,
            }),
        }
    }
    (recipes, refused)
}

// --- El validador ---

/// Dice si una receta se puede publicar, y por qué no si no.
///
/// El validador es **uno**, y las dos mitades del libro pasan por él: la
/// receta del producto y la del equipo se miden con la misma regla. Las
/// reglas son:
///
/// 1. la receta tiene nombre y tiene pasos;
/// 2. todo paso nombra un goal que este build declara;
/// 3. todo goal de un paso **lee** —una receta no escribe—;
/// 4. una receta no corre otra receta;
/// 5. todo `{{parámetro}}` nombra un parámetro declarado;
/// 6. todo parámetro declarado lo usa algún paso.
pub fn validate(recipe: &Recipe) -> Result<(), String> {
    if recipe.name.trim().is_empty() {
        return Err("the recipe has no name".to_string());
    }
    if recipe.steps.is_empty() {
        return Err(format!("the recipe `{}` has no steps", recipe.name));
    }

    let declared: BTreeSet<&str> = recipe.parameters.iter().map(String::as_str).collect();
    let mut used: BTreeSet<String> = BTreeSet::new();

    for step in &recipe.steps {
        let goal = crate::interface::goals::find(&step.goal).ok_or_else(|| {
            format!(
                "the recipe `{}` invokes the goal `{}`, which this build does not declare",
                recipe.name, step.goal
            )
        })?;
        if goal.name == "recipe" {
            return Err(format!(
                "the recipe `{}` invokes the goal `recipe`: a recipe cannot run another recipe",
                recipe.name
            ));
        }
        if goal.authority != GoalAuthority::Read {
            return Err(format!(
                "the recipe `{}` invokes the goal `{}`, which writes: a recipe only runs goals that read",
                recipe.name, step.goal
            ));
        }

        let mut named = BTreeSet::new();
        collect_placeholders(&Value::Object(step.args.clone()), &mut named);
        for parameter in &named {
            if !declared.contains(parameter.as_str()) {
                return Err(format!(
                    "the recipe `{}` uses the parameter `{{{parameter}}}`, which it does not declare",
                    recipe.name
                ));
            }
            used.insert(parameter.clone());
        }
    }

    for parameter in &recipe.parameters {
        if !used.contains(parameter) {
            return Err(format!(
                "the recipe `{}` declares the parameter `{parameter}` and no step uses it",
                recipe.name
            ));
        }
    }

    Ok(())
}

// --- Los parámetros ---

/// Los `{{parámetro}}` que hay en un valor.
fn collect_placeholders(value: &Value, out: &mut BTreeSet<String>) {
    match value {
        Value::String(text) => {
            for name in placeholders_in(text) {
                out.insert(name);
            }
        }
        Value::Array(items) => {
            for item in items {
                collect_placeholders(item, out);
            }
        }
        Value::Object(map) => {
            for item in map.values() {
                collect_placeholders(item, out);
            }
        }
        _ => {}
    }
}

/// Los `{{parámetro}}` que hay en un texto.
fn placeholders_in(text: &str) -> Vec<String> {
    let mut names = Vec::new();
    let mut rest = text;
    while let Some(start) = rest.find("{{") {
        let after = &rest[start + 2..];
        match after.find("}}") {
            Some(end) => {
                names.push(after[..end].to_string());
                rest = &after[end + 2..];
            }
            None => break,
        }
    }
    names
}

/// Sustituye los `{{parámetro}}` de un valor por los valores dados.
pub fn substitute(value: &Value, params: &BTreeMap<String, String>) -> Value {
    match value {
        Value::String(text) => Value::String(text.replace_placeholders(params)),
        Value::Array(items) => {
            Value::Array(items.iter().map(|item| substitute(item, params)).collect())
        }
        Value::Object(map) => Value::Object(
            map.iter()
                .map(|(name, item)| (name.clone(), substitute(item, params)))
                .collect(),
        ),
        other => other.clone(),
    }
}

/// Sustituye los `{{parámetro}}` de un texto por los valores dados.
trait ReplacePlaceholders {
    fn replace_placeholders(&self, params: &BTreeMap<String, String>) -> String;
}

impl ReplacePlaceholders for String {
    fn replace_placeholders(&self, params: &BTreeMap<String, String>) -> String {
        let mut out = String::new();
        let mut rest = self.as_str();
        while let Some(start) = rest.find("{{") {
            out.push_str(&rest[..start]);
            let after = &rest[start + 2..];
            match after.find("}}") {
                Some(end) => {
                    let name = &after[..end];
                    out.push_str(params.get(name).map(String::as_str).unwrap_or_default());
                    rest = &after[end + 2..];
                }
                None => {
                    out.push_str(&rest[start..]);
                    rest = "";
                }
            }
        }
        out.push_str(rest);
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn recipe_with(steps: Vec<RecipeStep>, parameters: Vec<String>) -> Recipe {
        Recipe {
            name: "prueba".to_string(),
            description: "Una receta de prueba.".to_string(),
            parameters,
            steps,
            origin: RecipeOrigin::Product,
        }
    }

    fn step(goal: &str, args: Map<String, Value>) -> RecipeStep {
        RecipeStep {
            goal: goal.to_string(),
            args,
        }
    }

    #[test]
    fn a_recipe_that_invokes_a_goal_which_writes_is_refused() {
        let recipe = recipe_with(
            vec![step(
                "change",
                Map::from_iter([("symbol".to_string(), json!("x"))]),
            )],
            Vec::new(),
        );

        let reason = validate(&recipe).expect_err("the recipe is refused");

        assert!(
            reason.contains("change"),
            "the reason names the goal that writes: {reason}"
        );
    }

    #[test]
    fn a_recipe_that_invokes_a_goal_which_does_not_exist_is_refused() {
        let recipe = recipe_with(vec![step("nope", Map::new())], Vec::new());

        let reason = validate(&recipe).expect_err("the recipe is refused");

        assert!(
            reason.contains("nope"),
            "the reason names the goal that does not exist: {reason}"
        );
    }

    #[test]
    fn a_recipe_that_cannot_run_another_recipe_is_refused() {
        let recipe = recipe_with(vec![step("recipe", Map::new())], Vec::new());

        let reason = validate(&recipe).expect_err("the recipe is refused");

        assert!(
            reason.contains("another recipe"),
            "the reason says why: {reason}"
        );
    }

    #[test]
    fn a_recipe_that_uses_a_parameter_it_does_not_declare_is_refused() {
        let recipe = recipe_with(
            vec![step(
                "find",
                Map::from_iter([("symbol".to_string(), json!("{{symbol}}"))]),
            )],
            Vec::new(),
        );

        let reason = validate(&recipe).expect_err("the recipe is refused");

        assert!(
            reason.contains("symbol"),
            "the reason names the parameter: {reason}"
        );
    }

    #[test]
    fn a_recipe_with_a_parameter_nobody_uses_is_refused() {
        let recipe = recipe_with(vec![step("status", Map::new())], vec!["symbol".to_string()]);

        let reason = validate(&recipe).expect_err("the recipe is refused");

        assert!(
            reason.contains("symbol"),
            "the reason names the dead parameter: {reason}"
        );
    }

    #[test]
    fn a_recipe_that_only_reads_is_valid() {
        let recipe = recipe_with(
            vec![
                step("status", Map::new()),
                step(
                    "find",
                    Map::from_iter([("symbol".to_string(), json!("{{symbol}}"))]),
                ),
            ],
            vec!["symbol".to_string()],
        );

        validate(&recipe).expect("the recipe is valid");
    }

    #[test]
    fn the_parameters_are_substituted_in_the_steps() {
        let params = params_of(&[("symbol", "helper")]);
        let value = json!({ "symbol": "{{symbol}}", "note": "about {{symbol}}" });

        let substituted = substitute(&value, &params);

        assert_eq!(substituted["symbol"], json!("helper"));
        assert_eq!(substituted["note"], json!("about helper"));
    }

    fn params_of(pairs: &[(&str, &str)]) -> BTreeMap<String, String> {
        pairs
            .iter()
            .map(|(name, value)| (name.to_string(), value.to_string()))
            .collect()
    }
}
