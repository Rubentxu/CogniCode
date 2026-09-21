// Fixture: nested module
pub mod deeply_nested;
pub fn mid_level() -> u32 {
    crate::nested::deeply_nested::leaf()
}
