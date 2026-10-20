// Corpus root for the impact analysis and the call hierarchy.
//
// The impact analysis takes impact_target as the analyzed symbol;
// its dependents come from direct.rs and transitive.rs.
//
// The outgoing call hierarchy needs impact_target to CALL
// other symbols so outgoing direction produces results. We define
// two helpers here that impact_target delegates to; outgoing depth=1
// must include both.

pub fn impact_target() -> u32 {
    impact_callee_a() + impact_callee_b()
}

pub fn impact_callee_a() -> u32 {
    1
}

pub fn impact_callee_b() -> u32 {
    2
}

pub fn impact_unused() -> u32 {
    3
}
