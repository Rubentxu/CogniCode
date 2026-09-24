// F3.W4 + F3.W5 corpus: callers of impact_target.
// F3.W4 needs direct dependents; F3.W5 needs callers that the
// transitive.rs further transitively calls.

use crate::impact_target;

pub fn impact_direct_caller() -> u32 {
    impact_target() + 1
}

pub fn impact_unrelated() -> u32 {
    impact_direct_caller() + 2
}
