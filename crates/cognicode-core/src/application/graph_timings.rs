//! What each stage of a graph answer cost, measured by the product itself.
//!
//! A total elapsed time cannot tell a cache hit from a cold parse, and a
//! baseline that cannot tell them apart cannot be compared. Every field is
//! optional because an answer only measures the stages it ran: a graph
//! served from the durable snapshot did not parse, and a graph built now
//! did not deserialize a snapshot. A missing field is "not measured on
//! this route", never zero.

use serde::{Deserialize, Serialize};

/// The cost of each stage of `load_or_build_graph`, in milliseconds.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct GraphPhaseTimings {
    /// Deserializing the durable snapshot on a cache hit.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cache_lookup_ms: Option<u64>,
    /// Walking the tree and deciding which files are sources.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub walk_ms: Option<u64>,
    /// Reading and parsing the files the file cache did not hold.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parse_ms: Option<u64>,
    /// Folding the parsed facts into declarations, calls and relations.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ingest_ms: Option<u64>,
    /// Tying references to declarations and assembling the graph.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resolve_ms: Option<u64>,
    /// The whole build, from the walk to the assembled graph.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub build_ms: Option<u64>,
    /// Serializing and writing the durable snapshot after a complete build.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub snapshot_serialize_ms: Option<u64>,
    /// Files parsed in this build (not served by the file cache).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub files_parsed: Option<usize>,
    /// Files served by the in-process file cache during this build.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub files_cached: Option<usize>,
}

impl GraphPhaseTimings {
    /// Whether any stage was measured at all.
    pub fn is_empty(&self) -> bool {
        self == &Self::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unmeasured_stages_stay_out_of_the_json() {
        let timings = GraphPhaseTimings {
            cache_lookup_ms: Some(3),
            ..Default::default()
        };
        let json = serde_json::to_value(timings).unwrap();
        assert_eq!(json["cache_lookup_ms"], 3);
        assert!(json.get("parse_ms").is_none());
        assert!(json.get("walk_ms").is_none());
    }

    #[test]
    fn a_json_without_the_field_reads_as_unmeasured() {
        let timings: GraphPhaseTimings = serde_json::from_str("{}").unwrap();
        assert!(timings.is_empty());
    }

    #[test]
    fn round_trip_keeps_every_measured_stage() {
        let timings = GraphPhaseTimings {
            walk_ms: Some(1),
            parse_ms: Some(2),
            ingest_ms: Some(3),
            resolve_ms: Some(4),
            build_ms: Some(10),
            snapshot_serialize_ms: Some(5),
            files_parsed: Some(6),
            files_cached: Some(7),
            cache_lookup_ms: None,
        };
        let json = serde_json::to_string(&timings).unwrap();
        let back: GraphPhaseTimings = serde_json::from_str(&json).unwrap();
        assert_eq!(back, timings);
        assert!(!back.is_empty());
    }
}
