//! La superficie que el servidor publica: los perfiles del catálogo.
//!
//! ## Qué es un perfil
//!
//! Un perfil decide **qué se anuncia** en `tools/list`. No decide qué se
//! calcula: un goal del perfil por defecto y el mismo goal del perfil `full`
//! contestan lo mismo, porque la implementación es la misma.
//!
//! | Perfil | Qué publica |
//! |---|---|
//! | `default` | los goals que responden antes del análisis completo |
//! | `full` | todos los goals y todas las tools |
//! | `legacy` | las tools, como alias, con el aviso que nombra al goal que las sustituye |
//!
//! ## La regla del perfil por defecto
//!
//! La fase mínima del goal. El perfil por defecto publica los goals que
//! responden en las fases de descubrir, indexar y unir, y deja para el
//! perfil `full` los que necesitan el análisis completo —y el que escribe—.
//! La regla tiene un dueño, la fase, y el perfil la deriva: un goal nuevo
//! que declare su fase sabe si sale por defecto o no.
//!
//! ## El perfil `legacy`
//!
//! Las tools que el producto publicó antes de los goals. Se siguen
//! anunciando —un nombre publicado que desaparece sin un aviso rompe a quien
//! lo usa— pero cada una lleva el aviso que dice qué goal contesta la misma
//! pregunta. El aviso lo deriva el goal que declara `replaces`, así que el
//! sustituto de un nombre tiene un solo dueño: el goal.

use crate::interface::goals::{GoalPhase, GoalSpec, registry};

/// La superficie que el servidor publica.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SurfaceProfile {
    /// Los goals que responden antes del análisis completo.
    Default,
    /// Todos los goals y todas las tools.
    Full,
    /// Las tools, con el aviso de deprecación.
    Legacy,
}

impl SurfaceProfile {
    /// Los perfiles que el servidor conoce.
    pub const ALL: [SurfaceProfile; 3] = [
        SurfaceProfile::Default,
        SurfaceProfile::Full,
        SurfaceProfile::Legacy,
    ];

    /// El nombre del perfil, tal y como se escribe en la línea de órdenes.
    pub fn as_str(self) -> &'static str {
        match self {
            SurfaceProfile::Default => "default",
            SurfaceProfile::Full => "full",
            SurfaceProfile::Legacy => "legacy",
        }
    }

    /// El perfil con ese nombre, si el servidor lo conoce.
    pub fn parse(name: &str) -> Option<Self> {
        match name {
            "default" => Some(SurfaceProfile::Default),
            "full" => Some(SurfaceProfile::Full),
            "legacy" => Some(SurfaceProfile::Legacy),
            _ => None,
        }
    }

    /// Los perfiles que el servidor conoce, para la ayuda.
    pub fn names() -> Vec<&'static str> {
        SurfaceProfile::ALL
            .iter()
            .map(|profile| profile.as_str())
            .collect()
    }

    /// Si este perfil publica los goals.
    pub fn publishes_goals(self) -> bool {
        self != SurfaceProfile::Legacy
    }

    /// Si este perfil publica las tools.
    pub fn publishes_tools(self) -> bool {
        self != SurfaceProfile::Default
    }

    /// Los goals que este perfil publica.
    pub fn goals(self) -> Vec<&'static GoalSpec> {
        if !self.publishes_goals() {
            return Vec::new();
        }
        registry()
            .iter()
            .filter(|goal| self.publishes_goal(goal))
            .collect()
    }

    /// Si este perfil publica ese goal.
    pub fn publishes_goal(self, goal: &GoalSpec) -> bool {
        match self {
            // La fase mínima dice cuándo puede el goal responder: el
            // perfil por defecto publica los que responden antes del
            // análisis completo.
            SurfaceProfile::Default => goal.minimum_phase < GoalPhase::Analyze,
            SurfaceProfile::Full => true,
            SurfaceProfile::Legacy => false,
        }
    }
}

/// Los nombres de los goals que este build declara.
pub fn goal_names() -> Vec<&'static str> {
    registry().iter().map(|goal| goal.name).collect()
}

/// El goal que sustituye a un nombre legado, si lo hay.
///
/// El sustituto lo declara el goal: el perfil no lo deduce, lo lee.
pub fn goal_replacing(tool_name: &str) -> Option<&'static GoalSpec> {
    registry()
        .iter()
        .find(|goal| goal.replaces.contains(&tool_name))
}

/// El aviso que un nombre legado lleva cuando se publica como alias.
///
/// El aviso dice qué goal contesta la misma pregunta. Cuando ningún goal la
/// contesta, el aviso lo dice: un nombre sin sustituto que se anuncia como
/// si lo tuviera sería una promesa que el producto no cumple.
pub fn deprecation_notice(tool_name: &str) -> String {
    match goal_replacing(tool_name) {
        Some(goal) => format!(
            " Deprecated: the goal `{}` answers this question.",
            goal.name
        ),
        None => " Deprecated: the goal surface does not answer this question yet; \
this tool still computes it."
            .to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_default_profile_publishes_the_goals_that_answer_before_the_analysis() {
        let names: Vec<&str> = SurfaceProfile::Default
            .goals()
            .iter()
            .map(|goal| goal.name)
            .collect();

        assert_eq!(
            names,
            vec!["status", "context", "find", "inspect", "trace"],
            "the default profile publishes the five goals that answer before the analysis"
        );
    }

    #[test]
    fn the_full_profile_publishes_every_goal() {
        assert_eq!(SurfaceProfile::Full.goals().len(), registry().len());
    }

    #[test]
    fn the_legacy_profile_publishes_no_goal() {
        assert!(SurfaceProfile::Legacy.goals().is_empty());
        assert!(SurfaceProfile::Legacy.publishes_tools());
        assert!(!SurfaceProfile::Default.publishes_tools());
    }

    #[test]
    fn a_name_with_a_goal_says_which_one() {
        let notice = deprecation_notice("analyze_impact");

        assert!(
            notice.contains("impact"),
            "the notice names the goal that answers it: {notice}"
        );
    }

    #[test]
    fn a_name_without_a_goal_says_so() {
        let notice = deprecation_notice("read_file");

        assert!(
            notice.contains("does not answer this question yet"),
            "a name without a substitute says so: {notice}"
        );
    }
}
