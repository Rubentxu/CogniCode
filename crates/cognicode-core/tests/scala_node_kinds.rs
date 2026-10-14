//! Pins the Scala node-kind facts that `Language`'s mappings depend on.
//!
//! Why this file exists separately from `advertised_language_extraction_matrix.rs`:
//! that suite asserts what CogniCode *extracts*. This one asserts what the
//! grammar actually *emits*, which is the fact the mappings are derived from.
//!
//! The distinction matters because tree-sitter's `node-types.json` is a schema
//! that is not a complete oracle for what the parser emits. `class_declaration`
//! is listed there for tree-sitter-scala 0.26.2 and never emitted at all, but
//! `function_declaration` is listed with no parent node and *is* emitted, for an
//! abstract `def` in a trait. An earlier commit reasoned from the schema alone
//! and concluded "val_declaration is only a supertype, so it can never appear in
//! a tree". That was wrong on the mechanism: these are ordinary named nodes,
//! just ones no parent lists as a child. The conclusion held; the reasoning did
//! not.
//!
//! So these tests check reachability against a live parse. Reading the schema
//! alone is not enough evidence, and conflating the two is what produced the
//! original bug in the first place.

use cognicode_core::domain::value_objects::Language;

/// Collects every node kind appearing anywhere in a parse of `src`.
fn kinds_in_parse(src: &str) -> Vec<String> {
    let mut parser = tree_sitter::Parser::new();
    parser
        .set_language(&tree_sitter_scala::LANGUAGE.into())
        .expect("tree-sitter-scala must load");
    let tree = parser.parse(src, None).expect("scala source must parse");

    let mut kinds = Vec::new();
    let mut stack = vec![tree.root_node()];
    while let Some(node) = stack.pop() {
        kinds.push(node.kind().to_string());
        for i in 0..node.child_count() {
            if let Some(child) = node.child(i) {
                stack.push(child);
            }
        }
    }
    kinds
}

const DEFINITIONS: &str = "\
class Config {
  val limit: Int = 5
  def run(): Int = 1
}
object Runner {
  def main(): Unit = println(\"x\")
}
trait Greeter {
  def greet(): String
}
";

/// An abstract `def` in a trait parses as `function_declaration`, not
/// `function_definition`.
///
/// This is the fact that silently rots. `function_node_type()` declares
/// `function_definition`, so an abstract trait method is currently invisible to
/// symbol extraction. Recorded as measured behaviour, not asserted as correct:
/// fixing it means letting the function check accept two kinds for Scala, which
/// is a follow-up rather than something to smuggle into this commit.
#[test]
fn abstract_trait_def_is_a_function_declaration() {
    let kinds = kinds_in_parse("trait Greeter {\n  def greet(): String\n}\n");
    assert!(
        kinds.iter().any(|k| k == "function_declaration"),
        "Expected an abstract trait def to parse as `function_declaration`. If \
         the grammar changed, `function_node_type()` must be re-measured. \
         Seen: {kinds:?}"
    );
}

/// The kinds the declared mappings depend on are all reachable in a real parse.
///
/// Before the fix, `function_node_type()` returned `function_declaration` and
/// `class_node_type()` returned `class_declaration`, neither of which appears in
/// this fixture, so the extraction filter could never fire for a Scala file.
#[test]
fn declared_scala_mappings_are_reachable_in_a_real_parse() {
    let kinds = kinds_in_parse(DEFINITIONS);

    for (what, declared) in [
        ("function_node_type", Language::Scala.function_node_type()),
        ("class_node_type", Language::Scala.class_node_type()),
        ("variable_node_type", Language::Scala.variable_node_type()),
    ] {
        assert!(
            kinds.iter().any(|k| k == declared),
            "{what}() = {declared:?} never appears when parsing real Scala. \
             The declared mapping is dead: the extraction filter can never fire \
             on it. Seen kinds: {kinds:?}."
        );
    }

    assert!(
        kinds.iter().any(|k| k == "object_definition"),
        "Scala `object` must reach `object_definition`; without it half of a \
         Scala file's class-shaped definitions are invisible. Seen: {kinds:?}"
    );
}

/// The Java-derived class/val spellings are unreachable, which is what the bug was.
///
/// `function_declaration` is deliberately NOT asserted unreachable. It is
/// reachable, for an abstract trait `def`, and the fixture's `def`s all carry
/// bodies so they parse as `function_definition`. It is listed with no parent in
/// node-types.json, which makes it look dead on paper while a real parse emits
/// it, so it is pinned explicitly by the test below.
#[test]
fn java_spellings_never_appear_in_a_scala_parse() {
    let kinds = kinds_in_parse(DEFINITIONS);

    for dead in ["class_declaration", "val_declaration"] {
        assert!(
            !kinds.iter().any(|k| k == dead),
            "{dead:?} unexpectedly appears in a Scala parse. If tree-sitter-scala \
             started emitting it, the mapping rationale must be re-measured. \
             Seen: {kinds:?}"
        );
    }
}
