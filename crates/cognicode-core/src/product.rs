//! The runtime posture of a public install profile.
//!
//! # Why this exists
//!
//! `product/profiles.json` is the public truth about the profiles CogniCode
//! publishes, and it carries a `mutating` flag per profile. That flag is a
//! promise: a profile that declares `mutating: false` must not be able to
//! modify the workspace of whoever installed it.
//!
//! Until now the promise had no enforcement behind it. The read-only
//! machinery existed and was tested (`--read-only` on `cognicode-mcp`, and
//! the filtering in [`crate::interface::mcp::rmcp_adapter`]), but nothing
//! connected the *profile* to it: `--read-only` is a flag a human types,
//! while a profile is something an installer resolves. `with_read_only` had
//! no caller at all.
//!
//! # Why the table lives here
//!
//! This is deliberately the single place that knows a profile's posture,
//! and the profile generator reads it from here rather than hard-coding a
//! second copy. A posture that exists only in a JSON file cannot be
//! enforced at runtime, and a posture duplicated in two places eventually
//! disagrees with itself.
//!
//! Unknown profiles are **not** treated as read-only. The default is the
//! permissive one on purpose: the CLI still has a `--read-only` flag a user
//! can choose, and silently muting a profile nobody has classified yet
//! would break existing installations. The set of known profiles is closed
//! and asserted in tests, so an unclassified profile is a compile-time
//! concern, not a runtime surprise.

use serde::{Deserialize, Serialize};

/// What a profile is allowed to do to the workspace.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ProfilePosture {
    /// Reads only. Mutating tools are neither advertised nor callable.
    ReadOnly,
    /// Reads and writes.
    ReadWrite,
}

impl ProfilePosture {
    /// Whether this posture forbids workspace mutation.
    pub fn is_read_only(self) -> bool {
        matches!(self, ProfilePosture::ReadOnly)
    }
}

/// Every public profile and its posture. One row per profile, no exceptions.
///
/// `developer` is the only mutating profile: it exists to drive the
/// SDDK/OpenSpec workflow, which writes changes by design. `experimental` is
/// read-only, and `install: false`, so it is not installable in the first
/// place.
pub const PROFILE_POSTURES: &[(&str, ProfilePosture)] = &[
    ("core", ProfilePosture::ReadOnly),
    ("reviewer", ProfilePosture::ReadOnly),
    ("developer", ProfilePosture::ReadWrite),
    ("experimental", ProfilePosture::ReadOnly),
];

impl ProfilePosture {
    /// The posture of a public profile id.
    ///
    /// Returns `None` for an id that is not a public profile, so callers
    /// have to decide what an unknown profile means rather than inheriting
    /// a silent default.
    pub fn for_profile(profile_id: &str) -> Option<Self> {
        PROFILE_POSTURES
            .iter()
            .find(|(id, _)| *id == profile_id)
            .map(|(_, posture)| *posture)
    }

    /// Whether a profile must run read-only.
    ///
    /// Unclassified ids return `false` (permissive) for the reason documented
    /// on the module. Callers that want strict behaviour should match on
    /// [`ProfilePosture::for_profile`] instead.
    pub fn is_profile_read_only(profile_id: &str) -> bool {
        Self::for_profile(profile_id).is_some_and(|p| p.is_read_only())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_published_profile_has_a_posture() {
        // Mirrors `PUBLISHED_PROFILES` in the release contract. If a profile
        // is published without a posture here, the installer would fall back
        // to the permissive default without anyone noticing.
        for published in ["core", "reviewer"] {
            assert!(
                ProfilePosture::for_profile(published).is_some(),
                "published profile {published} has no posture"
            );
        }
    }

    #[test]
    fn reviewer_is_read_only() {
        assert_eq!(
            ProfilePosture::for_profile("reviewer"),
            Some(ProfilePosture::ReadOnly)
        );
        assert!(ProfilePosture::is_profile_read_only("reviewer"));
    }

    #[test]
    fn developer_is_the_only_mutating_profile() {
        let mutating: Vec<&str> = PROFILE_POSTURES
            .iter()
            .filter(|(_, posture)| !posture.is_read_only())
            .map(|(id, _)| *id)
            .collect();
        assert_eq!(mutating, vec!["developer"]);
    }

    #[test]
    fn unknown_profile_is_reported_not_guessed() {
        assert_eq!(ProfilePosture::for_profile("nope"), None);
        assert!(!ProfilePosture::is_profile_read_only("nope"));
    }
}
