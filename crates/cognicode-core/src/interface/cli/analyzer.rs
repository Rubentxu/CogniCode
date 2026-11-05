//! The analyzer the CLI answers with.
//!
//! ## Why it is not built where it is used
//!
//! The CLI asks the product a question through the `Analyze` port, and
//! the port is implemented by `ServiceAnalyzer`. What the CLI adds on top
//! is not another analyzer: it is the daemon's word, when there is a
//! daemon to ask. Building that here —instead of in the command that
//! happens to need it— keeps the wiring in one place, so every command
//! that goes through the port asks the same daemon.
//!
//! ## What the daemon is worth to a CLI
//!
//! The product already shares the graph it calculated (the durable
//! snapshot), so a new process does not parse the tree again. What it did
//! pay, every time, was the freshness check: the content of every file,
//! hashed, to find out whether the index still describes the tree. A
//! daemon that watches the tree knows that without reading it.
//!
//! Without a daemon the analyzer is exactly the one there was: the
//! product decides the freshness by itself, exactly as it always did.

use std::path::Path;
use std::sync::Arc;

use crate::application::consumer_analysis::ServiceAnalyzer;
use crate::application::graph_cache;
use crate::interface::daemon::DaemonOracle;

/// The analyzer the CLI answers with, asking the daemon when there is one.
pub fn cli_analyzer(workspace: &Path) -> ServiceAnalyzer {
    let cache_root = graph_cache::default_cache_root();
    let analyzer = ServiceAnalyzer::with_cache_root(cache_root.clone());
    match cache_root {
        Some(root) => analyzer.with_oracle(Some(Arc::new(DaemonOracle::for_workspace(
            &root, workspace,
        )))),
        None => analyzer,
    }
}
