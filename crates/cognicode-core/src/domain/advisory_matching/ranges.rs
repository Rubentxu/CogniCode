//! Whether one version is inside what an OSV `affected` entry covers.
//!
//! The ordering is the one Cargo uses (Semantic Versioning, pre-releases
//! before their release, build metadata ignored), shared with the
//! requirement reader of the dependency inventory. Both `SEMVER` and
//! `ECOSYSTEM` ranges are read that way because that is crates.io's own
//! ordering. A range this module cannot decide yields
//! [`Containment::Unknown`], never "not affected".

use super::record::{AdvisoryRange, AffectedEntry, RangeEvent, RangeKind};
use crate::domain::dependency_inventory::version_req::{Version, parse_version};

/// What an entry says about a version.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Containment {
    /// The version is inside the affected set.
    Affected,
    /// The version is outside the affected set.
    NotAffected,
    /// The entry cannot be decided for this version.
    Unknown,
}

/// A bound of an event: `introduced: "0"` is the beginning of all versions.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
enum Bound {
    Zero,
    At(Version),
}

fn bound(text: &str) -> Option<Bound> {
    if text == "0" {
        return Some(Bound::Zero);
    }
    parse_version(text).map(Bound::At)
}

fn event_text(event: &RangeEvent) -> &str {
    match event {
        RangeEvent::Introduced(text)
        | RangeEvent::Fixed(text)
        | RangeEvent::LastAffected(text)
        | RangeEvent::Limit(text) => text,
    }
}

/// Evaluates the events of one range for `version`, in the order OSV
/// defines: events are ordered by version (ties keep the record's order);
/// `introduced` opens an interval, `fixed` closes it before that version,
/// `last_affected` closes it after that version, and an interval still open
/// at the end runs up to `limit` when there is one.
fn evaluate_events(events: &[RangeEvent], version: &Version) -> Containment {
    let mut ordered = Vec::with_capacity(events.len());
    for event in events {
        match bound(event_text(event)) {
            Some(at) => ordered.push((at, event)),
            None => return Containment::Unknown,
        }
    }
    ordered.sort_by(|a, b| a.0.cmp(&b.0));
    let at = Bound::At(version.clone());
    let limit = ordered.iter().find_map(|(bound, event)| match event {
        RangeEvent::Limit(_) => Some(bound),
        _ => None,
    });
    let mut open: Option<&Bound> = None;
    for (edge, event) in &ordered {
        match event {
            RangeEvent::Introduced(_) => {
                open.get_or_insert(edge);
            }
            RangeEvent::Fixed(_) => {
                if let Some(start) = open.take() {
                    if *start <= at && at < *edge {
                        return Containment::Affected;
                    }
                }
            }
            RangeEvent::LastAffected(_) => {
                if let Some(start) = open.take() {
                    if *start <= at && at <= *edge {
                        return Containment::Affected;
                    }
                }
            }
            RangeEvent::Limit(_) => {}
        }
    }
    match open {
        Some(start) if *start <= at && limit.is_none_or(|end| at < *end) => Containment::Affected,
        _ => Containment::NotAffected,
    }
}

fn evaluate_range(range: &AdvisoryRange, version: Option<&Version>) -> Containment {
    match (&range.kind, &range.events, version) {
        (RangeKind::Semver | RangeKind::Ecosystem, Some(events), Some(version)) => {
            evaluate_events(events, version)
        }
        _ => Containment::Unknown,
    }
}

fn same_version(listed: &str, version: &str, parsed: Option<&Version>) -> bool {
    listed == version || matches!((parse_version(listed), parsed), (Some(a), Some(b)) if a == *b)
}

/// Whether `entry` covers `version` (a locked version, as written).
///
/// * A version the entry lists explicitly is affected.
/// * `SEMVER` and `ECOSYSTEM` ranges are evaluated; any range that says
///   affected makes the answer affected.
/// * A `GIT` range names commits and cannot be decided from a version. It
///   only makes the answer unknown when nothing else in the entry (a version
///   list or a decidable range) settles it, because records that carry a
///   `GIT` range normally list the affected versions next to it.
/// * A version that is not a full semantic version, a range with an event
///   that is not one, or an entry with neither versions nor ranges, is
///   unknown.
pub fn contains(entry: &AffectedEntry, version: &str) -> Containment {
    let parsed = parse_version(version);
    if entry
        .versions
        .iter()
        .any(|listed| same_version(listed, version, parsed.as_ref()))
    {
        return Containment::Affected;
    }
    let mut settled = !entry.versions.is_empty();
    let mut undecided = false;
    for range in &entry.ranges {
        match evaluate_range(range, parsed.as_ref()) {
            Containment::Affected => return Containment::Affected,
            Containment::NotAffected => settled = true,
            Containment::Unknown if matches!(range.kind, RangeKind::Git) => {}
            Containment::Unknown => undecided = true,
        }
    }
    if undecided || !settled {
        Containment::Unknown
    } else {
        Containment::NotAffected
    }
}

#[cfg(test)]
mod tests;
