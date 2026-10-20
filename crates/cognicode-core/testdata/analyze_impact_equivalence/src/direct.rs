// Corpus for the impact analysis and the call hierarchy: callers of
// impact_target. The impact analysis needs direct dependents; the call
// hierarchy needs callers that transitive.rs further transitively calls.

use crate::impact_target;

pub fn impact_direct_caller() -> u32 {
    impact_target() + 1
}

pub fn impact_unrelated() -> u32 {
    impact_direct_caller() + 2
}
