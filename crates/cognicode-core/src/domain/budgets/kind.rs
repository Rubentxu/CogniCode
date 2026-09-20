//! Budget kinds (M7.4, cycle e65).
//!
//! What a budget limits. Each kind is independent; a behavior may declare a
//! ceiling for any subset.
//!
//! Pure domain: no I/O.

/// What a budget limits.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum BudgetKind {
    /// Wall-clock time, measured in milliseconds via an injected `Clock` port.
    Time,
    /// Number of effects requested, before any are authorized.
    EffectCount,
    /// Number of times a fact is visited / checked (read-set tracking).
    FactVisits,
}

impl BudgetKind {
    /// Stable name for diagnostics and event payloads.
    pub fn name(self) -> &'static str {
        match self {
            Self::Time => "time",
            Self::EffectCount => "effect_count",
            Self::FactVisits => "fact_visits",
        }
    }
}

impl std::fmt::Display for BudgetKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.name())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kind_names_are_stable() {
        assert_eq!(BudgetKind::Time.name(), "time");
        assert_eq!(BudgetKind::EffectCount.name(), "effect_count");
        assert_eq!(BudgetKind::FactVisits.name(), "fact_visits");
    }

    #[test]
    fn kinds_are_ordered_and_hashable() {
        use std::collections::HashSet;
        let mut set = HashSet::new();
        set.insert(BudgetKind::Time);
        set.insert(BudgetKind::EffectCount);
        set.insert(BudgetKind::FactVisits);
        assert_eq!(set.len(), 3, "each kind must be distinct");
    }

    #[test]
    fn kinds_derive_debug_and_clone() {
        let k = BudgetKind::Time;
        let debug = format!("{k:?}");
        assert!(debug.contains("Time"));
        let _cloned = k;
    }
}
