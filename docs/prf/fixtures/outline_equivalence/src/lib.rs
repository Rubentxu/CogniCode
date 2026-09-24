// F3.W3: get_outline equivalence corpus.
// All top-level symbols use unique outline_* names; private symbol
// uses underscore prefix.

pub fn outline_alpha() -> u32 {
    outline_beta() + 1
}

pub fn outline_beta() -> u32 {
    42
}

fn _outline_private() -> u32 {
    outline_alpha() + 1
}

pub mod nested {
    pub fn outline_gamma() -> u32 {
        super::outline_beta() + 1
    }
}
