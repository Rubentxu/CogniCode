//! Budget authorization (M7.4, cycle e65).
//!
//! ## The two-phase rule
//!
//! ```text
//! check  — would this spend exceed the ceiling?  (read-only, no mutation)
//! commit — record the spend as having happened    (mutates state)
//! ```
//!
//! Refusal via `check` does **not** decrement the counter. Only a successful
//! sink dispatch triggers `commit`. This ordering is what makes the invariant
//! "refuse BEFORE the adapter" hold for budgets as it does for authority.
//!
//! Pure domain: no I/O, no clocks.

use super::declaration::BudgetDeclaration;
use super::kind::BudgetKind;
use super::state::{self, BudgetState};

/// Why a budget was refused.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BudgetExhausted {
    /// The budget kind that was exceeded.
    pub kind: BudgetKind,
    /// How much was remaining before the attempted spend.
    pub remaining: u64,
    /// How much the effect tried to spend.
    pub attempted: u64,
}

impl BudgetExhausted {
    /// A one-line diagnostic.
    pub fn reason(&self) -> String {
        format!(
            "{} budget exhausted: {} remaining, {} attempted",
            self.kind.name(),
            self.remaining,
            self.attempted
        )
    }
}

impl std::fmt::Display for BudgetExhausted {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.reason())
    }
}

impl std::error::Error for BudgetExhausted {}

/// Authorizes budget spends against a declaration.
///
/// Constructed from a `BudgetDeclaration`; consults and mutates a `BudgetState`.
/// Two-step protocol:
///
/// 1. [`check`](BudgetAuthorizer::check) — would the spend fit? Returns `Ok(())` or `Err(BudgetExhausted)`.
/// 2. [`commit`](BudgetAuthorizer::commit) — record the spend. Call only after a successful sink dispatch.
///
/// The invariant: `check` never mutates; `commit` always follows a confirmed effect.
#[derive(Debug, Clone)]
pub struct BudgetAuthorizer {
    declaration: BudgetDeclaration,
}

impl BudgetAuthorizer {
    /// Construct an authorizer from a budget declaration.
    pub fn new(declaration: BudgetDeclaration) -> Self {
        Self { declaration }
    }

    /// Whether this authorizer has any budgets at all.
    pub fn is_unbounded(&self) -> bool {
        self.declaration.is_empty()
    }

    /// Check whether spending `amount` for `kind` would exceed the declared ceiling.
    ///
    /// Returns `Ok(remaining)` if the spend fits, where `remaining` is what
    /// would remain after the spend. Returns `Err(BudgetExhausted)` if the
    /// ceiling would be exceeded — **no state is mutated**.
    ///
    /// If the kind has no declared ceiling, this always returns `Ok(u64::MAX)`.
    ///
    /// ## Per-kind semantics
    ///
    /// - **`EffectCount` / `FactVisits`**: compared against the committed spend
    ///   counter (`state.spent(kind)`).
    /// - **`Time`**: compared against elapsed wall-clock time
    ///   (`state.elapsed_millis(now_millis)`). The `now_millis` parameter supplies
    ///   the current time from the injected `Clock` port. Each effect is assumed
    ///   to consume `amount` milliseconds.
    pub fn check(
        &self,
        state: &BudgetState,
        kind: BudgetKind,
        amount: u64,
    ) -> Result<u64, BudgetExhausted> {
        let Some(ceiling) = self.declaration.ceiling(kind) else {
            // No ceiling for this kind: always allowed.
            return Ok(u64::MAX);
        };

        let ceiling_val = ceiling.get();

        // Time budgets measure wall-clock elapsed time via the last recorded checkpoint;
        // other budgets measure committed spends. This is the only semantic difference.
        let reference = match kind {
            BudgetKind::Time => state.time_checkpoint(),
            BudgetKind::EffectCount | BudgetKind::FactVisits => state.spent(kind),
        };

        // Saturating arithmetic guards against counter corruption.
        let remaining = ceiling_val.saturating_sub(reference);
        if amount > remaining {
            return Err(BudgetExhausted {
                kind,
                remaining,
                attempted: amount,
            });
        }

        Ok(remaining.saturating_sub(amount))
    }

    /// Record a confirmed spend.
    ///
    /// Call this **only** after the effect sink has successfully applied the
    /// effect. It is a logic error to call this when `check` previously refused
    /// the same spend.
    pub fn commit(&self, state: &mut BudgetState, kind: BudgetKind, amount: u64) {
        state::commit_spend(state, kind, amount);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn time_decl(ms: u64) -> BudgetDeclaration {
        use std::num::NonZeroU64;
        BudgetDeclaration::time(NonZeroU64::new(ms).unwrap())
    }

    fn effects_decl(n: u64) -> BudgetDeclaration {
        use std::num::NonZeroU64;
        BudgetDeclaration::effects(NonZeroU64::new(n).unwrap())
    }

    fn state(start: u64) -> BudgetState {
        BudgetState::new(&BudgetDeclaration::none(), start)
    }

    // ── refusal does NOT decrement ───────────────────────────────────────────

    #[test]
    fn refused_spend_does_not_decrement() {
        let decl = effects_decl(2);
        let auth = BudgetAuthorizer::new(decl);
        let mut state = state(0);

        // First effect: allowed.
        auth.check(&state, BudgetKind::EffectCount, 1).unwrap();
        auth.commit(&mut state, BudgetKind::EffectCount, 1);
        assert_eq!(state.spent(BudgetKind::EffectCount), 1);

        // Second effect: allowed.
        auth.check(&state, BudgetKind::EffectCount, 1).unwrap();
        auth.commit(&mut state, BudgetKind::EffectCount, 1);
        assert_eq!(state.spent(BudgetKind::EffectCount), 2);

        // Third effect: refused.
        let err = auth.check(&state, BudgetKind::EffectCount, 1).unwrap_err();
        assert_eq!(err.kind, BudgetKind::EffectCount);
        assert_eq!(err.remaining, 0);
        assert_eq!(err.attempted, 1);

        // State unchanged after refusal.
        assert_eq!(state.spent(BudgetKind::EffectCount), 2);
    }

    // ── time budget ────────────────────────────────────────────────────────────

    #[test]
    fn time_budget_refuses_after_elapsed_exceeds_ceiling() {
        use std::num::NonZeroU64;
        let decl = BudgetDeclaration::time(NonZeroU64::new(100).unwrap());
        let auth = BudgetAuthorizer::new(decl);

        // First effect at t=50: checkpoint=50, ceiling=100, remaining=50 — 30ms fits.
        let mut state = BudgetState::new(&BudgetDeclaration::none(), 0);
        super::super::state::record_time(&mut state, 50);
        assert!(auth.check(&state, BudgetKind::Time, 30).is_ok());

        // Commit first effect: effect count increments.
        auth.commit(&mut state, BudgetKind::EffectCount, 1);
        assert_eq!(state.spent(BudgetKind::EffectCount), 1);

        // Second effect at t=80: checkpoint=80, ceiling=100, remaining=20 — 30ms does NOT fit.
        super::super::state::record_time(&mut state, 80);
        let err = auth
            .check(&state, BudgetKind::Time, 30)
            .unwrap_err();
        assert_eq!(err.kind, BudgetKind::Time);
        assert_eq!(err.remaining, 20);
        assert_eq!(err.attempted, 30);
    }

    // ── unbounded kind ────────────────────────────────────────────────────────

    #[test]
    fn undeclared_kind_is_always_allowed() {
        let decl = BudgetDeclaration::none(); // no budgets at all
        let auth = BudgetAuthorizer::new(decl);
        let state = state(0);

        // Any amount for any kind is fine when no budgets are declared.
        let remaining = auth
            .check(&state, BudgetKind::EffectCount, u64::MAX)
            .unwrap();
        assert_eq!(remaining, u64::MAX);
    }

    // ── exhaustion error carries right info ────────────────────────────────────

    #[test]
    fn exhaustion_error_message_is_descriptive() {
        let decl = effects_decl(5); // ceiling=5
        let auth = BudgetAuthorizer::new(decl);
        let state = state(0); // spent=0

        let err = auth
            .check(&state, BudgetKind::EffectCount, 10)
            .unwrap_err();
        assert!(err.reason().contains("effect_count"));
        // `remaining` in the error is ceiling - spent = 5 - 0 = 5.
        assert!(err.reason().contains("5 remaining"));
        assert!(err.reason().contains("10 attempted"));
    }

    // ── state isolation: no cross-kind bleed ───────────────────────────────────

    #[test]
    fn kinds_do_not_bleed_into_each_other() {
        use std::num::NonZeroU64;
        let mut decl = BudgetDeclaration::time(NonZeroU64::new(100).unwrap());
        decl = decl.merge(&BudgetDeclaration::effects(NonZeroU64::new(5).unwrap()));
        let auth = BudgetAuthorizer::new(decl);
        let mut state = state(0);

        // Spend 3 effects.
        for _ in 0..3 {
            auth.check(&state, BudgetKind::EffectCount, 1).unwrap();
            auth.commit(&mut state, BudgetKind::EffectCount, 1);
        }
        assert_eq!(state.spent(BudgetKind::EffectCount), 3);
        assert_eq!(state.spent(BudgetKind::Time), 0, "time untouched by effect spend");

        // Time budget still at 0 spent.
        assert!(auth.check(&state, BudgetKind::Time, 100).is_ok());
    }

    // ── commit only on success ────────────────────────────────────────────────

    #[test]
    fn committing_without_prior_check_is_harmless() {
        // It's a logic error (should always follow check), but it doesn't panic.
        let decl = effects_decl(1);
        let auth = BudgetAuthorizer::new(decl);
        let mut state = state(0);
        auth.commit(&mut state, BudgetKind::EffectCount, 1);
        assert_eq!(state.spent(BudgetKind::EffectCount), 1);
    }

    // ── is_unbounded ─────────────────────────────────────────────────────────

    #[test]
    fn empty_declaration_is_unbounded() {
        let auth = BudgetAuthorizer::new(BudgetDeclaration::none());
        assert!(auth.is_unbounded());
    }

    #[test]
    fn non_empty_declaration_is_bounded() {
        let auth = BudgetAuthorizer::new(effects_decl(1));
        assert!(!auth.is_unbounded());
    }
}
