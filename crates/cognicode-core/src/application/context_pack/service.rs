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

use super::measure::{MeasureKind, Measured, TokenMeasure};
use super::ports::{DeclarationReader, Relevance, SnapshotView, SourceReader};
use super::render::{RenderView, render};
use super::selection::{Candidate, ContextSelection, SelectionRequest, select};
use crate::domain::aggregates::call_graph::SymbolId;
use crate::domain::findings::digest::sha256_hex;
use crate::domain::value_objects::published_line;

/// The most omissions a pack lists by name; the rest are counted.
pub const OMISSIONS_LISTED: usize = 20;

/// How many characters of a declaration header a pack keeps.
const HEADER_LIMIT: usize = 240;

/// How many digest characters a continuation token binds to.
const TOKEN_DIGEST_PREFIX: usize = 16;

/// What a pack is asked for.
#[derive(Debug, Clone)]
pub struct PackRequest {
    /// Which candidates to select.
    pub selection: SelectionRequest,
    /// The most tokens the rendered pack may hold, under the pack's measure.
    pub max_tokens: u64,
    /// The token of the page to build; `None` builds the first.
    pub continuation: Option<String>,
}

/// Why a pack could not be built at all.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PackError {
    /// The continuation token is not one this product issues.
    InvalidContinuation,
    /// The continuation token was issued for another snapshot.
    StaleContinuation,
}

impl std::fmt::Display for PackError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PackError::InvalidContinuation => {
                write!(f, "the continuation token is not valid")
            }
            PackError::StaleContinuation => write!(
                f,
                "the continuation token was issued for another snapshot; \
                 run the request again without it"
            ),
        }
    }
}

impl std::error::Error for PackError {}

/// A line range of a source file, counted from one, both ends included.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct SourceRange {
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
    pub range: SourceRange,
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
    /// What the count covers: the rendered text of the pack, and nothing
    /// outside it.
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
    /// How many candidates this page did not deliver, listed or not.
    pub omitted: usize,
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
}

impl ContextPackService<'_> {
    /// Selects the candidates of `request` over the snapshot.
    pub fn selection(&self, request: &SelectionRequest) -> ContextSelection {
        select(&self.snapshot.graph, self.relevance, request)
    }

    /// Builds the pack of `request`.
    pub fn build(&self, request: &PackRequest) -> Result<ContextPack, PackError> {
        let offset = self.offset_of(request.continuation.as_deref())?;
        let selection = self.selection(&request.selection);
        Ok(self.pack_of(&selection, offset, request.max_tokens))
    }

    /// Builds the pack of a selection that is already made.
    pub fn pack_of(
        &self,
        selection: &ContextSelection,
        offset: usize,
        max_tokens: u64,
    ) -> ContextPack {
        let total = selection.candidates.len();
        let mut files: HashMap<String, Result<String, String>> = HashMap::new();
        let mut delivered: Vec<(usize, PackItem)> = Vec::new();
        let mut refused: Vec<(usize, Omission)> = Vec::new();
        let mut consumed = offset.min(total);

        while consumed < total {
            let candidate = &selection.candidates[consumed];
            match self.evidence(candidate, &mut files) {
                Err(omission) => refused.push((consumed, omission)),
                Ok(item) => {
                    delivered.push((consumed, item));
                    let text = self.text(&delivered, &refused, consumed + 1, total);
                    if self.measure.measure(&text).tokens > max_tokens {
                        delivered.pop();
                        break;
                    }
                }
            }
            consumed += 1;
        }

        let mut text = self.text(&delivered, &refused, consumed, total);
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
            text = self.text(&delivered, &refused, consumed, total);
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
            candidates: total,
            offset: offset.min(total),
            continuation_token: self.token_for(consumed, total),
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
                scope: "rendered".to_string(),
                verified,
            },
            rendered,
        }
    }

    /// The text of the pack for the state of the loop.
    fn text(
        &self,
        delivered: &[(usize, PackItem)],
        refused: &[(usize, Omission)],
        consumed: usize,
        total: usize,
    ) -> String {
        let items: Vec<PackItem> = delivered.iter().map(|(_, item)| item.clone()).collect();
        let remaining = total - consumed.min(total);
        let token = self.token_for(consumed, total);
        render(&RenderView {
            snapshot_digest: &self.snapshot.digest,
            items: &items,
            remaining,
            refused: refused.len(),
            continuation: token.as_deref(),
        })
    }

    /// The token of the page that starts at `next`, when there is one.
    ///
    /// The token holds the position and the prefix of the digest of the
    /// snapshot, so a page can only be built over the snapshot it was
    /// issued for.
    fn token_for(&self, next: usize, total: usize) -> Option<String> {
        (next < total).then(|| {
            format!(
                "v1.{}.{}",
                self.snapshot
                    .digest
                    .chars()
                    .take(TOKEN_DIGEST_PREFIX)
                    .collect::<String>(),
                next
            )
        })
    }

    fn offset_of(&self, token: Option<&str>) -> Result<usize, PackError> {
        let Some(token) = token else { return Ok(0) };
        let mut parts = token.splitn(3, '.');
        let (Some("v1"), Some(digest), Some(position)) = (parts.next(), parts.next(), parts.next())
        else {
            return Err(PackError::InvalidContinuation);
        };
        let position: usize = position
            .parse()
            .map_err(|_| PackError::InvalidContinuation)?;
        let current: String = self
            .snapshot
            .digest
            .chars()
            .take(TOKEN_DIGEST_PREFIX)
            .collect();
        if digest != current {
            return Err(PackError::StaleContinuation);
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
        let (range, range_basis, header) = match span {
            Some(span) => (
                SourceRange {
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
                    SourceRange {
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
