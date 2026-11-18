//! From the bytes of a change to the symbols it touches.
//!
//! This is the seed of a change pack: the exact lines that changed in each
//! file, and the symbols of the frozen snapshot whose declaration those lines
//! fall in. It decides nothing about what the change *affects*; relating the
//! seeds to consumers and tests is the graph's job and happens afterwards.
//!
//! ## Honest states
//!
//! A file only seeds symbols when the bytes of its head side are the bytes
//! the snapshot describes. Otherwise the positions of the graph would not
//! belong to the lines that changed, and mixing the two would answer about a
//! revision nobody asked about. The file is listed instead, with the reason:
//!
//! - `snapshot_differs`: the head side is not the version the graph holds;
//! - `not_in_snapshot`: the graph has no file by that path (not a source
//!   file, ignored, or outside the workspace);
//! - `unknown_change`: the SCM kernel could not classify the change (a merge
//!   conflict, for one);
//! - `unreadable`: a side is missing, too large or not UTF-8 text;
//! - `deleted`: the file is gone, so the snapshot holds nothing of it.
//!
//! An unmapped file is *unknown*, never "no impact".

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use super::input::{ChangeBytes, Content, FileDelta, SourceChange};
use super::line_diff::{Hunk, line_diff};
use crate::application::context_pack::ports::{DeclarationReader, DeclarationSpan, SnapshotView};
use crate::application::context_pack::service::ItemRange;
use crate::domain::aggregates::call_graph::SymbolId;
use crate::domain::findings::digest::sha256_hex;
use crate::domain::value_objects::{SymbolKind, published_line};

/// How a seed changed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SeedKind {
    /// The symbol is new: its file is new.
    Added,
    /// A changed line falls in the declaration up to where its body starts.
    Signature,
    /// Only lines of the body changed.
    Body,
}

impl SeedKind {
    /// The word the answer publishes.
    pub fn code(self) -> &'static str {
        match self {
            Self::Added => "added",
            Self::Signature => "signature",
            Self::Body => "body",
        }
    }
}

/// A symbol whose declaration holds changed lines.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SeedSymbol {
    /// The identity of the symbol in the snapshot.
    pub id: SymbolId,
    /// The file that declares it, relative to the snapshot root.
    pub file: String,
    /// How it changed.
    pub kind: SeedKind,
    /// The lines of the declaration.
    pub declaration: ItemRange,
    /// The changed lines that fall inside it.
    pub changed: Vec<ItemRange>,
}

/// One changed file of the answer.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct ChangedFile {
    /// The path of the head side.
    pub path: String,
    /// `added`, `deleted`, `modified`, `renamed`, `copied` or `unknown`.
    pub change: String,
    /// The path before a rename or a copy.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub from: Option<String>,
    /// `mapped` when the lines were derived and tied to the snapshot,
    /// `unchanged` when both sides hold the same lines, `unmapped` when the
    /// file could not be tied to the snapshot.
    pub state: String,
    /// Why the file is unmapped.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    /// What the reason says about this file.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
    /// The lines of the head side that were added or rewritten, counted from
    /// one, both ends included.
    pub changed_lines: Vec<ItemRange>,
    /// How many lines of the base side are gone.
    pub removed_lines: u32,
    /// How many hunks touch no symbol of the snapshot: their consumers are
    /// not known.
    pub outside_symbols: u32,
    /// The ids of the symbols the lines fall in.
    pub symbols: Vec<String>,
    /// Whether the changed lines are the minimal diff; `false` when they are
    /// a superset.
    pub exact: bool,
}

/// What the bytes of a change say.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChangeSeed {
    /// Every file of the change, in path order.
    pub files: Vec<ChangedFile>,
    /// The symbols the changed lines fall in, in file and line order.
    pub seeds: Vec<SeedSymbol>,
    /// How many changed files were past the bound and not read.
    pub files_not_read: usize,
}

/// The seed of `input` over the frozen `snapshot`.
pub fn derive(
    input: &ChangeBytes,
    snapshot: &SnapshotView,
    declarations: &dyn DeclarationReader,
) -> ChangeSeed {
    let wanted: BTreeSet<&str> = input.files.iter().map(|f| f.path.as_str()).collect();
    let declared = symbols_by_file(snapshot, &wanted);
    let mut files = Vec::new();
    let mut seeds = Vec::new();
    for change in &input.files {
        let symbols = declared.get(change.path.as_str());
        let (file, found) = read_change(change, snapshot, declarations, symbols);
        files.push(file);
        seeds.extend(found);
    }
    ChangeSeed {
        files,
        seeds,
        files_not_read: input.files_not_read,
    }
}

/// The symbols of the snapshot that live in the wanted files, by file.
fn symbols_by_file<'a>(
    snapshot: &'a SnapshotView,
    wanted: &BTreeSet<&str>,
) -> BTreeMap<String, Vec<(SymbolId, &'a crate::domain::aggregates::Symbol)>> {
    let mut by_file: BTreeMap<String, Vec<_>> = BTreeMap::new();
    for (id, symbol) in snapshot.graph.symbol_ids() {
        let relative = snapshot.relative(symbol.location().file());
        if wanted.contains(relative.as_str()) {
            by_file
                .entry(relative)
                .or_default()
                .push((id.clone(), symbol));
        }
    }
    for symbols in by_file.values_mut() {
        symbols.sort_by(|a, b| a.0.cmp(&b.0));
    }
    by_file
}

fn change_word(change: &FileDelta) -> (&'static str, Option<String>) {
    match change {
        FileDelta::Added => ("added", None),
        FileDelta::Deleted => ("deleted", None),
        FileDelta::Modified => ("modified", None),
        FileDelta::Renamed { from } => ("renamed", Some(from.clone())),
        FileDelta::Copied { from } => ("copied", Some(from.clone())),
        FileDelta::Unknown(_) => ("unknown", None),
    }
}

fn unmapped(
    change: &SourceChange,
    reason: &str,
    detail: Option<String>,
) -> (ChangedFile, Vec<SeedSymbol>) {
    let (word, from) = change_word(&change.change);
    (
        ChangedFile {
            path: change.path.clone(),
            change: word.to_string(),
            from,
            state: "unmapped".to_string(),
            reason: Some(reason.to_string()),
            detail,
            changed_lines: Vec::new(),
            removed_lines: 0,
            outside_symbols: 0,
            symbols: Vec::new(),
            exact: true,
        },
        Vec::new(),
    )
}

/// The text of one side, or why it cannot be used.
fn text_of(side: &Content, which: &str) -> Result<String, String> {
    match side {
        Content::Absent => Ok(String::new()),
        Content::Unreadable(why) => Err(format!("{which} side: {why}")),
        Content::Bytes(bytes) => {
            String::from_utf8(bytes.clone()).map_err(|_| format!("{which} side is not UTF-8 text"))
        }
    }
}

fn read_change(
    change: &SourceChange,
    snapshot: &SnapshotView,
    declarations: &dyn DeclarationReader,
    symbols: Option<&Vec<(SymbolId, &crate::domain::aggregates::Symbol)>>,
) -> (ChangedFile, Vec<SeedSymbol>) {
    if let FileDelta::Unknown(why) = &change.change {
        return unmapped(change, "unknown_change", Some(why.clone()));
    }
    if matches!(change.change, FileDelta::Deleted) {
        return unmapped(
            change,
            "deleted",
            Some("the file is gone: the snapshot holds nothing of it".to_string()),
        );
    }
    let Content::Bytes(after_bytes) = &change.after else {
        let detail = match &change.after {
            Content::Unreadable(why) => format!("head side: {why}"),
            _ => "head side is missing".to_string(),
        };
        return unmapped(change, "unreadable", Some(detail));
    };
    let Some(recorded) = snapshot.files.get(&change.path) else {
        return unmapped(
            change,
            "not_in_snapshot",
            Some("the graph holds no file by this path".to_string()),
        );
    };
    if sha256_hex(after_bytes) != *recorded {
        return unmapped(
            change,
            "snapshot_differs",
            Some(
                "the head side is not the version of the file the graph describes; \
                 check out the head or rebuild the graph"
                    .to_string(),
            ),
        );
    }
    let (before, after) = match (
        text_of(&change.before, "base"),
        text_of(&change.after, "head"),
    ) {
        (Ok(before), Ok(after)) => (before, after),
        (Err(why), _) | (_, Err(why)) => return unmapped(change, "unreadable", Some(why)),
    };

    let diff = line_diff(&before, &after);
    let (word, from) = change_word(&change.change);
    let mut file = ChangedFile {
        path: change.path.clone(),
        change: word.to_string(),
        from,
        state: if diff.hunks.is_empty() {
            "unchanged"
        } else {
            "mapped"
        }
        .to_string(),
        reason: None,
        detail: None,
        changed_lines: diff
            .hunks
            .iter()
            .filter_map(Hunk::added_lines)
            .map(|(start_line, end_line)| ItemRange {
                start_line,
                end_line,
            })
            .collect(),
        removed_lines: diff.hunks.iter().map(|hunk| hunk.removed.len).sum(),
        outside_symbols: 0,
        symbols: Vec::new(),
        exact: diff.exact,
    };
    let declared = declared_spans(
        change,
        &after,
        declarations,
        symbols.map(Vec::as_slice).unwrap_or_default(),
    );
    let added_file = matches!(change.change, FileDelta::Added);
    let (seeds, outside) = seeds_of(&change.path, &declared, &diff.hunks, added_file);
    file.outside_symbols = outside;
    file.symbols = seeds.iter().map(|seed| seed.id.to_string()).collect();
    (file, seeds)
}

/// A symbol with the lines of its declaration.
struct Declared {
    id: SymbolId,
    start: u32,
    end: u32,
    header_end: u32,
}

/// Whether a symbol is a declaration a change can be attributed to.
///
/// Parameters, generics and imports are parts of another declaration. A
/// variable is only its own declaration when its span starts on its own line:
/// a local binding is given the span of the function that holds it, and
/// seeding it would answer about a name nobody can depend on.
fn is_declaration(
    symbol: &crate::domain::aggregates::Symbol,
    span: Option<&DeclarationSpan>,
) -> bool {
    match symbol.kind() {
        SymbolKind::Parameter | SymbolKind::Generic | SymbolKind::Import => false,
        SymbolKind::Variable => {
            span.is_none_or(|span| span.start_line == published_line(symbol.location().line()))
        }
        _ => true,
    }
}

fn declared_spans(
    change: &SourceChange,
    after: &str,
    declarations: &dyn DeclarationReader,
    symbols: &[(SymbolId, &crate::domain::aggregates::Symbol)],
) -> Vec<Declared> {
    symbols
        .iter()
        .filter_map(|(id, symbol)| {
            let location = symbol.location();
            let span =
                declarations.declaration(&change.path, after, location.line(), location.column());
            if !is_declaration(symbol, span.as_ref()) {
                return None;
            }
            Some(match span {
                Some(span) => Declared {
                    id: id.clone(),
                    start: span.start_line,
                    end: span.end_line,
                    header_end: span.start_line
                        + span.header.trim_end().matches('\n').count() as u32,
                },
                None => {
                    let line = published_line(location.line());
                    Declared {
                        id: id.clone(),
                        start: line,
                        end: line,
                        header_end: line,
                    }
                }
            })
        })
        .collect()
}

/// The lines of the head side a hunk touches, as an inclusive range; a pure
/// deletion is the gap between two lines and has no lines of its own.
fn touches(declared: &Declared, hunk: &Hunk) -> bool {
    match hunk.added_lines() {
        Some((start, end)) => start <= declared.end && end >= declared.start,
        None => {
            let before_gap = hunk.added.at;
            declared.start <= before_gap && before_gap < declared.end
        }
    }
}

fn in_header(declared: &Declared, hunk: &Hunk) -> bool {
    match hunk.added_lines() {
        Some((start, _)) => start <= declared.header_end && hunk_end(hunk) >= declared.start,
        None => declared.start <= hunk.added.at && hunk.added.at < declared.header_end,
    }
}

fn hunk_end(hunk: &Hunk) -> u32 {
    hunk.added_lines().map_or(hunk.added.at, |(_, end)| end)
}

/// Whether `inner` sits inside `outer` and is not the same span.
fn nested(inner: &Declared, outer: &Declared) -> bool {
    inner.start >= outer.start
        && inner.end <= outer.end
        && (inner.start, inner.end) != (outer.start, outer.end)
}

fn seeds_of(
    file: &str,
    declared: &[Declared],
    hunks: &[Hunk],
    added_file: bool,
) -> (Vec<SeedSymbol>, u32) {
    let mut outside = 0;
    let mut touched: BTreeMap<usize, (SeedKind, Vec<ItemRange>)> = BTreeMap::new();
    for hunk in hunks {
        let hit: Vec<usize> = (0..declared.len())
            .filter(|&at| touches(&declared[at], hunk))
            .collect();
        let innermost: Vec<usize> = hit
            .iter()
            .copied()
            .filter(|&at| {
                !hit.iter()
                    .any(|&other| nested(&declared[other], &declared[at]))
            })
            .collect();
        if innermost.is_empty() {
            outside += 1;
        }
        for at in innermost {
            let kind = if added_file {
                SeedKind::Added
            } else if in_header(&declared[at], hunk) {
                SeedKind::Signature
            } else {
                SeedKind::Body
            };
            let entry = touched.entry(at).or_insert((kind, Vec::new()));
            if kind == SeedKind::Signature {
                entry.0 = SeedKind::Signature;
            }
            if let Some((start_line, end_line)) = hunk.added_lines() {
                entry.1.push(ItemRange {
                    start_line,
                    end_line,
                });
            }
        }
    }
    let mut seeds: Vec<SeedSymbol> = touched
        .into_iter()
        .map(|(at, (kind, changed))| SeedSymbol {
            id: declared[at].id.clone(),
            file: file.to_string(),
            kind,
            declaration: ItemRange {
                start_line: declared[at].start,
                end_line: declared[at].end,
            },
            changed,
        })
        .collect();
    seeds.sort_by(|a, b| {
        (a.declaration.start_line, a.id.as_str()).cmp(&(b.declaration.start_line, b.id.as_str()))
    });
    (seeds, outside)
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::*;
    use crate::application::context_pack::ports::DeclarationSpan;
    use crate::domain::aggregates::call_graph::CallGraph;
    use crate::domain::aggregates::symbol::Symbol;
    use crate::domain::value_objects::Location;

    /// The head side of `src/a.rs`: `parse` is lines 1-3, `run` lines 4-6.
    const HEAD: &str =
        "fn parse(input: &str) -> u32 {\n    1\n}\nfn run() {\n    parse(\"x\");\n}\n";

    /// Reads a declaration as the line of the symbol and the next two.
    struct ThreeLines;

    impl DeclarationReader for ThreeLines {
        fn declaration(
            &self,
            _file: &str,
            source: &str,
            line: u32,
            _column: u32,
        ) -> Option<DeclarationSpan> {
            Some(DeclarationSpan {
                start_line: line + 1,
                end_line: line + 3,
                header: source.lines().nth(line as usize)?.to_string(),
            })
        }
    }

    struct NoDeclarations;

    impl DeclarationReader for NoDeclarations {
        fn declaration(&self, _: &str, _: &str, _: u32, _: u32) -> Option<DeclarationSpan> {
            None
        }
    }

    fn snapshot() -> SnapshotView {
        let mut graph = CallGraph::new();
        for (name, line) in [("parse", 0), ("run", 3)] {
            graph.add_symbol(Symbol::new(
                name,
                SymbolKind::Function,
                Location::new("src/a.rs", line, 3),
            ));
        }
        let mut files = BTreeMap::new();
        files.insert("src/a.rs".to_string(), sha256_hex(HEAD.as_bytes()));
        SnapshotView {
            graph: Arc::new(graph),
            digest: "snapshot".to_string(),
            graph_source: None,
            root: std::path::PathBuf::new(),
            files,
        }
    }

    fn modified(before: &str, after: &str) -> SourceChange {
        SourceChange {
            path: "src/a.rs".to_string(),
            change: FileDelta::Modified,
            before: Content::Bytes(before.as_bytes().to_vec()),
            after: Content::Bytes(after.as_bytes().to_vec()),
        }
    }

    fn input(files: Vec<SourceChange>) -> ChangeBytes {
        ChangeBytes {
            base: "base".to_string(),
            head: "head".to_string(),
            files,
            files_not_read: 0,
        }
    }

    fn seed_of(change: SourceChange) -> ChangeSeed {
        derive(&input(vec![change]), &snapshot(), &ThreeLines)
    }

    fn names(seed: &ChangeSeed) -> Vec<(String, SeedKind)> {
        seed.seeds
            .iter()
            .map(|s| (s.id.as_str().split(':').nth(1).unwrap().to_string(), s.kind))
            .collect()
    }

    /// Gives every symbol the span of the function that holds it, as the
    /// reader does for a local binding.
    struct EnclosingFunction;

    impl DeclarationReader for EnclosingFunction {
        fn declaration(&self, _: &str, source: &str, _: u32, _: u32) -> Option<DeclarationSpan> {
            Some(DeclarationSpan {
                start_line: 1,
                end_line: 3,
                header: source.lines().next()?.to_string(),
            })
        }
    }

    #[test]
    fn a_local_binding_does_not_seed_next_to_the_function_that_holds_it() {
        let mut graph = CallGraph::new();
        graph.add_symbol(Symbol::new(
            "parse",
            SymbolKind::Function,
            Location::new("src/a.rs", 0, 3),
        ));
        graph.add_symbol(Symbol::new(
            "one",
            SymbolKind::Variable,
            Location::new("src/a.rs", 1, 8),
        ));
        graph.add_symbol(Symbol::new(
            "input",
            SymbolKind::Parameter,
            Location::new("src/a.rs", 0, 9),
        ));
        let mut view = snapshot();
        view.graph = Arc::new(graph);
        let before = HEAD.replace("input: &str", "input: &[u8]");

        let seed = derive(
            &input(vec![modified(&before, HEAD)]),
            &view,
            &EnclosingFunction,
        );

        assert_eq!(names(&seed), [("parse".to_string(), SeedKind::Signature)]);
    }

    #[test]
    fn a_change_inside_a_body_seeds_that_symbol_alone() {
        let before = HEAD.replace("    1\n", "    0\n");
        let seed = seed_of(modified(&before, HEAD));

        assert_eq!(names(&seed), [("parse".to_string(), SeedKind::Body)]);
        let file = &seed.files[0];
        assert_eq!(file.state, "mapped");
        assert_eq!(
            file.changed_lines,
            [ItemRange {
                start_line: 2,
                end_line: 2
            }]
        );
        assert_eq!(file.outside_symbols, 0);
        assert!(file.exact);
    }

    #[test]
    fn a_change_to_the_declaration_line_is_a_signature_change() {
        let before = HEAD.replace("input: &str", "input: &[u8]");
        let seed = seed_of(modified(&before, HEAD));

        assert_eq!(names(&seed), [("parse".to_string(), SeedKind::Signature)]);
    }

    #[test]
    fn a_signature_change_outranks_a_body_change_of_the_same_symbol() {
        let before = HEAD
            .replace("input: &str", "input: &[u8]")
            .replace("    1\n", "    0\n");
        let seed = seed_of(modified(&before, HEAD));

        assert_eq!(names(&seed), [("parse".to_string(), SeedKind::Signature)]);
        // The two adjacent lines are one hunk.
        assert_eq!(
            seed.seeds[0].changed,
            [ItemRange {
                start_line: 1,
                end_line: 2
            }]
        );
    }

    #[test]
    fn two_symbols_changed_are_two_seeds_in_line_order() {
        let before = HEAD.replace("    1\n", "    0\n").replace("\"x\"", "\"y\"");
        let seed = seed_of(modified(&before, HEAD));

        assert_eq!(
            names(&seed),
            [
                ("parse".to_string(), SeedKind::Body),
                ("run".to_string(), SeedKind::Body)
            ]
        );
    }

    #[test]
    fn lines_outside_every_symbol_are_counted_and_seed_nothing() {
        let before = format!("// header\n{HEAD}");
        let seed = seed_of(modified(&before, HEAD));

        assert!(seed.seeds.is_empty());
        assert_eq!(seed.files[0].state, "mapped");
        assert_eq!(seed.files[0].outside_symbols, 1);
        assert_eq!(seed.files[0].removed_lines, 1);
        assert!(seed.files[0].changed_lines.is_empty());
    }

    #[test]
    fn a_deleted_line_inside_a_body_still_seeds_the_symbol() {
        let before = HEAD.replace("    1\n", "    1\n    2\n");
        let seed = seed_of(modified(&before, HEAD));

        assert_eq!(names(&seed), [("parse".to_string(), SeedKind::Body)]);
        assert!(seed.seeds[0].changed.is_empty());
        assert_eq!(seed.files[0].removed_lines, 1);
    }

    #[test]
    fn an_added_file_seeds_its_symbols_as_added() {
        let seed = seed_of(SourceChange {
            path: "src/a.rs".to_string(),
            change: FileDelta::Added,
            before: Content::Absent,
            after: Content::Bytes(HEAD.as_bytes().to_vec()),
        });

        assert_eq!(
            names(&seed),
            [
                ("parse".to_string(), SeedKind::Added),
                ("run".to_string(), SeedKind::Added)
            ]
        );
    }

    #[test]
    fn a_head_that_is_not_the_snapshot_is_unmapped_not_guessed() {
        let other = HEAD.replace("    1\n", "    9\n");
        let seed = seed_of(modified(HEAD, &other));

        assert!(seed.seeds.is_empty());
        let file = &seed.files[0];
        assert_eq!(file.state, "unmapped");
        assert_eq!(file.reason.as_deref(), Some("snapshot_differs"));
        assert!(file.changed_lines.is_empty());
    }

    #[test]
    fn a_file_the_graph_does_not_hold_is_unmapped() {
        let mut change = modified("a\n", "b\n");
        change.path = "docs/notes.md".to_string();
        let seed = seed_of(change);

        assert_eq!(seed.files[0].reason.as_deref(), Some("not_in_snapshot"));
    }

    #[test]
    fn a_change_the_kernel_could_not_classify_is_unmapped() {
        let mut change = modified("a\n", HEAD);
        change.change = FileDelta::Unknown("conflict".to_string());
        let seed = seed_of(change);

        assert_eq!(seed.files[0].state, "unmapped");
        assert_eq!(seed.files[0].reason.as_deref(), Some("unknown_change"));
        assert_eq!(seed.files[0].detail.as_deref(), Some("conflict"));
    }

    #[test]
    fn a_deleted_file_is_listed_and_not_mapped() {
        let seed = seed_of(SourceChange {
            path: "src/a.rs".to_string(),
            change: FileDelta::Deleted,
            before: Content::Bytes(HEAD.as_bytes().to_vec()),
            after: Content::Absent,
        });

        assert_eq!(seed.files[0].reason.as_deref(), Some("deleted"));
        assert!(seed.seeds.is_empty());
    }

    #[test]
    fn an_unreadable_side_is_unmapped_with_its_reason() {
        let mut change = modified("a\n", HEAD);
        change.before = Content::Unreadable("larger than 2097152 bytes".to_string());
        let seed = seed_of(change);

        assert_eq!(seed.files[0].reason.as_deref(), Some("unreadable"));
        assert!(
            seed.files[0]
                .detail
                .as_deref()
                .is_some_and(|d| d.contains("base side"))
        );
    }

    #[test]
    fn equal_sides_are_unchanged_and_seed_nothing() {
        let seed = seed_of(modified(HEAD, HEAD));

        assert_eq!(seed.files[0].state, "unchanged");
        assert!(seed.seeds.is_empty());
    }

    #[test]
    fn a_language_that_cannot_say_where_a_declaration_ends_still_maps_its_line() {
        let before = HEAD.replace("    1\n", "    0\n");
        let seed = derive(
            &input(vec![modified(&before, HEAD)]),
            &snapshot(),
            &NoDeclarations,
        );

        // Only the declaration line is known, so a body line is outside.
        assert!(seed.seeds.is_empty());
        assert_eq!(seed.files[0].outside_symbols, 1);
    }

    #[test]
    fn a_rename_reports_where_the_file_came_from() {
        let mut change = modified(HEAD, HEAD);
        change.change = FileDelta::Renamed {
            from: "src/old.rs".to_string(),
        };
        let seed = seed_of(change);

        assert_eq!(seed.files[0].change, "renamed");
        assert_eq!(seed.files[0].from.as_deref(), Some("src/old.rs"));
    }
}
