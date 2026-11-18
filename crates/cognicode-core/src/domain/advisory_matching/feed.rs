//! Defensive reading of an OSV advisory feed.
//!
//! A feed is untrusted input an operator supplied. It is read as data: the
//! reader never executes, fetches or follows anything in it, bounds every
//! dimension an attacker could inflate, and treats a record it cannot read as
//! *rejected* (counted and named) instead of skipping it silently.
//!
//! The accepted shapes are an array of OSV records, an OSV API response
//! (`{"vulns": [...]}`) and a single record. Records stream one at a time, so
//! memory is bounded by the byte limit plus one record.

use serde::de::{DeserializeSeed, Deserializer, MapAccess, SeqAccess, Visitor};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::collections::BTreeMap;
use std::fmt;

use super::record::{
    AdvisoryRange, AdvisoryRecord, AffectedEntry, AffectedPackage, CRATES_IO, RangeEvent, RangeKind,
};

/// Everything the reader bounds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FeedLimits {
    /// The most bytes a feed may have.
    pub max_bytes: usize,
    /// The most records a feed may carry.
    pub max_records: usize,
    /// The most `affected` entries of one record.
    pub max_affected: usize,
    /// The most ranges of one `affected` entry.
    pub max_ranges: usize,
    /// The most events of one range.
    pub max_events: usize,
    /// The most explicit versions of one `affected` entry.
    pub max_versions: usize,
    /// The most aliases of one record.
    pub max_aliases: usize,
    /// The longest identifier, name or version string, in bytes.
    pub max_text: usize,
}

impl Default for FeedLimits {
    fn default() -> Self {
        Self {
            max_bytes: 32 * 1024 * 1024,
            max_records: 100_000,
            max_affected: 256,
            max_ranges: 64,
            max_events: 256,
            max_versions: 10_000,
            max_aliases: 256,
            max_text: 4_096,
        }
    }
}

/// The longest summary kept, in characters.
const SUMMARY_CHARS: usize = 200;
/// The most distinct ecosystem names counted in the coverage.
const MAX_ECOSYSTEMS: usize = 64;
/// The longest ecosystem name counted, in characters.
const ECOSYSTEM_CHARS: usize = 64;

/// Why a record was not accepted.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RejectReason {
    /// The record is not a JSON object.
    NotAnObject,
    /// The identifier is absent, empty, too long or has control characters.
    InvalidId,
    /// `modified` is absent or is not an RFC 3339 timestamp.
    InvalidModified,
    /// The record declares a schema this reader does not read.
    UnsupportedSchemaVersion,
    /// A field has a type or a size the reader does not accept.
    InvalidField,
    /// More `affected` entries than the limit.
    TooManyAffected,
    /// More ranges in one entry than the limit.
    TooManyRanges,
    /// More events in one range than the limit.
    TooManyEvents,
    /// More explicit versions in one entry than the limit.
    TooManyVersions,
    /// More aliases than the limit.
    TooManyAliases,
    /// Two records share an identifier and a `modified` instant but differ:
    /// which one is current cannot be decided.
    DuplicateIdConflict,
}

/// A record the reader did not accept.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct RejectedRecord {
    /// The position of the record in the feed, from zero.
    pub position: usize,
    /// The record identifier, when it could be read.
    pub id: Option<String>,
    /// Why it was rejected.
    pub reason: RejectReason,
}

/// A feed that could not be read at all.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FeedError {
    /// The feed is larger than the byte limit.
    TooLarge {
        /// The size of the feed.
        bytes: usize,
        /// The limit.
        limit: usize,
    },
    /// The bytes are not one JSON document. Carries the position only: a
    /// message quoting the input would echo untrusted content.
    Malformed {
        /// The line of the first error.
        line: usize,
        /// The column of the first error.
        column: usize,
    },
    /// The feed carries more records than the limit.
    TooManyRecords {
        /// The limit.
        limit: usize,
    },
    /// The document is JSON but not an array, an API response or a record.
    UnrecognisedShape,
}

impl fmt::Display for FeedError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TooLarge { bytes, limit } => {
                write!(
                    f,
                    "the feed has {bytes} bytes, above the {limit} byte limit"
                )
            }
            Self::Malformed { line, column } => {
                write!(
                    f,
                    "the feed is not valid JSON (line {line}, column {column})"
                )
            }
            Self::TooManyRecords { limit } => write!(f, "the feed has more than {limit} records"),
            Self::UnrecognisedShape => write!(
                f,
                "the feed is neither an array of records, a `vulns` response nor one record"
            ),
        }
    }
}

impl std::error::Error for FeedError {}

/// A feed read into the records matching can use, with the account of every
/// record that was not.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ParsedFeed {
    /// The current, non-withdrawn records, sorted by identifier.
    pub records: Vec<AdvisoryRecord>,
    /// The identifiers of withdrawn records: they match nothing.
    pub withdrawn: Vec<String>,
    /// The records that were not accepted, sorted by position.
    pub rejected: Vec<RejectedRecord>,
    /// Records that only name ecosystems other than crates.io. Their content
    /// is not validated: they have nothing to say about Cargo packages.
    pub other_ecosystem_records: usize,
    /// Copies of an identifier dropped because a newer or identical copy of
    /// the same record is also in the feed.
    pub duplicates_collapsed: usize,
    /// The records the feed carries.
    pub records_seen: usize,
    /// How many records name each ecosystem, as the feed spells it.
    pub ecosystems: BTreeMap<String, usize>,
}

impl ParsedFeed {
    /// The newest `modified` of the accepted records, as epoch seconds.
    pub fn newest_modified_unix(&self) -> Option<i64> {
        self.records.iter().map(|record| record.modified_unix).max()
    }

    /// The `modified` text of the newest accepted record.
    pub fn newest_modified(&self) -> Option<&str> {
        self.records
            .iter()
            .max_by_key(|record| record.modified_unix)
            .map(|record| record.modified.as_str())
    }

    /// How many accepted records say something about crates.io.
    pub fn crates_io_records(&self) -> usize {
        self.records
            .iter()
            .filter(|record| record.affects_cargo())
            .count()
    }
}

enum Candidate {
    Active(AdvisoryRecord),
    Withdrawn { id: String, modified_unix: i64 },
}

impl Candidate {
    fn id(&self) -> &str {
        match self {
            Self::Active(record) => &record.id,
            Self::Withdrawn { id, .. } => id,
        }
    }

    fn modified_unix(&self) -> i64 {
        match self {
            Self::Active(record) => record.modified_unix,
            Self::Withdrawn { modified_unix, .. } => *modified_unix,
        }
    }

    fn same_state(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Active(a), Self::Active(b)) => a == b,
            (Self::Withdrawn { .. }, Self::Withdrawn { .. }) => true,
            _ => false,
        }
    }
}

enum Read {
    Accepted(Candidate),
    OtherEcosystem,
    Rejected(Option<String>, RejectReason),
}

fn is_clean(text: &str, limits: &FeedLimits) -> bool {
    !text.is_empty() && text.len() <= limits.max_text && !text.chars().any(char::is_control)
}

fn timestamp(value: Option<&Value>) -> Option<(String, i64)> {
    let text = value?.as_str()?;
    let parsed = chrono::DateTime::parse_from_rfc3339(text).ok()?;
    Some((text.to_string(), parsed.timestamp()))
}

fn summary_of(value: Option<&Value>) -> Option<String> {
    let text = value?.as_str()?;
    let cleaned: String = text
        .chars()
        .map(|c| if c.is_control() { ' ' } else { c })
        .take(SUMMARY_CHARS)
        .collect();
    let cleaned = cleaned.trim().to_string();
    (!cleaned.is_empty()).then_some(cleaned)
}

fn ecosystem_of(entry: &Value) -> Option<&str> {
    entry.get("package")?.get("ecosystem")?.as_str()
}

fn ecosystems_of(object: &Map<String, Value>) -> Vec<&str> {
    match object.get("affected") {
        Some(Value::Array(entries)) => entries.iter().filter_map(ecosystem_of).collect(),
        _ => Vec::new(),
    }
}

/// A record whose every `affected` entry names another ecosystem has nothing
/// to say about crates.io, so a defect in it cannot hide a crates.io advisory.
fn names_only_other_ecosystems(object: &Map<String, Value>) -> bool {
    let Some(Value::Array(entries)) = object.get("affected") else {
        return false;
    };
    !entries.is_empty()
        && entries
            .iter()
            .all(|entry| ecosystem_of(entry).is_some_and(|name| name != CRATES_IO))
}

fn strings(
    value: Option<&Value>,
    cap: usize,
    too_many: RejectReason,
    limits: &FeedLimits,
) -> Result<Vec<String>, RejectReason> {
    let Some(value) = value else {
        return Ok(Vec::new());
    };
    let Value::Array(items) = value else {
        return Err(RejectReason::InvalidField);
    };
    if items.len() > cap {
        return Err(too_many);
    }
    items
        .iter()
        .map(|item| match item.as_str() {
            Some(text) if is_clean(text, limits) => Ok(text.to_string()),
            _ => Err(RejectReason::InvalidField),
        })
        .collect()
}

fn range_kind(text: &str) -> RangeKind {
    match text {
        "SEMVER" => RangeKind::Semver,
        "ECOSYSTEM" => RangeKind::Ecosystem,
        "GIT" => RangeKind::Git,
        other => RangeKind::Other(other.chars().take(32).collect()),
    }
}

fn read_event(value: &Value, limits: &FeedLimits) -> Option<RangeEvent> {
    let object = value.as_object()?;
    if object.len() != 1 {
        return None;
    }
    let (key, version) = object.iter().next()?;
    let version = version
        .as_str()
        .filter(|text| is_clean(text, limits))?
        .to_string();
    match key.as_str() {
        "introduced" => Some(RangeEvent::Introduced(version)),
        "fixed" => Some(RangeEvent::Fixed(version)),
        "last_affected" => Some(RangeEvent::LastAffected(version)),
        "limit" => Some(RangeEvent::Limit(version)),
        _ => None,
    }
}

fn read_range(value: &Value, limits: &FeedLimits) -> Result<AdvisoryRange, RejectReason> {
    let object = value.as_object().ok_or(RejectReason::InvalidField)?;
    let kind = object
        .get("type")
        .and_then(Value::as_str)
        .map(range_kind)
        .ok_or(RejectReason::InvalidField)?;
    let events = match object.get("events") {
        Some(Value::Array(items)) if items.len() > limits.max_events => {
            return Err(RejectReason::TooManyEvents);
        }
        Some(Value::Array(items)) => items
            .iter()
            .map(|item| read_event(item, limits))
            .collect::<Option<Vec<_>>>(),
        _ => None,
    };
    Ok(AdvisoryRange { kind, events })
}

fn read_package(entry: &Value, limits: &FeedLimits) -> Result<AffectedPackage, RejectReason> {
    let Some(package) = entry.get("package") else {
        return Ok(AffectedPackage::default());
    };
    let text = |key: &str| -> Result<Option<String>, RejectReason> {
        match package.get(key) {
            None => Ok(None),
            Some(Value::String(text)) if is_clean(text, limits) => Ok(Some(text.clone())),
            Some(_) => Err(RejectReason::InvalidField),
        }
    };
    Ok(AffectedPackage {
        ecosystem: text("ecosystem")?,
        name: text("name")?,
        purl: text("purl")?,
    })
}

fn read_entry(entry: &Value, limits: &FeedLimits) -> Result<AffectedEntry, RejectReason> {
    if !entry.is_object() {
        return Err(RejectReason::InvalidField);
    }
    let ranges = match entry.get("ranges") {
        None => Vec::new(),
        Some(Value::Array(items)) if items.len() > limits.max_ranges => {
            return Err(RejectReason::TooManyRanges);
        }
        Some(Value::Array(items)) => items
            .iter()
            .map(|item| read_range(item, limits))
            .collect::<Result<Vec<_>, _>>()?,
        Some(_) => return Err(RejectReason::InvalidField),
    };
    Ok(AffectedEntry {
        package: read_package(entry, limits)?,
        versions: strings(
            entry.get("versions"),
            limits.max_versions,
            RejectReason::TooManyVersions,
            limits,
        )?,
        ranges,
    })
}

fn read_affected(
    object: &Map<String, Value>,
    limits: &FeedLimits,
) -> Result<Vec<AffectedEntry>, RejectReason> {
    match object.get("affected") {
        None => Ok(Vec::new()),
        Some(Value::Array(items)) if items.len() > limits.max_affected => {
            Err(RejectReason::TooManyAffected)
        }
        Some(Value::Array(items)) => items.iter().map(|item| read_entry(item, limits)).collect(),
        Some(_) => Err(RejectReason::InvalidField),
    }
}

fn read_candidate(
    object: &Map<String, Value>,
    id: &str,
    limits: &FeedLimits,
) -> Result<Candidate, RejectReason> {
    match object.get("schema_version") {
        None => {}
        Some(Value::String(version)) if version.starts_with("1.") => {}
        Some(Value::String(_)) => return Err(RejectReason::UnsupportedSchemaVersion),
        Some(_) => return Err(RejectReason::InvalidField),
    }
    let (modified, modified_unix) =
        timestamp(object.get("modified")).ok_or(RejectReason::InvalidModified)?;
    if let Some(withdrawn) = object.get("withdrawn") {
        timestamp(Some(withdrawn)).ok_or(RejectReason::InvalidField)?;
        return Ok(Candidate::Withdrawn {
            id: id.to_string(),
            modified_unix,
        });
    }
    Ok(Candidate::Active(AdvisoryRecord {
        id: id.to_string(),
        aliases: strings(
            object.get("aliases"),
            limits.max_aliases,
            RejectReason::TooManyAliases,
            limits,
        )?,
        modified,
        modified_unix,
        summary: summary_of(object.get("summary")),
        affected: read_affected(object, limits)?,
    }))
}

fn read_record(value: &Value, limits: &FeedLimits) -> Read {
    let Some(object) = value.as_object() else {
        return Read::Rejected(None, RejectReason::NotAnObject);
    };
    if names_only_other_ecosystems(object) {
        return Read::OtherEcosystem;
    }
    let id = match object.get("id").and_then(Value::as_str) {
        Some(id) if is_clean(id, limits) => id,
        _ => return Read::Rejected(None, RejectReason::InvalidId),
    };
    match read_candidate(object, id, limits) {
        Ok(candidate) => Read::Accepted(candidate),
        Err(reason) => Read::Rejected(Some(id.to_string()), reason),
    }
}

/// Accumulates records as the JSON streams past.
struct Collector<'a> {
    limits: &'a FeedLimits,
    seen: usize,
    over_limit: bool,
    candidates: Vec<(usize, Candidate)>,
    rejected: Vec<RejectedRecord>,
    other_ecosystem_records: usize,
    ecosystems: BTreeMap<String, usize>,
}

impl<'a> Collector<'a> {
    fn new(limits: &'a FeedLimits) -> Self {
        Self {
            limits,
            seen: 0,
            over_limit: false,
            candidates: Vec::new(),
            rejected: Vec::new(),
            other_ecosystem_records: 0,
            ecosystems: BTreeMap::new(),
        }
    }

    fn count_ecosystems(&mut self, value: &Value) {
        let Some(object) = value.as_object() else {
            return;
        };
        let mut names: Vec<String> = ecosystems_of(object)
            .into_iter()
            .map(|name| name.chars().take(ECOSYSTEM_CHARS).collect())
            .collect();
        names.sort_unstable();
        names.dedup();
        for name in names {
            if self.ecosystems.contains_key(&name) || self.ecosystems.len() < MAX_ECOSYSTEMS {
                *self.ecosystems.entry(name).or_insert(0) += 1;
            }
        }
    }

    /// Takes one record; `false` when the record limit is exceeded.
    fn push(&mut self, value: &Value) -> bool {
        if self.seen >= self.limits.max_records {
            self.over_limit = true;
            return false;
        }
        let position = self.seen;
        self.seen += 1;
        self.count_ecosystems(value);
        match read_record(value, self.limits) {
            Read::Accepted(candidate) => self.candidates.push((position, candidate)),
            Read::OtherEcosystem => self.other_ecosystem_records += 1,
            Read::Rejected(id, reason) => self.rejected.push(RejectedRecord {
                position,
                id,
                reason,
            }),
        }
        true
    }
}

struct Records<'c, 'l>(&'c mut Collector<'l>);

impl<'de> DeserializeSeed<'de> for Records<'_, '_> {
    type Value = ();

    fn deserialize<D: Deserializer<'de>>(self, deserializer: D) -> Result<(), D::Error> {
        deserializer.deserialize_seq(self)
    }
}

impl<'de> Visitor<'de> for Records<'_, '_> {
    type Value = ();

    fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("an array of advisory records")
    }

    fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<(), A::Error> {
        while let Some(value) = seq.next_element::<Value>()? {
            if !self.0.push(&value) {
                return Err(serde::de::Error::custom("too many records"));
            }
        }
        Ok(())
    }
}

struct Document<'c, 'l>(&'c mut Collector<'l>);

impl<'de> Visitor<'de> for Document<'_, '_> {
    type Value = ();

    fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("an array of records, a `vulns` response or a record")
    }

    fn visit_seq<A: SeqAccess<'de>>(self, seq: A) -> Result<(), A::Error> {
        Records(self.0).visit_seq(seq)
    }

    fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<(), A::Error> {
        let mut object = Map::new();
        let mut streamed = false;
        while let Some(key) = map.next_key::<String>()? {
            if key == "vulns" && !streamed {
                map.next_value_seed(Records(&mut *self.0))?;
                streamed = true;
            } else if streamed {
                map.next_value::<serde::de::IgnoredAny>()?;
            } else {
                object.insert(key, map.next_value::<Value>()?);
            }
        }
        if !streamed {
            self.0.push(&Value::Object(object));
        }
        Ok(())
    }
}

/// Keeps one copy per identifier: the newest `modified`, with identical
/// copies merged. Two different copies at the same instant are a conflict
/// that is reported instead of resolved by guessing.
fn collapse(candidates: Vec<(usize, Candidate)>, parsed: &mut ParsedFeed) {
    let mut by_id: BTreeMap<String, Vec<(usize, Candidate)>> = BTreeMap::new();
    for (position, candidate) in candidates {
        by_id
            .entry(candidate.id().to_string())
            .or_default()
            .push((position, candidate));
    }
    for (id, mut group) in by_id {
        let copies = group.len();
        let newest = group
            .iter()
            .map(|(_, c)| c.modified_unix())
            .max()
            .unwrap_or(0);
        group.retain(|(_, c)| c.modified_unix() == newest);
        let first_position = group.first().map_or(0, |(position, _)| *position);
        if !group.iter().all(|(_, c)| group[0].1.same_state(c)) {
            parsed.rejected.push(RejectedRecord {
                position: first_position,
                id: Some(id),
                reason: RejectReason::DuplicateIdConflict,
            });
            continue;
        }
        parsed.duplicates_collapsed += copies - 1;
        match group.swap_remove(0).1 {
            Candidate::Active(record) => parsed.records.push(record),
            Candidate::Withdrawn { id, .. } => parsed.withdrawn.push(id),
        }
    }
}

/// Reads `bytes` as an OSV feed under `limits`.
///
/// A feed that cannot be read at all is a [`FeedError`]; a record that cannot
/// be read is listed in [`ParsedFeed::rejected`] and the others are kept.
pub fn parse_feed(bytes: &[u8], limits: &FeedLimits) -> Result<ParsedFeed, FeedError> {
    if bytes.len() > limits.max_bytes {
        return Err(FeedError::TooLarge {
            bytes: bytes.len(),
            limit: limits.max_bytes,
        });
    }
    let mut collector = Collector::new(limits);
    let mut deserializer = serde_json::Deserializer::from_slice(bytes);
    let outcome = deserializer
        .deserialize_any(Document(&mut collector))
        .and_then(|()| deserializer.end());
    if collector.over_limit {
        return Err(FeedError::TooManyRecords {
            limit: limits.max_records,
        });
    }
    outcome.map_err(|error| {
        if error.is_data() {
            FeedError::UnrecognisedShape
        } else {
            FeedError::Malformed {
                line: error.line(),
                column: error.column(),
            }
        }
    })?;
    let mut parsed = ParsedFeed {
        rejected: collector.rejected,
        other_ecosystem_records: collector.other_ecosystem_records,
        ecosystems: collector.ecosystems,
        records_seen: collector.seen,
        ..ParsedFeed::default()
    };
    collapse(collector.candidates, &mut parsed);
    parsed.records.sort_by(|a, b| a.id.cmp(&b.id));
    parsed.withdrawn.sort();
    parsed.rejected.sort();
    Ok(parsed)
}

#[cfg(test)]
mod tests;
