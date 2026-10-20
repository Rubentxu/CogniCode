// Fixture: file at top level of src/
pub fn top_level() -> u32 {
    nested::mid_level()
}

pub mod nested;
