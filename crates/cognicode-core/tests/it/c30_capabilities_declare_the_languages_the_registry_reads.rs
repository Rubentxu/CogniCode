//! The capabilities a tool declares must be the languages the product reads.
//!
//! A tool that announces "I read these languages" is making a promise a
//! consumer relies on to decide whether to point the tool at a file. The
//! announcement used to be a list written by hand, next to the registry
//! that says which grammars are actually compiled and wired. Two lists,
//! written at different times, by different hands: they drifted.
//!
//! What the drift cost, measured on the tree this contract was written
//! against:
//!
//! * Three of the announced names were not languages at all. `jsx` and
//!   `tsx` are extensions of JavaScript and TypeScript; `luau` is an
//!   extension of Lua. A consumer matching a language by name would look
//!   for a language the product has never heard of.
//! * Twelve languages the product does read were not announced: `bash`,
//!   `elixir`, `erlang`, `fortran`, `groovy`, `haskell`, `json`, `julia`,
//!   `powershell`, `r`, `systemverilog` and `verilog`. Their grammars are
//!   compiled into the binary and their configs are in the registry, so
//!   the product reads them — and told the consumer it did not.
//!
//! Neither direction was visible: the tests that existed asked whether the
//! declared list matched a *document* that listed the same twenty-one
//! names. A document and a list that were written to agree agree for as
//! long as nobody edits one of them, and the registry — the thing that
//! decides what the product can read — was in neither.
//!
//! ## What this file pins
//!
//! That the set of languages a tool declares is the set of languages the
//! registry reads, in both directions: nothing announced that is not read,
//! and nothing read that is not announced.
//!
//! The registry is the authority. The product reads what the registry
//! wires; a declaration that disagrees with it is a promise the product
//! cannot keep, and one the consumer pays for.

use cognicode_core::domain::value_objects::Language;
use cognicode_core::infrastructure::parser::language_config::LanguageConfig;
use cognicode_core::interface::mcp::capabilities::{all_stable_capabilities, tree_sitter_langs};
use std::collections::BTreeSet;

/// The languages the product can read, as the registry says.
///
/// The registry is the authority because the registry is what binds a
/// language to a compiled grammar. A language without a config has no
/// grammar to read it with, and a config without a language names
/// nothing a consumer could ask for.
fn languages_the_registry_reads() -> BTreeSet<&'static str> {
    LanguageConfig::all()
        .iter()
        .map(|config| config.language.id())
        .collect()
}

/// The languages the capabilities announce.
fn languages_the_capabilities_announce() -> BTreeSet<&'static str> {
    tree_sitter_langs().iter().copied().collect()
}

#[test]
fn the_announced_languages_are_the_languages_the_registry_reads() {
    let read = languages_the_registry_reads();
    let announced = languages_the_capabilities_announce();

    let announced_but_not_read: Vec<&&str> = announced.difference(&read).collect();
    let read_but_not_announced: Vec<&&str> = read.difference(&announced).collect();

    assert!(
        announced_but_not_read.is_empty(),
        "the capabilities announce languages the product does not read: {announced_but_not_read:?}. \
         A name that is not a language the registry wires is a promise the product cannot keep."
    );

    assert!(
        read_but_not_announced.is_empty(),
        "the product reads languages the capabilities do not announce: {read_but_not_announced:?}. \
         A consumer that reads the announcement to decide what to point the tool at would \
         leave those files out."
    );
}

#[test]
fn every_announced_name_is_a_language_and_not_an_extension() {
    // The registry knows the difference between a language and the file
    // extensions that select it: `jsx` belongs to JavaScript and `luau`
    // belongs to Lua. Announcing an extension as a language is the same
    // defect in miniature — a consumer matching by name finds nothing.
    let extensions: BTreeSet<&'static str> = Language::all_languages()
        .iter()
        .flat_map(|language| language.extensions().iter().copied())
        .collect();

    let announced_as_language: Vec<&str> = tree_sitter_langs()
        .iter()
        .copied()
        .filter(|name| {
            extensions.contains(name)
                && !Language::all_languages()
                    .iter()
                    .any(|language| language.id() == *name)
        })
        .collect();

    assert!(
        announced_as_language.is_empty(),
        "the capabilities announce extensions as if they were languages: {announced_as_language:?}. \
         An extension is how a file is written, not what the product reads."
    );
}

#[test]
fn every_tool_that_reads_code_announces_languages_the_product_reads() {
    // A tool's announcement has to name languages the product can actually
    // read. A tool that announces a language nobody can read is a promise
    // the consumer cannot rely on; a tool that announces nothing leaves the
    // consumer with no way to know what to point it at.
    //
    // The set a tool declares need not be the whole registry: a tool that
    // navigates with a language server covers the languages that server
    // speaks, and that is a smaller and honest set. What it must not do is
    // announce a language the product does not read.
    let read = languages_the_registry_reads();

    for (name, capabilities) in all_stable_capabilities() {
        if capabilities.precision.is_empty() {
            continue;
        }
        for announced in capabilities.langs {
            assert!(
                read.contains(announced),
                "the tool '{name}' announces '{announced}', which the registry does not read: \
                 the announcement is a promise the product cannot keep"
            );
        }
    }
}
