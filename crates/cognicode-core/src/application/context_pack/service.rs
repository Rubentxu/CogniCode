//! The context pack use case.
//!
//! [`ContextPackService`] is the one operation that turns a request into a
//! pack, for every surface. It orchestrates what already exists —the frozen
//! snapshot, the ranker, the declaration reader, the token measure— and owns
//! only the decisions that are the pack's: what is delivered, what does not
//! fit, what is refused, and where the next page starts.
//!
//! ## What the pack promises
//!
//! - **Identity.** Every item is keyed by the id of one symbol. A bare name
//!   that several symbols declare is reported as an ambiguity, never resolved.
//! - **Evidence.** Every item carries the source range, the digest of the
//!   file it was read from, the reason it was selected and the snapshot.
//! - **One revision.** The snapshot is frozen. A file whose bytes no longer
//!   hash to the digest the snapshot recorded is refused, so a pack never
//!   mixes revisions; the refusal is listed among the omissions.
//! - **A bound that holds.** The budget is measured on the exact text the
//!   pack publishes, with the measure the pack names. The text never exceeds
//!   the limit under that measure, and the measure says how certain it is.
//! - **Partial is not semantic.** The pack says what it delivered and what it
//!   left out. It never says that what it delivered is everything relevant:
//!   the selection is by identity, name weight and direct relation, and
//!   semantic coverage is reported as not assessed.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use super::adaptive::{Adaptive, AdaptiveReport, PoolItem, gather};
use super::detail::{Representation, represent};
use super::expansion::ExpansionStatus;
use super::measure::{MeasureKind, Measured, TokenMeasure};
use super::policy::{Detail, Role, SelectionPolicy};
use super::ports::{DeclarationReader, Relevance, SnapshotView, SourceReader};
use super::render::{RenderView, item_text, render_with_notes};
use super::rerank::{Entry, Signals, rerank};
use super::selection::{Candidate, ContextSelection, PlannedItem, SelectionRequest, select};
use crate::domain::aggregates::call_graph::SymbolId;
use crate::domain::findings::digest::sha256_hex;
use crate::domain::value_objects::published_line;

/// The most omissions a pack lists by name; the rest are counted.
pub const OMISSIONS_LISTED: usize = 20;

/// How many characters of a declaration header a pack keeps.
const HEADER_LIMIT: usize = 240;

/// How many digest characters a continuation token binds to.
const TOKEN_DIGEST_PREFIX: usize = 16;

/// How many of the best candidates of an adaptive pool are costed and
/// re-ordered by value per token; the rest keep their value order.
const RERANKED_HEAD: usize = 60;

/// What a pack is asked for.
#[derive(Debug, Clone)]
pub struct PackRequest {
    /// Which candidates to select.
    pub selection: SelectionRequest,
    /// The most tokens the rendered pack may hold, under the pack's measure.
    pub max_tokens: u64,
    /// The token of the page to build; `None` builds the first.
    pub continuation: Option<String>,
    /// The policy that chooses and orders the candidates by value per token
    /// and decides how much of each symbol is shown; `None` keeps the plain
    /// selection, every candidate a signature.
    pub adaptive: Option<Adaptive>,
}

/// Why a pack could not be built at all.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ContextPackError {
    /// The continuation token is not one this product issues.
    InvalidContinuation,
    /// The continuation token was issued for another snapshot.
    StaleContinuation,
}

impl std::fmt::Display for ContextPackError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ContextPackError::InvalidContinuation => {
                write!(f, "the continuation token is not valid")
            }
            ContextPackError::StaleContinuation => write!(
                f,
                "the continuation token was issued for another snapshot; \
                 run the request again without it"
            ),
        }
    }
}

impl std::error::Error for ContextPackError {}

/// A line range of a source file, counted from one, both ends included.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct ItemRange {
    /// The first line.
    pub start_line: u32,
    /// The last line.
    pub end_line: u32,
}

/// One symbol of a pack, with the evidence that backs it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct PackItem {
    /// The identity of the symbol: unique in the snapshot.
    pub id: String,
    /// The declared name.
    pub name: String,
    /// The kind, in the domain's word.
    pub kind: String,
    /// The file that declares it, as the graph records it.
    pub file: String,
    /// The lines of the declaration.
    pub range: ItemRange,
    /// Where the range comes from: `declaration` when a parser read the
    /// declaration, `line` when only the line of the symbol is known.
    pub range_basis: String,
    /// The declaration up to where its body starts.
    pub signature: String,
    /// The digest of the file the item was read from: `sha256:<hex>`.
    pub source_digest: String,
    /// Why the symbol was selected.
    pub reason: String,
    /// What the reason refers to, when it refers to something.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason_detail: Option<String>,
    /// The score of the source that proposed it.
    pub score: f64,
    /// What of the symbol the pack shows beyond its signature: present when
    /// the selection is adaptive.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub representation: Option<Representation>,
    /// Why the item is where it is: present when the selection is adaptive.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub selection: Option<ItemSelection>,
}

/// Why an item of an adaptive pack is where it is.
///
/// The signals are normalised over the pool of the request and the value is
/// their weighted mean: a ranking value for this request, not a probability
/// and not a confidence that the item is relevant.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct ItemSelection {
    /// The part it plays around the symbols the request is about.
    pub role: Role,
    /// The signals the value is made of.
    pub signals: Signals,
    /// The weighted mean of the signals under the policy.
    pub value: f64,
    /// How much it repeats what was chosen before it, from zero to one.
    pub redundancy: f64,
    /// The incremental value per token it was ordered by; absent for a symbol
    /// the caller named, which goes first whatever it costs.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub priority: Option<f64>,
}

/// A candidate the pack did not deliver.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct Omission {
    /// The identity of the symbol.
    pub id: String,
    /// Why: `budget`, `source_changed`, `unreadable` or `not_in_snapshot`.
    pub reason: String,
}

/// What the budget was, what the text cost and how sure that is.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct BudgetReport {
    /// The limit that was asked.
    pub limit: u64,
    /// What the text costs, and how certain that count is.
    pub used: Measured,
    /// What the count covers: `rendered_text`, the text a person reads, and
    /// nothing outside it. The structured payload that carries the same items
    /// with their evidence fields is not measured.
    pub scope: String,
    /// Whether the count proves that the text fits the limit: true for an
    /// exact count and for an upper bound under the limit.
    pub verified: bool,
}

/// A bare name that several symbols declare.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct AmbiguousAnchor {
    /// The name the caller wrote.
    pub query: String,
    /// Every symbol that declares it.
    pub candidates: Vec<String>,
}

/// The pack.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct ContextPack {
    /// The symbols delivered in this page, in order.
    pub items: Vec<PackItem>,
    /// The candidates not delivered, up to [`OMISSIONS_LISTED`] by name.
    pub omissions: Vec<Omission>,
    /// How many candidates this page did not deliver, listed or not:
    /// [`ContextPack::remaining`] plus [`ContextPack::refused`].
    pub omitted: usize,
    /// How many candidates are left for a next page.
    pub remaining: usize,
    /// How many candidates were refused: their source differs from the
    /// snapshot, is unreadable or is not part of it.
    pub refused: usize,
    /// How many candidates the selection holds, over all pages.
    pub candidates: usize,
    /// The index of the first candidate of this page.
    pub offset: usize,
    /// The token that builds the next page, when there is one.
    pub continuation_token: Option<String>,
    /// The bare names that more than one symbol declares.
    pub ambiguities: Vec<AmbiguousAnchor>,
    /// The anchors that name no symbol of the snapshot.
    pub unresolved: Vec<String>,
    /// The budget and the cost of the text.
    pub budget: BudgetReport,
    /// How the selection was made: the policy, how the walk of the graph
    /// went and what could not be used. Present when the selection is
    /// adaptive.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub adaptive: Option<AdaptiveReport>,
    /// The text of the pack: what the budget was measured on.
    pub rendered: String,
}

/// The one operation that builds a pack.
pub struct ContextPackService<'a> {
    /// The frozen snapshot every read is checked against.
    pub snapshot: &'a SnapshotView,
    /// The ranker of the intent.
    pub relevance: &'a dyn Relevance,
    /// The reader of source files.
    pub sources: &'a dyn SourceReader,
    /// The reader of declarations.
    pub declarations: &'a dyn DeclarationReader,
    /// The measure of the budget.
    pub measure: &'a dyn TokenMeasure,
    /// What else a continuation token is bound to besides the snapshot:
    /// empty for a pack that depends on the snapshot alone, the digest of
    /// the change for a pack about one. A token issued under one scope is
    /// refused under any other.
    pub token_scope: &'a str,
}

impl ContextPackService<'_> {
    /// Selects the candidates of `request` over the snapshot.
    pub fn selection(&self, request: &SelectionRequest) -> ContextSelection {
        select(&self.snapshot.graph, self.relevance, request)
    }

    /// Builds the pack of `request`.
    pub fn build(&self, request: &PackRequest) -> Result<ContextPack, ContextPackError> {
        let offset = self.offset_of(request.continuation.as_deref())?;
        let selection = match &request.adaptive {
            Some(adaptive) => self.adaptive_selection(&request.selection, adaptive),
            None => self.selection(&request.selection),
        };
        Ok(self.pack_of(&selection, offset, request.max_tokens))
    }

    /// The selection of `request` under the policy of `adaptive`.
    ///
    /// The pool is gathered from the signals the policy weighs, the best of
    /// it is costed at the representation the policy plans for each role,
    /// and the order is the greedy value per token without repeats. The
    /// rest of the pool follows in value order.
    pub fn adaptive_selection(
        &self,
        request: &SelectionRequest,
        adaptive: &Adaptive,
    ) -> ContextSelection {
        let intent = request
            .intent
            .as_deref()
            .map(str::trim)
            .filter(|text| !text.is_empty());
        let pool = gather(
            &self.snapshot.graph,
            self.relevance,
            &request.anchors,
            intent,
            adaptive,
        );
        let mut effective = adaptive.policy.clone();
        effective.weights = pool.weights;

        let head = pool.items.len().min(RERANKED_HEAD);
        let mut files = HashMap::new();
        let entries: Vec<Entry> = pool.items[..head]
            .iter()
            .map(|item| {
                let detail = effective.detail_for(item.role);
                let cost = self
                    .evidence(
                        &item.candidate,
                        &mut files,
                        Some(Shown {
                            planned: detail,
                            level: detail,
                            focus: &pool.focus,
                            trace: None,
                        }),
                    )
                    .map_or(0, |shown| self.measure.measure(&item_text(&shown)).tokens);
                Entry {
                    id: item.candidate.id.clone(),
                    signals: item.signals,
                    role: item.role,
                    container: item.container.clone(),
                    terms: item.terms.clone(),
                    cost,
                    pinned: item.pinned,
                }
            })
            .collect();
        let by_id: HashMap<&SymbolId, &PoolItem> = pool
            .items
            .iter()
            .map(|item| (&item.candidate.id, item))
            .collect();

        let mut selection = ContextSelection {
            ambiguities: pool.ambiguities.clone(),
            unresolved: pool.unresolved.clone(),
            ..ContextSelection::default()
        };
        selection.plan.focus = pool.focus.clone();
        for placed in rerank(entries, &effective) {
            let item = by_id[&placed.id];
            let trace = ItemSelection {
                role: item.role,
                signals: item.signals,
                value: placed.value,
                redundancy: placed.redundancy,
                priority: placed.priority.is_finite().then_some(placed.priority),
            };
            push_planned(&mut selection, item, &effective, trace);
        }
        for item in &pool.items[head..] {
            let trace = ItemSelection {
                role: item.role,
                signals: item.signals,
                value: item.candidate.score,
                redundancy: 0.0,
                priority: None,
            };
            push_planned(&mut selection, item, &effective, trace);
        }
        selection.adaptive = Some(AdaptiveReport {
            policy: adaptive.policy.clone(),
            weights_applied: pool.weights,
            degraded_to: (pool.expansion.status == ExpansionStatus::Unknown)
                .then(|| "lexical".to_string()),
            expansion: pool.expansion,
            gaps: pool.gaps,
            pool: pool.items.len(),
            reranked: head,
            score_semantics: SCORE_SEMANTICS.to_string(),
        });
        selection
    }

    /// Builds the pack of a selection that is already made.
    pub fn pack_of(
        &self,
        selection: &ContextSelection,
        offset: usize,
        max_tokens: u64,
    ) -> ContextPack {
        self.pack_with_notes(selection, offset, max_tokens, &[])
    }

    /// Builds the pack of a selection that is already made, with `notes`
    /// printed between the header and the items. The notes are part of the
    /// text the budget measures.
    pub fn pack_with_notes(
        &self,
        selection: &ContextSelection,
        offset: usize,
        max_tokens: u64,
        notes: &[String],
    ) -> ContextPack {
        let all_notes: Vec<String> = selection
            .adaptive
            .iter()
            .flat_map(AdaptiveReport::notes)
            .chain(notes.iter().cloned())
            .collect();
        let notes = all_notes.as_slice();
        let total = selection.candidates.len();
        let mut files: HashMap<String, Result<String, String>> = HashMap::new();
        let mut delivered: Vec<(usize, PackItem)> = Vec::new();
        let mut refused: Vec<(usize, Omission)> = Vec::new();
        let mut consumed = offset.min(total);

        while consumed < total {
            let candidate = &selection.candidates[consumed];
            let planned = selection.plan.items.get(&candidate.id);
            let mut level = planned.map(|planned| planned.detail);
            let outcome = loop {
                let shown = planned.map(|planned| Shown {
                    planned: planned.detail,
                    level: level.unwrap_or(planned.detail),
                    focus: &selection.plan.focus,
                    trace: Some(&planned.trace),
                });
                match self.evidence(candidate, &mut files, shown) {
                    Err(omission) => break Err(omission),
                    Ok(item) => {
                        delivered.push((consumed, item));
                        let text =
                            self.text(&delivered, &refused, (offset, consumed + 1), total, notes);
                        if self.measure.measure(&text).tokens <= max_tokens {
                            break Ok(true);
                        }
                        delivered.pop();
                        match level.and_then(Detail::lower) {
                            Some(lower) => level = Some(lower),
                            None => break Ok(false),
                        }
                    }
                }
            };
            match outcome {
                Err(omission) => refused.push((consumed, omission)),
                Ok(true) => {}
                Ok(false) => break,
            }
            consumed += 1;
        }

        let mut text = self.text(&delivered, &refused, (offset, consumed), total, notes);
        while self.measure.measure(&text).tokens > max_tokens {
            match delivered.pop() {
                Some((index, _)) => {
                    consumed = index;
                    refused.retain(|(at, _)| *at < index);
                }
                None => {
                    consumed = offset.min(total);
                    refused.clear();
                    text = String::new();
                    break;
                }
            }
            text = self.text(&delivered, &refused, (offset, consumed), total, notes);
        }

        self.assemble(
            selection, offset, max_tokens, consumed, delivered, refused, text,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn assemble(
        &self,
        selection: &ContextSelection,
        offset: usize,
        max_tokens: u64,
        consumed: usize,
        delivered: Vec<(usize, PackItem)>,
        refused: Vec<(usize, Omission)>,
        rendered: String,
    ) -> ContextPack {
        let total = selection.candidates.len();
        let remaining = total - consumed.min(total);
        let mut omissions: Vec<Omission> = refused.into_iter().map(|(_, o)| o).collect();
        let refused_count = omissions.len();
        omissions.extend(
            selection.candidates[consumed.min(total)..]
                .iter()
                .map(|candidate| Omission {
                    id: candidate.id.to_string(),
                    reason: "budget".to_string(),
                }),
        );
        let omitted = refused_count + remaining;
        omissions.truncate(OMISSIONS_LISTED);
        let used = self.measure.measure(&rendered);
        let verified = used.kind.bounds_the_real_count() && used.tokens <= max_tokens;
        ContextPack {
            items: delivered.into_iter().map(|(_, item)| item).collect(),
            omissions,
            omitted,
            remaining,
            refused: refused_count,
            candidates: total,
            offset: offset.min(total),
            continuation_token: self.token_for(consumed, offset, total),
            ambiguities: selection
                .ambiguities
                .iter()
                .map(|ambiguity| AmbiguousAnchor {
                    query: ambiguity.query.clone(),
                    candidates: ambiguity
                        .candidates
                        .iter()
                        .map(SymbolId::to_string)
                        .collect(),
                })
                .collect(),
            unresolved: selection.unresolved.clone(),
            budget: BudgetReport {
                limit: max_tokens,
                used,
                scope: "rendered_text".to_string(),
                verified,
            },
            adaptive: selection.adaptive.clone(),
            rendered,
        }
    }

    /// The text of the pack for the state of the loop.
    fn text(
        &self,
        delivered: &[(usize, PackItem)],
        refused: &[(usize, Omission)],
        (offset, consumed): (usize, usize),
        total: usize,
        notes: &[String],
    ) -> String {
        let items: Vec<PackItem> = delivered.iter().map(|(_, item)| item.clone()).collect();
        let remaining = total - consumed.min(total);
        let token = self.token_for(consumed, offset, total);
        render_with_notes(
            &RenderView {
                snapshot_digest: &self.snapshot.digest,
                items: &items,
                remaining,
                refused: refused.len(),
                continuation: token.as_deref(),
            },
            notes,
        )
    }

    /// The token of the page that starts at `next`, when there is one.
    ///
    /// The token holds the position and the prefix of the digest of the
    /// snapshot, so a page can only be built over the snapshot it was
    /// issued for.
    ///
    /// A page that made no progress —not even one candidate fits the
    /// budget— has no token: following it would ask for the same page
    /// again, forever. The budget is what has to change.
    fn token_for(&self, next: usize, offset: usize, total: usize) -> Option<String> {
        (next < total && next > offset).then(|| format!("v1.{}.{}", self.binding(), next))
    }

    /// The prefix of the digest a continuation token is bound to: the
    /// snapshot, and the token scope when the pack has one.
    fn binding(&self) -> String {
        let bound = if self.token_scope.is_empty() {
            self.snapshot.digest.clone()
        } else {
            sha256_hex(format!("{}\n{}", self.snapshot.digest, self.token_scope).as_bytes())
        };
        bound.chars().take(TOKEN_DIGEST_PREFIX).collect()
    }

    /// The position a continuation token asks for, or why it is refused.
    ///
    /// A token is bound to the digest of the snapshot it was issued over.
    pub fn offset_of(&self, token: Option<&str>) -> Result<usize, ContextPackError> {
        let Some(token) = token else { return Ok(0) };
        let mut parts = token.splitn(3, '.');
        let (Some("v1"), Some(digest), Some(position)) = (parts.next(), parts.next(), parts.next())
        else {
            return Err(ContextPackError::InvalidContinuation);
        };
        let position: usize = position
            .parse()
            .map_err(|_| ContextPackError::InvalidContinuation)?;
        if digest != self.binding() {
            return Err(ContextPackError::StaleContinuation);
        }
        Ok(position)
    }

    /// The evidence of a candidate, or the reason it is refused.
    ///
    /// The bytes of the file are read once per pack and checked against the
    /// digest the snapshot recorded: the range and the signature are
    /// computed from the very bytes that were checked.
    fn evidence(
        &self,
        candidate: &Candidate,
        files: &mut HashMap<String, Result<String, String>>,
        shown: Option<Shown<'_>>,
    ) -> Result<PackItem, Omission> {
        let refuse = |reason: &str| Omission {
            id: candidate.id.to_string(),
            reason: reason.to_string(),
        };
        let symbol = self
            .snapshot
            .graph
            .get_symbol(&candidate.id)
            .ok_or_else(|| refuse("not_in_snapshot"))?;
        let relative = self.snapshot.relative(symbol.location().file());
        let recorded = self
            .snapshot
            .files
            .get(&relative)
            .ok_or_else(|| refuse("not_in_snapshot"))?;
        let source = files
            .entry(relative.clone())
            .or_insert_with(|| self.read_verified(&relative, recorded))
            .as_ref()
            .map_err(|reason| refuse(reason))?;

        let line = symbol.location().line();
        let span =
            self.declarations
                .declaration(&relative, source, line, symbol.location().column());
        let representation = shown.as_ref().map(|shown| {
            span.as_ref()
                .map_or_else(Representation::signature, |span| {
                    represent(source, span, shown.level, shown.focus)
                })
                .degraded_from(shown.planned)
        });
        let (range, range_basis, header) = match span {
            Some(span) => (
                ItemRange {
                    start_line: span.start_line,
                    end_line: span.end_line,
                },
                "declaration",
                span.header,
            ),
            None => {
                let published = published_line(line);
                let text = source
                    .lines()
                    .nth(line as usize)
                    .unwrap_or_default()
                    .to_string();
                (
                    ItemRange {
                        start_line: published,
                        end_line: published,
                    },
                    "line",
                    text,
                )
            }
        };
        Ok(PackItem {
            id: candidate.id.to_string(),
            name: symbol.name().to_string(),
            kind: symbol.kind().to_string(),
            file: relative,
            range,
            range_basis: range_basis.to_string(),
            signature: one_line(&header),
            source_digest: format!("sha256:{recorded}"),
            reason: candidate.reason.code().to_string(),
            reason_detail: candidate.reason.detail(),
            score: candidate.score,
            representation,
            selection: shown.and_then(|shown| shown.trace.cloned()),
        })
    }

    /// The text of a file, when its bytes are the ones the snapshot saw.
    fn read_verified(&self, relative: &str, recorded: &str) -> Result<String, String> {
        let bytes = self
            .sources
            .read(relative)
            .map_err(|_| "unreadable".to_string())?;
        if sha256_hex(&bytes) != recorded {
            return Err("source_changed".to_string());
        }
        String::from_utf8(bytes).map_err(|_| "unreadable".to_string())
    }
}

/// What of a candidate an adaptive pack is asked to show.
struct Shown<'a> {
    /// The representation the policy planned for its role.
    planned: Detail,
    /// The representation to cut now: the planned one, or a smaller one when
    /// the planned one did not fit.
    level: Detail,
    /// The words the request is about.
    focus: &'a [String],
    /// Why the item is where it is; absent while it is only being costed.
    trace: Option<&'a ItemSelection>,
}

/// What a score means in an adaptive pack.
const SCORE_SEMANTICS: &str = "a ranking value for this request: the weighted mean of signals \
    normalised over its candidates; it is not a probability, not a confidence that the item is \
    relevant, and not comparable across requests";

/// Puts `item` in the selection with the way the policy shows it.
fn push_planned(
    selection: &mut ContextSelection,
    item: &PoolItem,
    policy: &SelectionPolicy,
    trace: ItemSelection,
) {
    let mut candidate = item.candidate.clone();
    candidate.score = trace.value;
    selection.plan.items.insert(
        candidate.id.clone(),
        PlannedItem {
            detail: policy.detail_for(item.role),
            trace,
        },
    );
    selection.candidates.push(candidate);
}

/// A header on one line, bounded.
fn one_line(header: &str) -> String {
    let joined = header.split_whitespace().collect::<Vec<_>>().join(" ");
    if joined.chars().count() <= HEADER_LIMIT {
        return joined;
    }
    let mut cut: String = joined.chars().take(HEADER_LIMIT).collect();
    cut.push('…');
    cut
}

/// What a measure kind is called in the pack.
pub fn measure_label(kind: &MeasureKind) -> String {
    match kind {
        MeasureKind::Exact { tokenizer } => format!("exact:{tokenizer}"),
        MeasureKind::UpperBound { method } => format!("upper_bound:{method}"),
        MeasureKind::Unknown => "unknown".to_string(),
    }
}
