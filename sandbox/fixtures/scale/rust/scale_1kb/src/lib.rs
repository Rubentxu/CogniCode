//! Scale fixture - auto-generated Rust library
//! Size: 1 modules

pub mod module_1;

/// Main entry point - computes across all modules
pub fn compute_all(input: u64) -> u64 {
    let mut result = input;
    result = result.wrapping_add(module_1::compute_1(result));
    result
}
