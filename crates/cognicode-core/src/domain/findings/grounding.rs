//! Grounding: the link from an *analysis* element back to the *canonical truth*
//! it was projected from.
//!
//! An analysis backend (AST, graph, dataflow) projects views out of the
//! evidence kernel. A [`GroundingRef`] on a projected element says: *this
//! element was derived from this canonical fact*. It is what allows a finding's
//! causal chain to be checked for **coherence** — not merely that the referenced
//! ids exist, but that the causal step, the evidence and the fact agree on what
//! happened.
//!
//! # Authority
//!
//! ```text
//! grounding.fact  = authority   (the canonical fact is the source of truth)
//! grounding.entity = hint       (navigation only; never trusted over the fact)
//! ```
//!
//! When `entity` is present, the write bridge must check
//! `grounding.entity == canonical_fact.subject` before persisting evidence. A
//! projection may *name* an entity, but the fact decides.
//!
//! # Conservative by construction
//!
//! `grounding` is `Option`. A backend that synthesised an element from several
//! observations must leave it `None` rather than pick a convenient fact: an
//! element that cannot be grounded may be explained, but it cannot open the
//! gate. `correct incomplete > fabricated complete`.

use serde::{Deserialize, Serialize};

use crate::domain::kernel_ids::{EntityId, FactId};

/// The canonical fact an analysis element was projected from, plus an optional
/// entity hint.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct GroundingRef {
    /// Entity hint for navigation. Not authoritative: the write bridge checks it
    /// against the canonical fact's subject and rejects a mismatch.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub entity: Option<EntityId>,
    /// The canonical fact that grounds this element. Authoritative.
    pub fact: FactId,
}

impl GroundingRef {
    /// Ground an element in `fact`, with no entity hint.
    pub fn fact(fact: FactId) -> Self {
        Self { entity: None, fact }
    }

    /// Ground an element in `fact` and name the entity it concerns.
    ///
    /// The entity is a hint; see the module docs.
    pub fn entity(entity: EntityId, fact: FactId) -> Self {
        Self {
            entity: Some(entity),
            fact,
        }
    }

    /// The authoritative fact.
    pub fn fact_id(&self) -> FactId {
        self.fact
    }

    /// The (non-authoritative) entity hint.
    pub fn entity_hint(&self) -> Option<EntityId> {
        self.entity
    }

    /// Whether the entity hint agrees with the canonical fact's subject.
    ///
    /// `None` means "no opinion" (no hint, so nothing to check).
    pub fn entity_agrees_with(&self, subject: Option<EntityId>) -> Option<bool> {
        self.entity.map(|hint| Some(hint) == subject)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fact_only_grounding_has_no_entity_hint() {
        let grounding = GroundingRef::fact(FactId::new(7));
        assert_eq!(grounding.fact_id(), FactId::new(7));
        assert_eq!(grounding.entity_hint(), None);
        assert_eq!(
            grounding.entity_agrees_with(Some(EntityId::new(3))),
            None,
            "no hint means nothing to contradict"
        );
    }

    #[test]
    fn entity_hint_is_checked_against_the_canonical_subject() {
        let grounding = GroundingRef::entity(EntityId::new(3), FactId::new(7));
        assert_eq!(
            grounding.entity_agrees_with(Some(EntityId::new(3))),
            Some(true)
        );
        assert_eq!(
            grounding.entity_agrees_with(Some(EntityId::new(9))),
            Some(false),
            "a hint that contradicts the canonical subject must be visible"
        );
        assert_eq!(grounding.entity_agrees_with(None), Some(false));
    }

    #[test]
    fn grounding_round_trips() {
        let grounding = GroundingRef::entity(EntityId::new(3), FactId::new(7));
        let json = serde_json::to_string(&grounding).unwrap();
        assert_eq!(
            serde_json::from_str::<GroundingRef>(&json).unwrap(),
            grounding
        );

        // An omitted entity hint deserialises to `None`.
        let bare = serde_json::to_string(&GroundingRef::fact(FactId::new(1))).unwrap();
        assert_eq!(bare, r#"{"fact":1}"#);
        assert_eq!(
            serde_json::from_str::<GroundingRef>(r#"{"fact":1}"#).unwrap(),
            GroundingRef::fact(FactId::new(1))
        );
    }
}
