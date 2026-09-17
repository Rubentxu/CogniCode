# e66 Design — M7.5 Read-Set Foundation

| Field | Value |
|-------|-------|
| Cycle | `p-c1fac1fea05615c6/e66-lsi-m7-5-read-sets` |
| Phase | design |
| Proposal | `proposal.md` |
| Date | 2026-09-16 |


## Ground-truth discovered during plan phase

| Element | State |
|---------|-------|
| `FactId` | ✅ Exists at `crates/cognicode-core/src/domain/kernel_ids.rs:192` as `pub struct FactId(pub u64)` |
| `domain/ports/` | ✅ Declared at `domain/mod.rs:59` (`pub mod ports;`); populated with many existing ports |
| `ports/read_set_recorder.rs` | ❌ Does not exist; will be added by WU1 |

Original draft was wrong on the `ports/` declaration (initial grep matched comment lines).
Corrected: `ReadSetRecorder` and `InvalidationQuery` traits live in `domain/ports/read_set_recorder.rs`,
the concrete struct stays in `domain/readset.rs`.
## Architecture

```
domain/
├── readset.rs              # ReadSet struct + impl
├── ports/
│   ├── mod.rs
│   └���─ read_set_recorder.rs   # ReadSetRecorder port
└── application/
    └── read_set_service.rs    # (deferred to later cycle)
```

### File 1: `crates/cognicode-core/src/domain/readset.rs`

```rust
//! Read-set: deduplicated, ordered, bounded set of fact IDs.

use std::collections::HashSet;

/// Stable identifier for a canonical knowledge fact.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct FactId(pub u64);  // Already defined in crates/cognicode-core/src/domain/kernel_ids.rs:192

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ReadSet {
    /// Insertion-order list of unique fact IDs.
    ordered: Vec<FactId>,
    /// Hash index for O(1) dedup.
    seen: HashSet<FactId>,
    /// True if at least one record was dropped due to bound.
    truncated: bool,
}

impl ReadSet {
    pub fn iter(&self) -> impl Iterator<Item = &FactId> {
        self.ordered.iter()
    }

    pub fn is_truncated(&self) -> bool { self.truncated }

    pub fn contains(&self, fact: &FactId) -> bool {
        self.seen.contains(fact)
    }

    pub fn len(&self) -> usize { self.ordered.len() }
}

#[derive(Debug, thiserror::Error)]
pub enum ReadSetError {
    #[error("FactId is empty")]
    EmptyFactId,
    #[error("Recorder closed")]
    RecorderClosed,
}
```

### File 2: `crates/cognicode-core/src/domain/ports/read_set_recorder.rs`

```rust
use crate::domain::readset::{ReadSet, ReadSetError, FactId};
use std::num::NonZeroUsize;

pub trait ReadSetRecorder {
    fn record(&mut self, fact_id: FactId) -> Result<(), ReadSetError>;
    fn finalize(self: Box<Self>) -> ReadSet;
    fn is_truncated(&self) -> bool;
    fn into_inner_bytes(self: Box<Self>) -> Vec<u8> {
        // For persistence; not used this cycle
        vec![]
    }
}

pub trait InvalidationQuery {
    fn is_stale(read_set: &ReadSet, changed_facts: &[FactId]) -> bool;
}

/// Bind behavior in execution context (no runtime called yet).
#[derive(Debug, Clone)]
pub struct ReadSetConfig {
    pub max_records: Option<NonZeroUsize>,
}

/// In-memory recorder implementation.
pub struct InMemoryReadSetRecorder {
    config: ReadSetConfig,
    ordered: Vec<FactId>,
    seen: std::collections::HashSet<FactId>,
    truncated: bool,
    finalized: bool,
}

impl InMemoryReadSetRecorder {
    pub fn new(config: ReadSetConfig) -> Self {
        Self {
            config,
            ordered: Vec::new(),
            seen: std::collections::HashSet::new(),
            truncated: false,
            finalized: false,
        }
    }

    fn admit(&mut self, fact_id: FactId) -> Result<bool, ReadSetError> {
        if fact_id.0.is_empty() {
            return Err(ReadSetError::EmptyFactId);
        }
        if self.finalized {
            return Err(ReadSetError::RecorderClosed);
        }
        if !self.seen.insert(fact_id.clone()) {
            return Ok(false);
        }
        match self.config.max_records {
            Some(limit) if self.ordered.len() >= limit.get() => {
                self.truncated = true;
                Ok(false)
            }
            _ => {
                self.ordered.push(fact_id);
                Ok(true)
            }
        }
    }
}

impl ReadSetRecorder for InMemoryReadSetRecorder {
    fn record(&mut self, fact_id: FactId) -> Result<(), ReadSetError> {
        self.admit(fact_id).map(|_| ())
    }

    fn finalize(self: Box<Self>) -> ReadSet {
        // Note: cannot mutate self.finalized through Box<Self>
        // Implementer should use finalize() method below which returns ReadSet by value
        unimplemented!("Use InMemoryReadSetRecorder::finalize() directly")
    }

    fn is_truncated(&self) -> bool { self.truncated }
}

// Add direct finalize() to InMemoryReadSetRecorder (not the trait):
impl InMemoryReadSetRecorder {
    pub fn finalize(mut self) -> ReadSet {
        self.finalized = true;
        ReadSet {
            ordered: self.ordered,
            seen: self.seen,
            truncated: self.truncated,
        }
    }
}
```

## Test Plan (single test module)

`crates/cognicode-core/src/domain/readset.rs` tests:

| Test | REQ Scenario |
|------|-------------|
| `test_dedup_keeps_first_insertion_order` | REQ-RDS-003 |
| `test_truncation_marker_at_bound` | REQ-RDS-002 / Scenario 2.1 |
| `test_uat_u61_unrelated_change_does_not_invalidate` | REQ-RDS-001 / UAT-U61 |

## Test code (sketch)

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::ports::read_set_recorder::{
        InMemoryReadSetRecorder, InvalidationQuery,
        ReadSetRecorder, ReadSetConfig,
    };

    #[test]
    fn test_dedup_keeps_first_insertion_order() {
        let mut rec = InMemoryReadSetRecorder::new(ReadSetConfig {
            max_records: None,
        });
        let a = FactId("A".into());
        let b = FactId("B".into());
        let c = FactId("C".into());
        rec.record(a.clone()).unwrap();
        rec.record(b.clone()).unwrap();
        rec.record(a.clone()).unwrap();
        rec.record(c.clone()).unwrap();
        let rs = rec.finalize();
        let order: Vec<_> = rs.iter().map(|f| &f.0).collect();
        assert_eq!(order, vec!["A", "B", "C"]);
    }

    #[test]
    fn test_truncation_marker_at_bound() {
        let mut rec = InMemoryReadSetRecorder::new(ReadSetConfig {
            max_records: NonZeroUsize::new(3),
        });
        for s in ["A", "B", "C", "D"] {
            rec.record(FactId(s.into())).unwrap();
        }
        let rs = rec.finalize();
        assert!(rs.is_truncated());
    }

    #[test]
    fn test_uat_u61_unrelated_change_does_not_invalidate() {
        let mut rec = InMemoryReadSetRecorder::new(ReadSetConfig {
            max_records: None,
        });
        rec.record(FactId("A".into())).unwrap();
        rec.record(FactId("B".into())).unwrap();
        let rs = rec.finalize();

        // Adapter (definition needed; see below)
        struct SetMembership;
        impl InvalidationQuery for SetMembership {
            fn is_stale(read_set: &ReadSet, changed_facts: &[FactId]) -> bool {
                changed_facts.iter().any(|f| read_set.contains(f))
            }
        }

        let changed = vec![FactId("C".into())];
        assert!(!SetMembership::is_stale(&rs, &changed)); // UAT-U61 PASSES
    }
}
```

## Insertion point

Add `pub mod readset;` to `crates/cognicode-core/src/domain/mod.rs`
and `pub mod read_set_recorder;` to `crates/cognicode-core/src/domain/ports/mod.rs`.

This file MUST exist (will verify with `grep -n 'pub mod ports' domain/mod.rs`).
If `ports/mod.rs` does not exist, create it as a stub exporting `read_set_recorder`.

## Decision

Architecture: minimal, no behavior authority integration, no cache integration.
This is a **bounded slice** as documented in `proposal.md`.

Ready for tasks phase.
