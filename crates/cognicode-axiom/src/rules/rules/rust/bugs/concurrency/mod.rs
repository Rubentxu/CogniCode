//! Concurrency bug detection rules for Rust
//!
//! Detects common concurrency issues including race conditions, mutex guard leaks,
//! deadlocks, channel issues, RefCell across await, and concurrent map access.

pub mod s1872_race_condition;
pub mod s1873_mutex_guard_leaked;
pub mod s1874_deadlock;
pub mod s1875_channel_closed_send;
pub mod s1876_refcell_await;
pub mod s1877_unbounded_channel;
pub mod s1878_arc_clone_hot_path;
pub mod s1879_concurrent_map_unsync;

pub use s1872_race_condition::{S1872aRule, S1872bRule};
pub use s1873_mutex_guard_leaked::S1873Rule;
pub use s1874_deadlock::{S1874aRule, S1874bRule};
pub use s1875_channel_closed_send::S1875Rule;
pub use s1876_refcell_await::S1876Rule;
pub use s1877_unbounded_channel::S1877Rule;
pub use s1878_arc_clone_hot_path::S1878Rule;
pub use s1879_concurrent_map_unsync::S1879Rule;
