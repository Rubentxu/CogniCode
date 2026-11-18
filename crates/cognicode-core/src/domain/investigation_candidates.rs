//! Reviewable candidates of an investigation: what an agent suspects, why,
//! and which evidence the suspicion rests on.
//!
//! The facts of a codebase belong to CogniCode. A candidate is not a fact and
//! is never written as one: it is an annotation an agent keeps while it
//! investigates, a **reference** to a symbol, a short reason and references to
//! the [`Evidence`](super::investigation::Evidence) items already pinned to
//! the investigation. Nothing here holds source text, a prompt or a
//! conversation, and nothing here decides: the state of a candidate changes
//! only when a caller asks for it, and evidence arriving never confirms.
//!
//! ## Scope
//!
//! A [`CandidateLedger`] belongs to exactly one [`CandidateScope`], a workspace
//! and a session. The service refuses a scope that is not the ledger's, so
//! two sessions of the same workspace never read each other's candidates.
//!
//! ## States
//!
//! | state | how a candidate gets there |
//! | --- | --- |
//! | `candidate` | proposed; reopened when new supporting evidence reaches a discarded one, when refuting evidence reaches a confirmed one, or when a revision could have changed what it rests on |
//! | `discarded` | asked for explicitly, and only with current refuting evidence |
//! | `confirmed` | asked for explicitly, and only with current supporting evidence and nothing stale |
//!
//! ## Freshness
//!
//! A candidate and each of its evidence links remember the snapshot they were
//! observed at and the last snapshot they were proven valid at. A candidate is
//! **stale** when anything it rests on was not proven valid at the ledger's
//! current snapshot. A revision keeps a reference current only when the caller
//! proves nothing it read changed; every other reference becomes stale and a
//! confirmed or discarded candidate is reopened. A stale candidate is revalidated
//! by observing its evidence again, never by waiting.
//!
//! ## Wire contract
//!
//! A ledger serializes as JSON under [`CANDIDATE_LEDGER_SCHEMA`]. Readers
//! ignore fields they do not know, a writer adds fields without changing the
//! schema string, and a change that removes or re-types a field publishes a
//! new one. A ledger under an unknown schema is refused, not guessed at.
//!
//! This module is pure: no I/O, no clock, no persistence.

use std::cmp::Reverse;

use serde::{Deserialize, Serialize};

use crate::domain::aggregates::call_graph::SymbolId;

/// The schema string of a serialized [`CandidateLedger`].
pub const CANDIDATE_LEDGER_SCHEMA: &str = "cognicode.investigation-candidates/v1";

/// The kind of the investigation artifact a ledger is stored in.
pub const CANDIDATE_ARTIFACT_KIND: &str = "investigation_candidates";

/// Reserved prefix for one session's candidate ledger artifact ID.
pub(crate) const CANDIDATE_ARTIFACT_ID_PREFIX: &str = "candidates:";

/// How many candidates one ledger holds.
pub const MAX_CANDIDATES: usize = 64;

/// How many evidence links one candidate holds.
pub const MAX_EVIDENCE_LINKS: usize = 32;

/// How many bytes the reason of a candidate may occupy. A reason names why,
/// it does not quote: a longer text is refused so a fragment of source or a
/// prompt cannot ride along.
pub const MAX_REASON_BYTES: usize = 512;

/// How many bytes an identifier (subject, evidence, session) may occupy.
pub const MAX_IDENTIFIER_BYTES: usize = 256;

/// Why a ledger operation was refused.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum CandidateError {
    /// An identifier or the reason was empty, too long or carried a control
    /// character.
    #[error("invalid {field}: {reason}")]
    Invalid {
        /// The field that was refused.
        field: &'static str,
        /// What is wrong with it.
        reason: &'static str,
    },
    /// The ledger or the candidate is full.
    #[error("quota exceeded: {what} holds at most {limit}")]
    QuotaExceeded {
        /// What is full.
        what: &'static str,
        /// The limit.
        limit: usize,
    },
    /// The subject already has a candidate in this ledger.
    #[error("subject already has candidate {0}")]
    DuplicateSubject(String),
    /// No candidate has this id in this ledger.
    #[error("candidate not found: {0}")]
    CandidateNotFound(String),
    /// The candidate has no link to this evidence.
    #[error("candidate {candidate} has no link to evidence {evidence}")]
    LinkNotFound {
        /// The candidate.
        candidate: String,
        /// The evidence.
        evidence: String,
    },
    /// The requested state change is not backed by evidence.
    #[error("cannot {action} candidate {candidate}: {reason}")]
    NotBackedByEvidence {
        /// The candidate.
        candidate: String,
        /// `discard` or `confirm`.
        action: &'static str,
        /// What is missing.
        reason: &'static str,
    },
    /// The revision was computed from a snapshot the ledger is no longer at.
    #[error("ledger is at snapshot {current}, the revision starts from {expected}")]
    RevisionConflict {
        /// The snapshot the ledger is at.
        current: String,
        /// The snapshot the caller started from.
        expected: String,
    },
    /// The candidate ledger artifact ID is already owned by another artifact or scope.
    #[error("candidate ledger artifact conflicts with existing artifact: {0}")]
    ArtifactConflict(String),
    /// The ledger was written under a schema this reader does not know.
    #[error("unsupported candidate ledger schema: {0}")]
    UnsupportedSchema(String),
}

/// Whose ledger it is: a workspace and one session of it.
///
/// This is a partition key, not an authorization credential. A CLI/MCP or
/// human-facing adapter must derive both IDs from its admitted caller context;
/// it must not trust arbitrary request fields to authorize access.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct CandidateScope {
    workspace_id: String,
    session_id: String,
}

impl CandidateScope {
    /// The scope of a session.
    ///
    /// # Errors
    /// An identifier is empty, too long or carries a control character.
    pub fn new(
        workspace_id: impl Into<String>,
        session_id: impl Into<String>,
    ) -> Result<Self, CandidateError> {
        let workspace_id = workspace_id.into();
        let session_id = session_id.into();
        check_identifier("workspace_id", &workspace_id)?;
        check_identifier("session_id", &session_id)?;
        Ok(Self {
            workspace_id,
            session_id,
        })
    }

    /// The workspace.
    pub fn workspace_id(&self) -> &str {
        &self.workspace_id
    }

    /// The session.
    pub fn session_id(&self) -> &str {
        &self.session_id
    }
}

/// Where a candidate stands.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CandidateState {
    /// Under review.
    Candidate,
    /// Ruled out by refuting evidence.
    Discarded,
    /// Held by supporting evidence.
    Confirmed,
}

/// Which way a piece of evidence points.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EvidencePolarity {
    /// The evidence supports the candidate.
    Supports,
    /// The evidence speaks against the candidate: negative evidence.
    Refutes,
}

/// A reference from a candidate to an evidence item of the investigation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvidenceLink {
    /// The id of the pinned [`Evidence`](super::investigation::Evidence).
    pub evidence_id: String,
    /// Which way it points.
    pub polarity: EvidencePolarity,
    /// The snapshot the evidence was observed at.
    pub observed_at: String,
    /// The last snapshot the evidence was proven valid at.
    pub valid_at: String,
}

/// One reviewable candidate.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InvestigationCandidate {
    /// The id of the candidate within its ledger.
    pub id: String,
    /// The canonical identity of the symbol this candidate points at. A
    /// surface verifies its workspace/snapshot-bound handle before creating
    /// this ID; the ledger never stores the signed transport handle or source.
    pub subject: SymbolId,
    /// Why the agent suspects it. Short, and data: it is never interpreted.
    pub reason: String,
    /// Where the candidate stands.
    pub state: CandidateState,
    /// The evidence the candidate rests on, by reference.
    pub evidence: Vec<EvidenceLink>,
    /// The snapshot the candidate was proposed at.
    pub observed_at: String,
    /// The last snapshot the subject was proven valid at.
    pub valid_at: String,
    /// A revision reopened the candidate; cleared by a new explicit decision.
    #[serde(default)]
    pub reopened_by_revision: bool,
}

impl InvestigationCandidate {
    /// Whether anything the candidate rests on was not proven valid at
    /// `snapshot`.
    pub fn is_stale(&self, snapshot: &str) -> bool {
        self.valid_at != snapshot || self.evidence.iter().any(|link| link.valid_at != snapshot)
    }

    /// Supporting minus refuting links.
    pub fn score(&self) -> i64 {
        self.evidence
            .iter()
            .map(|link| match link.polarity {
                EvidencePolarity::Supports => 1,
                EvidencePolarity::Refutes => -1,
            })
            .sum()
    }

    fn current_links(&self, snapshot: &str, polarity: EvidencePolarity) -> usize {
        self.evidence
            .iter()
            .filter(|link| link.polarity == polarity && link.valid_at == snapshot)
            .count()
    }
}

/// The candidates one session keeps for one investigation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CandidateLedger {
    /// The schema this value was written under.
    pub schema: String,
    scope: CandidateScope,
    snapshot: String,
    next_sequence: u64,
    candidates: Vec<InvestigationCandidate>,
}

impl CandidateLedger {
    /// An empty ledger anchored at `snapshot`.
    ///
    /// # Errors
    /// The snapshot identity is empty, too long or carries a control character.
    pub fn new(scope: CandidateScope, snapshot: impl Into<String>) -> Result<Self, CandidateError> {
        let snapshot = snapshot.into();
        check_identifier("snapshot", &snapshot)?;
        Ok(Self {
            schema: CANDIDATE_LEDGER_SCHEMA.to_string(),
            scope,
            snapshot,
            next_sequence: 1,
            candidates: Vec::new(),
        })
    }

    /// Reads a ledger back from its wire form.
    ///
    /// # Errors
    /// The text is not a ledger, or was written under a schema this reader
    /// does not know.
    pub fn from_wire(text: &str) -> Result<Self, CandidateError> {
        let ledger: Self = serde_json::from_str(text).map_err(|_| CandidateError::Invalid {
            field: "ledger",
            reason: "not a candidate ledger",
        })?;
        if ledger.schema != CANDIDATE_LEDGER_SCHEMA {
            return Err(CandidateError::UnsupportedSchema(ledger.schema));
        }
        Ok(ledger)
    }

    /// The wire form of the ledger.
    pub fn to_wire(&self) -> String {
        serde_json::to_string(self).expect("a ledger is plain data and always serializes")
    }

    /// Whose ledger it is.
    pub fn scope(&self) -> &CandidateScope {
        &self.scope
    }

    /// The snapshot the ledger is at.
    pub fn snapshot(&self) -> &str {
        &self.snapshot
    }

    /// The candidates, in the order they were proposed.
    pub fn candidates(&self) -> &[InvestigationCandidate] {
        &self.candidates
    }

    /// One candidate.
    pub fn candidate(&self, id: &str) -> Option<&InvestigationCandidate> {
        self.candidates.iter().find(|candidate| candidate.id == id)
    }

    /// Proposes a candidate for `subject` at the ledger's snapshot.
    ///
    /// # Errors
    /// The subject or reason is invalid, the subject already has a candidate,
    /// or the ledger is full.
    pub fn propose(
        &mut self,
        subject: &SymbolId,
        reason: &str,
    ) -> Result<&InvestigationCandidate, CandidateError> {
        check_identifier("subject", subject.as_str())?;
        check_reason(reason)?;
        if let Some(existing) = self
            .candidates
            .iter()
            .find(|candidate| candidate.subject.as_str() == subject.as_str())
        {
            return Err(CandidateError::DuplicateSubject(existing.id.clone()));
        }
        if self.candidates.len() >= MAX_CANDIDATES {
            return Err(CandidateError::QuotaExceeded {
                what: "a ledger",
                limit: MAX_CANDIDATES,
            });
        }
        let id = format!("cand-{}", self.next_sequence);
        self.next_sequence += 1;
        self.candidates.push(InvestigationCandidate {
            id,
            subject: subject.clone(),
            reason: reason.to_string(),
            state: CandidateState::Candidate,
            evidence: Vec::new(),
            observed_at: self.snapshot.clone(),
            valid_at: self.snapshot.clone(),
            reopened_by_revision: false,
        });
        Ok(self.candidates.last().expect("a candidate was just pushed"))
    }

    /// Links evidence to a candidate, observed at the ledger's snapshot.
    ///
    /// New supporting evidence reopens a discarded candidate and new refuting
    /// evidence reopens a confirmed one. Evidence never confirms or discards:
    /// the decision is a separate call. Linking the same evidence again
    /// replaces its polarity and refreshes its observation.
    ///
    /// # Errors
    /// The candidate does not exist, the evidence id is invalid, or the
    /// candidate holds its maximum of links.
    pub fn link_evidence(
        &mut self,
        candidate_id: &str,
        evidence_id: &str,
        polarity: EvidencePolarity,
    ) -> Result<&InvestigationCandidate, CandidateError> {
        check_identifier("evidence_id", evidence_id)?;
        let snapshot = self.snapshot.clone();
        let candidate = self.candidate_mut(candidate_id)?;
        let link = EvidenceLink {
            evidence_id: evidence_id.to_string(),
            polarity,
            observed_at: snapshot.clone(),
            valid_at: snapshot,
        };
        if let Some(index) = candidate
            .evidence
            .iter()
            .position(|existing| existing.evidence_id == evidence_id)
        {
            candidate.evidence[index] = link;
        } else if candidate.evidence.len() >= MAX_EVIDENCE_LINKS {
            return Err(CandidateError::QuotaExceeded {
                what: "a candidate",
                limit: MAX_EVIDENCE_LINKS,
            });
        } else {
            candidate.evidence.push(link);
        }
        let reopens = matches!(
            (candidate.state, polarity),
            (CandidateState::Discarded, EvidencePolarity::Supports)
                | (CandidateState::Confirmed, EvidencePolarity::Refutes)
        );
        if reopens {
            candidate.state = CandidateState::Candidate;
        }
        Ok(&*candidate)
    }

    /// Observes the evidence of a candidate again at the ledger's snapshot:
    /// the caller re-read it and it still stands. The state does not change.
    ///
    /// # Errors
    /// The candidate or the link does not exist.
    pub fn revalidate_link(
        &mut self,
        candidate_id: &str,
        evidence_id: &str,
    ) -> Result<&InvestigationCandidate, CandidateError> {
        let snapshot = self.snapshot.clone();
        let candidate = self.candidate_mut(candidate_id)?;
        let link = candidate
            .evidence
            .iter_mut()
            .find(|link| link.evidence_id == evidence_id)
            .ok_or_else(|| CandidateError::LinkNotFound {
                candidate: candidate_id.to_string(),
                evidence: evidence_id.to_string(),
            })?;
        link.observed_at = snapshot.clone();
        link.valid_at = snapshot;
        Ok(&*candidate)
    }

    /// Observes the subject of a candidate again at the ledger's snapshot.
    ///
    /// # Errors
    /// The candidate does not exist.
    pub fn revalidate_subject(
        &mut self,
        candidate_id: &str,
    ) -> Result<&InvestigationCandidate, CandidateError> {
        let snapshot = self.snapshot.clone();
        let candidate = self.candidate_mut(candidate_id)?;
        candidate.observed_at = snapshot.clone();
        candidate.valid_at = snapshot;
        candidate.reopened_by_revision = false;
        Ok(&*candidate)
    }

    /// Discards a candidate. Only current refuting evidence discards.
    ///
    /// # Errors
    /// The candidate does not exist or has no current refuting evidence.
    pub fn discard(
        &mut self,
        candidate_id: &str,
    ) -> Result<&InvestigationCandidate, CandidateError> {
        let snapshot = self.snapshot.clone();
        let candidate = self.candidate_mut(candidate_id)?;
        if candidate.current_links(&snapshot, EvidencePolarity::Refutes) == 0 {
            return Err(CandidateError::NotBackedByEvidence {
                candidate: candidate_id.to_string(),
                action: "discard",
                reason: "no current refuting evidence",
            });
        }
        candidate.state = CandidateState::Discarded;
        candidate.reopened_by_revision = false;
        Ok(&*candidate)
    }

    /// Confirms a candidate. Only current supporting evidence confirms, and
    /// never while anything the candidate rests on is stale.
    ///
    /// # Errors
    /// The candidate does not exist, has no current supporting evidence, or is
    /// stale.
    pub fn confirm(
        &mut self,
        candidate_id: &str,
    ) -> Result<&InvestigationCandidate, CandidateError> {
        let snapshot = self.snapshot.clone();
        let candidate = self.candidate_mut(candidate_id)?;
        if candidate.is_stale(&snapshot) {
            return Err(CandidateError::NotBackedByEvidence {
                candidate: candidate_id.to_string(),
                action: "confirm",
                reason: "stale references must be revalidated first",
            });
        }
        if candidate.current_links(&snapshot, EvidencePolarity::Supports) == 0 {
            return Err(CandidateError::NotBackedByEvidence {
                candidate: candidate_id.to_string(),
                action: "confirm",
                reason: "no current supporting evidence",
            });
        }
        candidate.state = CandidateState::Confirmed;
        candidate.reopened_by_revision = false;
        Ok(&*candidate)
    }

    /// Removes a candidate.
    ///
    /// # Errors
    /// The candidate does not exist.
    pub fn remove(&mut self, candidate_id: &str) -> Result<(), CandidateError> {
        let index = self
            .candidates
            .iter()
            .position(|candidate| candidate.id == candidate_id)
            .ok_or_else(|| CandidateError::CandidateNotFound(candidate_id.to_string()))?;
        self.candidates.remove(index);
        Ok(())
    }

    /// Moves the ledger from `from` to `to`.
    ///
    /// `still_valid` names the candidates a caller proved unaffected by the
    /// revision: their references stay current. Every other candidate becomes
    /// stale, and a confirmed or discarded one is reopened as a candidate.
    /// Nothing is confirmed here.
    ///
    /// # Errors
    /// The ledger is not at `from`: the revision was computed against a
    /// snapshot that is no longer current, and nothing changes.
    pub fn apply_revision(
        &mut self,
        from: &str,
        to: &str,
        still_valid: &[String],
    ) -> Result<(), CandidateError> {
        check_identifier("snapshot", to)?;
        if self.snapshot != from {
            return Err(CandidateError::RevisionConflict {
                current: self.snapshot.clone(),
                expected: from.to_string(),
            });
        }
        if from == to {
            return Ok(());
        }
        self.snapshot = to.to_string();
        for candidate in &mut self.candidates {
            if still_valid.contains(&candidate.id) {
                candidate.valid_at = to.to_string();
                for link in &mut candidate.evidence {
                    link.valid_at = to.to_string();
                }
                continue;
            }
            if candidate.state != CandidateState::Candidate {
                candidate.state = CandidateState::Candidate;
                candidate.reopened_by_revision = true;
            }
        }
        Ok(())
    }

    /// The candidates in review order: confirmed first, then open, then
    /// discarded; within a state current before stale, then by net evidence
    /// (negative evidence sinks a candidate), then in proposal order.
    pub fn ranked(&self) -> Vec<&InvestigationCandidate> {
        let mut ranked: Vec<(usize, &InvestigationCandidate)> =
            self.candidates.iter().enumerate().collect();
        ranked.sort_by_key(|(position, candidate)| {
            (
                match candidate.state {
                    CandidateState::Confirmed => 0,
                    CandidateState::Candidate => 1,
                    CandidateState::Discarded => 2,
                },
                candidate.is_stale(&self.snapshot),
                Reverse(candidate.score()),
                *position,
            )
        });
        ranked.into_iter().map(|(_, candidate)| candidate).collect()
    }

    fn candidate_mut(&mut self, id: &str) -> Result<&mut InvestigationCandidate, CandidateError> {
        self.candidates
            .iter_mut()
            .find(|candidate| candidate.id == id)
            .ok_or_else(|| CandidateError::CandidateNotFound(id.to_string()))
    }
}

fn check_identifier(field: &'static str, value: &str) -> Result<(), CandidateError> {
    let reason = if value.is_empty() {
        "empty"
    } else if value.len() > MAX_IDENTIFIER_BYTES {
        "too long"
    } else if value.chars().any(char::is_control) {
        "control character"
    } else {
        return Ok(());
    };
    Err(CandidateError::Invalid { field, reason })
}

fn check_reason(reason: &str) -> Result<(), CandidateError> {
    let problem = if reason.trim().is_empty() {
        "empty"
    } else if reason.len() > MAX_REASON_BYTES {
        "too long: a reason names why, it does not quote"
    } else if reason.chars().any(|c| c.is_control() && c != '\n') {
        "control character"
    } else {
        return Ok(());
    };
    Err(CandidateError::Invalid {
        field: "reason",
        reason: problem,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ledger() -> CandidateLedger {
        CandidateLedger::new(CandidateScope::new("ws", "s1").unwrap(), "snap-1").unwrap()
    }

    fn proposed(ledger: &mut CandidateLedger, subject: &str) -> String {
        ledger
            .propose(&SymbolId::new(subject), "suspected")
            .unwrap()
            .id
            .clone()
    }

    #[test]
    fn a_proposal_stores_identifiers_reason_state_and_snapshot_and_no_text() {
        let mut ledger = ledger();
        let id = proposed(&mut ledger, "src/a.rs:parse:10");
        let candidate = ledger.candidate(&id).unwrap();
        assert_eq!(candidate.subject.as_str(), "src/a.rs:parse:10");
        assert_eq!(candidate.state, CandidateState::Candidate);
        assert_eq!(candidate.observed_at, "snap-1");
        assert!(!ledger.to_wire().contains("prompt"));
    }

    #[test]
    fn a_reason_that_quotes_is_refused() {
        let mut ledger = ledger();
        let quote = "x".repeat(MAX_REASON_BYTES + 1);
        assert!(matches!(
            ledger.propose(&SymbolId::new("src/a.rs:parse:10"), &quote),
            Err(CandidateError::Invalid {
                field: "reason",
                ..
            })
        ));
    }

    #[test]
    fn the_same_subject_is_proposed_once() {
        let mut ledger = ledger();
        let id = proposed(&mut ledger, "src/a.rs:parse:10");
        assert_eq!(
            ledger
                .propose(&SymbolId::new("src/a.rs:parse:10"), "again")
                .unwrap_err(),
            CandidateError::DuplicateSubject(id)
        );
    }

    #[test]
    fn quotas_bound_the_ledger_and_each_candidate() {
        let mut ledger = ledger();
        for n in 0..MAX_CANDIDATES {
            proposed(&mut ledger, &format!("src/a.rs:f{n}:1"));
        }
        assert!(matches!(
            ledger.propose(&SymbolId::new("src/a.rs:extra:1"), "r"),
            Err(CandidateError::QuotaExceeded { .. })
        ));
        for n in 0..MAX_EVIDENCE_LINKS {
            ledger
                .link_evidence("cand-1", &format!("ev-{n}"), EvidencePolarity::Supports)
                .unwrap();
        }
        assert!(matches!(
            ledger.link_evidence("cand-1", "ev-extra", EvidencePolarity::Supports),
            Err(CandidateError::QuotaExceeded { .. })
        ));
    }

    #[test]
    fn evidence_never_confirms_and_confirmation_needs_supporting_evidence() {
        let mut ledger = ledger();
        let id = proposed(&mut ledger, "src/a.rs:parse:10");
        assert!(ledger.confirm(&id).is_err());
        ledger
            .link_evidence(&id, "ev-1", EvidencePolarity::Supports)
            .unwrap();
        assert_eq!(
            ledger.candidate(&id).unwrap().state,
            CandidateState::Candidate
        );
        assert_eq!(
            ledger.confirm(&id).unwrap().state,
            CandidateState::Confirmed
        );
    }

    #[test]
    fn discarding_needs_refuting_evidence_and_supporting_evidence_recovers() {
        let mut ledger = ledger();
        let id = proposed(&mut ledger, "src/a.rs:parse:10");
        assert!(ledger.discard(&id).is_err());
        ledger
            .link_evidence(&id, "ev-no", EvidencePolarity::Refutes)
            .unwrap();
        assert_eq!(
            ledger.discard(&id).unwrap().state,
            CandidateState::Discarded
        );
        let recovered = ledger
            .link_evidence(&id, "ev-yes", EvidencePolarity::Supports)
            .unwrap();
        assert_eq!(recovered.state, CandidateState::Candidate);
    }

    #[test]
    fn refuting_evidence_reopens_a_confirmed_candidate_without_discarding_it() {
        let mut ledger = ledger();
        let id = proposed(&mut ledger, "src/a.rs:parse:10");
        ledger
            .link_evidence(&id, "ev-1", EvidencePolarity::Supports)
            .unwrap();
        ledger.confirm(&id).unwrap();
        let reopened = ledger
            .link_evidence(&id, "ev-2", EvidencePolarity::Refutes)
            .unwrap();
        assert_eq!(reopened.state, CandidateState::Candidate);
    }

    #[test]
    fn negative_evidence_reorders_and_new_supporting_evidence_brings_a_candidate_back() {
        let mut ledger = ledger();
        let a = proposed(&mut ledger, "src/a.rs:parse:10");
        let b = proposed(&mut ledger, "src/b.rs:parse:3");
        ledger
            .link_evidence(&a, "ev-1", EvidencePolarity::Supports)
            .unwrap();
        let order = |ledger: &CandidateLedger| -> Vec<String> {
            ledger.ranked().iter().map(|c| c.id.clone()).collect()
        };
        assert_eq!(order(&ledger), vec![a.clone(), b.clone()]);

        ledger
            .link_evidence(&a, "ev-2", EvidencePolarity::Refutes)
            .unwrap();
        ledger
            .link_evidence(&a, "ev-3", EvidencePolarity::Refutes)
            .unwrap();
        assert_eq!(order(&ledger), vec![b.clone(), a.clone()]);

        ledger.discard(&a).unwrap();
        ledger
            .link_evidence(&a, "ev-4", EvidencePolarity::Supports)
            .unwrap();
        ledger
            .link_evidence(&a, "ev-5", EvidencePolarity::Supports)
            .unwrap();
        ledger
            .link_evidence(&a, "ev-6", EvidencePolarity::Supports)
            .unwrap();
        assert_eq!(order(&ledger), vec![a, b]);
    }

    #[test]
    fn a_revision_makes_unproven_references_stale_and_reopens_without_confirming() {
        let mut ledger = ledger();
        let a = proposed(&mut ledger, "src/a.rs:parse:10");
        let b = proposed(&mut ledger, "src/b.rs:parse:3");
        for id in [&a, &b] {
            ledger
                .link_evidence(id, "ev", EvidencePolarity::Supports)
                .unwrap();
            ledger.confirm(id).unwrap();
        }

        ledger
            .apply_revision("snap-1", "snap-2", std::slice::from_ref(&a))
            .unwrap();

        let kept = ledger.candidate(&a).unwrap();
        assert_eq!(kept.state, CandidateState::Confirmed);
        assert!(!kept.is_stale("snap-2"));
        let stale = ledger.candidate(&b).unwrap();
        assert_eq!(stale.state, CandidateState::Candidate);
        assert!(stale.reopened_by_revision && stale.is_stale("snap-2"));
        assert!(ledger.confirm(&b).is_err());

        ledger.revalidate_subject(&b).unwrap();
        ledger.revalidate_link(&b, "ev").unwrap();
        assert_eq!(
            ledger.candidate(&b).unwrap().state,
            CandidateState::Candidate
        );
        assert_eq!(ledger.confirm(&b).unwrap().state, CandidateState::Confirmed);
    }

    #[test]
    fn a_revision_from_a_snapshot_the_ledger_left_changes_nothing() {
        let mut ledger = ledger();
        proposed(&mut ledger, "src/a.rs:parse:10");
        ledger.apply_revision("snap-1", "snap-2", &[]).unwrap();
        let before = ledger.clone();
        assert_eq!(
            ledger.apply_revision("snap-1", "snap-3", &[]).unwrap_err(),
            CandidateError::RevisionConflict {
                current: "snap-2".to_string(),
                expected: "snap-1".to_string()
            }
        );
        assert_eq!(ledger, before);
    }

    #[test]
    fn the_wire_form_round_trips_and_an_unknown_schema_is_refused() {
        let mut ledger = ledger();
        proposed(&mut ledger, "src/a.rs:parse:10");
        assert_eq!(
            CandidateLedger::from_wire(&ledger.to_wire()).unwrap(),
            ledger
        );

        let foreign = ledger
            .to_wire()
            .replace(CANDIDATE_LEDGER_SCHEMA, "other/v9");
        assert_eq!(
            CandidateLedger::from_wire(&foreign).unwrap_err(),
            CandidateError::UnsupportedSchema("other/v9".to_string())
        );
    }

    #[test]
    fn a_reader_ignores_fields_a_later_writer_added() {
        let mut value: serde_json::Value = serde_json::from_str(&ledger().to_wire()).unwrap();
        value["added_later"] = serde_json::json!({"x": 1});
        assert!(CandidateLedger::from_wire(&value.to_string()).is_ok());
    }
}
