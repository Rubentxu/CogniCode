//! La proyección de una superficie a la lista que el cable anuncia.
//!
//! La superficie decide **qué se anuncia**, no qué se calcula: un goal del
//! perfil por defecto y el mismo goal del perfil `full` contestan lo mismo.
//! Lo que cambia entre perfiles es la lista que `tools/list` devuelve.
//!
//! - el perfil `default` publica los goals que responden antes del análisis
//!   completo;
//! - el perfil `full` publica todo: los goals y las tools;
//! - el perfil `legacy` publica las tools con el aviso que nombra al goal
//!   que las sustituye.
//!
//! Vive fuera del adaptador a propósito: el adaptador es el que **sirve** la
//! lista, y este módulo es el que la **decide**. Un fichero que crece es un
//! fichero que hay que partir, y partirlo por una frontera que ya existe —la
//! decisión de la superficie— sale mejor que partirlo por un número de líneas.

use rmcp::model::Tool;

use crate::rmcp_adapter::{build_all_tools, tool_is_mutating};
use cognicode_core::interface::surface::{self, SurfaceProfile};

/// Las tools que una superficie publica.
pub fn tools_for_profile(profile: SurfaceProfile) -> Vec<Tool> {
    let all = build_all_tools();
    match profile {
        SurfaceProfile::Full => all,
        SurfaceProfile::Default => {
            let goals = profile.goals();
            all.into_iter()
                .filter(|tool| goals.iter().any(|goal| goal.name == tool.name.as_ref()))
                .collect()
        }
        SurfaceProfile::Legacy => {
            let goals = surface::goal_names();
            all.into_iter()
                .filter(|tool| !goals.contains(&tool.name.as_ref()))
                .map(with_deprecation_notice)
                .collect()
        }
    }
}

/// Las tools que el servidor anuncia, con su postura.
///
/// La postura se cruza con la superficie: un servidor de sólo lectura no
/// anuncia las tools que escriben, y el perfil decide cuáles se anuncian.
/// El filtro vive aquí —y no en el `list`— para que se pueda comprobar sin
/// levantar un servidor.
pub fn visible_tools(profile: SurfaceProfile, read_only: bool) -> Vec<Tool> {
    tools_for_profile(profile)
        .into_iter()
        .filter(|tool| !read_only || !tool_is_mutating(&tool.name))
        .collect()
}

/// El aviso que lleva un nombre legado.
///
/// El nombre sigue funcionando y el aviso dice con qué se sustituye: un
/// nombre publicado que desaparece sin un aviso rompe a quien lo usa.
fn with_deprecation_notice(mut tool: Tool) -> Tool {
    let notice = surface::deprecation_notice(&tool.name);
    tool.description = Some(std::borrow::Cow::Owned(format!(
        "{}{notice}",
        tool.description.as_deref().unwrap_or_default()
    )));
    tool
}
