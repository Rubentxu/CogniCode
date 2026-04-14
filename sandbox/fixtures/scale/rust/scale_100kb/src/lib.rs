//! Scale fixture - auto-generated Rust library
//! Size: 25 modules

pub mod module_1;
pub mod module_2;
pub mod module_3;
pub mod module_4;
pub mod module_5;
pub mod module_6;
pub mod module_7;
pub mod module_8;
pub mod module_9;
pub mod module_10;
pub mod module_11;
pub mod module_12;
pub mod module_13;
pub mod module_14;
pub mod module_15;
pub mod module_16;
pub mod module_17;
pub mod module_18;
pub mod module_19;
pub mod module_20;
pub mod module_21;
pub mod module_22;
pub mod module_23;
pub mod module_24;
pub mod module_25;

/// Main entry point - computes across all modules
pub fn compute_all(input: u64) -> u64 {
    let mut result = input;
    result = result.wrapping_add(module_1::compute_1(result));
    result = result.wrapping_add(module_2::compute_2(result));
    result = result.wrapping_add(module_3::compute_3(result));
    result = result.wrapping_add(module_4::compute_4(result));
    result = result.wrapping_add(module_5::compute_5(result));
    result = result.wrapping_add(module_6::compute_6(result));
    result = result.wrapping_add(module_7::compute_7(result));
    result = result.wrapping_add(module_8::compute_8(result));
    result = result.wrapping_add(module_9::compute_9(result));
    result = result.wrapping_add(module_10::compute_10(result));
    result = result.wrapping_add(module_11::compute_11(result));
    result = result.wrapping_add(module_12::compute_12(result));
    result = result.wrapping_add(module_13::compute_13(result));
    result = result.wrapping_add(module_14::compute_14(result));
    result = result.wrapping_add(module_15::compute_15(result));
    result = result.wrapping_add(module_16::compute_16(result));
    result = result.wrapping_add(module_17::compute_17(result));
    result = result.wrapping_add(module_18::compute_18(result));
    result = result.wrapping_add(module_19::compute_19(result));
    result = result.wrapping_add(module_20::compute_20(result));
    result = result.wrapping_add(module_21::compute_21(result));
    result = result.wrapping_add(module_22::compute_22(result));
    result = result.wrapping_add(module_23::compute_23(result));
    result = result.wrapping_add(module_24::compute_24(result));
    result = result.wrapping_add(module_25::compute_25(result));
    result
}
