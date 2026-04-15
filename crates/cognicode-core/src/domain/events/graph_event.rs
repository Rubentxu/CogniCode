//! Domain events for incremental graph updates
//!
//! These events represent changes to symbols in the code graph
//! and are used to incrementally update the graph instead of
//! rebuilding it from scratch.

use std::collections::HashMap;

use crate::domain::value_objects::{DependencyType, Location, SymbolKind};

/// Events that represent changes to symbols in the call graph
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GraphEvent {
    /// A new symbol was added to the codebase
    SymbolAdded(SymbolAddedEvent),
    /// A symbol was removed from the codebase
    SymbolRemoved(SymbolRemovedEvent),
    /// A symbol was modified (signature, location, etc.)
    SymbolModified(SymbolModifiedEvent),
    /// A dependency edge was added
    DependencyAdded(DependencyEvent),
    /// A dependency edge was removed
    DependencyRemoved(DependencyEvent),
}

impl GraphEvent {
    /// Returns the file path associated with this event
    pub fn file_path(&self) -> Option<&str> {
        match self {
            GraphEvent::SymbolAdded(e) => Some(e.location.file()),
            GraphEvent::SymbolRemoved(e) => Some(e.location.file()),
            GraphEvent::SymbolModified(e) => Some(e.old_location.file()),
            GraphEvent::DependencyAdded(e) => Some(&e.file),
            GraphEvent::DependencyRemoved(e) => Some(&e.file),
        }
    }
}

/// Event fired when a new symbol is added
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SymbolAddedEvent {
    /// The name of the symbol
    pub name: String,
    /// The kind of symbol (function, class, etc.)
    pub kind: SymbolKind,
    /// The location in source code
    pub location: Location,
    /// Optional function signature
    pub signature: Option<String>,
}

/// Event fired when a symbol is removed
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SymbolRemovedEvent {
    /// The name of the symbol
    pub name: String,
    /// The location in source code (for identification)
    pub location: Location,
}

/// Event fired when a symbol is modified
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SymbolModifiedEvent {
    /// The name of the symbol
    pub name: String,
    /// The old location
    pub old_location: Location,
    /// The new location
    pub new_location: Location,
    /// Optional new signature (if changed)
    pub new_signature: Option<String>,
    /// Optional old signature (if changed)
    pub old_signature: Option<String>,
}

/// Event fired when a dependency is added or removed
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DependencyEvent {
    /// The file containing the dependency
    pub file: String,
    /// The source symbol (caller)
    pub source_name: String,
    /// The target symbol (callee)
    pub target_name: String,
    /// The type of dependency
    pub dependency_type: DependencyType,
}

/// Calculates the difference between two sets of symbols
pub struct GraphDiffCalculator;

impl GraphDiffCalculator {
    /// Calculates the diff between old and new symbols
    ///
    /// Returns a list of events that transform the old state to the new state.
    pub fn calculate_diff(
        old_symbols: &[(String, Location, Option<String>)],
        new_symbols: &[(String, Location, Option<String>)],
    ) -> Vec<GraphEvent> {
        let mut events = Vec::new();

        // Use location as the unique identifier since name + location = unique symbol
        // Key format: file:name:line:column to ensure uniqueness across files
        let old_set: std::collections::HashSet<String> = old_symbols
            .iter()
            .map(|(name, loc, _)| {
                format!("{}:{}:{}:{}", loc.file(), name, loc.line(), loc.column())
            })
            .collect();

        let new_set: std::collections::HashSet<String> = new_symbols
            .iter()
            .map(|(name, loc, _)| {
                format!("{}:{}:{}:{}", loc.file(), name, loc.line(), loc.column())
            })
            .collect();

        // Find removed symbols
        for (name, location, _signature) in old_symbols {
            let key = format!(
                "{}:{}:{}:{}",
                location.file(),
                name,
                location.line(),
                location.column()
            );
            if !new_set.contains(&key) {
                events.push(GraphEvent::SymbolRemoved(SymbolRemovedEvent {
                    name: name.clone(),
                    location: location.clone(),
                }));
            }
        }

        // Find added symbols and modifications
        for (name, location, signature) in new_symbols {
            let key = format!(
                "{}:{}:{}:{}",
                location.file(),
                name,
                location.line(),
                location.column()
            );
            if !old_set.contains(&key) {
                events.push(GraphEvent::SymbolAdded(SymbolAddedEvent {
                    name: name.clone(),
                    kind: SymbolKind::Unknown, // Will be determined by parser
                    location: location.clone(),
                    signature: signature.clone(),
                }));
            } else {
                // Check if signature changed
                let old_sig = old_symbols
                    .iter()
                    .find(|(n, l, _)| {
                        n == name
                            && l.file() == location.file()
                            && l.line() == location.line()
                            && l.column() == location.column()
                    })
                    .and_then(|(_, _, sig)| sig.clone());

                if old_sig != *signature {
                    events.push(GraphEvent::SymbolModified(SymbolModifiedEvent {
                        name: name.clone(),
                        old_location: location.clone(),
                        new_location: location.clone(),
                        old_signature: old_sig,
                        new_signature: signature.clone(),
                    }));
                }
            }
        }

        events
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::value_objects::Location;

    #[test]
    fn test_diff_calculator_added_symbol() {
        let old: Vec<(String, Location, Option<String>)> = vec![];
        let new = vec![("func1".to_string(), Location::new("test.rs", 10, 0), None)];

        let events = GraphDiffCalculator::calculate_diff(&old, &new);
        assert_eq!(events.len(), 1);
        match &events[0] {
            GraphEvent::SymbolAdded(e) => {
                assert_eq!(e.name, "func1");
                assert_eq!(e.location.line(), 10);
            }
            _ => panic!("Expected SymbolAdded"),
        }
    }

    #[test]
    fn test_diff_calculator_removed_symbol() {
        let old = vec![("func1".to_string(), Location::new("test.rs", 10, 0), None)];
        let new: Vec<(String, Location, Option<String>)> = vec![];

        let events = GraphDiffCalculator::calculate_diff(&old, &new);
        assert_eq!(events.len(), 1);
        match &events[0] {
            GraphEvent::SymbolRemoved(e) => {
                assert_eq!(e.name, "func1");
            }
            _ => panic!("Expected SymbolRemoved"),
        }
    }

    #[test]
    fn test_diff_calculator_modified_signature() {
        let old = vec![(
            "func1".to_string(),
            Location::new("test.rs", 10, 0),
            Some("fn()".to_string()),
        )];
        let new = vec![(
            "func1".to_string(),
            Location::new("test.rs", 10, 0),
            Some("fn(i32)".to_string()),
        )];

        let events = GraphDiffCalculator::calculate_diff(&old, &new);
        assert_eq!(events.len(), 1);
        match &events[0] {
            GraphEvent::SymbolModified(e) => {
                assert_eq!(e.name, "func1");
                assert_eq!(e.old_signature, Some("fn()".to_string()));
                assert_eq!(e.new_signature, Some("fn(i32)".to_string()));
            }
            _ => panic!("Expected SymbolModified"),
        }
    }

    #[test]
    fn test_diff_calculator_no_changes() {
        let old = vec![(
            "func1".to_string(),
            Location::new("test.rs", 10, 0),
            Some("fn()".to_string()),
        )];
        let new = vec![(
            "func1".to_string(),
            Location::new("test.rs", 10, 0),
            Some("fn()".to_string()),
        )];

        let events = GraphDiffCalculator::calculate_diff(&old, &new);
        assert!(events.is_empty());
    }

    #[test]
    fn test_diff_calculator_multiple_changes() {
        // Old has func1, func2, func3
        let old = vec![
            ("func1".to_string(), Location::new("test.rs", 10, 0), None),
            ("func2".to_string(), Location::new("test.rs", 20, 0), None),
            ("func3".to_string(), Location::new("test.rs", 30, 0), None),
        ];
        // New has func2 (modified), func3 (modified), func4 (added)
        let new = vec![
            (
                "func2".to_string(),
                Location::new("test.rs", 20, 0),
                Some("fn(i32)".to_string()),
            ),
            ("func3".to_string(), Location::new("test.rs", 30, 0), None),
            ("func4".to_string(), Location::new("test.rs", 40, 0), None),
        ];

        let events = GraphDiffCalculator::calculate_diff(&old, &new);

        // Should have: 1 removed (func1), 1 modified (func2), 1 added (func4)
        assert_eq!(events.len(), 3);

        // Verify we have the correct mix of events
        let added: Vec<_> = events
            .iter()
            .filter(|e| matches!(e, GraphEvent::SymbolAdded(_)))
            .collect();
        let removed: Vec<_> = events
            .iter()
            .filter(|e| matches!(e, GraphEvent::SymbolRemoved(_)))
            .collect();
        let modified: Vec<_> = events
            .iter()
            .filter(|e| matches!(e, GraphEvent::SymbolModified(_)))
            .collect();

        assert_eq!(added.len(), 1);
        assert_eq!(removed.len(), 1);
        assert_eq!(modified.len(), 1);
    }

    #[test]
    fn test_diff_calculator_empty_old_new() {
        // Empty old, empty new - no changes
        let old: Vec<(String, Location, Option<String>)> = vec![];
        let new: Vec<(String, Location, Option<String>)> = vec![];

        let events = GraphDiffCalculator::calculate_diff(&old, &new);
        assert!(events.is_empty());
    }

    #[test]
    fn test_diff_calculator_all_removed() {
        let old = vec![
            ("func1".to_string(), Location::new("test.rs", 10, 0), None),
            ("func2".to_string(), Location::new("test.rs", 20, 0), None),
        ];
        let new: Vec<(String, Location, Option<String>)> = vec![];

        let events = GraphDiffCalculator::calculate_diff(&old, &new);
        assert_eq!(events.len(), 2);

        // Both should be SymbolRemoved
        for event in &events {
            assert!(matches!(event, GraphEvent::SymbolRemoved(_)));
        }
    }

    #[test]
    fn test_diff_calculator_all_added() {
        let old: Vec<(String, Location, Option<String>)> = vec![];
        let new = vec![
            ("func1".to_string(), Location::new("test.rs", 10, 0), None),
            ("func2".to_string(), Location::new("test.rs", 20, 0), None),
        ];

        let events = GraphDiffCalculator::calculate_diff(&old, &new);
        assert_eq!(events.len(), 2);

        // Both should be SymbolAdded
        for event in &events {
            assert!(matches!(event, GraphEvent::SymbolAdded(_)));
        }
    }

    #[test]
    fn test_diff_calculator_same_location_different_names() {
        // Same location but different names - should be remove + add (not modify)
        let old = vec![(
            "old_func".to_string(),
            Location::new("test.rs", 10, 0),
            None,
        )];
        let new = vec![(
            "new_func".to_string(),
            Location::new("test.rs", 10, 0),
            None,
        )];

        let events = GraphDiffCalculator::calculate_diff(&old, &new);
        assert_eq!(events.len(), 2);

        let added: Vec<_> = events
            .iter()
            .filter(|e| matches!(e, GraphEvent::SymbolAdded(_)))
            .collect();
        let removed: Vec<_> = events
            .iter()
            .filter(|e| matches!(e, GraphEvent::SymbolRemoved(_)))
            .collect();

        assert_eq!(added.len(), 1);
        assert_eq!(removed.len(), 1);
    }

    #[test]
    fn test_diff_calculator_different_files() {
        let old = vec![("func1".to_string(), Location::new("file1.rs", 10, 0), None)];
        let new = vec![("func1".to_string(), Location::new("file2.rs", 10, 0), None)];

        let events = GraphDiffCalculator::calculate_diff(&old, &new);
        // Different files = different locations, so both removed and added
        assert_eq!(events.len(), 2);
    }

    #[test]
    fn test_diff_calculator_signature_change_only() {
        // Same symbol, same location, only signature changed
        let old = vec![(
            "calculate".to_string(),
            Location::new("test.rs", 5, 0),
            Some("fn(a: i32) -> i32".to_string()),
        )];
        let new = vec![(
            "calculate".to_string(),
            Location::new("test.rs", 5, 0),
            Some("fn(a: i32, b: i32) -> i32".to_string()),
        )];

        let events = GraphDiffCalculator::calculate_diff(&old, &new);
        assert_eq!(events.len(), 1);

        match &events[0] {
            GraphEvent::SymbolModified(e) => {
                assert_eq!(e.name, "calculate");
                assert_eq!(e.old_signature, Some("fn(a: i32) -> i32".to_string()));
                assert_eq!(
                    e.new_signature,
                    Some("fn(a: i32, b: i32) -> i32".to_string())
                );
            }
            _ => panic!("Expected SymbolModified"),
        }
    }

    #[test]
    fn test_diff_calculator_none_to_some_signature() {
        // Old has no signature, new has signature
        let old = vec![("func1".to_string(), Location::new("test.rs", 10, 0), None)];
        let new = vec![(
            "func1".to_string(),
            Location::new("test.rs", 10, 0),
            Some("fn()".to_string()),
        )];

        let events = GraphDiffCalculator::calculate_diff(&old, &new);
        assert_eq!(events.len(), 1);

        match &events[0] {
            GraphEvent::SymbolModified(e) => {
                assert_eq!(e.old_signature, None);
                assert_eq!(e.new_signature, Some("fn()".to_string()));
            }
            _ => panic!("Expected SymbolModified"),
        }
    }

    #[test]
    fn test_diff_calculator_some_to_none_signature() {
        // Old has signature, new has no signature
        let old = vec![(
            "func1".to_string(),
            Location::new("test.rs", 10, 0),
            Some("fn()".to_string()),
        )];
        let new = vec![("func1".to_string(), Location::new("test.rs", 10, 0), None)];

        let events = GraphDiffCalculator::calculate_diff(&old, &new);
        assert_eq!(events.len(), 1);

        match &events[0] {
            GraphEvent::SymbolModified(e) => {
                assert_eq!(e.old_signature, Some("fn()".to_string()));
                assert_eq!(e.new_signature, None);
            }
            _ => panic!("Expected SymbolModified"),
        }
    }

    #[test]
    fn test_graph_event_file_path() {
        let added_event = GraphEvent::SymbolAdded(SymbolAddedEvent {
            name: "func1".to_string(),
            kind: SymbolKind::Function,
            location: Location::new("test.rs", 10, 0),
            signature: None,
        });
        assert_eq!(added_event.file_path(), Some("test.rs"));

        let removed_event = GraphEvent::SymbolRemoved(SymbolRemovedEvent {
            name: "func1".to_string(),
            location: Location::new("test.rs", 10, 0),
        });
        assert_eq!(removed_event.file_path(), Some("test.rs"));

        let modified_event = GraphEvent::SymbolModified(SymbolModifiedEvent {
            name: "func1".to_string(),
            old_location: Location::new("old.rs", 10, 0),
            new_location: Location::new("new.rs", 20, 0),
            old_signature: None,
            new_signature: None,
        });
        // file_path returns old_location for modified
        assert_eq!(modified_event.file_path(), Some("old.rs"));
    }

    #[test]
    fn test_dependency_events_file_path() {
        let dep_added = GraphEvent::DependencyAdded(DependencyEvent {
            file: "main.rs".to_string(),
            source_name: "main".to_string(),
            target_name: "helper".to_string(),
            dependency_type: DependencyType::Calls,
        });
        assert_eq!(dep_added.file_path(), Some("main.rs"));

        let dep_removed = GraphEvent::DependencyRemoved(DependencyEvent {
            file: "lib.rs".to_string(),
            source_name: "init".to_string(),
            target_name: "setup".to_string(),
            dependency_type: DependencyType::Calls,
        });
        assert_eq!(dep_removed.file_path(), Some("lib.rs"));
    }

    // Note: DependencyAdded/DependencyRemoved events are NOT generated by
    // GraphDiffCalculator::calculate_diff() - it only handles symbol-level changes.
    // The dependency events are defined for future use when dependency diffing is implemented.
}
