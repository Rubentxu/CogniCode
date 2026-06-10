//! In-memory `SpaceRegistry` — CRUD over federation spaces.
//!
//! Holds an ordered `VecDeque<(SpaceId, Space)>` for stable
//! insertion-order iteration, plus a `HashMap<SpaceId, Space>` for
//! O(1) lookups by id. The two structures are kept in sync on
//! every mutating operation; `register` is the only place that
//! can fail (on a duplicate id), and the rest are infallible
//! reads or boolean tells.
//!
//! Used by `BrainSessionService` to track the spaces a session
//! has registered. Persisted in the session state as
//! `Vec<SpaceId>`; the registry is a per-session in-memory cache.
//!
//! Gated behind the `multimodal` Cargo feature. On a default build
//! the module is absent from the crate.

use std::collections::{HashMap, VecDeque};

use cognicode_core::domain::value_objects::SpaceError;
use cognicode_core::domain::value_objects::{Space, SpaceId};

/// In-memory space registry. Cheap to clone (the inner data sits
/// behind `Arc<Mutex<_>>` is NOT what we do here — this is a
/// session-private cache; consumers either wrap it in `Arc<Mutex<_>>`
/// at the call site or hold it as a private field).
#[derive(Debug, Default, Clone)]
pub struct SpaceRegistry {
    by_id: HashMap<SpaceId, Space>,
    order: VecDeque<SpaceId>,
}

impl SpaceRegistry {
    /// Construct an empty registry. Cheap; no I/O.
    pub fn new() -> Self {
        Self {
            by_id: HashMap::new(),
            order: VecDeque::new(),
        }
    }

    /// Register a new space. Re-registering the same id returns
    /// `Err(SpaceError::Duplicate)`. Existing entries are NOT
    /// overwritten — the call is atomic.
    pub fn register(&mut self, space: Space) -> Result<SpaceId, SpaceError> {
        if self.by_id.contains_key(&space.id) {
            return Err(SpaceError::Duplicate(space.id.to_string()));
        }
        self.order.push_back(space.id.clone());
        self.by_id.insert(space.id.clone(), space);
        Ok(self.order.back().cloned().expect("just pushed"))
    }

    /// Look up a space by id. Returns `None` for unknown ids.
    pub fn get(&self, id: &SpaceId) -> Option<&Space> {
        self.by_id.get(id)
    }

    /// List every registered space in insertion order.
    pub fn list(&self) -> Vec<Space> {
        self.order
            .iter()
            .filter_map(|id| self.by_id.get(id).cloned())
            .collect()
    }

    /// List the ids in insertion order. Useful for serialisation
    /// and for callers that need to know "is space X registered?"
    /// without cloning the full `Space`.
    pub fn list_ids(&self) -> Vec<SpaceId> {
        self.order.iter().cloned().collect()
    }

    /// Remove a space by id. Returns `true` when the id was
    /// present and removed, `false` when it was unknown (idempotent).
    pub fn unregister(&mut self, id: &SpaceId) -> bool {
        if self.by_id.remove(id).is_some() {
            self.order.retain(|sid| sid != id);
            true
        } else {
            false
        }
    }

    /// Number of registered spaces. O(1).
    pub fn len(&self) -> usize {
        self.by_id.len()
    }

    /// `true` when no space is registered.
    pub fn is_empty(&self) -> bool {
        self.by_id.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cognicode_core::domain::value_objects::SpaceKind;

    fn make_space(id: &str, name: &str) -> Space {
        Space::try_new(SpaceId::try_new(id).unwrap(), name.into(), SpaceKind::Repo).unwrap()
    }

    /// `register` inserts a new space; `get` returns it by id.
    #[test]
    fn space_registry_register_and_get() {
        let mut reg = SpaceRegistry::new();
        let space = make_space("a", "auth");
        reg.register(space.clone()).expect("register ok");
        let got = reg.get(&SpaceId::try_new("a").unwrap());
        assert_eq!(got, Some(&space));
    }

    /// Re-registering the same id returns `Duplicate`. Existing
    /// entry is NOT overwritten.
    #[test]
    fn space_registry_register_duplicate_returns_err() {
        let mut reg = SpaceRegistry::new();
        reg.register(make_space("a", "auth")).unwrap();
        let result = reg.register(make_space("a", "other"));
        assert_eq!(result, Err(SpaceError::Duplicate("a".into())));
        // The original entry is intact.
        let got = reg.get(&SpaceId::try_new("a").unwrap()).unwrap();
        assert_eq!(got.name, "auth");
    }

    /// `get` on an unknown id returns `None`.
    #[test]
    fn space_registry_get_unknown_returns_none() {
        let reg = SpaceRegistry::new();
        assert!(reg.get(&SpaceId::try_new("missing").unwrap()).is_none());
    }

    /// `list` preserves insertion order across multiple registers.
    #[test]
    fn space_registry_list_preserves_insertion_order() {
        let mut reg = SpaceRegistry::new();
        reg.register(make_space("a", "alpha")).unwrap();
        reg.register(make_space("b", "beta")).unwrap();
        reg.register(make_space("c", "gamma")).unwrap();
        let ids: Vec<String> = reg
            .list()
            .into_iter()
            .map(|s| s.id.to_string())
            .collect();
        assert_eq!(ids, vec!["a", "b", "c"]);
    }

    /// `unregister` on an existing id returns `true` and removes
    /// the entry.
    #[test]
    fn space_registry_unregister_existing_returns_true() {
        let mut reg = SpaceRegistry::new();
        reg.register(make_space("a", "alpha")).unwrap();
        reg.register(make_space("b", "beta")).unwrap();
        assert!(reg.unregister(&SpaceId::try_new("a").unwrap()));
        assert!(reg.get(&SpaceId::try_new("a").unwrap()).is_none());
        assert_eq!(reg.len(), 1);
    }

    /// `unregister` on an unknown id returns `false` (idempotent).
    #[test]
    fn space_registry_unregister_unknown_returns_false() {
        let mut reg = SpaceRegistry::new();
        assert!(!reg.unregister(&SpaceId::try_new("missing").unwrap()));
    }

    /// After 5 sequential `register` calls, `list_ids` is the
    /// exact insertion order.
    #[test]
    fn space_registry_sequential_registers_in_order() {
        let mut reg = SpaceRegistry::new();
        for letter in ["a", "b", "c", "d", "e"] {
            reg.register(make_space(letter, letter)).unwrap();
        }
        let ids: Vec<String> = reg
            .list_ids()
            .into_iter()
            .map(|s| s.to_string())
            .collect();
        assert_eq!(ids, vec!["a", "b", "c", "d", "e"]);
    }

    /// `list` returns cloned `Space`s — mutations to the registry
    /// do not retroactively change the returned vec.
    #[test]
    fn space_registry_list_returns_clones() {
        let mut reg = SpaceRegistry::new();
        reg.register(make_space("a", "alpha")).unwrap();
        let snapshot = reg.list();
        reg.register(make_space("b", "beta")).unwrap();
        assert_eq!(snapshot.len(), 1);
        assert_eq!(reg.len(), 2);
    }
}
