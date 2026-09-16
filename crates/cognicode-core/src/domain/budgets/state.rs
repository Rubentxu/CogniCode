//! Budget state (M7.4, cycle e65).
//!
//! The state tracks what a behavior has actually spent against its declaration.
//! It is owned by the `BehaviorRuntime` and never exposed as a public field
//! on a permit — store-allocated ids make public fields decorative.
//!
//! Pure domain: no I/O.

use std::collections::BTreeMap;

use super::kind::BudgetKind;

/// Budget spending state, owned by the runtime.
///
/// **Private fields**: the only way to read or mutate this is through
/// `BudgetAuthorizer`, which enforces the refusal-before-commit invariant.
/// External code may only construct a `BudgetState` from a `BudgetDeclaration`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BudgetState {
    /// How much has been spent per kind. Only kinds that have been touched are
    /// present; untouched kinds are absent (equivalent to 0 spent).
    per_kind: BTreeMap<BudgetKind, u64>,
    /// When this execution started, in monotonic milliseconds.
    started_at_millis: u64,
}

impl BudgetState {
    /// Construct state from a declaration and the wall-clock start time.
    ///
    /// Starts with all counters at 0. A kind with ceiling=10 but spent=0 has
    /// 10 remaining; it is absent from `per_kind` until the first spend.
    pub fn new(_declaration: &super::declaration::BudgetDeclaration, started_at_millis: u64) -> Self {
        Self {
            per_kind: BTreeMap::new(),
            started_at_millis,
        }
    }

    /// How much has been spent for `kind` (0 if never touched).
    pub fn spent(&self, kind: BudgetKind) -> u64 {
        self.per_kind.get(&kind).copied().unwrap_or(0)
    }

    /// The wall-clock elapsed since this execution started, in milliseconds.
    ///
    /// `clock` supplies `now_millis()`; callers must pass a monotonic source.
    pub fn elapsed_millis(&self, now_millis: u64) -> u64 {
        now_millis.saturating_sub(self.started_at_millis)
    }

    /// The start timestamp, in monotonic milliseconds.
    pub fn started_at(&self) -> u64 {
        self.started_at_millis
    }

    /// Increment the spent counter for `kind` by `amount`.
    ///
    /// **Callers must verify this does not exceed the declaration ceiling
    /// before calling.** This method mutates state unconditionally.
    fn spend(&mut self, kind: BudgetKind, amount: u64) {
        *self.per_kind.entry(kind).or_insert(0) += amount;
    }
}

/// Internal mutation for budget state. Exposed only to `BudgetAuthorizer`.
pub(super) fn commit_spend(state: &mut BudgetState, kind: BudgetKind, amount: u64) {
    state.spend(kind, amount);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::budgets::declaration::BudgetDeclaration;

    fn decl(time: u64, effects: u64) -> BudgetDeclaration {
        use std::num::NonZeroU64;
        let mut d = BudgetDeclaration::time(NonZeroU64::new(time).unwrap());
        if effects > 0 {
            d = d.merge(&BudgetDeclaration::effects(NonZeroU64::new(effects).unwrap()));
        }
        d
    }

    #[test]
    fn new_starts_at_zero() {
        let d = decl(100, 5);
        let state = BudgetState::new(&d, 0);
        assert_eq!(state.spent(BudgetKind::Time), 0);
        assert_eq!(state.spent(BudgetKind::EffectCount), 0);
    }

    #[test]
    fn new_records_start_time() {
        let d = BudgetDeclaration::none();
        let state = BudgetState::new(&d, 42);
        assert_eq!(state.started_at(), 42);
    }

    #[test]
    fn spend_increments_counter() {
        let d = decl(100, 5);
        let mut state = BudgetState::new(&d, 0);
        commit_spend(&mut state, BudgetKind::EffectCount, 1);
        assert_eq!(state.spent(BudgetKind::EffectCount), 1);
        commit_spend(&mut state, BudgetKind::EffectCount, 1);
        assert_eq!(state.spent(BudgetKind::EffectCount), 2);
    }

    #[test]
    fn untuched_kind_returns_zero() {
        let d = decl(100, 5);
        let state = BudgetState::new(&d, 0);
        // FactVisits was never declared, so spent is 0.
        assert_eq!(state.spent(BudgetKind::FactVisits), 0);
    }

    #[test]
    fn elapsed_millis_is_monotonic() {
        let state = BudgetState::new(&BudgetDeclaration::none(), 100);
        assert_eq!(state.elapsed_millis(150), 50);
        assert_eq!(state.elapsed_millis(50), 0, "clock must not go backwards");
        assert_eq!(state.elapsed_millis(100), 0, "zero elapsed at start");
    }

    #[test]
    fn state_is_cloneable() {
        let d = decl(100, 5);
        let state = BudgetState::new(&d, 0);
        let _clone = state.clone();
    }
}
