//! The verification of the retrieved candidates, and the budget that bounds it.
//!
//! ## What it owns
//!
//! `retrieve_and_verify` retrieves the candidates with a lexical search and
//! then compiles every Rust candidate it found. The compilation is the work
//! that can take long: a candidate that does not compile can spend the whole
//! per-candidate timeout inside `rustc`, and there are as many candidates as
//! the caller asked for. Without a total budget the work of the tool was
//! unbounded — the doc of the handler promised a total timeout that nothing
//! executed, and the dispatch window of the category (`search`, 500 ms) was a
//! promise the tool could not keep either.
//!
//! Here the budget is a number with one owner, and two readers:
//!
//! - the tool stops the verification when the budget is gone and says which
//!   candidates it did not verify;
//! - the dispatch window of the server is derived from it, so the window and
//!   the work cannot disagree about the same call.
//!
//! ## Why the owner is this layer
//!
//! The catalogue could have declared the budget, and it does declare what a
//! tool *is* (`requires_graph`, `mutates_workspace`). It cannot declare how
//! long this work takes, for two reasons that are both architecture and not
//! taste:
//!
//! - the catalogue lives in `infrastructure/`, and CR-06 does not let the
//!   application read it. The allowlist only shrinks, so opening that door
//!   is not an option;
//! - the published method `retrieve_and_verify(&self, input)` cannot grow a
//!   parameter without breaking the consumers of a published crate. A
//!   budget that reached the work as an argument would have to come from
//!   somewhere the application can read, and the only such place is here.
//!
//! So the work declares its own budget, and the interface is the bridge that
//! carries it to the server (`interface/mcp/work_budget.rs`).
//!
//! ## The order of the candidates
//!
//! The candidates are verified in a written order: the files with more
//! matches first, and the path ascending for the ties. The order is not a
//! detail — with a budget, the set of verified files depends on the order in
//! which they are visited, and the order of a `HashMap` is not written
//! anywhere. With this order the files that carry more of the answer are the
//! ones that get verified first, which is also what the caller wants.
//!
//! The answer's own order is the same one, so the response does not depend on
//! which files the budget reached.

use crate::application::dto::{ContentMatch, UnreadEntry, VerificationStatus, VerifiedMatchDto};
use crate::domain::traits::code_verifier::{CodeVerifier, CompilationResult};
use std::collections::BTreeMap;
use std::path::Path;
use std::time::{Duration, Instant};

/// How long the whole work of the verification may take.
///
/// The number is a promise to the caller: the tool answers within it, and
/// the answer says what it did not verify. It is also the number the
/// dispatch window is derived from, so the window covers the work instead
/// of cutting it.
///
/// Forty seconds, plus the timeout of the candidate that was already in
/// flight when the budget ran out, is the longest the work can take
/// (`worst_case`). The window of the dispatch is derived from that worst
/// case, and the worst case is below the ceiling the server publishes
/// (sixty), with room for the answer to travel back.
pub const VERIFICATION_BUDGET: Duration = Duration::from_secs(40);

/// The timeout of a single candidate.
pub const CANDIDATE_TIMEOUT: Duration = Duration::from_secs(10);

/// The longest the work of the verification can take.
///
/// The budget stops the verification from starting a new candidate, and
/// the candidate that was in flight when the budget ran out can still
/// spend its own timeout. The window of the dispatch has to cover this,
/// or the caller gets a timeout instead of the answer.
pub fn worst_case() -> Duration {
    VERIFICATION_BUDGET + CANDIDATE_TIMEOUT
}

/// A candidate: the file it was found in, and the matches it holds.
#[derive(Debug, Clone)]
pub struct Candidate {
    /// The file the matches were found in.
    pub file: String,
    /// The matches of that file.
    pub matches: Vec<ContentMatch>,
}

/// What the retrieval of the candidates found.
///
/// The retrieval is the walk of the tree; the verification is what comes
/// after it. The two travel together because the budget of the
/// verification covers the walk as well: the walk is part of the work.
#[derive(Debug, Clone)]
pub struct Retrieval {
    /// The candidates, in the order they are verified.
    pub candidates: Vec<Candidate>,
    /// The entries the walk could not read.
    pub unread: Vec<UnreadEntry>,
    /// Whether the caller asked for the verification at all.
    pub verify: bool,
}

/// Groups the matches by file and puts the files in the order they are
/// verified: more matches first, path ascending for the ties.
pub fn candidates_in_verification_order(matches: Vec<ContentMatch>) -> Vec<Candidate> {
    let mut by_file: BTreeMap<String, Vec<ContentMatch>> = BTreeMap::new();
    for m in matches {
        by_file.entry(m.file.clone()).or_default().push(m);
    }
    let mut candidates: Vec<Candidate> = by_file
        .into_iter()
        .map(|(file, matches)| Candidate { file, matches })
        .collect();
    candidates.sort_by(|a, b| {
        b.matches
            .len()
            .cmp(&a.matches.len())
            .then_with(|| a.file.cmp(&b.file))
    });
    candidates
}

/// Verifies the candidates until the budget is gone, and says what it did not.
///
/// A candidate that is not Rust is skipped with its reason. A candidate
/// that the budget did not reach is skipped with the reason that names the
/// bound that was reached: the answer says what it did not cover instead of
/// pretending it covered everything.
///
/// The budget covers the whole call, not only this part of it: `started` is
/// the moment the call began, so the walk that retrieved the candidates
/// spends the budget too.
pub async fn verify_candidates(
    verifier: &dyn CodeVerifier,
    candidates: &[Candidate],
    budget: Duration,
    started: Instant,
) -> Vec<VerifiedMatchDto> {
    let mut results: Vec<VerifiedMatchDto> = Vec::new();

    for candidate in candidates {
        if started.elapsed() >= budget {
            let reason = format!("the verification budget ({budget:?}) was exhausted");
            for m in &candidate.matches {
                results.push(skipped_match(m, reason.clone()));
            }
            continue;
        }

        if !is_rust(&candidate.file) {
            for m in &candidate.matches {
                results.push(skipped_match(m, "not-rust".to_string()));
            }
            continue;
        }

        match verifier
            .verify_with_timeout(&candidate.file, CANDIDATE_TIMEOUT.as_secs())
            .await
        {
            Ok(outcome) => {
                let verdict = verdict_of(outcome);
                for m in &candidate.matches {
                    let mut verified = verified_match(m, verdict.status);
                    verified.check_output = verdict.check_output.clone();
                    verified.error_snippet = verdict.error_snippet.clone();
                    verified.reason = verdict.reason.clone();
                    results.push(verified);
                }
            }
            Err(error) => {
                let reason = format!("error: {}", error);
                for m in &candidate.matches {
                    results.push(skipped_match(m, reason.clone()));
                }
            }
        }
    }

    results
}

/// Whether the file is one the verifier can compile.
fn is_rust(file: &str) -> bool {
    Path::new(file).extension().and_then(|e| e.to_str()) == Some("rs")
}

/// The candidates of a retrieval whose verification was not asked for.
///
/// Every match is in the answer, and every one of them says why it was
/// not verified.
pub fn skipped_candidates(candidates: &[Candidate], reason: &str) -> Vec<VerifiedMatchDto> {
    candidates
        .iter()
        .flat_map(|candidate| candidate.matches.iter())
        .map(|m| skipped_match(m, reason.to_string()))
        .collect()
}

/// What the verification of one candidate found.
struct Verdict {
    status: VerificationStatus,
    check_output: Option<String>,
    error_snippet: Option<String>,
    reason: Option<String>,
}

/// The verdict of a compilation result.
fn verdict_of(outcome: CompilationResult) -> Verdict {
    match outcome {
        CompilationResult::Verified { stdout } => Verdict {
            status: VerificationStatus::Verified,
            check_output: Some(stdout),
            error_snippet: None,
            reason: None,
        },
        CompilationResult::Rejected { error } => Verdict {
            status: VerificationStatus::Rejected,
            check_output: None,
            error_snippet: Some(error),
            reason: None,
        },
        CompilationResult::Skipped { reason } => Verdict {
            status: VerificationStatus::Skipped,
            check_output: None,
            error_snippet: None,
            reason: Some(reason),
        },
    }
}

/// A match the retrieval found and did not verify, and why.
fn skipped_match(m: &ContentMatch, reason: String) -> VerifiedMatchDto {
    VerifiedMatchDto {
        file: m.file.clone(),
        line: m.line,
        col: m.col,
        matched_text: m.text.clone(),
        context: m.context.clone(),
        status: VerificationStatus::Skipped,
        check_output: None,
        error_snippet: None,
        reason: Some(reason),
    }
}

/// A match the retrieval found, with the verdict of its verification.
fn verified_match(m: &ContentMatch, status: VerificationStatus) -> VerifiedMatchDto {
    VerifiedMatchDto {
        file: m.file.clone(),
        line: m.line,
        col: m.col,
        matched_text: m.text.clone(),
        context: m.context.clone(),
        status,
        check_output: None,
        error_snippet: None,
        reason: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::traits::code_verifier::{CodeVerifierError, CompilationResult};
    use std::sync::Arc;

    /// A verifier that says every file compiles, and counts the calls.
    struct CountingVerifier {
        calls: std::sync::Mutex<Vec<String>>,
    }

    impl CountingVerifier {
        fn new() -> Self {
            Self {
                calls: std::sync::Mutex::new(Vec::new()),
            }
        }
    }

    #[async_trait::async_trait]
    impl CodeVerifier for CountingVerifier {
        fn toolchain_available(&self) -> Result<(), String> {
            Ok(())
        }

        fn verify(&self, _path: &str) -> Result<CompilationResult, CodeVerifierError> {
            Ok(CompilationResult::Verified {
                stdout: String::new(),
            })
        }

        async fn verify_with_timeout(
            &self,
            path: &str,
            _timeout_secs: u64,
        ) -> Result<CompilationResult, CodeVerifierError> {
            self.calls
                .lock()
                .expect("the lock is not poisoned")
                .push(path.to_string());
            Ok(CompilationResult::Verified {
                stdout: String::new(),
            })
        }
    }

    /// A match in a file, for the order and for the budget.
    fn match_in(file: &str, line: u32) -> ContentMatch {
        ContentMatch {
            file: file.to_string(),
            line,
            col: 1,
            text: "fn".to_string(),
            context: Vec::new(),
        }
    }

    /// The files with more matches are verified first, and the path
    /// ascending breaks the ties.
    ///
    /// With a budget, the set of verified files depends on the order in
    /// which they are visited: an order that is not written makes the
    /// answer depend on the hash of the map that held the matches.
    #[test]
    fn the_candidates_are_verified_in_a_written_order() {
        let candidates = candidates_in_verification_order(vec![
            match_in("b.rs", 1),
            match_in("a.rs", 1),
            match_in("b.rs", 2),
        ]);

        let files: Vec<&str> = candidates.iter().map(|c| c.file.as_str()).collect();
        assert_eq!(
            files,
            vec!["b.rs", "a.rs"],
            "the file with more matches comes first: {files:?}"
        );
    }

    /// A budget that is gone stops the verification, and the answer says
    /// what it did not cover.
    #[tokio::test]
    async fn a_budget_that_is_gone_names_the_candidates_it_did_not_verify() {
        let verifier = CountingVerifier::new();
        let candidates =
            candidates_in_verification_order(vec![match_in("a.rs", 1), match_in("b.rs", 1)]);

        let results =
            verify_candidates(&verifier, &candidates, Duration::ZERO, Instant::now()).await;

        assert_eq!(
            results.len(),
            2,
            "every match is in the answer: {results:?}"
        );
        for result in &results {
            assert_eq!(result.status, VerificationStatus::Skipped);
            let reason = result.reason.as_deref().expect("the reason is there");
            assert!(
                reason.contains("the verification budget"),
                "the reason names the bound that was reached: {reason}"
            );
        }
        assert!(
            verifier
                .calls
                .lock()
                .expect("the lock is not poisoned")
                .is_empty(),
            "a budget that is gone does not compile anything"
        );
    }

    /// A budget that is not gone verifies the candidates, in order.
    #[tokio::test]
    async fn a_budget_that_is_not_gone_verifies_the_candidates() {
        let verifier = CountingVerifier::new();
        let candidates =
            candidates_in_verification_order(vec![match_in("a.rs", 1), match_in("b.rs", 1)]);

        let results = verify_candidates(
            &verifier,
            &candidates,
            Duration::from_secs(60),
            Instant::now(),
        )
        .await;

        assert_eq!(
            results.len(),
            2,
            "every match is in the answer: {results:?}"
        );
        for result in &results {
            assert_eq!(result.status, VerificationStatus::Verified);
            assert!(result.reason.is_none(), "a verified match has no reason");
        }
        let calls = verifier
            .calls
            .lock()
            .expect("the lock is not poisoned")
            .clone();
        assert_eq!(calls, vec!["a.rs".to_string(), "b.rs".to_string()]);
    }

    /// A file that is not Rust is skipped with its reason, and it does not
    /// spend a compilation.
    #[tokio::test]
    async fn a_file_that_is_not_rust_is_skipped() {
        let verifier = CountingVerifier::new();
        let candidates = candidates_in_verification_order(vec![match_in("a.py", 1)]);

        let results = verify_candidates(
            &verifier,
            &candidates,
            Duration::from_secs(60),
            Instant::now(),
        )
        .await;

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].status, VerificationStatus::Skipped);
        assert_eq!(results[0].reason.as_deref(), Some("not-rust"));
        assert!(
            verifier
                .calls
                .lock()
                .expect("the lock is not poisoned")
                .is_empty(),
            "a file that is not Rust is not compiled"
        );
    }

    /// A candidate cannot cost more than the whole budget.
    ///
    /// The budget is what bounds the work, and a candidate that costs
    /// more than the budget would break the bound it is under. The
    /// ceiling the server publishes is the server's business: the
    /// contract that ties the two is there, where the ceiling lives.
    #[test]
    fn a_candidate_cannot_cost_more_than_the_budget() {
        assert!(
            CANDIDATE_TIMEOUT <= VERIFICATION_BUDGET,
            "a candidate cannot cost more than the whole budget: {CANDIDATE_TIMEOUT:?}"
        );
    }

    /// The verifier is used through the trait object the service holds.
    #[test]
    fn the_verifier_is_a_trait_object() {
        let verifier: Arc<dyn CodeVerifier> = Arc::new(CountingVerifier::new());
        assert!(verifier.toolchain_available().is_ok());
    }
}
