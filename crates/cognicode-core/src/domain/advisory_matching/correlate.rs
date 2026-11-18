//! Correlation of resolved packages with the records of a feed.
//!
//! Pure: it takes values and returns values. The key is the crate identity
//! the inventory gives each package (name and version, the content of its
//! `pkg:cargo` Package URL), compared with the crate each record names by
//! ecosystem and name or by Package URL. Names are compared as Cargo does:
//! lowercase, with `_` and `-` equal.

use std::collections::{BTreeMap, BTreeSet};

use super::model::{Absence, AdvisoryHit, Certainty, PackageResult, PackageState, Reason, Subject};
use super::ranges::{Containment, contains};
use super::record::{AdvisoryRecord, AffectedEntry, RangeEvent, canonical_crate_name};

/// The most fixed versions listed per advisory.
const MAX_FIXED: usize = 16;

/// One record that names a crate, with the entries that do.
type Index<'a> = BTreeMap<String, Vec<(&'a AdvisoryRecord, &'a AffectedEntry)>>;

fn index(records: &[AdvisoryRecord]) -> Index<'_> {
    let mut index: Index<'_> = BTreeMap::new();
    for record in records {
        for entry in &record.affected {
            if let Some(name) = entry.cargo_crate() {
                index.entry(name).or_default().push((record, entry));
            }
        }
    }
    index
}

fn fixed_versions(entry: &AffectedEntry) -> BTreeSet<String> {
    entry
        .ranges
        .iter()
        .flat_map(|range| range.events.iter().flatten())
        .filter_map(|event| match event {
            RangeEvent::Fixed(version) => Some(version.clone()),
            _ => None,
        })
        .collect()
}

/// What one record says about one version: the strongest of its entries.
struct RecordHit<'a> {
    record: &'a AdvisoryRecord,
    certainty: Certainty,
    fixed: BTreeSet<String>,
}

fn record_hits<'a>(
    candidates: &[(&'a AdvisoryRecord, &'a AffectedEntry)],
    version: &str,
) -> Vec<RecordHit<'a>> {
    let mut hits: Vec<RecordHit<'a>> = Vec::new();
    for (record, entry) in candidates {
        let certainty = match contains(entry, version) {
            Containment::Affected => Certainty::Affected,
            Containment::Unknown => Certainty::Undecided,
            Containment::NotAffected => continue,
        };
        match hits
            .iter_mut()
            .find(|hit| std::ptr::eq(hit.record, *record))
        {
            Some(hit) => {
                if certainty == Certainty::Affected {
                    hit.certainty = Certainty::Affected;
                }
                hit.fixed.extend(fixed_versions(entry));
            }
            None => hits.push(RecordHit {
                record,
                certainty,
                fixed: fixed_versions(entry),
            }),
        }
    }
    hits
}

fn root(parents: &mut [usize], mut node: usize) -> usize {
    while parents[node] != node {
        parents[node] = parents[parents[node]];
        node = parents[node];
    }
    node
}

/// Groups the hits that are the same advisory: records that share an
/// identifier or an alias (a RUSTSEC record and the GHSA record that lists it
/// as an alias are one finding, not two).
fn cluster(hits: Vec<RecordHit<'_>>) -> Vec<AdvisoryHit> {
    let mut parents: Vec<usize> = (0..hits.len()).collect();
    let mut owner: BTreeMap<&str, usize> = BTreeMap::new();
    for (position, hit) in hits.iter().enumerate() {
        let names = std::iter::once(&hit.record.id).chain(&hit.record.aliases);
        for name in names {
            match owner.get(name.as_str()) {
                Some(&other) => {
                    let (a, b) = (root(&mut parents, position), root(&mut parents, other));
                    parents[a.max(b)] = a.min(b);
                }
                None => {
                    owner.insert(name, position);
                }
            }
        }
    }
    let mut groups: BTreeMap<usize, Vec<usize>> = BTreeMap::new();
    for position in 0..hits.len() {
        let group = root(&mut parents, position);
        groups.entry(group).or_default().push(position);
    }
    let mut out: Vec<AdvisoryHit> = groups
        .into_values()
        .map(|members| merge(members.iter().map(|&position| &hits[position]).collect()))
        .collect();
    out.sort_by(|a, b| a.id.cmp(&b.id));
    out
}

fn merge(members: Vec<&RecordHit<'_>>) -> AdvisoryHit {
    let mut names: BTreeSet<&str> = BTreeSet::new();
    let mut fixed: BTreeSet<&String> = BTreeSet::new();
    for member in &members {
        names.insert(&member.record.id);
        names.extend(member.record.aliases.iter().map(String::as_str));
        fixed.extend(&member.fixed);
    }
    let primary = members
        .iter()
        .min_by(|a, b| a.record.id.cmp(&b.record.id))
        .map(|member| member.record)
        .expect("a cluster has at least one member");
    let latest = members
        .iter()
        .max_by_key(|member| member.record.modified_unix)
        .map(|member| member.record.modified.clone())
        .unwrap_or_default();
    let certainty = if members.iter().any(|m| m.certainty == Certainty::Affected) {
        Certainty::Affected
    } else {
        Certainty::Undecided
    };
    let id = primary.id.clone();
    AdvisoryHit {
        aliases: names
            .into_iter()
            .filter(|name| *name != id.as_str())
            .map(str::to_string)
            .collect(),
        id,
        certainty,
        fixed_in: fixed.into_iter().take(MAX_FIXED).cloned().collect(),
        summary: primary.summary.clone(),
        modified: latest,
    }
}

fn state_of(hits: &[AdvisoryHit], absence: Absence) -> (PackageState, Vec<Reason>) {
    if hits.iter().any(|hit| hit.certainty == Certainty::Affected) {
        return (PackageState::Affected, Vec::new());
    }
    if !hits.is_empty() {
        return (PackageState::Unknown, vec![Reason::UndecidableRange]);
    }
    match absence {
        Absence::Conclusive => (PackageState::NotFoundInScope, Vec::new()),
        Absence::Inconclusive(reason @ (Reason::EcosystemNotCovered | Reason::FeedUnavailable)) => {
            (PackageState::NotEvaluated, vec![reason])
        }
        Absence::Inconclusive(reason) => (PackageState::Unknown, vec![reason]),
    }
}

/// Correlates `subjects` with `records`.
///
/// `absence` says whether finding nothing may be reported as
/// `not_found_in_scope`; the caller decides it from how well the feed was
/// read, how old it is and whether it covers crates.io. Results keep the
/// order of `subjects`.
pub fn correlate(
    records: &[AdvisoryRecord],
    subjects: &[Subject],
    absence: Absence,
) -> Vec<PackageResult> {
    let index = index(records);
    subjects
        .iter()
        .map(|subject| {
            let (state, reasons, advisories) = if subject.from_crates_io {
                let candidates = index
                    .get(&canonical_crate_name(&subject.name))
                    .map_or(&[][..], Vec::as_slice);
                let advisories = cluster(record_hits(candidates, &subject.version));
                let (state, reasons) = state_of(&advisories, absence);
                (state, reasons, advisories)
            } else {
                (
                    PackageState::NotEvaluated,
                    vec![Reason::SourceNotInEcosystem],
                    Vec::new(),
                )
            };
            PackageResult {
                package: subject.id.clone(),
                name: subject.name.clone(),
                version: subject.version.clone(),
                purl: subject.purl.clone(),
                state,
                reasons,
                advisories,
            }
        })
        .collect()
}

#[cfg(test)]
mod tests;
