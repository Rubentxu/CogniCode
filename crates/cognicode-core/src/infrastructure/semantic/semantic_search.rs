//! Semantic Search - Fast symbol search with filtering
//!
//! This module provides name search over symbols — exact, prefix,
//! substring and fuzzy — with kind filtering, for finding symbols across the
//! codebase.
// e30.1 clippy baseline reset: pre-existing lint debt (see fix/e30.1-clippy-baseline-reset)
#![allow(clippy::len_zero)]

use super::name_index::{NameTerms, classify, signature};
use crate::application::ingest::types::SemanticOutcome;
use crate::domain::aggregates::symbol::Symbol;
use crate::domain::evidence_kernel::SymbolFqn;
use crate::domain::value_objects::{Location, SymbolKind, SymbolSearchQuery};
use crate::infrastructure::graph::per_file_graph::{SkipReason, SkippedFile};
use crate::infrastructure::graph::semantic_layer::semantic_facts_of;
use crate::infrastructure::graph::source_files;
use crate::infrastructure::parser::Language;
use crate::infrastructure::parser::language_config::LanguageConfig;
use parking_lot::RwLock;
use std::cmp::Ordering;
use std::collections::hash_map::Entry as MapEntry;
use std::collections::{BinaryHeap, HashMap};
use std::path::Path;
use std::sync::Arc;

/// How the error of a file that could not be read begins.
///
/// One constant, because the code that writes the error and the code that
/// tells a read failure from a parse failure have to agree on it.
const READ_FAILURE: &str = "Failed to read file";

/// A search result with relevance scoring
#[derive(Debug, Clone)]
pub struct SearchResult {
    /// The matching symbol
    pub symbol: Symbol,
    /// Relevance score (higher is better)
    pub score: f32,
    /// Type of match for display purposes
    pub match_type: MatchType,
}

impl SearchResult {
    /// Creates a new search result
    pub fn new(symbol: Symbol, score: f32, match_type: MatchType) -> Self {
        Self {
            symbol,
            score,
            match_type,
        }
    }
}

/// Type of match for ranking purposes
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum MatchType {
    /// Exact match (same as query)
    Exact = 3,
    /// Prefix match (query is prefix of name)
    Prefix = 2,
    /// Contains match (query is substring)
    Contains = 1,
    /// Fuzzy match (approximate)
    Fuzzy = 0,
}

impl PartialEq for SearchResult {
    fn eq(&self, other: &Self) -> bool {
        self.symbol == other.symbol
    }
}

impl Eq for SearchResult {}

impl PartialOrd for SearchResult {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for SearchResult {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        // First by match type (higher is better)
        let match_cmp = self.match_type.cmp(&other.match_type).reverse();
        if match_cmp != std::cmp::Ordering::Equal {
            return match_cmp;
        }
        // Then by score
        self.score
            .partial_cmp(&other.score)
            .unwrap_or(std::cmp::Ordering::Equal)
            .reverse()
    }
}

/// Symbol names by lowercase name, searchable without looking at every
/// symbol.
///
/// # What a search answers
///
/// A query is lowercased (`str::to_lowercase`, no normalization: see the
/// text semantics of the name rules) and each indexed symbol is ranked by
/// how its lowercase name relates to it: the same name, a name that starts
/// with it, a name that contains it, a name that has its characters in
/// order. The kind filter and the limit apply after that. Results come
/// best first in a total order — match type, score, lowercase name, name,
/// file, line — so the same index and the same query always answer the same
/// list, and a limit of `n` is the first `n` of the list without a limit.
/// A limit of zero answers nothing.
///
/// # How
///
/// The symbols are the source of truth. Beside them the index keeps
/// *derived* tables of names (the names and a signature of each) that let a search look at
/// the few names that can match instead of at every symbol: an exact name
/// is a lookup, a prefix is a range, and a substring or subsequence query
/// only looks at the names that have all its characters. Every candidate is
/// then judged by the same rules a full scan applies, so the tables can make
/// a search faster and cannot make it say something else.
///
/// A write replaces the symbols of a file as one step under a lock, so a
/// search sees a file as it was or as it is, never half of each. If a write
/// is interrupted, the derived tables are marked as not trustworthy: searches
/// answer by scanning the symbols (slower, same answer) until the next write
/// rebuilds the tables from them.
pub struct SearchIndex {
    state: RwLock<State>,
}

/// What the index holds. Everything but `entries` can be rebuilt from it.
struct State {
    /// One slot per symbol ever indexed; a forgotten symbol leaves a hole
    /// that the next insertion reuses.
    entries: Vec<Option<Entry>>,
    free_slots: Vec<u32>,
    /// The slots each file contributed, by the file's name: what has to go
    /// when the file is indexed again or forgotten.
    files: HashMap<Arc<str>, Vec<u32>>,
    terms: NameTerms,
    live: usize,
    /// `false` from the moment a write starts until it ends. A reader that
    /// finds it `false` knows a write never ended.
    derived_in_sync: bool,
}

struct Entry {
    name: Arc<str>,
    name_lower: Arc<str>,
    kind: SymbolKind,
    location: Location,
    /// The file the caller indexed the symbol under, which is not always the
    /// file the symbol says it is in.
    file: Arc<str>,
    term: u32,
}

/// A symbol as it is handed to the index, before it has a slot.
struct Declared {
    name: String,
    kind: SymbolKind,
    location: Location,
}

/// The identity an entry has in the index: `{file}:{name}:{line}`.
///
/// The grammar belongs to [`SymbolFqn`], the same one the graph writes for
/// its symbols, so the index and the graph cannot disagree about which
/// symbols are the same one. The file is the one the caller indexed the entry
/// under, because that is the name the entry is forgotten by: a reader that
/// stamped its symbols with a placeholder place must not make two files
/// collide.
fn entry_identity(file_path: &str, symbol: &Symbol) -> String {
    SymbolFqn::from_legacy_side(file_path, symbol.name(), symbol.location().line()).assemble()
}

/// The symbols of one file, one per identity.
///
/// A file that declares the same identity twice has one entry: the last one.
fn declared_in(file_path: &str, symbols: Vec<Symbol>) -> Vec<Declared> {
    let mut by_identity: HashMap<String, usize> = HashMap::with_capacity(symbols.len());
    let mut declared: Vec<Declared> = Vec::with_capacity(symbols.len());
    for symbol in symbols {
        let entry = Declared {
            name: symbol.name().to_string(),
            kind: *symbol.kind(),
            location: symbol.location().clone(),
        };
        match by_identity.entry(entry_identity(file_path, &symbol)) {
            MapEntry::Occupied(seen) => declared[*seen.get()] = entry,
            MapEntry::Vacant(free) => {
                free.insert(declared.len());
                declared.push(entry);
            }
        }
    }
    declared
}

/// A symbol that answers a query, with how well it does.
struct Hit<'a> {
    match_type: MatchType,
    score: f32,
    entry: &'a Entry,
}

impl Hit<'_> {
    /// The order results come in: `Less` is better.
    fn rank(&self, other: &Self) -> Ordering {
        other
            .match_type
            .cmp(&self.match_type)
            .then_with(|| other.score.total_cmp(&self.score))
            .then_with(|| self.entry.name_lower.cmp(&other.entry.name_lower))
            .then_with(|| self.entry.name.cmp(&other.entry.name))
            .then_with(|| self.entry.file.cmp(&other.entry.file))
            .then_with(|| self.entry.location.line().cmp(&other.entry.location.line()))
    }

    fn into_result(self) -> SearchResult {
        let symbol = Symbol::new(
            self.entry.name.to_string(),
            self.entry.kind,
            self.entry.location.clone(),
        );
        SearchResult::new(symbol, self.score, self.match_type)
    }
}

/// A [`Hit`] in a max-heap that keeps the best ones: the worst of them is
/// on top, ready to be replaced.
struct WorstOnTop<'a>(Hit<'a>);

impl PartialEq for WorstOnTop<'_> {
    fn eq(&self, other: &Self) -> bool {
        self.0.rank(&other.0) == Ordering::Equal
    }
}

impl Eq for WorstOnTop<'_> {}

impl PartialOrd for WorstOnTop<'_> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for WorstOnTop<'_> {
    fn cmp(&self, other: &Self) -> Ordering {
        self.0.rank(&other.0)
    }
}

/// What a search asks, once its text has been lowercased.
struct Question<'a> {
    lower: String,
    kinds: &'a [SymbolKind],
    limit: usize,
}

impl<'a> Question<'a> {
    /// `None` when there is nothing to look for: an empty query or a limit
    /// of zero.
    fn of(query: &'a SymbolSearchQuery) -> Option<Self> {
        if query.query.is_empty() || query.max_results == 0 {
            return None;
        }
        Some(Self {
            lower: query.query.to_lowercase(),
            kinds: &query.kinds,
            limit: query.max_results,
        })
    }

    fn allows(&self, kind: SymbolKind) -> bool {
        self.kinds.is_empty() || self.kinds.contains(&kind)
    }
}

impl State {
    fn new() -> Self {
        Self {
            entries: Vec::new(),
            free_slots: Vec::new(),
            files: HashMap::new(),
            terms: NameTerms::new(),
            live: 0,
            derived_in_sync: true,
        }
    }

    /// Replaces what the index holds for `file` with `declared`.
    fn replace_file(&mut self, file: &str, declared: Vec<Declared>) {
        self.repair_if_interrupted();
        self.derived_in_sync = false;
        self.remove_file(file);
        let file: Arc<str> = Arc::from(file);
        let mut slots = Vec::with_capacity(declared.len());
        for symbol in declared {
            slots.push(self.insert(&file, symbol));
        }
        if !slots.is_empty() {
            self.files.insert(file, slots);
        }
        self.derived_in_sync = true;
    }

    fn insert(&mut self, file: &Arc<str>, symbol: Declared) -> u32 {
        let slot = self.free_slots.pop().unwrap_or_else(|| {
            self.entries.push(None);
            (self.entries.len() - 1) as u32
        });
        let (term, name_lower) = self.terms.attach(&symbol.name.to_lowercase(), slot);
        let name = if *name_lower == *symbol.name {
            Arc::clone(&name_lower)
        } else {
            Arc::from(symbol.name)
        };
        self.entries[slot as usize] = Some(Entry {
            name,
            name_lower,
            kind: symbol.kind,
            location: symbol.location,
            file: Arc::clone(file),
            term,
        });
        self.live += 1;
        slot
    }

    fn remove_file(&mut self, file: &str) {
        let Some(slots) = self.files.remove(file) else {
            return;
        };
        for slot in slots {
            if let Some(entry) = self.entries[slot as usize].take() {
                self.terms.detach(entry.term, slot);
                self.free_slots.push(slot);
                self.live -= 1;
            }
        }
    }

    fn clear(&mut self) {
        *self = Self::new();
    }

    /// Rebuilds everything that is derived from the entries, if a write
    /// never finished.
    fn repair_if_interrupted(&mut self) {
        if self.derived_in_sync {
            return;
        }
        let entries = std::mem::take(&mut self.entries);
        *self = Self::new();
        self.derived_in_sync = false;
        for (slot, entry) in entries.into_iter().enumerate() {
            let Some(entry) = entry else {
                self.entries.push(None);
                self.free_slots.push(slot as u32);
                continue;
            };
            let slot = slot as u32;
            let (term, name_lower) = self.terms.attach(&entry.name_lower, slot);
            self.files
                .entry(Arc::clone(&entry.file))
                .or_default()
                .push(slot);
            self.entries.push(Some(Entry {
                term,
                name_lower,
                ..entry
            }));
            self.live += 1;
        }
        self.derived_in_sync = true;
    }

    fn search(&self, question: &Question<'_>) -> Vec<SearchResult> {
        if self.derived_in_sync {
            self.search_candidates(question)
        } else {
            self.search_scan(question)
        }
    }

    /// The reference: judge every symbol.
    fn search_scan(&self, question: &Question<'_>) -> Vec<SearchResult> {
        let mut best: BinaryHeap<WorstOnTop<'_>> = BinaryHeap::new();
        for entry in self.entries.iter().flatten() {
            if !question.allows(entry.kind) {
                continue;
            }
            let Some((match_type, score)) = classify(&entry.name_lower, &question.lower) else {
                continue;
            };
            let hit = Hit {
                match_type,
                score,
                entry,
            };
            if best.len() < question.limit {
                best.push(WorstOnTop(hit));
            } else if best
                .peek()
                .is_some_and(|worst| hit.rank(&worst.0) == Ordering::Less)
            {
                best.pop();
                best.push(WorstOnTop(hit));
            }
        }
        best.into_sorted_vec()
            .into_iter()
            .map(|hit| hit.0.into_result())
            .collect()
    }

    /// The same answer as [`search_scan`](Self::search_scan), looking only at
    /// the names that can match: the exact name, then the names that start
    /// with the query, and only if the limit is not reached, the names that
    /// have its characters.
    ///
    /// The tiers are the match types, which come first in the order of the
    /// results, so a tier that fills the limit leaves nothing for the next.
    fn search_candidates(&self, question: &Question<'_>) -> Vec<SearchResult> {
        let mut hits: Vec<Hit<'_>> = Vec::new();
        let query = question.lower.as_str();

        let exact = self.terms.exact(query);
        if let Some(term) = exact {
            self.take_term(&mut hits, term, MatchType::Exact, 1.0, question);
        }
        for term in self.terms.with_prefix(query) {
            if hits.len() >= question.limit {
                break;
            }
            if Some(term) != exact {
                self.take_term(&mut hits, term, MatchType::Prefix, 0.9, question);
            }
        }
        if hits.len() < question.limit {
            self.take_approximate(&mut hits, question);
        }
        hits.into_iter().map(Hit::into_result).collect()
    }

    /// The names that contain the query or have its characters in order,
    /// best first, until the limit is reached.
    fn take_approximate<'a>(&'a self, hits: &mut Vec<Hit<'a>>, question: &Question<'_>) {
        let query = question.lower.as_str();
        let mut matches: Vec<(MatchType, f32, u32)> = self
            .terms
            .with_signature(signature(query))
            .filter_map(|term| match classify(self.terms.name(term), query) {
                Some((kind @ (MatchType::Contains | MatchType::Fuzzy), score)) => {
                    Some((kind, score, term))
                }
                _ => None,
            })
            .collect();

        let order = |a: &(MatchType, f32, u32), b: &(MatchType, f32, u32)| {
            b.0.cmp(&a.0)
                .then_with(|| b.1.total_cmp(&a.1))
                .then_with(|| self.terms.name(a.2).cmp(self.terms.name(b.2)))
        };
        // Sort only as many names as could fill what is missing, a chunk at
        // a time: a name can have no symbol of the wanted kinds, so one chunk
        // may not be enough, and the chunks grow so that a rare kind does not
        // turn into a sort per name.
        let mut rest: &mut [(MatchType, f32, u32)] = matches.as_mut_slice();
        let mut chunk = question.limit.saturating_sub(hits.len()).max(MIN_CHUNK);
        while !rest.is_empty() && hits.len() < question.limit {
            let take = chunk.min(rest.len());
            if take < rest.len() {
                rest.select_nth_unstable_by(take, order);
            }
            rest[..take].sort_unstable_by(order);
            let (now, later) = std::mem::take(&mut rest).split_at_mut(take);
            for &(match_type, score, term) in now.iter() {
                if hits.len() >= question.limit {
                    break;
                }
                self.take_term(hits, term, match_type, score, question);
            }
            rest = later;
            chunk = chunk.saturating_mul(2);
        }
    }

    /// Adds the symbols of one name that the kind filter lets through, in
    /// result order, without going over the limit.
    fn take_term<'a>(
        &'a self,
        hits: &mut Vec<Hit<'a>>,
        term: u32,
        match_type: MatchType,
        score: f32,
        question: &Question<'_>,
    ) {
        let mut of_term: Vec<Hit<'a>> = self
            .terms
            .slots(term)
            .iter()
            .filter_map(|&slot| self.entries[slot as usize].as_ref())
            .filter(|entry| question.allows(entry.kind))
            .map(|entry| Hit {
                match_type,
                score,
                entry,
            })
            .collect();
        of_term.sort_unstable_by(Hit::rank);
        of_term.truncate(question.limit - hits.len());
        hits.extend(of_term);
    }
}

/// The least number of names a search sorts at once.
const MIN_CHUNK: usize = 32;

impl SearchIndex {
    /// Creates a new search index
    pub fn new() -> Self {
        Self {
            state: RwLock::new(State::new()),
        }
    }

    /// Indexes a file's symbols, replacing whatever the index held for it.
    ///
    /// A file that is indexed again declares what it declares now: a symbol
    /// it no longer declares (renamed, moved, deleted) must stop being
    /// searchable, so the previous entries go first. The swap is one step:
    /// a search that runs meanwhile sees the file as it was or as it is.
    pub fn index_file(&self, file_path: &str, symbols: Vec<Symbol>) {
        let declared = declared_in(file_path, symbols);
        self.state.write().replace_file(file_path, declared);
    }

    /// Forgets every entry indexed under `file_path`.
    pub fn clear_file(&self, file_path: &str) {
        self.state.write().replace_file(file_path, Vec::new());
    }

    /// Clears the entire index
    pub fn clear(&self) {
        self.state.write().clear();
    }

    /// Searches for symbols matching the query.
    ///
    /// Best first, at most `query.max_results`; an empty query or a limit
    /// of zero answers nothing. See the type's documentation for what
    /// matches and in which order.
    pub fn search(&self, query: &SymbolSearchQuery) -> Vec<SearchResult> {
        match Question::of(query) {
            Some(question) => self.state.read().search(&question),
            None => Vec::new(),
        }
    }

    /// The same answer as [`search`](Self::search), reached by judging every
    /// symbol instead of the candidates.
    ///
    /// It is what a search falls back to when the tables that find the
    /// candidates cannot be trusted, and the reference the accelerated
    /// search is compared against.
    pub fn search_by_scan(&self, query: &SymbolSearchQuery) -> Vec<SearchResult> {
        match Question::of(query) {
            Some(question) => self.state.read().search_scan(&question),
            None => Vec::new(),
        }
    }

    /// Returns the count of indexed symbols
    pub fn len(&self) -> usize {
        self.state.read().live
    }

    /// Returns true if the index is empty
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

impl Default for SearchIndex {
    fn default() -> Self {
        Self::new()
    }
}

/// Search service that provides symbol search functionality
pub struct SemanticSearchService {
    index: Arc<SearchIndex>,
}

/// How long the reading, the interpreting and the writing of one file
/// took.
///
/// The reading and the interpreting are one phase —the file has to be
/// read before it can be understood— and the writing into the index is
/// another.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct IndexTimings {
    /// Reading the file and interpreting it.
    pub parse_ms: u64,
    /// Writing the symbols into the index.
    pub index_ms: u64,
}

impl SemanticSearchService {
    /// Creates a new semantic search service
    pub fn new() -> Self {
        Self {
            index: Arc::new(SearchIndex::new()),
        }
    }

    /// Returns a reference to the search index
    pub fn index(&self) -> &SearchIndex {
        &self.index
    }

    /// Indexes the symbols of a file
    ///
    /// A language with a reader of its own is read by that reader, the one
    /// the graph is built from and `reparse_on_edit` hands to the index
    /// ([`index_symbols`](Self::index_symbols)): a Terraform block or an
    /// Ansible task is a symbol under the name the tools that ask about
    /// them use. Every other file — and a file whose own reader fell over,
    /// because the index is more tolerant than the graph — is read the
    /// generic way, so that the same file does not declare one set of
    /// symbols after an edit and another after a reindex.
    pub fn index_file(
        &self,
        file_path: &str,
        source: &str,
        language: Language,
    ) -> Result<(), String> {
        let own_reader =
            semantic_facts_of(LanguageConfig::for_language(language), source, file_path);
        if let SemanticOutcome::Handled(facts) = own_reader {
            self.index_symbols(file_path, facts.symbols);
            return Ok(());
        }

        let parser = crate::infrastructure::parser::TreeSitterParser::new(language)
            .map_err(|e| e.to_string())?;

        // The symbols carry the path they were read from: a result that
        // names the file "source" cannot be opened by whoever asked.
        let symbols = parser
            .find_all_symbols_with_path(source, file_path)
            .map_err(|e| e.to_string())?;

        self.index_symbols(file_path, symbols);

        Ok(())
    }

    /// Indexes symbols that have already been read.
    ///
    /// A caller that interpreted the file for another purpose hands the
    /// result here instead of paying for a second reading of the same
    /// bytes.
    pub fn index_symbols(&self, file_path: &str, symbols: Vec<Symbol>) {
        self.index.index_file(file_path, symbols);
    }

    /// Forgets every symbol indexed for `file_path`.
    ///
    /// `file_path` is the name the file was indexed under: the same
    /// `to_string_lossy` form [`index_file_from_path`](Self::index_file_from_path)
    /// and the incremental routes use.
    pub fn forget_file(&self, file_path: &Path) {
        self.index.clear_file(&file_path.to_string_lossy());
    }

    /// Indexes a single file from path
    pub fn index_file_from_path(&self, file_path: &Path) -> Result<(), String> {
        self.index_file_from_path_timed(file_path).map(|_| ())
    }

    /// Indexes a single file from path, saying how long each part took.
    ///
    /// The reading and the interpreting are one phase, and the writing
    /// into the index is another: they are told apart because they do
    /// not cost the same, and a consumer that wants to know what an
    /// operation paid for needs to know which one it paid for.
    pub fn index_file_from_path_timed(&self, file_path: &Path) -> Result<IndexTimings, String> {
        let read_start = std::time::Instant::now();
        let extension = file_path.extension().and_then(|e| e.to_str());

        let language = Language::from_extension(extension.as_ref().map(std::ffi::OsStr::new))
            .ok_or_else(|| "Unsupported file type".to_string())?;

        let source =
            std::fs::read_to_string(file_path).map_err(|e| format!("{READ_FAILURE}: {e}"))?;

        let parse_ms = read_start.elapsed().as_millis() as u64;

        let write_start = std::time::Instant::now();
        self.index_file(&file_path.to_string_lossy(), &source, language)?;
        let index_ms = write_start.elapsed().as_millis() as u64;

        Ok(IndexTimings { parse_ms, index_ms })
    }

    /// Searches for symbols by name in the in-memory [`SearchIndex`].
    pub fn search(&self, query: SymbolSearchQuery) -> Vec<SearchResult> {
        self.index.search(&query)
    }

    /// Clears the search index
    pub fn clear(&self) {
        self.index.clear();
    }

    /// Populates the index with the files of `dir` and reports what it
    /// could not cover.
    ///
    /// The set of files is the inventory's — the one the graph is built
    /// from — so the search index and the graph describe the same tree.
    /// Non-fatal: a file that cannot be read or indexed does not stop the
    /// others, and it is not forgotten either: it comes back as a hole.
    ///
    /// An empty list is the claim that nothing was left out.
    pub fn populate_from_directory_with_holes(
        &self,
        dir: &Path,
    ) -> Result<Vec<SkippedFile>, String> {
        if !dir.exists() {
            return Err(format!("Directory does not exist: {}", dir.display()));
        }

        let inventory = source_files::inventory(dir);
        let mut holes = inventory.coverage_holes();

        for file in inventory.source_files() {
            if let Err(err) = self.index_file_from_path(&file.path) {
                holes.push(SkippedFile {
                    path: file.path.to_string_lossy().into_owned(),
                    reason: if err.starts_with(READ_FAILURE) {
                        SkipReason::Read(err)
                    } else {
                        SkipReason::Parse(err)
                    },
                });
            }
        }

        Ok(holes)
    }

    /// Populates the index by walking a directory and indexing all supported files.
    ///
    /// The same walk as [`populate_from_directory_with_holes`](Self::populate_from_directory_with_holes),
    /// for the caller that does not publish the holes.
    pub fn populate_from_directory(&self, dir: &Path) -> Result<(), String> {
        self.populate_from_directory_with_holes(dir).map(|_| ())
    }
}

impl Default for SemanticSearchService {
    fn default() -> Self {
        Self::new()
    }
}

impl crate::application::ports::SymbolSearch for SemanticSearchService {
    fn index_workspace(&self, root: &Path) -> crate::application::AppResult<()> {
        SemanticSearchService::populate_from_directory(self, root)
            .map_err(|e| crate::application::ports::parser_failure("indexing the workspace", e))
    }

    fn index_file(&self, path: &Path) -> crate::application::AppResult<()> {
        SemanticSearchService::index_file_from_path(self, path)
            .map_err(|e| crate::application::ports::parser_failure("indexing a file", e))
    }

    fn forget_file(&self, path: &Path) {
        SemanticSearchService::forget_file(self, path);
    }

    fn search(&self, query: &SymbolSearchQuery) -> Vec<crate::domain::aggregates::symbol::Symbol> {
        SemanticSearchService::search(self, query.clone())
            .into_iter()
            .map(|r| r.symbol)
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_exact_match_ranking() {
        let index = SearchIndex::new();

        index.index_file(
            "test.rs",
            vec![
                Symbol::new("foo", SymbolKind::Function, Location::new("test.rs", 1, 0)),
                Symbol::new(
                    "foobar",
                    SymbolKind::Function,
                    Location::new("test.rs", 2, 0),
                ),
                Symbol::new("bar", SymbolKind::Function, Location::new("test.rs", 3, 0)),
            ],
        );

        let query = SymbolSearchQuery {
            query: "foo".to_string(),
            kinds: vec![],
            max_results: 10,
        };

        let results = index.search(&query);

        assert!(!results.is_empty());
        // Exact match should be first
        assert_eq!(results[0].symbol.name(), "foo");
        assert_eq!(results[0].match_type, MatchType::Exact);
    }

    #[test]
    fn test_prefix_match_ranking() {
        let index = SearchIndex::new();

        index.index_file(
            "test.py",
            vec![
                Symbol::new(
                    "get_value",
                    SymbolKind::Function,
                    Location::new("test.py", 1, 0),
                ),
                Symbol::new(
                    "get_name",
                    SymbolKind::Function,
                    Location::new("test.py", 2, 0),
                ),
                Symbol::new(
                    "set_value",
                    SymbolKind::Function,
                    Location::new("test.py", 3, 0),
                ),
            ],
        );

        let query = SymbolSearchQuery {
            query: "get".to_string(),
            kinds: vec![],
            max_results: 10,
        };

        let results = index.search(&query);

        // Prefix matches should come before contains matches
        for result in &results {
            assert!(
                result.symbol.name().starts_with("get") || result.match_type == MatchType::Contains
            );
        }
    }

    #[test]
    fn test_kind_filter() {
        let index = SearchIndex::new();

        index.index_file(
            "test.rs",
            vec![
                Symbol::new(
                    "my_function",
                    SymbolKind::Function,
                    Location::new("test.rs", 1, 0),
                ),
                Symbol::new("MyClass", SymbolKind::Class, Location::new("test.rs", 2, 0)),
                Symbol::new(
                    "my_variable",
                    SymbolKind::Variable,
                    Location::new("test.rs", 3, 0),
                ),
            ],
        );

        let query = SymbolSearchQuery {
            query: "my".to_string(),
            kinds: vec![SymbolKind::Function],
            max_results: 10,
        };

        let results = index.search(&query);

        assert!(!results.is_empty());
        for result in &results {
            assert_eq!(result.symbol.kind(), &SymbolKind::Function);
        }
    }

    #[test]
    fn test_fuzzy_match() {
        let index = SearchIndex::new();

        index.index_file(
            "test.py",
            vec![
                Symbol::new(
                    "calculate_total",
                    SymbolKind::Function,
                    Location::new("test.py", 1, 0),
                ),
                Symbol::new(
                    "calc_value",
                    SymbolKind::Function,
                    Location::new("test.py", 2, 0),
                ),
            ],
        );

        let query = SymbolSearchQuery {
            query: "calc".to_string(),
            kinds: vec![],
            max_results: 10,
        };

        let results = index.search(&query);

        // Should find both calc* functions
        assert!(results.len() >= 1);
    }

    #[test]
    fn test_max_results() {
        let index = SearchIndex::new();

        let symbols: Vec<Symbol> = (0..100)
            .map(|i| {
                Symbol::new(
                    format!("func_{}", i),
                    SymbolKind::Function,
                    Location::new("test.rs", i, 0),
                )
            })
            .collect();

        index.index_file("test.rs", symbols);

        let query = SymbolSearchQuery {
            query: "func".to_string(),
            kinds: vec![],
            max_results: 10,
        };

        let results = index.search(&query);

        assert_eq!(results.len(), 10);
    }

    #[test]
    fn test_fuzzy_score_calculation() {
        use super::super::name_index::fuzzy_score;
        // Fuzzy score caps at 0.6 since exact/prefix/contains are handled separately
        // Exact
        assert!(fuzzy_score("foo", "foo") > 0.5);
        // Prefix
        assert!(fuzzy_score("foo", "foobar") > 0.5);
        // Contains (fuzzy - query is substring but not prefix)
        assert!(fuzzy_score("bar", "foobar") > 0.3);
        // No match - "xyz" is not in "foobar" at all
        assert_eq!(fuzzy_score("xyz", "foobar"), 0.0);
    }

    #[test]
    fn test_search_result_ordering() {
        let results = vec![
            SearchResult::new(
                Symbol::new("a", SymbolKind::Function, Location::new("test.rs", 1, 0)),
                0.5,
                MatchType::Contains,
            ),
            SearchResult::new(
                Symbol::new("b", SymbolKind::Function, Location::new("test.rs", 2, 0)),
                1.0,
                MatchType::Exact,
            ),
            SearchResult::new(
                Symbol::new("c", SymbolKind::Function, Location::new("test.rs", 3, 0)),
                0.9,
                MatchType::Prefix,
            ),
        ];

        let mut sorted = results.clone();
        sorted.sort();

        // Exact should come first
        assert_eq!(sorted[0].symbol.name(), "b");
        assert_eq!(sorted[0].match_type, MatchType::Exact);
    }

    /// The names the index answers for `query`, as `(name, file)` pairs.
    fn found(index: &SearchIndex, query: &str) -> Vec<(String, String)> {
        let mut hits: Vec<(String, String)> = index
            .search(&SymbolSearchQuery {
                query: query.to_string(),
                kinds: vec![],
                max_results: 50,
            })
            .into_iter()
            .map(|hit| {
                (
                    hit.symbol.name().to_string(),
                    hit.symbol.location().file().to_string(),
                )
            })
            .collect();
        hits.sort();
        hits
    }

    fn function(name: &str, file: &str, line: u32, column: u32) -> Symbol {
        Symbol::new(
            name,
            SymbolKind::Function,
            Location::new(file, line, column),
        )
    }

    /// Indexing a file again replaces what the index knew of it. A symbol the
    /// file no longer declares must not stay searchable: the index would keep
    /// answering for code that is not there.
    #[test]
    fn a_renamed_symbol_is_not_searchable_after_the_file_is_indexed_again() {
        let index = SearchIndex::new();
        index.index_file("src/a.rs", vec![function("old_name", "src/a.rs", 3, 0)]);
        assert_eq!(
            found(&index, "old_name").len(),
            1,
            "the premise: it was indexed"
        );

        // The same declaration, renamed: the position does not change, the name does.
        index.index_file("src/a.rs", vec![function("new_name", "src/a.rs", 3, 0)]);

        assert!(
            found(&index, "old_name").is_empty(),
            "the file no longer declares `old_name`, and the index still finds it: {:?}",
            found(&index, "old_name")
        );
        assert_eq!(found(&index, "new_name").len(), 1);
        assert_eq!(
            index.len(),
            1,
            "one symbol in the file, one entry in the index"
        );
    }

    /// A symbol that moved to another line is the same case with the position
    /// changed instead of the name.
    #[test]
    fn a_symbol_that_moved_is_found_once_after_the_file_is_indexed_again() {
        let index = SearchIndex::new();
        index.index_file("src/a.rs", vec![function("helper", "src/a.rs", 3, 0)]);
        index.index_file("src/a.rs", vec![function("helper", "src/a.rs", 9, 0)]);

        assert_eq!(
            found(&index, "helper").len(),
            1,
            "`helper` is declared once and is listed {} times: {:?}",
            found(&index, "helper").len(),
            found(&index, "helper")
        );
        assert_eq!(index.len(), 1);
    }

    /// Two files that declare a symbol with the same name at the same line and
    /// column are two symbols. The identity of a symbol includes its file.
    #[test]
    fn two_files_with_a_symbol_at_the_same_line_and_column_stay_two_results() {
        let index = SearchIndex::new();
        index.index_file("src/a.rs", vec![function("new", "src/a.rs", 5, 4)]);
        index.index_file("src/b.rs", vec![function("new", "src/b.rs", 5, 4)]);

        assert_eq!(
            found(&index, "new"),
            vec![
                ("new".to_string(), "src/a.rs".to_string()),
                ("new".to_string(), "src/b.rs".to_string()),
            ],
            "the second file overwrote the first one in the index"
        );
        assert_eq!(index.len(), 2);
    }

    /// Forgetting one file leaves the other one alone, even when their symbols
    /// share a name, a line and a column.
    #[test]
    fn clearing_a_file_does_not_forget_the_symbols_of_another_file() {
        let index = SearchIndex::new();
        index.index_file("src/a.rs", vec![function("new", "src/a.rs", 5, 4)]);
        index.index_file("src/b.rs", vec![function("new", "src/b.rs", 5, 4)]);

        index.clear_file("src/a.rs");

        assert_eq!(
            found(&index, "new"),
            vec![("new".to_string(), "src/b.rs".to_string())],
            "clearing `src/a.rs` took the symbol of `src/b.rs` with it"
        );
    }

    /// The file a caller names when it indexes is the file the entries belong
    /// to, even if a reader stamped its symbols with another place: the entry
    /// can be forgotten by the name it was indexed under.
    #[test]
    fn a_file_is_forgotten_by_the_name_it_was_indexed_under() {
        let index = SearchIndex::new();
        index.index_file("src/a.rs", vec![function("only_here", "source", 2, 0)]);
        index.index_file("src/b.rs", vec![function("only_here", "source", 2, 0)]);
        assert_eq!(index.len(), 2, "two files, two entries");

        index.clear_file("src/a.rs");

        assert_eq!(index.len(), 1);
        index.clear_file("src/b.rs");
        assert!(index.is_empty());
    }

    /// The service forgets a file as a whole, and says so through the port the
    /// application depends on.
    #[test]
    fn the_service_forgets_a_deleted_file() {
        use crate::application::ports::SymbolSearch;

        let service = SemanticSearchService::new();
        service.index_symbols(
            "src/gone.rs",
            vec![function("vanished", "src/gone.rs", 1, 0)],
        );
        service.index_symbols("src/kept.rs", vec![function("stays", "src/kept.rs", 1, 0)]);

        SymbolSearch::forget_file(&service, Path::new("src/gone.rs"));

        assert!(found(service.index(), "vanished").is_empty());
        assert_eq!(found(service.index(), "stays").len(), 1);
    }
}

/// What a name search means, and that the candidates answer it the way a
/// full scan does.
#[cfg(test)]
mod search_semantics {
    use super::*;
    use std::collections::BTreeMap;

    /// A small deterministic generator: the corpora and the operations are
    /// the same on every run.
    struct Lcg(u64);

    impl Lcg {
        fn next(&mut self) -> u64 {
            self.0 = self
                .0
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            self.0 >> 33
        }

        fn below(&mut self, n: usize) -> usize {
            (self.next() % n as u64) as usize
        }
    }

    /// A fragment written in a script the search has to index as text. The
    /// characters are unicode escapes: the source carries no foreign script
    /// and the data stays the same one the search must handle.
    const JAPANESE: &str = "\u{65e5}\u{672c}";
    /// The same script with its third character and a suffix.
    const JAPANESE_NAME: &str = "\u{65e5}\u{672c}\u{8a9e}_x";
    /// The last two characters of [`JAPANESE_NAME`].
    const JAPANESE_TAIL: &str = "\u{672c}\u{8a9e}";

    const FRAGMENTS: [&str; 18] = [
        "get", "set", "new", "handle", "Request", "Parser", "parse", "_", JAPANESE, "ñ", "Ñ", "x",
        "a", "b", "index", "Index", "find", "İ",
    ];
    const KINDS: [SymbolKind; 5] = [
        SymbolKind::Function,
        SymbolKind::Class,
        SymbolKind::Method,
        SymbolKind::Variable,
        SymbolKind::Struct,
    ];

    fn random_name(rng: &mut Lcg) -> String {
        (0..1 + rng.below(3))
            .map(|_| FRAGMENTS[rng.below(FRAGMENTS.len())])
            .collect()
    }

    fn random_symbols(rng: &mut Lcg, file: &str) -> Vec<Symbol> {
        (0..rng.below(26))
            .map(|_| {
                Symbol::new(
                    random_name(rng),
                    KINDS[rng.below(KINDS.len())],
                    Location::new(file, 1 + rng.below(40) as u32, rng.below(8) as u32),
                )
            })
            .collect()
    }

    fn function(name: &str, file: &str, line: u32, column: u32) -> Symbol {
        Symbol::new(
            name,
            SymbolKind::Function,
            Location::new(file, line, column),
        )
    }

    fn query(text: &str, kinds: &[SymbolKind], limit: usize) -> SymbolSearchQuery {
        SymbolSearchQuery {
            query: text.to_string(),
            kinds: kinds.to_vec(),
            max_results: limit,
        }
    }

    type Shape = (String, String, u32, u32, SymbolKind, MatchType, u32);

    /// Everything a result says, comparable.
    fn shape(results: &[SearchResult]) -> Vec<Shape> {
        results
            .iter()
            .map(|r| {
                (
                    r.symbol.name().to_string(),
                    r.symbol.location().file().to_string(),
                    r.symbol.location().line(),
                    r.symbol.location().column(),
                    *r.symbol.kind(),
                    r.match_type,
                    r.score.to_bits(),
                )
            })
            .collect()
    }

    /// Queries that hit every class: whole names, prefixes, substrings,
    /// subsequences, other casing and things that match nothing.
    fn queries_over(index: &SearchIndex, rng: &mut Lcg) -> Vec<String> {
        let names: Vec<String> = {
            let state = index.state.read();
            state
                .entries
                .iter()
                .flatten()
                .map(|e| e.name.to_string())
                .collect()
        };
        let mut queries = vec!["zzz".to_string(), "_".to_string(), "é".to_string()];
        for name in names.iter().step_by(3) {
            let chars: Vec<char> = name.chars().collect();
            let from = rng.below(chars.len());
            let to = from + 1 + rng.below(chars.len() - from);
            queries.push(name.clone());
            queries.push(name.to_uppercase());
            queries.push(chars[..to].iter().collect());
            queries.push(chars[from..to].iter().collect());
            queries.push(chars.iter().step_by(2).collect());
        }
        queries.sort();
        queries.dedup();
        queries
    }

    fn assert_search_agrees_with_the_scan(index: &SearchIndex, rng: &mut Lcg) {
        let kinds: [&[SymbolKind]; 3] = [&[], &[SymbolKind::Function], &KINDS[1..3]];
        for text in queries_over(index, rng) {
            for kinds in kinds {
                let everything = shape(&index.search_by_scan(&query(&text, kinds, usize::MAX)));
                assert_eq!(
                    shape(&index.search(&query(&text, kinds, usize::MAX))),
                    everything,
                    "query {text:?} kinds {kinds:?} without a limit"
                );
                for limit in [0, 1, 2, 7, 50] {
                    let wanted = &everything[..limit.min(everything.len())];
                    assert_eq!(
                        shape(&index.search(&query(&text, kinds, limit))),
                        wanted,
                        "query {text:?} kinds {kinds:?} limit {limit}: not the first {limit} of the \
                         unlimited answer"
                    );
                    assert_eq!(
                        shape(&index.search_by_scan(&query(&text, kinds, limit))),
                        wanted,
                        "scan, query {text:?} kinds {kinds:?} limit {limit}"
                    );
                }
            }
        }
    }

    /// The tables must say what the entries say, and nothing else.
    fn assert_consistent(index: &SearchIndex) {
        let state = index.state.read();
        assert!(state.derived_in_sync, "a write left itself unfinished");
        let mut rebuilt = NameTerms::new();
        let mut live = 0;
        let mut holes = Vec::new();
        for (slot, entry) in state.entries.iter().enumerate() {
            let slot = slot as u32;
            let Some(entry) = entry else {
                holes.push(slot);
                continue;
            };
            live += 1;
            rebuilt.attach(&entry.name_lower, slot);
            assert_eq!(state.terms.name(entry.term), &*entry.name_lower);
            assert!(state.terms.slots(entry.term).contains(&slot));
            assert!(state.files[&entry.file].contains(&slot));
            assert_eq!(entry.name.to_lowercase(), *entry.name_lower);
        }
        assert_eq!(state.live, live);
        assert!(state.terms.describes_the_same_as(&rebuilt));
        assert!(rebuilt.describes_the_same_as(&state.terms));
        assert_eq!(state.files.values().map(Vec::len).sum::<usize>(), live);
        assert!(state.files.values().all(|slots| !slots.is_empty()));
        let mut free = state.free_slots.clone();
        free.sort_unstable();
        assert_eq!(free, holes, "the holes and the free slots disagree");
    }

    #[test]
    fn a_limit_returns_the_best_symbols_and_not_the_last_ones_seen() {
        let index = SearchIndex::new();
        let mut symbols = vec![Symbol::new(
            "target",
            SymbolKind::Function,
            Location::new("src/a.rs", 1, 0),
        )];
        for i in 0..120u32 {
            symbols.push(Symbol::new(
                format!("target_x{i}"),
                SymbolKind::Function,
                Location::new("src/a.rs", 2 + i, 0),
            ));
            symbols.push(Symbol::new(
                format!("xtargetx{i}"),
                SymbolKind::Function,
                Location::new("src/a.rs", 200 + i, 0),
            ));
        }
        index.index_file("src/a.rs", symbols);

        let top: Vec<String> = index
            .search(&query("target", &[], 5))
            .iter()
            .map(|r| r.symbol.name().to_string())
            .collect();

        assert_eq!(
            top,
            [
                "target",
                "target_x0",
                "target_x1",
                "target_x10",
                "target_x100"
            ],
            "the exact name and then the first prefixes by name, not whatever the \
             search saw last"
        );
    }

    #[test]
    fn the_candidates_answer_what_a_scan_answers() {
        let mut rng = Lcg(7);
        let index = SearchIndex::new();
        for n in 0..12 {
            let file = format!("src/f{n}.rs");
            let symbols = random_symbols(&mut rng, &file);
            index.index_file(&file, symbols);
        }
        assert!(index.len() > 100, "the corpus has to be worth searching");
        assert_consistent(&index);
        assert_search_agrees_with_the_scan(&index, &mut rng);
    }

    #[test]
    fn adds_removals_and_renames_leave_the_index_as_a_fresh_one() {
        let mut rng = Lcg(11);
        let index = SearchIndex::new();
        let mut model: BTreeMap<String, Vec<Symbol>> = BTreeMap::new();
        for step in 0..300 {
            let file = format!("src/f{}.rs", rng.below(9));
            if rng.below(4) == 0 {
                index.clear_file(&file);
                model.remove(&file);
            } else {
                let symbols = random_symbols(&mut rng, &file);
                index.index_file(&file, symbols.clone());
                model.insert(file, symbols);
            }
            if step % 30 != 29 {
                continue;
            }
            assert_consistent(&index);
            let fresh = SearchIndex::new();
            for (file, symbols) in &model {
                fresh.index_file(file, symbols.clone());
            }
            assert_eq!(index.len(), fresh.len(), "step {step}");
            for text in queries_over(&fresh, &mut rng) {
                for limit in [3, usize::MAX] {
                    assert_eq!(
                        shape(&index.search(&query(&text, &[], limit))),
                        shape(&fresh.search(&query(&text, &[], limit))),
                        "step {step}, query {text:?}, limit {limit}"
                    );
                }
            }
        }
        assert_search_agrees_with_the_scan(&index, &mut rng);
    }

    #[test]
    fn a_zero_limit_answers_nothing_and_a_huge_one_does_not_reserve_memory() {
        let index = SearchIndex::new();
        index.index_file("src/a.rs", vec![function("alpha", "src/a.rs", 1, 0)]);

        assert!(index.search(&query("alpha", &[], 0)).is_empty());
        assert!(index.search_by_scan(&query("alpha", &[], 0)).is_empty());
        assert_eq!(index.search(&query("alpha", &[], usize::MAX)).len(), 1);
        assert_eq!(
            index.search_by_scan(&query("alpha", &[], usize::MAX)).len(),
            1
        );
        assert!(index.search(&query("", &[], 10)).is_empty());
    }

    /// Case is folded with `str::to_lowercase` and nothing else is
    /// normalized: what is the same text in Unicode but not the same code
    /// points is not the same name.
    #[test]
    fn text_is_lowercased_and_not_normalized() {
        let index = SearchIndex::new();
        index.index_file(
            "src/a.rs",
            vec![
                function("Ñandú", "src/a.rs", 1, 0),
                function("caf\u{e9}", "src/a.rs", 2, 0),
                function("cafe\u{301}", "src/a.rs", 3, 0),
                function(JAPANESE_NAME, "src/a.rs", 4, 0),
                function("İstanbul", "src/a.rs", 5, 0),
            ],
        );
        let kinds_of = |text: &str| -> Vec<(String, MatchType)> {
            let results = index.search(&query(text, &[], 10));
            assert_eq!(
                shape(&results),
                shape(&index.search_by_scan(&query(text, &[], 10))),
                "{text:?}"
            );
            results
                .iter()
                .map(|r| (r.symbol.name().to_string(), r.match_type))
                .collect()
        };

        assert_eq!(
            kinds_of("ÑAN"),
            [("Ñandú".to_string(), MatchType::Prefix)],
            "an upper case query finds a name with other casing"
        );
        assert_eq!(kinds_of("NDÚ")[0].1, MatchType::Contains);
        assert_eq!(
            kinds_of("caf\u{e9}"),
            [("caf\u{e9}".to_string(), MatchType::Exact)],
            "the precomposed `é` is not the decomposed one"
        );
        assert_eq!(
            kinds_of(JAPANESE_TAIL),
            [(JAPANESE_NAME.to_string(), MatchType::Contains)]
        );
        assert_eq!(
            kinds_of("istanbul"),
            [("İstanbul".to_string(), MatchType::Fuzzy)],
            "`İ` lowercases to `i` and a combining dot: the characters are there, in order, \
             but not next to each other"
        );
    }

    /// Two files that declare the same name stay two symbols, with the
    /// limit and the kind filter applied to symbols and not to names.
    #[test]
    fn homonyms_are_all_found_and_ordered_the_same_way_every_time() {
        let index = SearchIndex::new();
        for file in ["src/c.rs", "src/a.rs", "src/b.rs"] {
            index.index_file(
                file,
                vec![
                    function("new", file, 9, 0),
                    Symbol::new("new", SymbolKind::Method, Location::new(file, 3, 0)),
                    function("New", file, 1, 0),
                ],
            );
        }

        let files = |limit, kinds: &[SymbolKind]| -> Vec<(String, u32)> {
            index
                .search(&query("new", kinds, limit))
                .iter()
                .map(|r| {
                    (
                        r.symbol.location().file().to_string(),
                        r.symbol.location().line(),
                    )
                })
                .collect()
        };

        assert_eq!(files(100, &[]).len(), 9);
        assert_eq!(
            files(4, &[SymbolKind::Method]),
            [
                ("src/a.rs".to_string(), 3),
                ("src/b.rs".to_string(), 3),
                ("src/c.rs".to_string(), 3)
            ]
        );
        assert_eq!(files(5, &[]), files(100, &[])[..5]);
    }

    /// If a write never finishes, the tables that find candidates cannot be
    /// trusted: searches judge every symbol instead, the answer is the same,
    /// and the next write rebuilds the tables.
    #[test]
    fn an_interrupted_write_degrades_to_a_scan_and_the_next_write_repairs_it() {
        let mut rng = Lcg(5);
        let index = SearchIndex::new();
        for n in 0..6 {
            let file = format!("src/f{n}.rs");
            let symbols = random_symbols(&mut rng, &file);
            index.index_file(&file, symbols);
        }
        let texts = queries_over(&index, &mut rng);
        let expected: Vec<Vec<Shape>> = texts
            .iter()
            .map(|t| shape(&index.search(&query(t, &[], 20))))
            .collect();

        {
            // What a write that died half way leaves behind.
            let mut state = index.state.write();
            state.terms = NameTerms::new();
            state.derived_in_sync = false;
        }
        for (text, wanted) in texts.iter().zip(&expected) {
            assert_eq!(
                &shape(&index.search(&query(text, &[], 20))),
                wanted,
                "the scan fallback answered something else for {text:?}"
            );
        }

        index.index_file("src/f0.rs", vec![function("after", "src/f0.rs", 1, 0)]);
        assert_consistent(&index);
        assert_eq!(index.search(&query("after", &[], 5)).len(), 1);
    }

    /// A search sees a file as it was or as it is: here a file that
    /// alternates between two sets of fifty symbols never shows a mix.
    #[test]
    fn a_search_never_sees_half_of_a_rewritten_file() {
        let index = Arc::new(SearchIndex::new());
        let set = |letter: char| -> Vec<Symbol> {
            (0..50)
                .map(|i| function(&format!("x_{letter}{i}"), "src/a.rs", i + 1, 0))
                .collect()
        };
        index.index_file("src/a.rs", set('a'));
        let writer = {
            let index = Arc::clone(&index);
            std::thread::spawn(move || {
                for round in 0..400 {
                    index.index_file("src/a.rs", set(if round % 2 == 0 { 'b' } else { 'a' }));
                }
            })
        };
        for _ in 0..400 {
            let names: Vec<String> = index
                .search(&query("x_", &[], 1000))
                .iter()
                .map(|r| r.symbol.name().to_string())
                .collect();
            assert_eq!(names.len(), 50, "half a file: {names:?}");
            let letter = names[0].as_bytes()[2];
            assert!(names.iter().all(|n| n.as_bytes()[2] == letter), "a mix");
        }
        writer.join().expect("the writer finished");
        assert_consistent(&index);
    }
}
