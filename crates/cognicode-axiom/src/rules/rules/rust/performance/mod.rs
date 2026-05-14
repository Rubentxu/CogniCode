//! Performance bug detection rules for Rust
//!
//! Detects common performance issues including unnecessary allocations,
//! clones in hot paths, blocking in async contexts, and inefficient patterns.

pub mod perf_helpers;
pub mod perf_001_forgotten_alloc;
pub mod perf_002_unnecessary_alloc;
pub mod perf_003_clone_hot_path;
pub mod perf_004_vec_push_no_reserve;
pub mod perf_005_string_concat_loop;
pub mod perf_006_n_plus_one_query;
pub mod perf_007_unnecessary_async;
pub mod perf_008_sync_in_async;
pub mod perf_009_large_stack_alloc;
pub mod perf_010_missing_drop;
pub mod perf_011_inefficient_iterator;
pub mod perf_012_box_vec_indirection;

pub use perf_001_forgotten_alloc::PERF_001Rule;
pub use perf_002_unnecessary_alloc::PERF_002Rule;
pub use perf_003_clone_hot_path::PERF_003Rule;
pub use perf_004_vec_push_no_reserve::PERF_004Rule;
pub use perf_005_string_concat_loop::PERF_005Rule;
pub use perf_006_n_plus_one_query::PERF_006Rule;
pub use perf_007_unnecessary_async::PERF_007Rule;
pub use perf_008_sync_in_async::PERF_008Rule;
pub use perf_009_large_stack_alloc::PERF_009Rule;
pub use perf_010_missing_drop::PERF_010Rule;
pub use perf_011_inefficient_iterator::PERF_011Rule;
pub use perf_012_box_vec_indirection::PERF_012Rule;
