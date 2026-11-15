//! The retrieval of the candidates and their verification.
//!
//! The route that the `retrieve_and_verify` tool publishes lives here, and
//! it lives apart from the rest of the file operations because it is a
//! different kind of work: it has a budget, a bound and a walk of the tree
//! that the other operations do not have.
//!
//! ## The three steps of the call
//!
//! 1. The retrieval: a lexical search over the tree, off the runtime that
//!    awaits the answer.
//! 2. The verification: every candidate is compiled, with a timeout per
//!    candidate.
//! 3. The answer: what was verified, what was rejected, what was skipped,
//!    and what the walk could not read.
//!
//! ## The budget
//!
//! The budget of the call covers the walk too, and not only the
//! verification: the bound starts when the call starts, so the walk that
//! retrieves the candidates spends it as well. What the budget did not
//! reach comes out of the answer with the reason that names the bound that
//! was reached. The budget and the bound belong to
//! `application/services/verification.rs`.

use super::FileOperationsService;
use crate::application::dto::{
    RetrieveAndVerifyRequest, RetrieveAndVerifyResult, SearchContentRequest, VerificationStatus,
};
use crate::application::error::{AppError, AppResult};
use crate::application::services::verification::{
    Bound, Retrieval, VERIFICATION_BUDGET, candidates_in_verification_order, skipped_candidates,
    verify_candidates,
};
use crate::application::services::workspace_walk::off_the_runtime;
use std::time::Duration;

impl FileOperationsService {
    /// The retrieval of the candidates and their verification.
    ///
    /// The retrieval is a lexical search over the tree; the verification
    /// compiles every Rust candidate it found, with a timeout per candidate
    /// and a budget for the whole call.
    ///
    /// The walk of the tree is the only part of this call that does not
    /// yield, so it moves off the runtime that awaits the answer: the
    /// method hands the walk to the blocking pool and awaits it there.
    /// The published signature does not change, and this method is the
    /// entry point for a consumer that calls the application directly.
    pub async fn retrieve_and_verify(
        &self,
        input: RetrieveAndVerifyRequest,
    ) -> AppResult<RetrieveAndVerifyResult> {
        self.retrieve_and_verify_within(input, VERIFICATION_BUDGET)
            .await
    }

    /// The same work, with the budget the caller gives it.
    ///
    /// The budget is a parameter of the work and not a constant inside it:
    /// the product's own budget is the one the published entry point uses,
    /// and a caller that needs another bound says so.
    ///
    /// The walk of the tree happens off the runtime that awaits the
    /// answer, and the verification that follows waits for a subprocess, so
    /// it yields while it waits. Nothing of this work occupies the thread
    /// that serves the calls.
    pub(crate) async fn retrieve_and_verify_within(
        &self,
        input: RetrieveAndVerifyRequest,
        budget: Duration,
    ) -> AppResult<RetrieveAndVerifyResult> {
        let bound = Bound::starting_now(budget);
        let service = self.clone();
        let walk_bound = bound.clone();
        let retrieval =
            off_the_runtime(move || service.retrieve_candidates(&input, Some(walk_bound)))
                .await
                .map_err(|e| {
                    AppError::InternalError(format!(
                        "the retrieval of the candidates panicked: {e}"
                    ))
                })??;
        Ok(self.verify_retrieval(retrieval, budget, &bound).await)
    }

    /// The retrieval of the candidates: the walk of the tree.
    ///
    /// Synchronous on purpose: this is the part that does not yield, and
    /// the caller that awaits it moves it off the runtime as a whole.
    ///
    /// The walk answers to the bound when there is one: it stops when the
    /// bound is reached, and the answer says that it stopped.
    pub(crate) fn retrieve_candidates(
        &self,
        input: &RetrieveAndVerifyRequest,
        bound: Option<Bound>,
    ) -> AppResult<Retrieval> {
        // Validate query is not empty
        if input.query.trim().is_empty() {
            return Err(AppError::InvalidParameter(
                "Empty query not allowed".to_string(),
            ));
        }

        // Early check: the verifier's toolchain must be available before doing
        // any verification work. This implements the spec requirement
        // "rustc not found → error 'rustc not found'".
        //
        // The probe goes through the injected `code_verifier` rather than
        // calling `which::which("rustc")` here. Two reasons, and the second
        // one is a bug fix:
        //
        // 1. A hardcoded `rustc` string in the application layer duplicates the
        //    knowledge that belongs to the adapter, so swapping the verifier
        //    for any other backend silently kept probing for rustc.
        // 2. Because the probe ignored the injected port, the "toolchain is
        //    absent" path could not be exercised without mutating the
        //    process-wide PATH. `test_retrieve_and_verify_rustc_not_found`
        //    did exactly that, and the window it opened was visible to every
        //    concurrently running lib test: a sibling whose verification
        //    reached this guard got "rustc not found" back and failed for a
        //    reason unrelated to what it was testing. That is how
        //    `test_retrieve_and_verify_deterministic` failed in merge-gate
        //    while passing in isolation. The port is the seam, so the test
        //    now injects a verifier that reports the toolchain missing and
        //    no environment is touched.
        //
        // The `which` lookup itself is kept inside `RustVerifier` where it
        // belongs: it is a PATH walk, not a fork, which matters under
        // parallel load — see M0.5 in MAINTENANCE.md, where a fork()+exec
        // probe failed with EAGAIN and was misreported as "rustc not found".
        if input.verify
            && let Err(error) = self.code_verifier.toolchain_available()
        {
            return Err(AppError::InvalidParameter(error));
        }

        // Perform lexical search for .rs files matching the query
        let search_input = SearchContentRequest {
            pattern: input.query.clone(),
            path: None,
            file_glob: Some("*.rs".to_string()),
            regex: Some(false), // Literal search
            case_insensitive: Some(false),
            max_results: Some(input.max_results as usize),
            context_lines: Some(2),
        };

        let search_result = self.search_content_until(search_input, bound)?;

        Ok(Retrieval {
            candidates: candidates_in_verification_order(search_result.matches),
            unread: search_result.unread,
            verify: input.verify,
        })
    }

    /// The verification of the retrieved candidates.
    ///
    /// The budget covers the whole call: the bound starts when the call
    /// begins, so the walk that retrieved the candidates spends it too.
    /// What the budget did not reach comes out of the answer with the
    /// reason that names the bound that was reached.
    pub(crate) async fn verify_retrieval(
        &self,
        retrieval: Retrieval,
        budget: Duration,
        bound: &Bound,
    ) -> RetrieveAndVerifyResult {
        let remaining = || bound.remaining();
        let results = if retrieval.verify {
            verify_candidates(
                self.code_verifier.as_ref(),
                &retrieval.candidates,
                budget,
                &remaining,
            )
            .await
        } else {
            // The verification was not asked for: every match is in the
            // answer, and every one of them says why it was not verified.
            skipped_candidates(&retrieval.candidates, "verify-disabled")
        };

        let mut verified_count = 0u32;
        let mut rejected_count = 0u32;
        let mut skipped_count = 0u32;
        for result in &results {
            match result.status {
                VerificationStatus::Verified => verified_count += 1,
                VerificationStatus::Rejected => rejected_count += 1,
                VerificationStatus::Skipped => skipped_count += 1,
            }
        }

        let total = results.len() as u32;

        RetrieveAndVerifyResult {
            results,
            total,
            verified_count,
            rejected_count,
            skipped_count,
            unread: retrieval.unread,
        }
    }
}
