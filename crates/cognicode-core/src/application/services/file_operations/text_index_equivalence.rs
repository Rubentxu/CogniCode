//! A content search with the text index answers what one without it answers.
//!
//! Every test runs the same tree through two services, one with the index
//! and one without, and compares what they say: the matches with their
//! columns and context, the number of files scanned, and the holes. The
//! index may only change how much is read.

use super::*;
use crate::application::dto::TextIndexReport;
use crate::application::ports::SyntaxReport;
use crate::domain::aggregates::Symbol;
use crate::domain::traits::{CodeVerifier, CodeVerifierError, CompilationResult};
use std::path::PathBuf;
use std::time::Duration;
use tempfile::TempDir;

/// A policy that lets every path through: the searches under test are about
/// what the walk reads, not about which paths a workspace admits.
struct OpenPolicy;

impl PathPolicy for OpenPolicy {
    fn validate_path(&self, _path: &Path) -> Result<(), PathPolicyError> {
        Ok(())
    }

    fn resolve_for_creation(&self, path: &Path) -> Result<PathBuf, PathPolicyError> {
        Ok(path.to_path_buf())
    }
}

/// A verifier no content search ever calls.
struct UnusedVerifier;

#[async_trait::async_trait]
impl CodeVerifier for UnusedVerifier {
    fn verify(&self, _path: &str) -> Result<CompilationResult, CodeVerifierError> {
        Err(CodeVerifierError::ToolchainUnavailable("none".to_string()))
    }

    async fn verify_with_timeout(
        &self,
        _path: &str,
        _timeout_secs: u64,
    ) -> Result<CompilationResult, CodeVerifierError> {
        Err(CodeVerifierError::ToolchainUnavailable("none".to_string()))
    }
}

/// A syntax analysis no content search ever calls.
struct UnusedSyntax;

impl SyntaxAnalysis for UnusedSyntax {
    fn find_all_symbols(&self, _language: &str, _source: &str) -> AppResult<Vec<Symbol>> {
        Ok(Vec::new())
    }

    fn parses_cleanly(&self, _language: &str, _source: &str) -> AppResult<SyntaxReport> {
        Ok(SyntaxReport { clean: true })
    }
}

fn service_over(root: &Path, index: Option<TextIndexConfig>) -> FileOperationsService {
    let service = FileOperationsService::new(
        root.to_string_lossy().to_string(),
        Arc::new(OpenPolicy),
        Arc::new(UnusedVerifier),
        Arc::new(UnusedSyntax),
    );
    match index {
        Some(config) => service.with_text_index(config),
        None => service,
    }
}

/// The index trusts a stamp as soon as the file is older than the moment it
/// was indexed, so the tests do not have to wait for the file system's clock.
fn eager() -> TextIndexConfig {
    TextIndexConfig {
        quiet_period: Duration::ZERO,
        ..TextIndexConfig::default()
    }
}

/// A service without the index and one with it, over the same root.
struct Pair {
    plain: FileOperationsService,
    indexed: FileOperationsService,
}

impl Pair {
    fn over(root: &Path, config: TextIndexConfig) -> Self {
        Self {
            plain: service_over(root, None),
            indexed: service_over(root, Some(config)),
        }
    }

    /// Runs `request` through both and fails if they differ; returns what the
    /// index reported.
    fn same(&self, request: &SearchContentRequest) -> TextIndexReport {
        let plain = self.plain.search_content(request.clone()).expect("plain");
        let indexed = self
            .indexed
            .search_content(request.clone())
            .expect("indexed");
        let show = |result: &SearchContentResult| {
            serde_json::json!({
                "matches": result.matches,
                "total": result.total,
                "files_scanned": result.files_scanned,
                "unread": result.unread,
            })
        };
        assert_eq!(
            show(&indexed),
            show(&plain),
            "the index changed the answer of {request:?}"
        );
        assert!(plain.text_index.is_none());
        indexed.text_index.expect("an indexed service reports")
    }

    fn documents(&self) -> usize {
        self.indexed.text_index.as_ref().expect("index").documents()
    }
}

fn request(pattern: &str, regex: bool, case_insensitive: bool) -> SearchContentRequest {
    SearchContentRequest {
        pattern: pattern.to_string(),
        regex: Some(regex),
        case_insensitive: Some(case_insensitive),
        max_results: Some(10_000),
        context_lines: Some(1),
        ..SearchContentRequest::new(pattern)
    }
}

fn write(root: &Path, name: &str, content: &str) {
    let path = root.join(name);
    fs::create_dir_all(path.parent().expect("parent")).expect("directory");
    fs::write(path, content).expect("written");
}

const WORDS: &[&str] = &[
    "alpha",
    "beta",
    "gamma",
    "delta",
    "needle",
    "Needle",
    "NEEDLE",
    "parse_value",
    "fn main() {}",
    "café needle é",
    "Straße",
    "ΣΑΣ ας",
    "İstanbul needle",
    "\u{212A}elvin",
    "kelvin",
    "aaa",
    "a.c",
    "x\ty",
];

/// A tree of forty files in four directories, made of a small vocabulary so
/// that most queries match some files and miss others.
fn tree() -> TempDir {
    let dir = TempDir::new().expect("tempdir");
    let mut seed: u32 = 0x2545_F491;
    let mut next = move || {
        seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        (seed >> 8) as usize
    };
    for file in 0..40 {
        let mut text = String::new();
        for _ in 0..(5 + next() % 20) {
            for _ in 0..(1 + next() % 3) {
                text.push_str(WORDS[next() % WORDS.len()]);
                text.push(' ');
            }
            text.push_str(if next() % 5 == 0 { "\r\n" } else { "\n" });
        }
        write(dir.path(), &format!("d{}/f{file}.txt", file % 4), &text);
    }
    dir
}

const QUERIES: &[(&str, bool, bool)] = &[
    ("needle", false, false),
    ("needle", false, true),
    ("NEEDLE", false, true),
    ("needle", true, false),
    ("(?i)needle|alpha", true, false),
    (r"parse_\w+", true, false),
    ("ne{2}dle", true, false),
    (r"\w+", true, false),
    ("ab", false, false),
    ("café", false, true),
    ("é", false, false),
    ("straße", false, true),
    ("σας", false, true),
    ("\u{212A}elvin", false, true),
    ("kelvin", false, true),
    ("kelvin", true, true),
    ("İstanbul", false, true),
    ("istanbul", false, true),
    ("zzzz_absent", false, false),
    ("a.c", true, false),
    ("a.c", false, false),
    ("aaa", false, false),
    ("x\ty", false, false),
    ("^fn main", true, false),
    ("(alpha|beta)gamma", true, false),
    ("alpha beta", false, false),
];

#[test]
fn every_query_answers_the_same_cold_and_warm() {
    let dir = tree();
    let pair = Pair::over(dir.path(), eager());
    let mut skipped_somewhere = false;
    for (pattern, regex, insensitive) in QUERIES {
        let query = request(pattern, *regex, *insensitive);
        // Cold: the index is built while this runs. Then warm, twice.
        for pass in 0..3 {
            let report = pair.same(&query);
            if pass > 0 && report.used {
                skipped_somewhere |= report.skipped > 0;
            }
        }
    }
    assert!(
        skipped_somewhere,
        "a warm index must have skipped reads for some query"
    );
}

#[test]
fn a_warm_index_reads_only_the_files_that_can_match() {
    let dir = tree();
    let pair = Pair::over(dir.path(), eager());
    let query = request("zzzz_absent", false, false);
    let cold = pair.same(&query);
    assert!(cold.used);
    assert_eq!((cold.skipped, cold.indexed_now), (0, 40));
    let warm = pair.same(&query);
    assert_eq!(
        (warm.skipped, warm.candidates, warm.indexed_now),
        (40, 0, 0)
    );
}

#[test]
fn an_index_that_cannot_narrow_the_query_says_why_and_reads_everything() {
    let dir = tree();
    let pair = Pair::over(dir.path(), eager());
    for (pattern, regex) in [(r"\w+", true), ("ab", false)] {
        let report = pair.same(&request(pattern, regex, false));
        assert!(!report.used, "{pattern}");
        assert!(report.reason.is_some(), "{pattern}");
        assert_eq!(
            (report.skipped, report.candidates, report.indexed_now),
            (0, 0, 0)
        );
    }
    assert_eq!(pair.documents(), 0, "a fallback search builds nothing");
}

#[test]
fn a_file_rewritten_with_the_same_length_and_timestamp_is_read_again() {
    let dir = tree();
    let pair = Pair::over(dir.path(), eager());
    let target = dir.path().join("d0/f0.txt");
    let old = fs::read_to_string(&target).expect("read");
    let query = request("zzzz_absent", false, false);
    pair.same(&query);
    assert_eq!(pair.same(&query).skipped, 40);

    // Same length, same modification time: only the status-change time
    // tells the rewrite apart.
    let modified = fs::metadata(&target)
        .expect("metadata")
        .modified()
        .expect("mtime");
    let mut rewritten = String::from("zzzz_absent");
    rewritten.push_str(&" ".repeat(old.len() - rewritten.len()));
    fs::write(&target, &rewritten).expect("rewrite");
    fs::File::options()
        .write(true)
        .open(&target)
        .expect("open")
        .set_modified(modified)
        .expect("restore mtime");

    let report = pair.same(&query);
    assert_eq!(report.skipped, 39);
    assert_eq!(
        report.indexed_now, 1,
        "the rewritten file was indexed again"
    );
    let found = pair
        .indexed
        .search_content(query.clone())
        .expect("search")
        .matches;
    assert_eq!(found.len(), 1, "the match comes from the file as it is now");
}

#[test]
fn a_file_changed_within_the_quiet_period_is_read_and_never_indexed() {
    let dir = tree();
    // The default quiet period: a file this fresh is never vouched for.
    let pair = Pair::over(dir.path(), TextIndexConfig::default());
    let query = request("zzzz_absent", false, false);
    pair.same(&query);
    let target = dir.path().join("d1/f1.txt");
    let len = fs::metadata(&target).expect("metadata").len() as usize;
    let mut rewritten = String::from("zzzz_absent");
    rewritten.push_str(&" ".repeat(len - rewritten.len()));
    fs::write(&target, rewritten).expect("rewrite");
    let report = pair.same(&query);
    assert_eq!(report.skipped, 0, "everything is still too fresh to trust");
    assert_eq!(
        report.indexed_now, 0,
        "and nothing is indexed that cannot be used"
    );
    assert_eq!(pair.documents(), 0);
    assert_eq!(
        pair.indexed
            .search_content(query)
            .expect("search")
            .matches
            .len(),
        1
    );
}

#[test]
fn a_removed_file_leaves_the_index_after_a_complete_walk() {
    let dir = tree();
    let pair = Pair::over(dir.path(), eager());
    let query = request("needle", false, false);
    pair.same(&query);
    assert_eq!(pair.documents(), 40);
    fs::remove_file(dir.path().join("d2/f2.txt")).expect("remove");
    pair.same(&query);
    assert_eq!(pair.documents(), 39);
}

#[test]
fn a_search_that_filters_files_does_not_forget_the_ones_it_did_not_visit() {
    let dir = tree();
    let pair = Pair::over(dir.path(), eager());
    pair.same(&request("needle", false, false));
    let mut query = request("needle", false, false);
    query.file_glob = Some("**/f1*.txt".to_string());
    pair.same(&query);
    assert_eq!(pair.documents(), 40);
    let mut truncated = request("needle", false, false);
    truncated.max_results = Some(1);
    pair.same(&truncated);
    assert_eq!(
        pair.documents(),
        40,
        "a walk that stopped early sweeps nothing"
    );
}

#[test]
fn the_result_limit_is_the_same_with_and_without_the_index() {
    let dir = tree();
    let pair = Pair::over(dir.path(), eager());
    for limit in [1, 2, 7] {
        let mut query = request("needle", false, false);
        query.max_results = Some(limit);
        for _ in 0..3 {
            pair.same(&query);
        }
    }
}

/// Pagination is the contract: `max_results = 0` is a typed refusal, not an
/// unbounded read (76432bff). The equivalence still holds on the refusal —
/// both services answer the same way whether the index exists or not — so a
/// zero limit must not smuggle a different answer through the indexed path.
#[test]
fn a_zero_result_limit_is_refused_the_same_way_with_and_without_the_index() {
    let dir = tree();
    let pair = Pair::over(dir.path(), eager());
    let mut query = request("needle", false, false);
    query.max_results = Some(0);
    let plain = pair
        .plain
        .search_content(query.clone())
        .expect_err("zero is not a page size");
    let indexed = pair
        .indexed
        .search_content(query)
        .expect_err("zero is not a page size with the index either");
    assert_eq!(plain.to_string(), indexed.to_string());
    assert!(
        plain.to_string().contains("max_results"),
        "the refusal names the parameter: {plain}"
    );
}

#[test]
fn a_secret_named_file_is_searched_and_never_indexed() {
    let dir = tree();
    write(dir.path(), "config/server.pem", "the needle is a secret\n");
    write(dir.path(), "config/credentials.json", "{\"needle\": 1}\n");
    let pair = Pair::over(dir.path(), eager());
    let query = request("needle", false, false);
    for _ in 0..3 {
        let report = pair.same(&query);
        assert_eq!(
            report.read_directly, 2,
            "the two secret names are read directly"
        );
    }
    assert_eq!(pair.documents(), 40, "nothing was derived from them");
    let found = pair.indexed.search_content(query).expect("search").matches;
    assert!(found.iter().any(|m| m.file.ends_with("server.pem")));
}

#[cfg(unix)]
#[test]
fn a_link_that_leaves_the_root_is_declined_and_not_indexed() {
    let dir = tree();
    let outside = TempDir::new().expect("outside");
    write(outside.path(), "elsewhere.txt", "needle outside the root\n");
    std::os::unix::fs::symlink(
        outside.path().join("elsewhere.txt"),
        dir.path().join("d0/link.txt"),
    )
    .expect("symlink");
    let pair = Pair::over(dir.path(), eager());
    let query = request("needle", false, false);
    for _ in 0..3 {
        pair.same(&query);
    }
    assert_eq!(pair.documents(), 40);
    let result = pair.indexed.search_content(query).expect("search");
    assert!(result.matches.iter().all(|m| !m.file.ends_with("link.txt")));
    assert_eq!(result.unread.len(), 1, "the declined link is a hole");
}

#[cfg(unix)]
#[test]
fn a_file_that_became_unreadable_is_a_hole_with_the_index_too() {
    use std::os::unix::fs::PermissionsExt;
    let dir = tree();
    let pair = Pair::over(dir.path(), eager());
    let query = request("zzzz_absent", false, false);
    pair.same(&query);
    assert_eq!(pair.same(&query).skipped, 40);
    let target = dir.path().join("d3/f3.txt");
    fs::set_permissions(&target, fs::Permissions::from_mode(0o000)).expect("chmod");
    if fs::File::open(&target).is_ok() {
        // Running with a privilege that reads it anyway: nothing to see.
        return;
    }
    pair.same(&query);
    let result = pair.indexed.search_content(query).expect("search");
    assert_eq!(result.unread.len(), 1);
    fs::set_permissions(&target, fs::Permissions::from_mode(0o644)).expect("restore");
}

#[test]
fn a_file_that_is_not_text_is_scanned_and_not_read_again() {
    let dir = tree();
    fs::write(
        dir.path().join("d0/blob.dat"),
        [0xff, 0xfe, 0x00, 0x9f, 0x92],
    )
    .expect("blob");
    let pair = Pair::over(dir.path(), eager());
    let query = request("needle", false, false);
    for _ in 0..3 {
        pair.same(&query);
    }
    assert_eq!(pair.documents(), 41);
}

#[test]
fn the_report_carries_the_tokenizer_version() {
    let dir = tree();
    let pair = Pair::over(dir.path(), eager());
    let report = pair.same(&request("needle", false, false));
    assert_eq!(
        report.tokenizer_version,
        crate::application::services::text_index::TOKENIZER_VERSION
    );
}
