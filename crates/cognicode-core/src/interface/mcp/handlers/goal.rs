//! El handler de un goal: la declaración del registro, con el workspace de
//! la sesión.
//!
//! Un goal no es una tool con un brazo de su propio: es una declaración en
//! el registro ([`crate::interface::goals`]) que dice cómo se implementa.
//! Este handler es el puente —le da a la implementación el workspace de la
//! sesión y los argumentos del cable, y devuelve su respuesta—.
//!
//! La misma implementación es la que llama el CLI, así que el sobre que ve
//! un cliente MCP y el sobre que ve quien escribe en la terminal son el
//! mismo.

use super::{HandlerContext, HandlerError, HandlerResult};
use crate::interface::goals::GoalSpec;

/// Ejecuta un goal del registro y devuelve su respuesta.
///
/// TASK-82.
pub async fn handle_goal(
    ctx: &HandlerContext,
    goal: &GoalSpec,
    arguments: serde_json::Value,
) -> HandlerResult<serde_json::Value> {
    // El contexto de la sesión es el contexto del goal: el workspace que se
    // analiza y dónde guarda el producto lo que deriva de él.
    let context =
        crate::interface::goals::GoalEnvironment::new(ctx.working_dir(), ctx.cache_root());
    (goal.run)(&context, &arguments).map_err(HandlerError::InvalidInput)
}
