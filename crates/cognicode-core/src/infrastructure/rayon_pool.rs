//! The rayon pool every binary of the product shares.

use rayon::ThreadPoolBuilder;

/// Stack of each worker thread. The parsers recurse over the syntax tree and
/// the default stack is too small for deeply nested sources.
const WORKER_STACK_BYTES: usize = 8 * 1024 * 1024;

/// Starts the global rayon pool with the product's stack size.
///
/// A pool that already exists is kept and reported: the process that started
/// it chose its configuration first. Any other failure to start it is fatal,
/// because the analysis would run on threads that overflow on real sources.
pub fn init_rayon() {
    match ThreadPoolBuilder::new()
        .stack_size(WORKER_STACK_BYTES)
        .build_global()
    {
        Ok(()) => tracing::info!("Rayon global thread pool initialized with 8 MB stack size"),
        Err(e) => {
            let msg = e.to_string();
            if msg.contains("already been initialized") {
                tracing::warn!(
                    "Rayon global thread pool already initialized; using existing configuration"
                );
            } else {
                panic!("Failed to initialize Rayon global thread pool: {e}");
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::init_rayon;

    #[test]
    fn starting_the_pool_twice_keeps_the_first_one() {
        init_rayon();
        init_rayon();
        assert!(rayon::current_num_threads() >= 1);
    }
}
