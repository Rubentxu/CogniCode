//! A bounded cache of one derived value per published snapshot of the call
//! graph.
//!
//! Several things are a pure function of one immutable [`CallGraph`] and cost
//! more to compute than a query costs to answer: the ranking statistics, the
//! read projection. [`SnapshotCache`] lets every query of a snapshot share one
//! of them. It knows nothing about the value it holds, only how to key it,
//! bound it and build it once.
//!
//! # Identity and invalidation
//!
//! Entries are keyed by the identity of the published immutable
//! `Arc<CallGraph>` (the entry of the checkpoint ring), not by a revision
//! number: the in-memory single-version store reports the same checkpoint id
//! after every overwrite, so a number cannot tell two snapshots apart, while
//! the allocation can. The cache holds a `Weak` pointer to the graph, which
//! has two consequences the cache relies on:
//!
//! - the allocation of a keyed graph cannot be reused for another graph
//!   while the entry exists, so an address never aliases two snapshots;
//! - `Arc::get_mut` refuses to hand out `&mut` to a graph that has a `Weak`,
//!   so a keyed graph cannot be edited in place under the cache.
//!
//! When the last strong reference to a snapshot goes away its entry is
//! dropped on the next lookup: a value of a retired snapshot is never served.
//!
//! # Memory
//!
//! The cache holds at most [`SnapshotCacheLimits::max_entries`] snapshots and
//! at most [`SnapshotCacheLimits::max_symbols`] symbols summed over them,
//! evicting the least recently used first. A single snapshot larger than the
//! symbol budget is answered from a value that is not retained.
//!
//! # Concurrency
//!
//! Lookups take a short lock that never covers a build. Concurrent first
//! lookups of the same snapshot build once: the others wait for that build
//! and share its result.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, OnceLock, PoisonError, Weak};

use crate::domain::aggregates::CallGraph;

/// How much a [`SnapshotCache`] may keep.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SnapshotCacheLimits {
    /// Most snapshots kept at once. The snapshot being served is always
    /// kept while it is in use, so values below one behave as one.
    pub max_entries: usize,
    /// Most symbols kept, summed over the snapshots. A snapshot with more
    /// symbols than this is never retained.
    pub max_symbols: usize,
}

impl Default for SnapshotCacheLimits {
    /// Two snapshots (the head and the one before it, as the checkpoint ring
    /// retains) within two million symbols.
    fn default() -> Self {
        Self {
            max_entries: 2,
            max_symbols: 2_000_000,
        }
    }
}

/// What a [`SnapshotCache`] has done so far.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct SnapshotCacheCounters {
    /// Times a value was built.
    pub builds: u64,
    /// Lookups served by an entry that already existed.
    pub hits: u64,
    /// Entries dropped for the budget or because their snapshot is gone.
    pub evictions: u64,
    /// Lookups answered from a value that was not retained because the
    /// snapshot alone is over the symbol budget.
    pub bypasses: u64,
    /// Snapshots held now.
    pub entries: usize,
    /// Symbols held now, summed over the snapshots.
    pub symbols: usize,
}

struct Slot<V> {
    graph: Weak<CallGraph>,
    symbols: usize,
    value: Arc<OnceLock<V>>,
}

/// Shares one value of type `V` per published snapshot between queries. See
/// the [module documentation](self) for identity, invalidation, memory and
/// concurrency. `V` is what a lookup hands out, so it is cheap to clone
/// (an `Arc`).
pub struct SnapshotCache<V> {
    limits: SnapshotCacheLimits,
    /// Least recently used first.
    slots: Mutex<Vec<Slot<V>>>,
    builds: AtomicU64,
    hits: AtomicU64,
    evictions: AtomicU64,
    bypasses: AtomicU64,
}

impl<V: Clone> Default for SnapshotCache<V> {
    fn default() -> Self {
        Self::new(SnapshotCacheLimits::default())
    }
}

impl<V: Clone> SnapshotCache<V> {
    /// An empty cache that keeps at most what `limits` allow.
    pub fn new(limits: SnapshotCacheLimits) -> Self {
        Self {
            limits,
            slots: Mutex::new(Vec::new()),
            builds: AtomicU64::new(0),
            hits: AtomicU64::new(0),
            evictions: AtomicU64::new(0),
            bypasses: AtomicU64::new(0),
        }
    }

    /// The value of `graph`, built by `build` for the first caller and shared
    /// by the rest for as long as the entry lives.
    pub fn get_or_build(&self, graph: &Arc<CallGraph>, build: impl FnOnce() -> V) -> V {
        let symbols = graph.symbol_count();
        if symbols > self.limits.max_symbols {
            self.bypasses.fetch_add(1, Ordering::Relaxed);
            self.builds.fetch_add(1, Ordering::Relaxed);
            return build();
        }
        let cell = self.slot_for(graph, symbols);
        cell.get_or_init(|| {
            self.builds.fetch_add(1, Ordering::Relaxed);
            build()
        })
        .clone()
    }

    /// Finds or creates the entry of `graph`, most recently used last, and
    /// brings the cache back within its limits. Never builds.
    fn slot_for(&self, graph: &Arc<CallGraph>, symbols: usize) -> Arc<OnceLock<V>> {
        let key = Arc::downgrade(graph);
        let mut slots = self.slots.lock().unwrap_or_else(PoisonError::into_inner);

        let before = slots.len();
        slots.retain(|slot| slot.graph.strong_count() > 0);
        self.count_evictions(before - slots.len());

        if let Some(position) = slots.iter().position(|slot| slot.graph.ptr_eq(&key)) {
            self.hits.fetch_add(1, Ordering::Relaxed);
            let slot = slots.remove(position);
            let cell = Arc::clone(&slot.value);
            slots.push(slot);
            return cell;
        }

        let cell = Arc::new(OnceLock::new());
        slots.push(Slot {
            graph: key,
            symbols,
            value: Arc::clone(&cell),
        });
        // The entry just added is last and is never the one to go: it fits
        // the symbol budget on its own.
        let mut evicted = 0;
        while slots.len() > 1
            && (slots.len() > self.limits.max_entries
                || slots.iter().map(|slot| slot.symbols).sum::<usize>() > self.limits.max_symbols)
        {
            slots.remove(0);
            evicted += 1;
        }
        self.count_evictions(evicted);
        cell
    }

    fn count_evictions(&self, evicted: usize) {
        if evicted > 0 {
            self.evictions.fetch_add(evicted as u64, Ordering::Relaxed);
        }
    }

    /// What the cache has done and holds right now.
    pub fn counters(&self) -> SnapshotCacheCounters {
        let slots = self.slots.lock().unwrap_or_else(PoisonError::into_inner);
        SnapshotCacheCounters {
            builds: self.builds.load(Ordering::Relaxed),
            hits: self.hits.load(Ordering::Relaxed),
            evictions: self.evictions.load(Ordering::Relaxed),
            bypasses: self.bypasses.load(Ordering::Relaxed),
            entries: slots.len(),
            symbols: slots.iter().map(|slot| slot.symbols).sum(),
        }
    }
}
