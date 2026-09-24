// F3.W2: query_symbol_index equivalence corpus.
// All symbols have unique_* names so they don't collide with other
// test corpora in the repo.

pub fn unique_alpha() -> u32 {
    1
}

pub fn unique_beta() -> u32 {
    unique_alpha() + 1
}

pub mod nested {
    pub fn unique_gamma() -> u32 {
        super::unique_beta() + 1
    }
}
