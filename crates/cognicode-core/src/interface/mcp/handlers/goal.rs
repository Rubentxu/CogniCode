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
use crate::interface::goals::{GoalEnvironment, GoalSpec};
use std::path::Path;

/// Ejecuta un goal del registro y devuelve su respuesta.
///
/// La implementación de un goal es síncrona y puede bloquear (Git, disco,
/// grafo). Se ejecuta en el pool de bloqueo y no en el worker asíncrono:
/// dentro del worker, el plazo del despacho no puede observarse hasta que el
/// trabajo termina, así que un goal lento respondía tarde y con su resultado
/// como si la ventana se hubiera respetado. Fuera del worker, el plazo corta
/// la llamada; la tarea del pool no se aborta y su resultado se descarta, que
/// es lo que la respuesta de timeout ya declara.
///
/// TASK-82.
pub async fn handle_goal(
    ctx: &HandlerContext,
    goal: &GoalSpec,
    arguments: serde_json::Value,
) -> HandlerResult<serde_json::Value> {
    // El contexto de la sesión es el contexto del goal: el workspace que se
    // analiza y dónde guarda el producto lo que deriva de él.
    let workspace = ctx.working_dir().to_path_buf();
    let cache_root = ctx.cache_root().map(Path::to_path_buf);
    let run = goal.run;
    tokio::task::spawn_blocking(move || {
        let context = GoalEnvironment::new(&workspace, cache_root.as_deref());
        run(&context, &arguments)
    })
    .await
    .map_err(|error| HandlerError::Internal(format!("the goal task did not finish: {error}")))?
    .map_err(HandlerError::InvalidInput)
}
