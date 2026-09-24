// F3.W4 + F3.W5 corpus root.
//
// F3.W4 (analyze_impact): impact_target is the analyzed symbol;
// its dependents come from direct.rs and transitive.rs.
//
// F3.W5 (get_call_hierarchy outgoing): impact_target must CALL
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
