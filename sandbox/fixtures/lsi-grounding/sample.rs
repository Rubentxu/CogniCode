//! Small canonical fixture for e67 grounded ingest.
//!
//! Shape rationale (mirrors e37 goldens convention):
//!
//! - One file-level `mod inner { fn helper() }` exercises `core:contains`
//!   (sample.rs contains inner) and a nested `core:defines` for `helper`.
//! - One top-level `fn greet()` adds a second `core:defines`.
//! - The call `inner::helper()` inside `greet` adds `core:calls` (subject =
//!   `greet`'s FQN, object = `inner::helper`'s FQN).
//!
//! Expected canonical predicate set (deterministic via tree_sitter_facts):
//! `{core:defines, core:contains, core:calls}` (the other two predicates
//! from the bridge — `core:imports`, `core:references` — require no
//! corresponding source construct in this fixture).

pub mod inner {
    pub fn helper() -> u32 {
        42
    }
}

pub fn greet() -> u32 {
    inner::helper()
}