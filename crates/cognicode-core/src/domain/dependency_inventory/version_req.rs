//! A small, dependency-free reading of Cargo version requirements.
//!
//! It answers one question: does a locked version satisfy the requirement a
//! manifest declares? Anything it cannot read is [`RequirementMatch::Unverifiable`]
//! and never a guess, so a stale-lockfile verdict is not built on a
//! requirement this module does not understand.

use std::cmp::Ordering;

/// Whether a locked version satisfies a declared requirement.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RequirementMatch {
    /// The version is inside the requirement.
    Satisfied,
    /// The version is outside the requirement.
    Unsatisfied,
    /// The requirement or the version is not in a grammar this module reads.
    Unverifiable,
}

/// A semantic version without build metadata, which never takes part in
/// precedence.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Version {
    major: u64,
    minor: u64,
    patch: u64,
    pre: Vec<PreIdentifier>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum PreIdentifier {
    Numeric(u64),
    Text(String),
}

impl Ord for PreIdentifier {
    fn cmp(&self, other: &Self) -> Ordering {
        match (self, other) {
            (Self::Numeric(a), Self::Numeric(b)) => a.cmp(b),
            (Self::Numeric(_), Self::Text(_)) => Ordering::Less,
            (Self::Text(_), Self::Numeric(_)) => Ordering::Greater,
            (Self::Text(a), Self::Text(b)) => a.cmp(b),
        }
    }
}

impl PartialOrd for PreIdentifier {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Version {
    fn cmp(&self, other: &Self) -> Ordering {
        self.major
            .cmp(&other.major)
            .then(self.minor.cmp(&other.minor))
            .then(self.patch.cmp(&other.patch))
            .then_with(|| match (self.pre.is_empty(), other.pre.is_empty()) {
                (true, true) => Ordering::Equal,
                (true, false) => Ordering::Greater,
                (false, true) => Ordering::Less,
                (false, false) => self.pre.cmp(&other.pre),
            })
    }
}

impl PartialOrd for Version {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Version {
    fn triple(major: u64, minor: u64, patch: u64) -> Self {
        Self {
            major,
            minor,
            patch,
            pre: Vec::new(),
        }
    }

    fn same_triple(&self, other: &Self) -> bool {
        (self.major, self.minor, self.patch) == (other.major, other.minor, other.patch)
    }
}

/// A version as a requirement writes it: trailing components may be absent
/// or a wildcard.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Partial {
    major: u64,
    minor: Option<u64>,
    patch: Option<u64>,
    pre: Vec<PreIdentifier>,
    /// The requirement ended in a wildcard component (`1.*`, `1.2.*`).
    wildcard: bool,
}

impl Partial {
    fn floor(&self) -> Version {
        Version {
            major: self.major,
            minor: self.minor.unwrap_or(0),
            patch: self.patch.unwrap_or(0),
            pre: self.pre.clone(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Comparator {
    /// `>= low` and `< high` (`high` absent: unbounded above).
    Range {
        low: Version,
        high: Option<Version>,
    },
    Exact(Version),
    GreaterEq(Version),
    Less(Version),
    Any,
}

fn parse_numbers(text: &str) -> Option<(Vec<Option<u64>>, Vec<PreIdentifier>)> {
    let core = text.split('+').next().unwrap_or(text);
    let (numbers, pre) = match core.split_once('-') {
        Some((numbers, pre)) => (numbers, Some(pre)),
        None => (core, None),
    };
    let mut parts = Vec::new();
    for part in numbers.split('.') {
        if matches!(part, "*" | "x" | "X") {
            parts.push(None);
        } else {
            parts.push(Some(part.parse::<u64>().ok()?));
        }
    }
    if parts.is_empty() || parts.len() > 3 {
        return None;
    }
    let pre = match pre {
        None => Vec::new(),
        Some(pre) => pre
            .split('.')
            .map(|identifier| {
                if identifier.is_empty() {
                    None
                } else if let Ok(number) = identifier.parse::<u64>() {
                    Some(PreIdentifier::Numeric(number))
                } else {
                    Some(PreIdentifier::Text(identifier.to_string()))
                }
            })
            .collect::<Option<Vec<_>>>()?,
    };
    Some((parts, pre))
}

fn parse_version(text: &str) -> Option<Version> {
    let (parts, pre) = parse_numbers(text)?;
    match parts.as_slice() {
        [Some(major), Some(minor), Some(patch)] => Some(Version {
            major: *major,
            minor: *minor,
            patch: *patch,
            pre,
        }),
        _ => None,
    }
}

fn parse_partial(text: &str) -> Option<Partial> {
    let (parts, pre) = parse_numbers(text)?;
    // A wildcard ends the version: `1.*` is `1`, and `*.2` is not a version.
    let mut numbers = Vec::new();
    let mut wildcard = false;
    for part in parts {
        match (part, wildcard) {
            (Some(number), false) => numbers.push(number),
            (None, _) => wildcard = true,
            (Some(_), true) => return None,
        }
    }
    let major = *numbers.first()?;
    Some(Partial {
        major,
        minor: numbers.get(1).copied(),
        patch: numbers.get(2).copied(),
        pre,
        wildcard,
    })
}

fn caret(partial: &Partial) -> Comparator {
    let low = partial.floor();
    let high = match (partial.major, partial.minor, partial.patch) {
        (0, Some(0), Some(patch)) => Version::triple(0, 0, patch + 1),
        (0, Some(minor), _) => Version::triple(0, minor + 1, 0),
        (major, _, _) => Version::triple(major + 1, 0, 0),
    };
    Comparator::Range {
        low,
        high: Some(high),
    }
}

fn tilde(partial: &Partial) -> Comparator {
    let low = partial.floor();
    let high = match partial.minor {
        Some(minor) => Version::triple(partial.major, minor + 1, 0),
        None => Version::triple(partial.major + 1, 0, 0),
    };
    Comparator::Range {
        low,
        high: Some(high),
    }
}

fn exact(partial: &Partial) -> Comparator {
    match (partial.minor, partial.patch) {
        (Some(minor), Some(patch)) => Comparator::Exact(Version {
            major: partial.major,
            minor,
            patch,
            pre: partial.pre.clone(),
        }),
        (Some(minor), None) => Comparator::Range {
            low: partial.floor(),
            high: Some(Version::triple(partial.major, minor + 1, 0)),
        },
        (None, _) => Comparator::Range {
            low: partial.floor(),
            high: Some(Version::triple(partial.major + 1, 0, 0)),
        },
    }
}

/// The first version above everything a partial version names.
fn above(partial: &Partial) -> Version {
    match (partial.minor, partial.patch) {
        (Some(minor), Some(patch)) => Version::triple(partial.major, minor, patch + 1),
        (Some(minor), None) => Version::triple(partial.major, minor + 1, 0),
        (None, _) => Version::triple(partial.major + 1, 0, 0),
    }
}

fn parse_comparator(text: &str) -> Option<Comparator> {
    let text = text.trim();
    if text == "*" {
        return Some(Comparator::Any);
    }
    let (operator, rest) = ["<=", ">=", "<", ">", "=", "^", "~"]
        .iter()
        .find_map(|operator| text.strip_prefix(operator).map(|rest| (*operator, rest)))
        .unwrap_or(("", text));
    let partial = parse_partial(rest.trim())?;
    Some(match operator {
        "" if partial.wildcard => exact(&partial),
        "" | "^" => caret(&partial),
        "~" => tilde(&partial),
        "=" => exact(&partial),
        ">" => Comparator::GreaterEq(above(&partial)),
        ">=" => Comparator::GreaterEq(partial.floor()),
        "<" => Comparator::Less(partial.floor()),
        "<=" => Comparator::Less(above(&partial)),
        _ => return None,
    })
}

fn comparator_allows(comparator: &Comparator, version: &Version) -> bool {
    match comparator {
        Comparator::Any => true,
        Comparator::Exact(exact) => version == exact,
        Comparator::GreaterEq(low) => version >= low,
        Comparator::Less(high) => version < high,
        Comparator::Range { low, high } => {
            version >= low && high.as_ref().is_none_or(|high| version < high)
        }
    }
}

fn comparator_floor(comparator: &Comparator) -> Option<&Version> {
    match comparator {
        Comparator::Range { low, .. } => Some(low),
        Comparator::Exact(version) | Comparator::GreaterEq(version) | Comparator::Less(version) => {
            Some(version)
        }
        Comparator::Any => None,
    }
}

/// Does `locked` satisfy `requirement`, a Cargo version requirement such as
/// `^1.2`, `~0.3.4`, `>=1, <2` or `1.*`?
///
/// A pre-release is satisfied only by a comparator that names a pre-release
/// of the same `major.minor.patch`, as Cargo does. Unknown grammar (a
/// requirement or a locked version this module does not parse) is
/// [`RequirementMatch::Unverifiable`].
pub fn requirement_matches(requirement: &str, locked: &str) -> RequirementMatch {
    let Some(version) = parse_version(locked) else {
        return RequirementMatch::Unverifiable;
    };
    let comparators: Option<Vec<Comparator>> =
        requirement.split(',').map(parse_comparator).collect();
    let Some(comparators) = comparators else {
        return RequirementMatch::Unverifiable;
    };
    let allowed = comparators
        .iter()
        .all(|comparator| comparator_allows(comparator, &version));
    let prerelease_named = version.pre.is_empty()
        || comparators.iter().any(|comparator| {
            comparator_floor(comparator)
                .is_some_and(|floor| !floor.pre.is_empty() && floor.same_triple(&version))
        });
    if allowed && prerelease_named {
        RequirementMatch::Satisfied
    } else {
        RequirementMatch::Unsatisfied
    }
}

#[cfg(test)]
mod tests {
    use super::RequirementMatch::{Satisfied, Unsatisfied, Unverifiable};
    use super::*;

    #[test]
    fn caret_requirements_follow_the_cargo_compatibility_rules() {
        assert_eq!(requirement_matches("1.2.3", "1.9.0"), Satisfied);
        assert_eq!(requirement_matches("1.2.3", "2.0.0"), Unsatisfied);
        assert_eq!(requirement_matches("^1.2.3", "1.2.2"), Unsatisfied);
        assert_eq!(requirement_matches("0.2.3", "0.2.9"), Satisfied);
        assert_eq!(requirement_matches("0.2.3", "0.3.0"), Unsatisfied);
        assert_eq!(requirement_matches("0.0.3", "0.0.3"), Satisfied);
        assert_eq!(requirement_matches("0.0.3", "0.0.4"), Unsatisfied);
        assert_eq!(requirement_matches("1", "1.40.2"), Satisfied);
        assert_eq!(requirement_matches("0", "0.9.9"), Satisfied);
    }

    #[test]
    fn tilde_exact_and_wildcard_requirements_are_read() {
        assert_eq!(requirement_matches("~1.2.3", "1.2.9"), Satisfied);
        assert_eq!(requirement_matches("~1.2.3", "1.3.0"), Unsatisfied);
        assert_eq!(requirement_matches("~1", "1.9.0"), Satisfied);
        assert_eq!(requirement_matches("=1.2.3", "1.2.3"), Satisfied);
        assert_eq!(requirement_matches("=1.2.3", "1.2.4"), Unsatisfied);
        assert_eq!(requirement_matches("1.*", "1.7.0"), Satisfied);
        assert_eq!(requirement_matches("1.2.*", "1.3.0"), Unsatisfied);
        assert_eq!(requirement_matches("*", "9.9.9"), Satisfied);
    }

    #[test]
    fn comparator_lists_are_a_conjunction() {
        assert_eq!(requirement_matches(">=1.2, <1.5", "1.4.0"), Satisfied);
        assert_eq!(requirement_matches(">=1.2, <1.5", "1.5.0"), Unsatisfied);
        assert_eq!(requirement_matches(">1.2", "1.2.9"), Unsatisfied);
        assert_eq!(requirement_matches(">1.2", "1.3.0"), Satisfied);
        assert_eq!(requirement_matches("<=1.2", "1.2.9"), Satisfied);
    }

    #[test]
    fn a_prerelease_matches_only_a_comparator_that_names_one() {
        assert_eq!(requirement_matches("1", "1.0.0-rc.1"), Unsatisfied);
        assert_eq!(requirement_matches("^1.0.0-rc.1", "1.0.0-rc.2"), Satisfied);
        assert_eq!(
            requirement_matches("^1.0.0-rc.2", "1.0.0-rc.1"),
            Unsatisfied
        );
        assert_eq!(requirement_matches("^1.0.0-rc.1", "1.0.0"), Satisfied);
    }

    #[test]
    fn build_metadata_never_takes_part_in_the_match() {
        assert_eq!(requirement_matches("=1.2.3", "1.2.3+build.5"), Satisfied);
    }

    #[test]
    fn what_the_grammar_does_not_cover_is_unverifiable_not_guessed() {
        assert_eq!(requirement_matches("not a version", "1.0.0"), Unverifiable);
        assert_eq!(requirement_matches("1", "not-a-version"), Unverifiable);
        assert_eq!(requirement_matches("*.2", "1.2.0"), Unverifiable);
    }
}
