//! Behavior definitions and admission (M7.3, cycle e64, ADR-044).
//!
//! ## Why admission exists at all
//!
//! A behavior must not declare its own class:
//!
//! ```text
//! runtime.run(BehaviorDefinition { class: PureDerivation, .. })   ← an agent
//!                                                                   could declare
//!                                                                   itself trusted
//! ```
//!
//! So the class a behavior *runs under* is decided by
//! [`BehaviorAdmission`] and travels in a sealed [`BehaviorPermit`], exactly as
//! `ExecutionPermit` does for detectors. The permit has private fields, a
//! private seal, no public constructor and no `Serialize`: it cannot be forged
//! and it cannot be restored from storage into authority by accident.
//!
//! ## The downgrade rule
//!
//! ```text
//! Builtin | HumanCurated  →  keep the declared class
//! AiGenerated | Imported  →  effective class = AgentBehavior
//! ```
//!
//! An AI-proposed behavior that declares `PureDerivation` does not error; it runs
//! as `AgentBehavior` and therefore cannot commit canonical facts. This mirrors
//! the M6 rule "an AI detector is a `Candidate`", and it is the same principle:
//! **a declaration never escalates**. A behavior that legitimately needs to
//! commit facts can only get there by being curated by a human, which is a
//! decision, not a claim.

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

use super::class::{BehaviorAuthorityPolicy, BehaviorClass, BehaviorEffectKind};
use crate::domain::naming::{NamespacedError, NamespacedName};
use crate::domain::trust::AdmissionSource;

/// Why a behavior definition or admission was rejected.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BehaviorAdmissionError {
    /// The behavior id is not a valid `namespace.name`.
    InvalidId(NamespacedError),
    /// The behavior name is empty.
    EmptyName,
    /// The declared class is `PureDerivation` but the behavior declares effects
    /// that class may not request.
    ///
    /// Not an escalation risk — admission would downgrade it anyway — but a
    /// definition whose own declared class forbids its own declared effects is
    /// incoherent and would surprise its author silently.
    IncoherentEffects {
        /// The class the definition declared.
        declared: BehaviorClass,
        /// The effects it declared.
        disallowed: Vec<BehaviorEffectKind>,
    },
}

impl std::fmt::Display for BehaviorAdmissionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidId(err) => write!(f, "invalid behavior id: {err}"),
            Self::EmptyName => f.write_str("a behavior name must not be empty"),
            Self::IncoherentEffects {
                declared,
                disallowed,
            } => write!(
                f,
                "a behavior declaring itself {declared} may not request {:?}",
                disallowed.iter().map(|e| e.name()).collect::<Vec<_>>()
            ),
        }
    }
}

impl std::error::Error for BehaviorAdmissionError {}

/// Identifies a behavior definition.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct BehaviorId(NamespacedName);

impl BehaviorId {
    /// Validate and construct a behavior id.
    pub fn new(value: impl Into<String>) -> Result<Self, NamespacedError> {
        NamespacedName::new(value).map(Self)
    }

    /// Borrow the raw value.
    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

impl std::fmt::Display for BehaviorId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.0.as_str())
    }
}

impl TryFrom<String> for BehaviorId {
    type Error = NamespacedError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

impl From<BehaviorId> for String {
    fn from(value: BehaviorId) -> Self {
        value.0.as_str().to_string()
    }
}

/// What a behavior *is*, as declared by its author.
///
/// `declared_class` is a claim. What the behavior actually runs as is
/// [`AdmittedBehavior::effective_class`], and only admission can say.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BehaviorDefinition {
    /// Stable identity.
    pub id: BehaviorId,
    /// Human-readable name.
    pub name: String,
    /// The class the author claims.
    pub declared_class: BehaviorClass,
    /// The effects the behavior intends to request.
    pub effects: BTreeSet<BehaviorEffectKind>,
}

impl BehaviorDefinition {
    /// Construct a definition, rejecting an incoherent one.
    ///
    /// "Incoherent" means the definition contradicts itself: it claims a class
    /// whose table forbids the effects it declares. Admission would downgrade
    /// such a definition anyway, so refusing here is about not silently
    /// surprising the author, not about safety.
    pub fn new(
        id: impl Into<String>,
        name: impl Into<String>,
        declared_class: BehaviorClass,
        effects: impl IntoIterator<Item = BehaviorEffectKind>,
    ) -> Result<Self, BehaviorAdmissionError> {
        let id = BehaviorId::new(id).map_err(BehaviorAdmissionError::InvalidId)?;
        let name = name.into();
        if name.trim().is_empty() {
            return Err(BehaviorAdmissionError::EmptyName);
        }
        let effects: BTreeSet<BehaviorEffectKind> = effects.into_iter().collect();
        let disallowed: Vec<BehaviorEffectKind> = effects
            .iter()
            .copied()
            .filter(|e| !BehaviorAuthorityPolicy::allows(declared_class, *e))
            .collect();
        if !disallowed.is_empty() {
            return Err(BehaviorAdmissionError::IncoherentEffects {
                declared: declared_class,
                disallowed,
            });
        }
        Ok(Self {
            id,
            name,
            declared_class,
            effects,
        })
    }

    /// Whether the definition declares it will request `effect`.
    pub fn declares(&self, effect: BehaviorEffectKind) -> bool {
        self.effects.contains(&effect)
    }
}

/// A behavior as admitted: its definition plus the class it actually runs as.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdmittedBehavior {
    definition: BehaviorDefinition,
    effective_class: BehaviorClass,
    source: AdmissionSource,
}

impl AdmittedBehavior {
    /// The definition.
    pub fn definition(&self) -> &BehaviorDefinition {
        &self.definition
    }

    /// The behavior id.
    pub fn id(&self) -> &BehaviorId {
        &self.definition.id
    }

    /// The class this behavior actually runs as.
    pub fn effective_class(&self) -> BehaviorClass {
        self.effective_class
    }

    /// The class its author claimed.
    pub fn declared_class(&self) -> BehaviorClass {
        self.definition.declared_class
    }

    /// Where the definition came from.
    pub fn source(&self) -> AdmissionSource {
        self.source
    }

    /// Whether the admission downgraded the declared class.
    pub fn was_downgraded(&self) -> bool {
        self.effective_class != self.declared_class()
    }

    /// Whether this behavior may request `effect`.
    ///
    /// Answers from the *effective* class, never the declared one.
    pub fn may(&self, effect: BehaviorEffectKind) -> bool {
        BehaviorAuthorityPolicy::allows(self.effective_class, effect)
    }
}

/// Private seal: only this module can construct a [`BehaviorPermit`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct BehaviorSeal(());

/// The capability token that authorises a behavior execution.
///
/// Private fields plus a private seal mean the only way to obtain one is
/// [`BehaviorAdmission::admit`]. It is deliberately **not**
/// `Serialize`/`Deserialize`: authority is minted, not restored.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BehaviorPermit {
    admitted: AdmittedBehavior,
    _seal: BehaviorSeal,
}

impl BehaviorPermit {
    /// The admitted behavior this permit authorises.
    pub fn admitted(&self) -> &AdmittedBehavior {
        &self.admitted
    }

    /// The behavior id.
    pub fn id(&self) -> &BehaviorId {
        self.admitted.id()
    }

    /// The class this permit carries.
    pub fn effective_class(&self) -> BehaviorClass {
        self.admitted.effective_class()
    }

    /// Whether this permit may request `effect`.
    pub fn may(&self, effect: BehaviorEffectKind) -> bool {
        self.admitted.may(effect)
    }
}

/// The single trust boundary for behaviors.
#[derive(Debug, Clone, Copy, Default)]
pub struct BehaviorAdmission;

impl BehaviorAdmission {
    /// Admit a behavior definition from `source`.
    ///
    /// The effective class is the declared one **only** for
    /// [`AdmissionSource::Builtin`] and [`AdmissionSource::HumanCurated`].
    /// Anything else runs as [`BehaviorClass::AgentBehavior`]: a declaration
    /// never escalates.
    pub fn admit(
        definition: BehaviorDefinition,
        source: AdmissionSource,
    ) -> Result<BehaviorPermit, BehaviorAdmissionError> {
        let effective_class = match source {
            AdmissionSource::Builtin | AdmissionSource::HumanCurated => definition.declared_class,
            AdmissionSource::AiGenerated | AdmissionSource::Imported => {
                BehaviorClass::AgentBehavior
            }
        };
        Ok(BehaviorPermit {
            admitted: AdmittedBehavior {
                definition,
                effective_class,
                source,
            },
            _seal: BehaviorSeal(()),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pure_definition() -> BehaviorDefinition {
        BehaviorDefinition::new(
            "derivation.symbol_index",
            "symbol index",
            BehaviorClass::PureDerivation,
            [BehaviorEffectKind::CommitCanonicalFact],
        )
        .unwrap()
    }

    #[test]
    fn a_curated_pure_derivation_keeps_its_class() {
        let permit = BehaviorAdmission::admit(pure_definition(), AdmissionSource::HumanCurated)
            .expect("admitted");
        assert_eq!(permit.effective_class(), BehaviorClass::PureDerivation);
        assert!(!permit.admitted().was_downgraded());
        assert!(permit.may(BehaviorEffectKind::CommitCanonicalFact));
    }

    /// The rule that matters: an AI-proposed behavior that claims to be a pure
    /// derivation runs as an agent behavior and therefore cannot commit facts.
    #[test]
    fn an_ai_generated_definition_is_downgraded_and_loses_the_fact() {
        let permit =
            BehaviorAdmission::admit(pure_definition(), AdmissionSource::AiGenerated).unwrap();
        assert_eq!(permit.effective_class(), BehaviorClass::AgentBehavior);
        assert_eq!(
            permit.admitted().declared_class(),
            BehaviorClass::PureDerivation
        );
        assert!(permit.admitted().was_downgraded());
        assert!(
            !permit.may(BehaviorEffectKind::CommitCanonicalFact),
            "the claim must not survive admission"
        );
        // …but it keeps what an agent may legitimately do.
        assert!(permit.may(BehaviorEffectKind::ProposeChange));
    }

    #[test]
    fn an_imported_definition_is_downgraded_too() {
        let permit =
            BehaviorAdmission::admit(pure_definition(), AdmissionSource::Imported).unwrap();
        assert_eq!(permit.effective_class(), BehaviorClass::AgentBehavior);
        assert!(!permit.may(BehaviorEffectKind::CommitCanonicalFact));
    }

    #[test]
    fn a_builtin_definition_keeps_its_class() {
        let permit = BehaviorAdmission::admit(pure_definition(), AdmissionSource::Builtin).unwrap();
        assert_eq!(permit.effective_class(), BehaviorClass::PureDerivation);
    }

    #[test]
    fn a_definition_that_contradicts_itself_is_refused() {
        let err = BehaviorDefinition::new(
            "derivation.liar",
            "liar",
            BehaviorClass::PureDerivation,
            [
                BehaviorEffectKind::CommitCanonicalFact,
                BehaviorEffectKind::ProposeChange,
            ],
        )
        .unwrap_err();
        match err {
            BehaviorAdmissionError::IncoherentEffects {
                declared,
                disallowed,
            } => {
                assert_eq!(declared, BehaviorClass::PureDerivation);
                assert_eq!(disallowed, vec![BehaviorEffectKind::ProposeChange]);
            }
            other => panic!("expected IncoherentEffects, got {other:?}"),
        }
    }

    #[test]
    fn malformed_ids_and_names_are_refused() {
        assert!(matches!(
            BehaviorDefinition::new("nons", "n", BehaviorClass::AgentBehavior, []),
            Err(BehaviorAdmissionError::InvalidId(_))
        ));
        assert!(matches!(
            BehaviorDefinition::new("a.b", "  ", BehaviorClass::AgentBehavior, []),
            Err(BehaviorAdmissionError::EmptyName)
        ));
    }

    /// A permit cannot be serialized, so authority cannot be smuggled through
    /// storage and restored by accident.
    #[test]
    fn a_permit_is_not_serializable() {
        fn assert_not_serializable<T: serde::Serialize>() {}
        // Compile-time proof by absence: this would not build with `T = BehaviorPermit`.
        assert_not_serializable::<BehaviorClass>();
        let _ = BehaviorPermit::may;
    }
}
