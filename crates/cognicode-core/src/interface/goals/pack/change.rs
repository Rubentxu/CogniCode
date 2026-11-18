//! The pack of a change: what to read around the exact lines a change
//! touches.
//!
//! The pack is a reading aid for a review or an edit. Its seed is the set of
//! lines a change scope names; the consumers, contracts and tests it
//! selects come from the relations the graph already holds, each with the
//! edge that justifies it. It does not compute what the change affects, it
//! does not decide whether the change may land and it suggests no mutation or
//! verification: those belong to the impact kernel, the verifier and the
//! gate.
//!
//! What the change cannot say is said next to it. A file that could not be
//! tied to the snapshot, a hunk outside every symbol, a reference the graph
//! could not resolve and a changed symbol no test reaches are listed as
//! gaps, and a pack with gaps is never reported as complete.

use std::collections::BTreeMap;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::application::context_pack::change::input::{
    ChangeInput, ChangeScope, ChangeSource, MAX_CHANGED_FILES,
};
use crate::application::context_pack::change::seed::{ChangedFile, derive};
use crate::application::context_pack::change::select::{
    ChangedSymbol, Gap, PendingRef, select_change,
};
use crate::application::context_pack::{ContextPack, ContextPackError, ContextPackService};
use crate::infrastructure::git::GitChangeSource;
use crate::interface::coverage::{PendingReason, PendingReferenceInfo};

use super::PackInput;

/// How many changed files the answer lists; the total is always exact.
const FILES_LISTED: usize = 100;

/// How many gaps the answer lists; the total is always exact.
const GAPS_LISTED: usize = 50;

/// How many digest characters of a revision the notes show.
const REVISION_PREFIX: usize = 12;

/// What the pack says about the change it is about.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, schemars::JsonSchema)]
pub struct ChangeReport {
    /// The scope that was read: `staged`, `working_tree`, `commit`, `range`
    /// or `merge_candidate`.
    pub scope: String,
    /// Where the change starts: a revision, or the empty tree of an unborn
    /// branch.
    pub base: String,
    /// Where the change ends: a revision, `index` or `working_tree`.
    pub head: String,
    /// The digest of the change, bytes of both sides included. A
    /// continuation token is bound to it.
    pub digest: String,
    /// The changed files, in path order, up to a bound.
    pub files: Vec<ChangedFile>,
    /// How many files the change holds in the files read.
    pub files_total: usize,
    /// How many changed files were past the bound and were not read.
    pub files_not_read: usize,
    /// The changed symbols and how many consumers, contracts and tests were
    /// selected around each.
    pub symbols: Vec<ChangedSymbol>,
    /// What the selection cannot say, up to a bound.
    pub gaps: Vec<Gap>,
    /// How many gaps there are, listed or not.
    pub gaps_total: usize,
    /// The lines the text of the pack carries about the change.
    pub notes: Vec<String>,
    /// What this pack is and is not.
    pub advisory: String,
}

/// What the goal needs of a change pack besides the page itself.
pub(super) struct ChangePack {
    /// The page of the pack.
    pub page: ContextPack,
    /// What the pack says about the change.
    pub report: ChangeReport,
}

/// The scope a request names, or why it names none.
///
/// A change pack takes its seed from the change, so it takes no symbols and
/// no intent; references that no scope uses are refused instead of ignored.
pub(super) fn scope_of(input: &PackInput) -> Result<Option<ChangeScope>, String> {
    let Some(scope) = input.change_scope.as_deref() else {
        if input.base.is_some() || input.head.is_some() {
            return Err("`base` and `head` name a change: pass `change_scope` too.".to_string());
        }
        return Ok(None);
    };
    if !input.symbols.is_empty() || input.intent.is_some() {
        return Err(
            "a pack of a change takes its seed from the change: do not pass `symbols` or \
             `intent` with `change_scope`."
                .to_string(),
        );
    }
    ChangeScope::parse(scope, input.base.as_deref(), input.head.as_deref()).map(Some)
}

/// Reads the change of `scope` from the repository at `workspace`.
pub(super) fn read_change(workspace: &Path, scope: &ChangeScope) -> Result<ChangeInput, String> {
    GitChangeSource::new(workspace)?.read(scope)
}

/// The references of the graph that could not be tied to a declaration.
pub(super) fn pending_of(pending: &[PendingReferenceInfo]) -> Vec<PendingRef> {
    pending
        .iter()
        .map(|reference| PendingRef {
            file: reference.file.clone(),
            line: reference.line,
            target: reference.target.clone(),
            ambiguous: reference.reason == PendingReason::Ambiguous,
        })
        .collect()
}

/// Builds the page of the change over the frozen snapshot.
pub(super) fn build(
    service: &ContextPackService<'_>,
    scope: &ChangeScope,
    change: &ChangeInput,
    pending: &[PendingRef],
    continuation: Option<&str>,
    max_tokens: u64,
) -> Result<ChangePack, ContextPackError> {
    let offset = service.offset_of(continuation)?;
    let seed = derive(change, service.snapshot, service.declarations);
    let selected = select_change(&service.snapshot.graph, &seed, pending);
    let notes = notes_of(
        scope,
        change,
        &seed.files,
        selected.symbols.len(),
        &selected.gaps,
    );
    let page = service.pack_with_notes(&selected.selection, offset, max_tokens, &notes);
    let files_total = seed.files.len();
    let gaps_total = selected.gaps.len();
    Ok(ChangePack {
        page,
        report: ChangeReport {
            scope: scope.label().to_string(),
            base: change.base.clone(),
            head: change.head.clone(),
            digest: change.digest(),
            files: seed.files.into_iter().take(FILES_LISTED).collect(),
            files_total,
            files_not_read: change.files_not_read,
            symbols: selected.symbols,
            gaps: selected.gaps.into_iter().take(GAPS_LISTED).collect(),
            gaps_total,
            notes,
            advisory: "a reading selection around the changed lines, each item with the \
                       relation that justifies it; it is not an impact analysis, a \
                       verification or a gate decision, and what it lists under gaps is \
                       not known"
                .to_string(),
        },
    })
}

impl ChangeReport {
    /// Whether anything of the change is not known: a gap, or files that
    /// were not read.
    pub(super) fn is_partial(&self) -> bool {
        self.gaps_total > 0 || self.files_not_read > 0
    }
}

fn short(revision: &str) -> String {
    revision.chars().take(REVISION_PREFIX).collect()
}

/// The lines the text of the pack carries about the change.
fn notes_of(
    scope: &ChangeScope,
    change: &ChangeInput,
    files: &[ChangedFile],
    symbols: usize,
    gaps: &[Gap],
) -> Vec<String> {
    let mut notes = vec![format!(
        "change {} {}..{}: {} file(s), {} changed symbol(s)",
        scope.label(),
        short(&change.base),
        short(&change.head),
        files.len(),
        symbols
    )];
    if change.files_not_read > 0 {
        notes.push(format!(
            "not read: {} file(s) past the {MAX_CHANGED_FILES} file bound",
            change.files_not_read
        ));
    }
    if !gaps.is_empty() {
        let mut by_kind: BTreeMap<&str, usize> = BTreeMap::new();
        for gap in gaps {
            *by_kind.entry(gap.kind.as_str()).or_default() += 1;
        }
        let listed: Vec<String> = by_kind
            .into_iter()
            .map(|(kind, count)| format!("{kind}={count}"))
            .collect();
        notes.push(format!("gaps (not known): {}", listed.join(", ")));
    }
    notes.push(
        "advisory: a reading selection, not an impact analysis or a gate decision".to_string(),
    );
    notes
}
