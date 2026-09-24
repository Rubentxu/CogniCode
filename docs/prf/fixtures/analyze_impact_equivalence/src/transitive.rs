// F3.W4: transitive caller of impact_target (via impact_direct_caller).

use crate::direct::impact_direct_caller;

pub fn impact_transitive_caller() -> u32 {
    impact_direct_caller() + 3
}
