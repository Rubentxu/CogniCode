//! Scale fixture - auto-generated Rust library
//! Size: 2 modules

pub mod module_1;
pub mod module_2;

/// Main entry point - computes across all modules
pub fn compute_all(input: u64) -> u64 {
    let mut result = input;
    result = result.wrapping_add(module_1::compute_1(result));
    result = result.wrapping_add(module_2::compute_2(result));
    result
}
