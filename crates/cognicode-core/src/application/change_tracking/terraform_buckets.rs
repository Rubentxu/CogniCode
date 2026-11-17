//! The Terraform check behind the unprotected-bucket detector: a pure
//! reading of revision facts.
//!
//! ## What it says
//!
//! An `aws_s3_bucket` is **unprotected** when no
//! `aws_s3_bucket_public_access_block` of its Terraform module references
//! it. The public access block is a separate resource and has no inline
//! form, so "no block names this bucket" is a statement the declaration
//! and reference facts can carry on their own. It does not judge what the
//! block says (its flags are attribute values, which the facts do not
//! hold): that is the job of the attribute rules the security pack
//! already runs over a block that exists. A bucket the account-level block
//! covers is still reported, because the finding is about the bucket's own
//! configuration.
//!
//! ## What it reads
//!
//! Exactly three kinds of facts, and their ids are the read-set:
//!
//! - the `core:defines` fact of every bucket and every public access block;
//! - the `core:references` facts of the public access blocks.
//!
//! A change anywhere else in the revision cannot move the answer.
//!
//! ## Absence is only claimed over a module that was read whole
//!
//! A module is the directory of its `.tf` files. Whether a block exists
//! somewhere in it is an absence claim, and a file that has no facts (a
//! syntax error, an unreadable or oversized file) may be hiding the very
//! block that would protect the bucket. A bucket of a module with such a
//! file is therefore never reported: the module is not known well enough to
//! say, and the gap itself already keeps every verdict of the revision from
//! being a pass. Silence here means "not established", not "protected".
//!
//! A reference is resolved by the extractor only within its own file; a
//! traversal into another file of the module stays as written
//! (`aws_s3_bucket.logs.id`). Both spellings count, within the module of
//! the block that holds the reference.

use std::collections::{BTreeMap, BTreeSet};

use crate::domain::evidence_kernel::fact::{Fact, FactValue};
use crate::domain::evidence_kernel::ids::{EntityId, FactId};

use super::revision_facts::FactsCoverage;
use super::scm_delta::RevisionFacts;

const DEFINES: &str = "core:defines";
const REFERENCES: &str = "core:references";
const BUCKET: &str = "aws_s3_bucket";
const PUBLIC_ACCESS_BLOCK: &str = "aws_s3_bucket_public_access_block";
/// The extractor names a Terraform declaration `tf:<path>:<identity>`.
const TERRAFORM_PREFIX: &str = "tf:";

/// A bucket no public access block of its module references.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct UnprotectedBucket {
    /// The file that declares it.
    pub path: String,
    /// The declaration's fully qualified name.
    pub fqn: String,
    /// The `core:defines` fact the finding is grounded on.
    pub fact: FactId,
}

/// What the scan concluded and what it consulted to conclude it.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(super) struct BucketScan {
    /// Buckets of fully read modules that no block references, in path and
    /// fact order.
    pub unprotected: Vec<UnprotectedBucket>,
    /// Every fact the scan consulted.
    pub observed: BTreeSet<FactId>,
}

/// The directory of a repository path; empty for the repository root.
fn directory_of(path: &str) -> &str {
    path.rsplit_once('/').map_or("", |(directory, _)| directory)
}

/// The identity of the declaration a fact name denotes in `path`:
/// `tf:terraform/main.tf:aws_s3_bucket.logs` is `aws_s3_bucket.logs`.
fn identity_in<'a>(path: &str, fqn: &'a str) -> Option<&'a str> {
    fqn.strip_prefix(TERRAFORM_PREFIX)?
        .strip_prefix(path)?
        .strip_prefix(':')
}

fn type_of(identity: &str) -> &str {
    identity.split('.').next().unwrap_or(identity)
}

/// The module and the identity a reference target names, seen from a block
/// of module `from`.
///
/// A target the extractor resolved names its file; one it did not is the
/// traversal as written, whose first two segments are the declaration it
/// points at.
fn referenced(target: &str, from: &str) -> (String, String) {
    if let Some(resolved) = target.strip_prefix(TERRAFORM_PREFIX)
        && let Some((path, identity)) = resolved.rsplit_once(':')
    {
        return (directory_of(path).to_string(), identity.to_string());
    }
    let identity = target
        .split('.')
        .take(2)
        .collect::<Vec<_>>()
        .join(".")
        .split('[')
        .next()
        .unwrap_or_default()
        .to_string();
    (from.to_string(), identity)
}

/// Scans the Terraform facts of one revision.
pub(super) fn scan(facts: &RevisionFacts, coverage: &FactsCoverage) -> BucketScan {
    let by_id: BTreeMap<FactId, &Fact> = facts.facts.iter().map(|fact| (fact.id, fact)).collect();
    let unread: BTreeSet<&str> = coverage
        .gaps
        .iter()
        .filter(|gap| gap.path.as_str().ends_with(".tf"))
        .map(|gap| directory_of(gap.path.as_str()))
        .collect();

    let mut observed = BTreeSet::new();
    let mut buckets: Vec<(&str, &str, &str, FactId)> = Vec::new();
    let mut blocks: BTreeMap<EntityId, &str> = BTreeMap::new();
    for (path, ids) in &facts.path_facts {
        for id in ids {
            let Some(fact) = by_id.get(id) else { continue };
            let FactValue::Text(fqn) = &fact.object else {
                continue;
            };
            if fact.predicate.as_str() != DEFINES {
                continue;
            }
            let Some(identity) = identity_in(path.as_str(), fqn) else {
                continue;
            };
            match type_of(identity) {
                BUCKET => {
                    observed.insert(fact.id);
                    buckets.push((path.as_str(), fqn.as_str(), identity, fact.id));
                }
                PUBLIC_ACCESS_BLOCK => {
                    observed.insert(fact.id);
                    blocks.insert(fact.subject, directory_of(path.as_str()));
                }
                _ => {}
            }
        }
    }

    let mut protected: BTreeSet<(String, String)> = BTreeSet::new();
    for fact in &facts.facts {
        if fact.predicate.as_str() != REFERENCES {
            continue;
        }
        let (Some(module), FactValue::Text(target)) = (blocks.get(&fact.subject), &fact.object)
        else {
            continue;
        };
        observed.insert(fact.id);
        protected.insert(referenced(target, module));
    }

    let unprotected = buckets
        .into_iter()
        .filter(|(path, _, identity, _)| {
            let module = directory_of(path);
            !unread.contains(module)
                && !protected.contains(&(module.to_string(), identity.to_string()))
        })
        .map(|(path, fqn, _, fact)| UnprotectedBucket {
            path: path.to_string(),
            fqn: fqn.to_string(),
            fact,
        })
        .collect();
    BucketScan {
        unprotected,
        observed,
    }
}
