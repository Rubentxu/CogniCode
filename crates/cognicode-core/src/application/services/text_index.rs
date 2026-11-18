//! A derived trigram index over the text of files, used to decide which files
//! a content search has to read.
//!
//! The index answers one question: "can this file, as it was when it was
//! indexed, hold a match for this query?". It never answers with matches. A
//! file the index calls a candidate is read and verified by the same matcher
//! the unindexed search uses, so every match, offset and context line comes
//! from the file and not from the index. A file the index cannot vouch for
//! (never seen, changed, still too fresh to trust, too big to index) is
//! read, which is exactly what the unindexed search does.
//!
//! ## What is indexed
//!
//! For each line of a file, the line is case-folded (see the fold below) and every
//! window of three bytes of the folded line is a trigram. Trigrams never cross
//! a line, because the matcher never matches across one. A file is a
//! candidate for a plan when, for at least one of the plan's alternatives,
//! every trigram of that alternative is in the file.
//!
//! The fold is what makes the index sound for case-insensitive queries: if two
//! characters are the same under the matcher's case rules (simple case
//! folding of the regex engine, or the per-character lowercase of a literal
//! search), they fold to the same bytes. The tests check that over every
//! character, so a change to the fold cannot silently lose a match.
//!
//! ## What is not promised
//!
//! The index is derived and lives in memory: it is rebuilt by searching, and
//! it is never a source of truth. A regex is only narrowed when it has a
//! literal of at least three bytes that every match must start or end with;
//! any other regex falls back to reading every file, and says why.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use regex_syntax::hir::literal::{ExtractKind, Extractor};

/// The version of the tokenizer: the fold and the way a line becomes
/// trigrams. Any change to either changes what a trigram means, so it must
/// bump this number; the golden test over [`trigrams_of`] fails until it does.
pub(crate) const TOKENIZER_VERSION: u32 = 1;

/// How long a file must have been quiet before the index trusts its stamp,
/// unless the configuration says otherwise.
const DEFAULT_QUIET_PERIOD: Duration = Duration::from_secs(2);

/// The largest file the index keeps a document for. A bigger file is read
/// directly on every search, as it is without an index.
pub(crate) const MAX_INDEXED_BYTES: usize = 2 * 1024 * 1024;

/// The most alternatives a plan may have. A plan with more costs more to
/// check than it saves, so the search reads every file instead.
const MAX_PLAN_ALTERNATIVES: usize = 32;

/// The shortest literal, in folded bytes, that yields a trigram.
const TRIGRAM_BYTES: usize = 3;

const NO_USABLE_LITERAL: &str =
    "the pattern has no literal of at least 3 bytes that every match must start or end with";

/// What the index leaves out, and what the index is told about a search.
///
/// The names are matched against the file name (not the directory), with `*`
/// standing for any run of characters, ignoring ASCII case. A file whose name
/// matches is never given a document: the index holds nothing derived from
/// it. The search still reads it directly, so the answer is the one the
/// unindexed search gives.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TextIndexConfig {
    /// File-name patterns that are never indexed.
    pub excluded_names: Vec<String>,
    /// How long a file must have been quiet before the index trusts its
    /// stamp.
    ///
    /// A file written twice within the resolution of the file system's clock
    /// keeps the same length and timestamp, so a stamp alone cannot tell the
    /// two contents apart. A file changed more recently than this before it
    /// was indexed is read again by every search, and trusted once it has
    /// been quiet for longer.
    pub quiet_period: Duration,
}

impl Default for TextIndexConfig {
    /// Excludes the names that commonly hold secrets: environment files,
    /// private keys, key stores and credential files.
    fn default() -> Self {
        let names = [
            ".env",
            ".env.*",
            "*.pem",
            "*.key",
            "*.p12",
            "*.pfx",
            "*.jks",
            "*.keystore",
            "id_rsa*",
            "id_dsa*",
            "id_ecdsa*",
            "id_ed25519*",
            "credentials*",
            "*.secret",
            "*.secrets",
            "secrets.*",
            ".netrc",
            ".npmrc",
            ".pypirc",
        ];
        Self {
            excluded_names: names.iter().map(|n| (*n).to_string()).collect(),
            quiet_period: DEFAULT_QUIET_PERIOD,
        }
    }
}

impl TextIndexConfig {
    /// Whether the file at `path` must be kept out of the index.
    pub(crate) fn excludes(&self, path: &Path) -> bool {
        let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
            return false;
        };
        self.excluded_names
            .iter()
            .any(|pattern| wildcard_matches(pattern, name))
    }
}

/// Matches `name` against a pattern where `*` stands for any run of
/// characters (including none) and every other character stands for itself,
/// ignoring ASCII case.
fn wildcard_matches(pattern: &str, name: &str) -> bool {
    let (pattern, name) = (pattern.as_bytes(), name.as_bytes());
    let (mut p, mut n) = (0, 0);
    let mut resume: Option<(usize, usize)> = None;
    while n < name.len() {
        if p < pattern.len() && pattern[p] == b'*' {
            resume = Some((p, n));
            p += 1;
        } else if p < pattern.len() && pattern[p].eq_ignore_ascii_case(&name[n]) {
            p += 1;
            n += 1;
        } else if let Some((star, matched)) = resume {
            p = star + 1;
            n = matched + 1;
            resume = Some((star, matched + 1));
        } else {
            return false;
        }
    }
    pattern[p..].iter().all(|c| *c == b'*')
}

/// Appends the folded form of `c` to `out`.
///
/// Two characters that the matcher treats as the same letter fold to the same
/// bytes: the fold goes to lowercase, to uppercase and back to lowercase, so
/// the letters that only simple case folding relates (a final sigma and a
/// sigma, a sharp s and its capital, the Kelvin sign and a `k`) meet in the
/// same place. The fold may merge more characters than the matcher does,
/// which costs selectivity and never a match.
fn fold_char(c: char, out: &mut Vec<u8>) {
    if c.is_ascii() {
        out.push(c.to_ascii_lowercase() as u8);
        return;
    }
    let mut buf = [0u8; 4];
    for lower in c.to_lowercase() {
        for upper in lower.to_uppercase() {
            for folded in upper.to_lowercase() {
                out.extend_from_slice(folded.encode_utf8(&mut buf).as_bytes());
            }
        }
    }
}

/// Appends the folded form of `text` to `out`.
pub(crate) fn fold_into(text: &str, out: &mut Vec<u8>) {
    for c in text.chars() {
        fold_char(c, out);
    }
}

/// The trigram that starts at the first of three folded bytes.
fn trigram_key(window: &[u8]) -> u32 {
    (u32::from(window[0]) << 16) | (u32::from(window[1]) << 8) | u32::from(window[2])
}

/// The distinct trigrams of `folded`, sorted.
fn trigrams_of_folded(folded: &[u8]) -> Vec<u32> {
    let mut keys: Vec<u32> = folded.windows(TRIGRAM_BYTES).map(trigram_key).collect();
    keys.sort_unstable();
    keys.dedup();
    keys
}

/// The distinct trigrams of the lines of `content`, sorted.
///
/// A line is what the matcher sees: `str::lines`, without its terminator.
pub(crate) fn trigrams_of(content: &str) -> Vec<u32> {
    let mut keys: Vec<u32> = Vec::new();
    let mut folded = Vec::new();
    for line in content.lines() {
        folded.clear();
        fold_into(line, &mut folded);
        keys.extend(folded.windows(TRIGRAM_BYTES).map(trigram_key));
    }
    keys.sort_unstable();
    keys.dedup();
    keys
}

/// What a query needs from a file before the file is worth reading.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum TrigramPlan {
    /// A file can hold a match only if, for at least one alternative, it
    /// holds all of the alternative's trigrams.
    Narrow(Vec<Vec<u32>>),
    /// The query cannot be narrowed; every file has to be read. The reason
    /// is what the answer tells the caller.
    Unusable(&'static str),
}

/// The plan for a literal query.
pub(crate) fn plan_literal(pattern: &str) -> TrigramPlan {
    let mut folded = Vec::new();
    fold_into(pattern, &mut folded);
    if folded.len() < TRIGRAM_BYTES {
        return TrigramPlan::Unusable("the pattern is shorter than 3 bytes");
    }
    TrigramPlan::Narrow(vec![trigrams_of_folded(&folded)])
}

/// The plan for a regex query.
///
/// The pattern is read as written, without the case-insensitive flag: the
/// literals it yields are case-sensitive ones, and the fold takes care of
/// the case rules on both sides. It tries the literals every match starts
/// with and the literals every match ends with, and keeps whichever has the
/// longer shortest literal.
pub(crate) fn plan_regex(pattern: &str) -> TrigramPlan {
    let Ok(hir) = regex_syntax::Parser::new().parse(pattern) else {
        return TrigramPlan::Unusable("the pattern is not a valid regex");
    };
    let best = [ExtractKind::Prefix, ExtractKind::Suffix]
        .into_iter()
        .filter_map(|kind| plan_from_extracted(&hir, kind))
        .max_by_key(|alternatives| alternatives.iter().map(Vec::len).min().unwrap_or_default());
    match best {
        Some(alternatives) => TrigramPlan::Narrow(alternatives),
        None => TrigramPlan::Unusable(NO_USABLE_LITERAL),
    }
}

fn plan_from_extracted(hir: &regex_syntax::hir::Hir, kind: ExtractKind) -> Option<Vec<Vec<u32>>> {
    let from_start = matches!(kind, ExtractKind::Prefix);
    let sequence = Extractor::new().kind(kind).extract(hir);
    let literals = sequence.literals()?;
    if literals.is_empty() {
        return None;
    }
    let mut alternatives: Vec<Vec<u32>> = Vec::new();
    for literal in literals {
        let text = literal_text(literal.as_bytes(), from_start)?;
        let mut folded = Vec::new();
        fold_into(text, &mut folded);
        if folded.len() < TRIGRAM_BYTES {
            return None;
        }
        alternatives.push(trigrams_of_folded(&folded));
    }
    alternatives.sort();
    alternatives.dedup();
    (alternatives.len() <= MAX_PLAN_ALTERNATIVES).then_some(alternatives)
}

/// The text of a literal the extractor cut at a byte count.
///
/// A literal that stops in the middle of a character is cut back to the last
/// whole one (a prefix) or forward to the first whole one (a suffix); what is
/// left is still a prefix or a suffix of every match that had the original.
fn literal_text(bytes: &[u8], from_start: bool) -> Option<&str> {
    match std::str::from_utf8(bytes) {
        Ok(text) => Some(text),
        Err(error) if from_start => std::str::from_utf8(&bytes[..error.valid_up_to()]).ok(),
        Err(_) => {
            let start = bytes
                .iter()
                .position(|b| (b & 0b1100_0000) != 0b1000_0000)?;
            std::str::from_utf8(&bytes[start..]).ok()
        }
    }
}

/// What identifies one content of a file without reading it.
///
/// On Unix the status-change time is part of it: it moves when the file is
/// written, renamed over, `chmod`ed or `chown`ed, so a file that became
/// unreadable is not mistaken for the file that was indexed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct FileStamp {
    /// Length in bytes.
    pub len: u64,
    /// Last modification, in nanoseconds since the Unix epoch.
    pub modified_ns: i128,
    /// Last status change, in nanoseconds since the Unix epoch; the
    /// modification time where the platform has no such thing.
    pub changed_ns: i128,
}

impl FileStamp {
    /// Whether the file may still change without the stamp showing it: a
    /// write within the clock's resolution of the last one leaves the same
    /// stamp. True while less than `quiet_ns` separates the last change from
    /// `observed_ns`.
    pub(crate) fn is_racy(&self, observed_ns: i128, quiet_ns: i128) -> bool {
        self.modified_ns
            .max(self.changed_ns)
            .saturating_add(quiet_ns)
            >= observed_ns
    }
}

/// What the index says about one file for one plan.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum GateVerdict {
    /// The file, as indexed, can hold a match: read it and verify.
    Candidate,
    /// The file, as indexed, cannot hold a match: it need not be read.
    NotCandidate,
    /// The index cannot vouch for the file: read it, and index it.
    Unknown,
}

struct Doc {
    id: u32,
    stamp: FileStamp,
    /// The stamp was too close to the moment of indexing to be trusted.
    racy: bool,
    /// The last walk that went through the file.
    seen: AtomicU64,
}

/// The trigram index. Pure: it holds no handle to a file and reads no clock;
/// the caller brings stamps, trigrams and the time of observation.
pub(crate) struct TextIndex {
    quiet_period_ns: i128,
    docs: HashMap<PathBuf, Doc>,
    /// For each trigram, the ids of the documents that hold it, ascending.
    /// A document is given a fresh, larger id every time it is indexed, so
    /// the lists stay sorted by appending; ids of documents that were
    /// replaced or removed linger here until the next compaction and are
    /// never reached, because documents are looked up by path.
    postings: HashMap<u32, Vec<u32>>,
    next_id: u32,
    dead: usize,
    walks: AtomicU64,
}

impl Default for TextIndex {
    fn default() -> Self {
        Self::with_quiet_period(DEFAULT_QUIET_PERIOD)
    }
}

impl TextIndex {
    /// An empty index that trusts a file's stamp once the file has been quiet
    /// for `quiet_period`.
    pub(crate) fn with_quiet_period(quiet_period: Duration) -> Self {
        Self {
            quiet_period_ns: quiet_period.as_nanos() as i128,
            docs: HashMap::new(),
            postings: HashMap::new(),
            next_id: 0,
            dead: 0,
            walks: AtomicU64::new(0),
        }
    }

    /// Starts a walk: the number every file the walk goes through is marked
    /// with, so that [`TextIndex::sweep`] can tell what the walk did not see.
    pub(crate) fn begin_walk(&self) -> u64 {
        self.walks.fetch_add(1, Ordering::Relaxed) + 1
    }

    /// The number of documents.
    #[cfg(test)]
    pub(crate) fn len(&self) -> usize {
        self.docs.len()
    }

    /// What the index says about `path` as it is now (`stamp`), for `plan`.
    ///
    /// A file that is not indexed, whose stamp changed, or whose stamp is too
    /// recent to trust is [`GateVerdict::Unknown`].
    pub(crate) fn judge(
        &self,
        path: &Path,
        stamp: &FileStamp,
        plan: &[Vec<u32>],
        walk: u64,
    ) -> GateVerdict {
        let Some(doc) = self.docs.get(path) else {
            return GateVerdict::Unknown;
        };
        if doc.racy || doc.stamp != *stamp {
            return GateVerdict::Unknown;
        }
        doc.seen.store(walk, Ordering::Relaxed);
        let holds = |trigram: &u32| {
            self.postings
                .get(trigram)
                .is_some_and(|ids| ids.binary_search(&doc.id).is_ok())
        };
        if plan.iter().any(|alternative| alternative.iter().all(holds)) {
            GateVerdict::Candidate
        } else {
            GateVerdict::NotCandidate
        }
    }

    /// Records what `path` held when it had `stamp`, replacing any earlier
    /// document for it. `observed_ns` is when the stamp was taken; `trigrams`
    /// are the sorted, distinct trigrams of the content read after it.
    pub(crate) fn upsert(
        &mut self,
        path: PathBuf,
        stamp: FileStamp,
        observed_ns: i128,
        trigrams: &[u32],
        walk: u64,
    ) {
        if self.docs.remove(&path).is_some() {
            self.dead += 1;
        }
        if self.next_id == u32::MAX {
            self.clear();
        }
        let id = self.next_id;
        self.next_id += 1;
        for trigram in trigrams {
            self.postings.entry(*trigram).or_default().push(id);
        }
        let racy = stamp.is_racy(observed_ns, self.quiet_period_ns);
        self.docs.insert(
            path,
            Doc {
                id,
                stamp,
                racy,
                seen: AtomicU64::new(walk),
            },
        );
        self.compact_if_sparse();
    }

    /// Drops the document of `path`, if any.
    pub(crate) fn remove(&mut self, path: &Path) {
        if self.docs.remove(path).is_some() {
            self.dead += 1;
            self.compact_if_sparse();
        }
    }

    /// Drops the documents under `root` that the walk numbered `walk` did not
    /// go through, and says how many there were.
    ///
    /// Only a walk that covered everything under `root` may sweep: a walk
    /// that stopped early, skipped files or filtered them did not see files
    /// that still exist.
    pub(crate) fn sweep(&mut self, root: &Path, walk: u64) -> usize {
        let before = self.docs.len();
        self.docs.retain(|path, doc| {
            !path.starts_with(root) || doc.seen.load(Ordering::Relaxed) == walk
        });
        let removed = before - self.docs.len();
        self.dead += removed;
        self.compact_if_sparse();
        removed
    }

    /// Forgets everything.
    pub(crate) fn clear(&mut self) {
        self.docs.clear();
        self.postings.clear();
        self.next_id = 0;
        self.dead = 0;
    }

    /// Rebuilds the posting lists without the ids of documents that are gone,
    /// when they are most of what the lists hold.
    fn compact_if_sparse(&mut self) {
        if self.dead < 64 || self.dead < self.docs.len() {
            return;
        }
        let live: HashSet<u32> = self.docs.values().map(|doc| doc.id).collect();
        self.postings.retain(|_, ids| {
            ids.retain(|id| live.contains(id));
            !ids.is_empty()
        });
        self.postings.shrink_to_fit();
        self.dead = 0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use regex_syntax::hir::{ClassUnicode, ClassUnicodeRange};

    fn folded(text: &str) -> Vec<u8> {
        let mut out = Vec::new();
        fold_into(text, &mut out);
        out
    }

    fn all_chars() -> impl Iterator<Item = char> {
        (0..=0x10FFFFu32).filter_map(char::from_u32)
    }

    /// The soundness of the whole index rests on this: characters the
    /// matcher treats as the same letter fold to the same bytes.
    #[test]
    fn characters_the_matcher_treats_as_one_letter_fold_alike() {
        let mut failures = Vec::new();
        for c in all_chars() {
            let own = folded(&c.to_string());
            // The literal search lowers each character and compares.
            let lower: String = c.to_lowercase().collect();
            let upper: String = c.to_uppercase().collect();
            if folded(&lower) != own || folded(&upper) != own {
                failures.push(format!("{c:?} lower/upper"));
            }
            // The regex engine's `(?i)` relates a character to its simple
            // case-folding orbit.
            let mut class = ClassUnicode::new([ClassUnicodeRange::new(c, c)]);
            class.case_fold_simple();
            for range in class.iter() {
                for d in range.start()..=range.end() {
                    if folded(&d.to_string()) != own {
                        failures.push(format!("{c:?} ~ {d:?}"));
                    }
                }
            }
        }
        assert!(
            failures.is_empty(),
            "{} characters fold apart from a case variant: {:?}",
            failures.len(),
            &failures[..failures.len().min(10)]
        );
    }

    #[test]
    fn the_fold_keeps_ascii_one_byte_per_character_and_lowers_it() {
        assert_eq!(folded("AbC_9"), b"abc_9");
    }

    /// The tokenizer is part of what an index means. If this fails, the
    /// fold or the line splitting changed: bump `TOKENIZER_VERSION`, then
    /// update the expectation.
    #[test]
    fn the_tokenizer_is_pinned() {
        assert_eq!(TOKENIZER_VERSION, 1);
        assert_eq!(folded("Straße ΣΑΣ Kelvin"), "strasse σασ kelvin".as_bytes());
        let keys = trigrams_of("AbcD\r\nxyz\n");
        let key = |s: &[u8; 3]| trigram_key(s);
        let mut expected = vec![key(b"abc"), key(b"bcd"), key(b"xyz")];
        expected.sort_unstable();
        assert_eq!(
            keys, expected,
            "lines are split like the matcher does and no trigram crosses one"
        );
    }

    #[test]
    fn a_literal_plan_is_the_trigrams_of_the_folded_pattern() {
        let TrigramPlan::Narrow(plan) = plan_literal("Hello") else {
            panic!("a five byte literal is narrowable");
        };
        assert_eq!(plan.len(), 1);
        assert_eq!(plan[0], trigrams_of("hello"));
    }

    #[test]
    fn a_literal_shorter_than_a_trigram_is_unusable_and_says_why() {
        assert_eq!(
            plan_literal("ab"),
            TrigramPlan::Unusable("the pattern is shorter than 3 bytes")
        );
        assert!(matches!(plan_literal(""), TrigramPlan::Unusable(_)));
    }

    #[test]
    fn a_regex_with_alternatives_plans_one_alternative_per_literal() {
        let TrigramPlan::Narrow(plan) = plan_regex("foo|bar") else {
            panic!("two literals are narrowable");
        };
        assert_eq!(plan.len(), 2);
    }

    #[test]
    fn a_regex_whose_literals_are_all_too_short_is_unusable() {
        assert_eq!(
            plan_regex(r"fn\s+[a-z_]+\("),
            TrigramPlan::Unusable(NO_USABLE_LITERAL)
        );
    }

    #[test]
    fn a_regex_with_a_long_literal_prefix_is_narrowed_even_when_the_rest_is_open() {
        let TrigramPlan::Narrow(plan) = plan_regex(r"parse_[a-z]+\d*") else {
            panic!("`parse_` is a literal prefix");
        };
        assert_eq!(plan.len(), 1);
    }

    #[test]
    fn a_case_insensitive_literal_inside_a_regex_plans_one_alternative() {
        let TrigramPlan::Narrow(plan) = plan_regex("(?i)hello") else {
            panic!("a literal in a case-insensitive group is narrowable");
        };
        assert_eq!(plan.len(), 1, "the case variants fold to one alternative");
    }

    #[test]
    fn a_regex_without_a_useful_literal_is_unusable_and_says_why() {
        for pattern in [r"\w+", r"[a-z]+", r".*", r"a|b", r"\d{3}", "x"] {
            assert_eq!(
                plan_regex(pattern),
                TrigramPlan::Unusable(NO_USABLE_LITERAL),
                "{pattern}"
            );
        }
    }

    #[test]
    fn an_invalid_regex_is_unusable() {
        assert!(matches!(plan_regex("("), TrigramPlan::Unusable(_)));
    }

    fn stamp(len: u64, ns: i128) -> FileStamp {
        FileStamp {
            len,
            modified_ns: ns,
            changed_ns: ns,
        }
    }

    const DAY_NS: i128 = 86_400_000_000_000;
    const OLD: i128 = 1_000 * DAY_NS;
    const NOW: i128 = 2_000 * DAY_NS;

    fn plan(pattern: &str) -> Vec<Vec<u32>> {
        match plan_literal(pattern) {
            TrigramPlan::Narrow(plan) => plan,
            TrigramPlan::Unusable(reason) => panic!("{reason}"),
        }
    }

    #[test]
    fn a_file_the_index_never_saw_is_unknown() {
        let index = TextIndex::default();
        let verdict = index.judge(Path::new("/a"), &stamp(1, OLD), &plan("needle"), 1);
        assert_eq!(verdict, GateVerdict::Unknown);
    }

    #[test]
    fn an_indexed_file_is_a_candidate_only_for_what_it_holds() {
        let mut index = TextIndex::default();
        index.upsert(
            "/a".into(),
            stamp(5, OLD),
            NOW,
            &trigrams_of("find the Needle here"),
            1,
        );
        let at = |p: &str| index.judge(Path::new("/a"), &stamp(5, OLD), &plan(p), 1);
        assert_eq!(at("needle"), GateVerdict::Candidate);
        assert_eq!(at("NEEDLE"), GateVerdict::Candidate);
        assert_eq!(at("haystack"), GateVerdict::NotCandidate);
    }

    #[test]
    fn a_changed_stamp_makes_the_file_unknown_again() {
        let mut index = TextIndex::default();
        index.upsert("/a".into(), stamp(5, OLD), NOW, &trigrams_of("needle"), 1);
        for changed in [stamp(6, OLD), stamp(5, OLD + 1)] {
            assert_eq!(
                index.judge(Path::new("/a"), &changed, &plan("needle"), 1),
                GateVerdict::Unknown
            );
        }
    }

    #[test]
    fn a_file_indexed_right_after_it_changed_is_not_trusted() {
        let mut index = TextIndex::default();
        let fresh = stamp(5, NOW - 1_000_000);
        index.upsert("/a".into(), fresh, NOW, &trigrams_of("needle"), 1);
        assert_eq!(
            index.judge(Path::new("/a"), &fresh, &plan("needle"), 1),
            GateVerdict::Unknown,
            "a write within the clock's resolution would not move the stamp"
        );
        // Indexed again once it has been quiet for longer than the window.
        index.upsert(
            "/a".into(),
            fresh,
            NOW + 3_000_000_000,
            &trigrams_of("needle"),
            2,
        );
        assert_eq!(
            index.judge(Path::new("/a"), &fresh, &plan("needle"), 2),
            GateVerdict::Candidate
        );
    }

    #[test]
    fn replacing_a_document_forgets_what_the_old_one_held() {
        let mut index = TextIndex::default();
        index.upsert("/a".into(), stamp(5, OLD), NOW, &trigrams_of("alpha"), 1);
        index.upsert("/a".into(), stamp(6, OLD), NOW, &trigrams_of("omega"), 2);
        let now = stamp(6, OLD);
        assert_eq!(
            index.judge(Path::new("/a"), &now, &plan("alpha"), 2),
            GateVerdict::NotCandidate
        );
        assert_eq!(
            index.judge(Path::new("/a"), &now, &plan("omega"), 2),
            GateVerdict::Candidate
        );
    }

    #[test]
    fn a_removed_document_is_unknown_and_its_postings_are_compacted() {
        let mut index = TextIndex::default();
        for n in 0..200 {
            index.upsert(
                format!("/f{n}").into(),
                stamp(5, OLD),
                NOW,
                &trigrams_of("shared words"),
                1,
            );
        }
        // Compaction runs once the gone documents are as many as the live ones.
        for n in 0..100 {
            index.remove(Path::new(&format!("/f{n}")));
        }
        assert_eq!(index.len(), 100);
        assert_eq!(index.dead, 0, "compaction ran");
        let ids: usize = index.postings.values().map(Vec::len).max().unwrap();
        assert_eq!(ids, 100, "the lists keep only live documents");
        assert_eq!(
            index.judge(Path::new("/f0"), &stamp(5, OLD), &plan("shared"), 1),
            GateVerdict::Unknown
        );
    }

    #[test]
    fn a_sweep_drops_what_the_walk_did_not_go_through_under_the_root() {
        let mut index = TextIndex::default();
        // Indexed by an earlier walk than the one that sweeps.
        for path in ["/root/kept", "/root/gone", "/elsewhere/untouched"] {
            index.upsert(path.into(), stamp(5, OLD), NOW, &trigrams_of("needle"), 0);
        }
        let walk = index.begin_walk();
        assert_eq!(
            index.judge(
                Path::new("/root/kept"),
                &stamp(5, OLD),
                &plan("needle"),
                walk
            ),
            GateVerdict::Candidate
        );
        assert_eq!(index.sweep(Path::new("/root"), walk), 1);
        assert_eq!(index.len(), 2);
        assert_eq!(
            index.judge(
                Path::new("/root/gone"),
                &stamp(5, OLD),
                &plan("needle"),
                walk
            ),
            GateVerdict::Unknown
        );
        assert_ne!(
            index.judge(
                Path::new("/elsewhere/untouched"),
                &stamp(5, OLD),
                &plan("needle"),
                walk
            ),
            GateVerdict::Unknown,
            "what is outside the walk's root is not the walk's to forget"
        );
    }

    #[test]
    fn the_secret_names_are_excluded_and_ordinary_ones_are_not() {
        let config = TextIndexConfig::default();
        for name in [
            ".env",
            ".env.production",
            "server.PEM",
            "tls.key",
            "id_rsa",
            "id_rsa.pub",
            "credentials.json",
            "Secrets.yaml",
        ] {
            assert!(config.excludes(Path::new(&format!("/p/{name}"))), "{name}");
        }
        for name in [
            "main.rs",
            "keyboard.rs",
            "environment.md",
            "README.md",
            "monkey.txt",
        ] {
            assert!(!config.excludes(Path::new(&format!("/p/{name}"))), "{name}");
        }
    }

    #[test]
    fn an_empty_exclusion_list_excludes_nothing() {
        let config = TextIndexConfig {
            excluded_names: Vec::new(),
            ..TextIndexConfig::default()
        };
        assert!(!config.excludes(Path::new("/p/.env")));
    }

    #[test]
    fn ids_that_run_out_clear_the_index_instead_of_wrapping() {
        let mut index = TextIndex::default();
        index.upsert("/a".into(), stamp(5, OLD), NOW, &trigrams_of("needle"), 1);
        index.next_id = u32::MAX;
        index.upsert("/b".into(), stamp(5, OLD), NOW, &trigrams_of("needle"), 1);
        assert_eq!(index.len(), 1);
        assert_eq!(
            index.judge(Path::new("/a"), &stamp(5, OLD), &plan("needle"), 1),
            GateVerdict::Unknown
        );
        assert_eq!(
            index.judge(Path::new("/b"), &stamp(5, OLD), &plan("needle"), 1),
            GateVerdict::Candidate
        );
    }
}
