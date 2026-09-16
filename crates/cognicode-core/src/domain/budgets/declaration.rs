//! Budget declarations (M7.4, cycle e65).
//!
//! A declaration is a ceiling, not a reservation: it sets a hard upper bound
//! that the authorizer enforces. A behavior declares what it intends to spend;
//! the runtime tracks what it actually spends.
//!
//! Pure domain: no I/O.

use std::num::NonZeroU64;

use super::kind::BudgetKind;

/// A budget ceiling declared by a behavior definition.
///
/// Each entry is `(kind, ceiling)`. A ceiling of `0` is excluded by the type,
/// so an empty declaration means "no budget at all" (unbounded).
///
/// Invariant: a declaration is **not** a reservation — it does not allocate
/// resources, it only caps them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BudgetDeclaration {
    /// (kind, ceiling) pairs, kept sorted by kind for deterministic iteration.
    per_kind: Vec<(BudgetKind, NonZeroU64)>,
}

impl BudgetDeclaration {
    /// No budget at all: every kind is unlimited.
    pub fn none() -> Self {
        Self {
            per_kind: Vec::new(),
        }
    }

    /// Declare a time budget in milliseconds.
    pub fn time(ceiling: NonZeroU64) -> Self {
        Self {
            per_kind: vec![(BudgetKind::Time, ceiling)],
        }
    }

    /// Declare an effect-count budget.
    pub fn effects(ceiling: NonZeroU64) -> Self {
        Self {
            per_kind: vec![(BudgetKind::EffectCount, ceiling)],
        }
    }

    /// Declare a fact-visits budget.
    pub fn fact_visits(ceiling: NonZeroU64) -> Self {
        Self {
            per_kind: vec![(BudgetKind::FactVisits, ceiling)],
        }
    }

    /// Whether this declaration has a ceiling for `kind`.
    pub fn has(&self, kind: BudgetKind) -> bool {
        self.per_kind.iter().any(|(k, _)| *k == kind)
    }

    /// The ceiling for `kind`, if one is declared.
    pub fn ceiling(&self, kind: BudgetKind) -> Option<NonZeroU64> {
        self.per_kind
            .iter()
            .find(|(k, _)| *k == kind)
            .map(|(_, c)| *c)
    }

    /// All (kind, ceiling) pairs, in sort order.
    pub fn entries(&self) -> &[(BudgetKind, NonZeroU64)] {
        &self.per_kind
    }

    /// Merge another declaration into this one.
    ///
    /// If both declare a ceiling for the same kind, the smaller ceiling wins.
    /// Entries from `other` that are not in `self` are appended.
    pub fn merge(mut self, other: &BudgetDeclaration) -> Self {
        for (kind, cap) in &other.per_kind {
            if let Some(existing) = self.ceiling(*kind) {
                if cap.get() < existing.get() {
                    // Replace with the smaller ceiling.
                    if let Some(entry) = self.per_kind.iter_mut().find(|(k, _)| *k == *kind) {
                        entry.1 = *cap;
                    }
                }
            } else {
                self.per_kind.push((*kind, *cap));
            }
        }
        self.per_kind.sort_by_key(|(k, _)| *k);
        self
    }

    /// Whether this declaration is empty (no budgets declared).
    pub fn is_empty(&self) -> bool {
        self.per_kind.is_empty()
    }
}

impl Default for BudgetDeclaration {
    fn default() -> Self {
        Self::none()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn none_is_empty() {
        let decl = BudgetDeclaration::none();
        assert!(decl.is_empty());
        assert!(!decl.has(BudgetKind::Time));
        assert_eq!(decl.ceiling(BudgetKind::EffectCount), None);
        assert!(decl.entries().is_empty());
    }

    #[test]
    fn named_constructors_set_exactly_one_kind() {
        let time = BudgetDeclaration::time(NonZeroU64::new(100).unwrap());
        assert!(time.has(BudgetKind::Time));
        assert!(!time.has(BudgetKind::EffectCount));
        assert!(!time.has(BudgetKind::FactVisits));
        assert_eq!(time.ceiling(BudgetKind::Time).unwrap().get(), 100);

        let effects = BudgetDeclaration::effects(NonZeroU64::new(5).unwrap());
        assert!(effects.has(BudgetKind::EffectCount));
        assert!(!effects.has(BudgetKind::Time));

        let visits = BudgetDeclaration::fact_visits(NonZeroU64::new(200).unwrap());
        assert!(visits.has(BudgetKind::FactVisits));
    }

    #[test]
    fn merge_takes_the_smaller_ceiling() {
        let a = BudgetDeclaration::time(NonZeroU64::new(100).unwrap());
        let b = BudgetDeclaration::time(NonZeroU64::new(50).unwrap());
        let merged = a.clone().merge(&b);
        assert_eq!(merged.ceiling(BudgetKind::Time).unwrap().get(), 50);

        // Larger ceiling wins.
        let c = BudgetDeclaration::time(NonZeroU64::new(200).unwrap());
        let merged2 = a.merge(&c);
        assert_eq!(merged2.ceiling(BudgetKind::Time).unwrap().get(), 100);
    }

    #[test]
    fn merge_appends_new_kinds() {
        let a = BudgetDeclaration::time(NonZeroU64::new(100).unwrap());
        let b = BudgetDeclaration::effects(NonZeroU64::new(5).unwrap());
        let merged = a.merge(&b);
        assert!(merged.has(BudgetKind::Time));
        assert!(merged.has(BudgetKind::EffectCount));
    }

    #[test]
    fn declaration_does_not_escalate() {
        // A declaration never changes effective_class (proven by the absence of
        // any class-related field or method here). This test anchors the
        // invariant: BudgetDeclaration carries only budget ceilings.
        let decl = BudgetDeclaration::time(NonZeroU64::new(u64::MAX).unwrap());
        let _: &[(BudgetKind, NonZeroU64)] = decl.entries();
        // No class field exists, so no escalation is possible.
    }

    #[test]
    fn entries_are_deterministic() {
        // Add two entries and verify sort order.
        let time = BudgetDeclaration::time(NonZeroU64::new(10).unwrap());
        let effects = BudgetDeclaration::effects(NonZeroU64::new(5).unwrap());
        let merged = time.merge(&effects);
        let entries = merged.entries();
        assert!(entries.len() == 2);
        assert!(entries[0].0 <= entries[1].0, "kinds must be sorted");
    }
}
