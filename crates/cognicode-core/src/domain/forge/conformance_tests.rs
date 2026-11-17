//! The forge conformance suite run over the in-memory forge, and over
//! deliberately broken forges that it must reject. A suite that passes
//! every forge proves nothing: each control below breaks one clause and
//! the suite has to name it.

use std::sync::{Arc, Mutex};

use super::conformance::{
    ForgeFixture, ForgeHarness, ProvisionedForge, Shared, clause, run_forge_conformance,
};
use super::fake::FakeForgeHarness;
use super::{
    CheckPublisher, CheckReport, ForgeCapabilities, ForgeError, HeadOrigin, MergeBase,
    MergePreview, PublishedCheck, PullRequestId, PullRequestState, ReviewReader,
};
use crate::domain::scm::SourceRef;

pub(crate) const BOTH: ForgeCapabilities = ForgeCapabilities {
    merge_base: true,
    merge_candidate: true,
};

/// One way a forge can break the contract.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Flaw {
    /// Keeps answering with the first head it saw: it ignores pushes.
    FrozenHead,
    /// Reports a base tip that is not the base's.
    WrongBaseTip,
    /// Invents a pull request nobody opened.
    FabricatesUnknownPullRequest,
    /// Reports every head as one that lives in the base repository.
    UpgradesForkOrigin,
    /// Answers readings it declared it cannot answer.
    AnswersWhatItDeclaredUnsupported,
    /// Reports unrelated histories as having the base tip in common.
    MissingBaseAsFound,
    /// Reports the base tip as the merge-base, whatever it is.
    WrongMergeBase,
    /// Reports a merge it has not computed as ready.
    PendingAsReady,
    /// Reports a ready merge as another revision than the computed one.
    WrongCandidate,
    /// Keeps reporting a ready merge after the head moved.
    StaleCandidate,
    /// Reports a conflicting merge as merely not computed yet.
    ConflictHidden,
    /// Records every check on the base tip instead of the head.
    PublishesOnBaseTip,
    /// Records checks without any way to find them again.
    PublishesWithoutReference,
}

/// A forge that behaves as a conforming one except for one [`Flaw`].
struct Flawed {
    reader: Arc<dyn ReviewReader>,
    publisher: Arc<dyn CheckPublisher>,
    flaw: Flaw,
    template: PullRequestState,
    first_seen: Mutex<Option<PullRequestState>>,
    last_ready: Mutex<Option<MergePreview>>,
}

impl ReviewReader for Flawed {
    fn capabilities(&self) -> ForgeCapabilities {
        self.reader.capabilities()
    }

    fn pull_request(&self, id: &PullRequestId) -> Result<PullRequestState, ForgeError> {
        let read = self.reader.pull_request(id);
        match (self.flaw, read) {
            (Flaw::FrozenHead, Ok(state)) => Ok(self
                .first_seen
                .lock()
                .expect("lock")
                .get_or_insert(state)
                .clone()),
            (Flaw::WrongBaseTip, Ok(state)) => Ok(PullRequestState {
                base_tip: state.head.clone(),
                ..state
            }),
            (Flaw::UpgradesForkOrigin, Ok(state)) => Ok(PullRequestState {
                origin: HeadOrigin::SameRepository,
                ..state
            }),
            (Flaw::FabricatesUnknownPullRequest, Err(ForgeError::PullRequestNotFound { .. })) => {
                Ok(PullRequestState {
                    id: id.clone(),
                    ..self.template.clone()
                })
            }
            (_, read) => read,
        }
    }

    fn merge_base(&self, state: &PullRequestState) -> Result<MergeBase, ForgeError> {
        let read = self.reader.merge_base(state);
        match (self.flaw, read) {
            (Flaw::AnswersWhatItDeclaredUnsupported, _)
            | (Flaw::MissingBaseAsFound, Ok(MergeBase::Absent))
            | (Flaw::WrongMergeBase, Ok(MergeBase::Found(_))) => {
                Ok(MergeBase::Found(state.base_tip.clone()))
            }
            (_, read) => read,
        }
    }

    fn merge_candidate(&self, state: &PullRequestState) -> Result<MergePreview, ForgeError> {
        let read = self.reader.merge_candidate(state);
        match (self.flaw, read) {
            (Flaw::AnswersWhatItDeclaredUnsupported, _)
            | (Flaw::PendingAsReady, Ok(MergePreview::Pending)) => {
                Ok(MergePreview::Ready(state.head.clone()))
            }
            (Flaw::WrongCandidate, Ok(MergePreview::Ready(_))) => {
                Ok(MergePreview::Ready(state.base_tip.clone()))
            }
            (Flaw::ConflictHidden, Ok(MergePreview::Conflicting)) => Ok(MergePreview::Pending),
            (Flaw::StaleCandidate, read) => {
                let mut remembered = self.last_ready.lock().expect("lock");
                match read {
                    Ok(ready @ MergePreview::Ready(_)) => {
                        *remembered = Some(ready.clone());
                        Ok(ready)
                    }
                    Ok(_) if remembered.is_some() => Ok(remembered.clone().expect("checked")),
                    read => read,
                }
            }
            (_, read) => read,
        }
    }
}

impl CheckPublisher for Flawed {
    fn publish(&self, report: &CheckReport) -> Result<PublishedCheck, ForgeError> {
        let receipt = self.publisher.publish(report)?;
        Ok(match self.flaw {
            Flaw::PublishesOnBaseTip => PublishedCheck {
                attached_to: self.template.base_tip.clone(),
                ..receipt
            },
            Flaw::PublishesWithoutReference => PublishedCheck {
                reference: "  ".to_string(),
                ..receipt
            },
            _ => receipt,
        })
    }
}

/// Builds the in-memory forge, broken by `flaw` when there is one.
pub(crate) struct FlawedHarness {
    pub(crate) capabilities: ForgeCapabilities,
    pub(crate) flaw: Flaw,
}

impl ForgeHarness for FlawedHarness {
    fn source(&self) -> SourceRef {
        FakeForgeHarness::new(self.capabilities).source()
    }

    fn provision(&self, fixture: &ForgeFixture) -> ProvisionedForge {
        let inner = FakeForgeHarness::new(self.capabilities).provision(fixture);
        let flawed = Arc::new(Flawed {
            reader: inner.reader,
            publisher: inner.publisher,
            flaw: self.flaw,
            template: fixture.pull_request.clone(),
            first_seen: Mutex::new(None),
            last_ready: Mutex::new(None),
        });
        ProvisionedForge {
            reader: Arc::new(Shared(Arc::clone(&flawed))),
            publisher: Arc::new(Shared(flawed)),
            scenario: inner.scenario,
        }
    }
}

const NEITHER: ForgeCapabilities = ForgeCapabilities::NONE;

/// Each flaw, the clause the suite must report for it and the
/// capabilities the forge declares while it misbehaves.
const CONTROLS: &[(Flaw, &str, ForgeCapabilities)] = &[
    (Flaw::FrozenHead, clause::PR_HEAD_MOVES, BOTH),
    (Flaw::WrongBaseTip, clause::PR_FACTS, BOTH),
    (Flaw::FabricatesUnknownPullRequest, clause::PR_UNKNOWN, BOTH),
    (Flaw::UpgradesForkOrigin, clause::PR_ORIGIN, BOTH),
    (
        Flaw::AnswersWhatItDeclaredUnsupported,
        clause::BASE_UNDECLARED,
        NEITHER,
    ),
    (
        Flaw::AnswersWhatItDeclaredUnsupported,
        clause::CANDIDATE_UNDECLARED,
        NEITHER,
    ),
    (Flaw::MissingBaseAsFound, clause::BASE_ABSENT, BOTH),
    (Flaw::WrongMergeBase, clause::BASE_FOUND, BOTH),
    (Flaw::PendingAsReady, clause::CANDIDATE_PENDING, BOTH),
    (Flaw::WrongCandidate, clause::CANDIDATE_READY, BOTH),
    (Flaw::StaleCandidate, clause::CANDIDATE_STALE, BOTH),
    (Flaw::ConflictHidden, clause::CANDIDATE_CONFLICT, BOTH),
    (Flaw::PublishesOnBaseTip, clause::PUBLISH_AT_HEAD, BOTH),
    (
        Flaw::PublishesWithoutReference,
        clause::PUBLISH_REFERENCE,
        BOTH,
    ),
];

#[test]
fn the_in_memory_forge_meets_the_contract_for_every_declaration_of_capabilities() {
    for capabilities in [
        BOTH,
        NEITHER,
        ForgeCapabilities {
            merge_base: true,
            merge_candidate: false,
        },
        ForgeCapabilities {
            merge_base: false,
            merge_candidate: true,
        },
    ] {
        run_forge_conformance(&FakeForgeHarness::new(capabilities))
            .assert_conforms(&format!("FakeForge declaring {capabilities:?}"));
    }
}

#[test]
fn the_suite_rejects_every_deliberately_broken_forge() {
    for (flaw, expected, capabilities) in CONTROLS {
        let report = run_forge_conformance(&FlawedHarness {
            capabilities: *capabilities,
            flaw: *flaw,
        });
        report.assert_detects(&format!("{flaw:?}"), expected);
    }
}

#[test]
fn every_clause_the_suite_can_report_has_a_negative_control() {
    let reportable = [
        clause::PR_FACTS,
        clause::PR_ORIGIN,
        clause::PR_HEAD_MOVES,
        clause::PR_UNKNOWN,
        clause::BASE_UNDECLARED,
        clause::BASE_FOUND,
        clause::BASE_ABSENT,
        clause::CANDIDATE_UNDECLARED,
        clause::CANDIDATE_PENDING,
        clause::CANDIDATE_READY,
        clause::CANDIDATE_CONFLICT,
        clause::CANDIDATE_STALE,
        clause::PUBLISH_AT_HEAD,
        clause::PUBLISH_REFERENCE,
    ];
    let uncontrolled: Vec<&str> = reportable
        .into_iter()
        .filter(|reportable| !CONTROLS.iter().any(|(_, clause, _)| clause == reportable))
        .collect();
    assert!(
        uncontrolled.is_empty(),
        "no negative control for {uncontrolled:?}"
    );
}
