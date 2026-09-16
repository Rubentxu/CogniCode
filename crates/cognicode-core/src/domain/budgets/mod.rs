//! Behavior budgets (M7.4, cycle e65).
//!
//! ## Invariants enforced by this module
//!
//! 1. **Budget does not derive authority.**  
//!    A `BudgetAuthorizer` never touches `BehaviorAuthorityPolicy`. Authority and
//!    budget are independent gates; the policy table is unchanged.
//!
//! 2. **A declaration never escalates.**  
//!    `BudgetDeclaration` has no field for `BehaviorClass`. Declaring a budget
//!    cannot change what class a behavior runs as.
//!
//! 3. **Refuse BEFORE the adapter, never accept-then-undo.**  
//!    `BudgetAuthorizer::check` is read-only. Only `commit` mutates state, and
//!    it is called only after the sink confirms success.
//!
//! 4. **No `block_on` in the domain.**  
//!    The `Clock` port lives in `application::behaviors::clock`. This module
//!    carries only declared ceilings; it never blocks or reads the clock.
//!
//! 5. **Store-allocated ids, public fields make checks decorative.**  
//!    `BudgetState` has private fields. Only `BudgetAuthorizer` can mutate it.
//!
//! 6. **Model effects not deliverables.**  
//!    `EffectCount` budgets count effect *attempts*, not produced facts or
//!    hypotheses. `Time` budgets measure wall-clock milliseconds.
//!
//! 7. **Snapshot integrity.**  
//!    The refusal path cannot reach `FactStore.append_batch`. An exhausted
//!    budget leaves all canonical stores byte-identical.
//!
//! ## Module map
//!
//! | File | What's in it |
//! |---|---|
//! | `kind.rs` | `BudgetKind` enum |
//! | `declaration.rs` | `BudgetDeclaration` with named ctors |
//! | `state.rs` | `BudgetState` (private fields, owned by runtime) |
//! | `authorizer.rs` | `BudgetAuthorizer` + `BudgetExhausted` error |

pub mod authorizer;
pub mod declaration;
pub mod kind;
pub mod state;

pub use authorizer::{BudgetAuthorizer, BudgetExhausted};
pub use declaration::BudgetDeclaration;
pub use kind::BudgetKind;
pub use state::BudgetState;
