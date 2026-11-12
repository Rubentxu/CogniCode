//! `cognicode-core::refactor::proposal` — lo que el preview emite y
//! persiste (TASK-220).
//!
//! La guarda «el workspace cambió desde el preview» solo puede saltar
//! si el digest que `apply` comprueba fue calculado EN el momento del
//! preview y viajó hasta el apply sin recalcularse. Lo mismo vale para
//! la ventana del token: si el CLI acuñara el token en el momento del
//! apply con `Utc::now()`, ningún token caducaría nunca.
//!
//! [`RefactorProposal`] es ese registro: el plan al que pertenece (por
//! id y digest), el estado del workspace que el preview vio, y el
//! token acuñado al emitirla. El CLI la persiste junto al plan en el
//! preview y la lee en el apply; `apply` sigue recalculando el estado
//! ACTUAL y comparándolo con el que trae la proposal — eso sí es la
//! guarda, no una tautología.

use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::path::Path;
use uuid::Uuid;

use super::apply::{ApplyError, AuthorizationToken, compute_workspace_state_digest};
use super::plan::{PlanIoError, PlanIoResult, RefactorPlan};

/// Versión del esquema del documento persistido. Cambiarla es un
/// contrato publicado: exige compatibilidad o deprecación explícita.
pub const PROPOSAL_SCHEMA_VERSION: &str = "cognicode.refactor.proposal/v1";

/// La proposal del preview: el plan que propone, el estado que vio y
/// el consentimiento que se acuñó para ella.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RefactorProposal {
    pub schema_version: String,
    pub plan_id: Uuid,
    pub plan_digest: String,
    pub workspace_state_digest: String,
    pub token: AuthorizationToken,
}

impl RefactorProposal {
    /// Emite la proposal de un plan. El digest de estado es el mismo
    /// que publica `render_preview` (misma función, mismo algoritmo) y
    /// el token se acuña en el momento de emisión: su ventana de una
    /// hora corre desde aquí, no desde el apply.
    pub fn issue(plan: &RefactorPlan) -> Self {
        Self {
            schema_version: PROPOSAL_SCHEMA_VERSION.to_string(),
            plan_id: plan.id,
            plan_digest: plan.digest.clone(),
            workspace_state_digest: compute_workspace_state_digest(plan),
            token: AuthorizationToken {
                value: format!("cgt-{}", Uuid::new_v4()),
                created_at: Utc::now(),
            },
        }
    }

    /// La proposal habla de ESTE plan: mismo id y mismo digest. Una
    /// proposal de otro plan — o de una versión anterior del mismo —
    /// no autoriza nada.
    pub fn matches_plan(&self, plan: &RefactorPlan) -> bool {
        self.plan_id == plan.id && self.plan_digest == plan.digest
    }

    /// El token que `apply` comprobará: el creado por esta proposal,
    /// con su `created_at`, y el valor que el operador devolvió por el
    /// canal separado. Si el valor no es el emitido, no es el
    /// consentimiento de esta proposal.
    pub fn authorization_token(
        &self,
        out_of_band_value: &str,
    ) -> Result<AuthorizationToken, ApplyError> {
        if out_of_band_value != self.token.value {
            return Err(ApplyError::AuthorizationInvalid);
        }
        Ok(AuthorizationToken {
            value: self.token.value.clone(),
            created_at: self.token.created_at,
        })
    }
}

/// Persiste la proposal junto al plan: el apply la leerá de ahí.
pub fn save_proposal_to_file(proposal: &RefactorProposal, path: &Path) -> PlanIoResult<()> {
    let bytes = serde_json::to_vec_pretty(proposal).map_err(PlanIoError::Parse)?;
    std::fs::write(path, bytes).map_err(PlanIoError::Io)?;
    Ok(())
}

/// Carga la proposal persistida junto al plan.
pub fn load_proposal_from_file(path: &Path) -> PlanIoResult<RefactorProposal> {
    let bytes = std::fs::read(path).map_err(PlanIoError::Io)?;
    if bytes.is_empty() {
        return Err(PlanIoError::Empty);
    }
    serde_json::from_slice(&bytes).map_err(PlanIoError::Parse)
}

/// La ruta convencional de la proposal de un plan: junto a él, con el
/// mismo nombre. `plan.json` -> `plan.proposal.json`.
pub fn proposal_path_for(plan_path: &Path) -> std::path::PathBuf {
    plan_path.with_extension("proposal.json")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::application::analysis_basis::AnalysisBasis;
    use crate::refactor::plan::{PlannedEdit, build_plan};
    use tempfile::TempDir;

    fn plan_for(tmp: &TempDir) -> RefactorPlan {
        let file = tmp.path().join("a.rs");
        std::fs::write(&file, "hello world\n").expect("write");
        build_plan(
            tmp.path().to_path_buf(),
            AnalysisBasis::for_workspace(tmp.path().to_path_buf()),
            vec![file.clone()],
            vec![PlannedEdit {
                file,
                byte_range: 6..11,
                replacement: "safe".to_string(),
                deterministic: true,
            }],
            vec![],
            vec![],
        )
        .expect("build plan")
    }

    /// El digest de la proposal es el mismo que publica el preview:
    /// misma función, mismo algoritmo, byte a byte.
    #[test]
    fn the_proposal_digest_is_the_one_the_preview_publishes() {
        let tmp = TempDir::new().unwrap();
        let plan = plan_for(&tmp);
        let proposal = RefactorProposal::issue(&plan);

        let preview =
            crate::refactor::preview::render_preview(&plan, tmp.path()).expect("render preview");
        assert_eq!(
            proposal.workspace_state_digest,
            preview.workspace_state_digest
        );
    }

    /// Roundtrip de disco: lo que se persiste es lo que se lee.
    #[test]
    fn the_proposal_roundtrips_through_the_file() {
        let tmp = TempDir::new().unwrap();
        let plan = plan_for(&tmp);
        let proposal = RefactorProposal::issue(&plan);

        let path = tmp.path().join("plan.proposal.json");
        save_proposal_to_file(&proposal, &path).expect("save");
        let loaded = load_proposal_from_file(&path).expect("load");
        assert_eq!(loaded, proposal);
    }

    /// Una proposal de otro plan no autoriza nada.
    #[test]
    fn a_proposal_from_another_plan_does_not_match() {
        let tmp = TempDir::new().unwrap();
        let plan = plan_for(&tmp);
        let proposal = RefactorProposal::issue(&plan);

        let other = plan_for(&tmp);
        assert!(!proposal.matches_plan(&other));
        assert!(proposal.matches_plan(&plan));
    }

    /// El token del apply es el de la proposal: el valor tiene que ser
    /// el emitido, y el `created_at` el de la emisión, no el del apply.
    #[test]
    fn the_token_applies_is_the_one_the_proposal_issued() {
        let tmp = TempDir::new().unwrap();
        let plan = plan_for(&tmp);
        let proposal = RefactorProposal::issue(&plan);

        let wrong = proposal.authorization_token("otro-token");
        assert!(matches!(wrong, Err(ApplyError::AuthorizationInvalid)));

        let right = proposal
            .authorization_token(&proposal.token.value)
            .expect("the issued token is accepted");
        assert_eq!(right.created_at, proposal.token.created_at);
        assert_eq!(right.value, proposal.token.value);
    }
}
