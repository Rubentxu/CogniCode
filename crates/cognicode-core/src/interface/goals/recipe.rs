//! El goal `recipe`: una pregunta compuesta que el producto sabe responder.
//!
//! El goal no corre los pasos: los pide al ejecutor
//! ([`crate::interface::recipes::executor`]), que es quien sabe qué se
//! memoiza, en qué fase va cada paso y cuál corre en paralelo. El goal sólo
//! lee sus argumentos, busca la receta y devuelve la respuesta.
//!
//! ## Los parámetros
//!
//! Los parámetros de una receta viajan en `params`, un objeto: el esquema del
//! goal es el mismo para todas las recetas, y el de una receta depende de lo
//! que esa receta declare. Un parámetro que no existe en la receta se ignora;
//! uno que la receta declara y no llega es un error, y el error lo dice antes
//! de correr nada.
//!
//! ## `--plan`
//!
//! Con `plan: true` el goal **no corre ningún goal**: dice qué correría, qué
//! está en la caché y lo que costaría. La terminal lo pide con `--plan`, y el
//! cable con `plan: true`; el flag y el campo son el mismo argumento.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::interface::goals::GoalEnvironment;
use crate::interface::recipes::RecipeBook;
use crate::interface::recipes::executor::{GoalRecipe, RecipeExecutor, RegistryRunner};

#[derive(Debug, Clone, Default, Deserialize, Serialize, schemars::JsonSchema)]
/// Input for the `recipe` goal
pub struct RecipeInput {
    /// The name of the recipe to run
    pub name: String,
    /// Say what would run, what is in the cache and what it would cost, without running it
    #[serde(default)]
    pub plan: bool,
    /// The values of the recipe's parameters
    #[serde(default)]
    pub params: BTreeMap<String, String>,
}

/// La implementación del goal: lee sus argumentos y devuelve su respuesta.
pub fn run(ctx: &GoalEnvironment, arguments: &Value) -> Result<Value, String> {
    let arguments = normalised(arguments)?;
    let input: RecipeInput = serde_json::from_value(arguments.clone())
        .map_err(|error| format!("the arguments of the goal `recipe` are not valid: {error}"))?;

    let book = RecipeBook::load(ctx.workspace);
    let recipe = book.get(&input.name).ok_or_else(|| {
        match book
            .refused()
            .iter()
            .find(|refusal| refusal.name == input.name)
        {
            Some(refusal) => format!(
                "the recipe `{}` is not available: {}",
                input.name, refusal.reason
            ),
            None => format!(
                "there is no recipe named `{}`; the ones there are: {}",
                input.name,
                book.names().join(", ")
            ),
        }
    })?;

    let runner = RegistryRunner;
    let executor = RecipeExecutor::new(ctx.workspace, ctx.cache_root, &runner);
    let answer = if input.plan {
        executor.plan(recipe, &input.params)?
    } else {
        executor.execute(recipe, &input.params)?
    };

    serde_json::to_value(answer)
        .map_err(|error| format!("the answer of the goal `recipe` is not serialisable: {error}"))
}

/// La respuesta del goal `recipe` en texto.
pub fn render(answer: &Value) -> String {
    let answer: GoalRecipe = serde_json::from_value(answer.clone())
        .expect("the answer of the goal `recipe` is the answer its own type describes");
    let mut text = String::new();
    text.push_str(&answer.summary);
    text.push('\n');
    text.push_str(&format!(
        "  recipe:  {} ({})\n",
        answer.recipe, answer.origin
    ));
    for step in &answer.steps {
        text.push_str(&format!(
            "  - {} [{}] {}\n",
            step.goal, step.phase, step.state
        ));
    }
    text.push_str(&format!(
        "  budget:  ~{} ms estimated, {} ms elapsed\n",
        answer.budget.estimated_latency_ms, answer.budget.elapsed_ms
    ));
    if let Some(failure) = &answer.failure {
        text.push_str(&format!(
            "  failed:  {} — {}\n",
            failure.goal, failure.reason
        ));
    }
    text
}

/// Los argumentos del goal, con `params` en la forma que el struct lee.
///
/// El cable manda un objeto; la terminal manda el **texto** JSON del objeto,
/// porque un flag de la terminal lleva texto. Las dos formas se leen aquí y
/// salen en la misma: el struct que lee los argumentos es el mismo, y el
/// error de un JSON que no se puede leer lo dice aquí y no en el struct.
fn normalised(arguments: &Value) -> Result<Value, String> {
    let Some(object) = arguments.as_object() else {
        return Ok(arguments.clone());
    };
    let Some(Value::String(text)) = object.get("params") else {
        return Ok(arguments.clone());
    };
    let parsed: Value = serde_json::from_str(text).map_err(|error| {
        format!("the `params` of the goal `recipe` are not valid JSON: {error}")
    })?;
    let mut object = object.clone();
    object.insert("params".to_string(), parsed);
    Ok(Value::Object(object))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_params_can_arrive_as_an_object_or_as_the_text_of_one() {
        let as_object = serde_json::json!({ "name": "x", "params": { "symbol": "helper" } });
        let as_text = serde_json::json!({ "name": "x", "params": "{\"symbol\":\"helper\"}" });

        let one = normalised(&as_object).expect("the object is read");
        let two = normalised(&as_text).expect("the text is read");

        assert_eq!(one, two);
        assert_eq!(one["params"]["symbol"], serde_json::json!("helper"));
    }

    #[test]
    fn the_params_that_are_not_json_say_so() {
        let broken = serde_json::json!({ "name": "x", "params": "{not json" });

        let error = normalised(&broken).expect_err("the broken text is refused");

        assert!(
            error.contains("not valid JSON"),
            "the error says what is wrong: {error}"
        );
    }
}
