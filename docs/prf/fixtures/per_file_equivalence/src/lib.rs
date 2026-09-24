// F3.W1.a: intra-file dependency corpus for CLI↔MCP equivalence.
// `caller` depends on `callee` within the same file.

fn callee() -> u32 {
    42
}

fn caller() -> u32 {
    callee() + 1
}

pub mod nested {
    pub fn inner_caller() -> u32 {
        super::caller() + 1
    }
}
