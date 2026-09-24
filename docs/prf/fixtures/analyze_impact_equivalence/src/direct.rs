// F3.W4: direct caller of impact_target.

use crate::impact_target;

pub fn impact_direct_caller() -> u32 {
    impact_target() + 1
}

pub fn impact_unrelated() -> u32 {
    impact_direct_caller() + 2
}
