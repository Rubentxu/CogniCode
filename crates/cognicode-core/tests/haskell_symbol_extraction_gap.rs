//! RED characterization: Haskell symbol extraction reports the wrong symbols at
//! the wrong locations.
//!
//! Found during the acceptance run for the advertised-surface blocker block,
//! exercised through the real release server (`cognicode-mcp` 0.101.16 over
//! stdio JSON-RPC) rather than through a unit test.
//!
//! Observed on a 12-line Haskell file whose `data Config` is on line 3:
//!
//! ```text
//! query_symbol_index "Config" -> total: 1
//!   location: src/Small.hs line 10 column 7  symbol_kind Function
//! ```
//!
//! Line 10 does not define `Config`. Measured directly against the parser, the
//! whole 12-line file yields exactly ONE symbol: `("Config", line 10)`. So of
//! the four definitions in the file (`module Small`, `data Config`, `mkAll`,
//! `use`), three are never indexed, and the one that is reported points at the
//! wrong line with the wrong kind.
//!
//! Measured cause, enumerating the node kinds the grammar actually produces for
//! that file (not assumed — read off the parse tree):
//!
//! ```text
//! bind      @line9  : mkAll = [Config "a" 1]
//! data_type @line3  : data Config = Config
//! function  @line11 : Config -> Int
//! function  @line12 : use c = cfgCount c
//! module    @line1  : Small
//! signature @line11 : use :: Config -> Int
//! signature @line8  : mkAll :: [Config]
//! ```
//!
//! `TreeSitterParser::find_all_symbols_with_path` only matches a language's
//! `function_node_type`, `class_node_type` and `variable_node_type`. For
//! Haskell `Language::Haskell` declares those as the strings `"function"`,
//! `"module"` and `"declaration"`. So:
//!
//! - `data_type` and `bind` — the nodes that actually carry Haskell's `data`
//!   and top-level `=` bindings — are matched by nothing at all, which is why
//!   `mkAll` and `use` never appear;
//! - the single surviving `Config` came from the `function` node at line 11,
//!   whose text is the *type signature body* `Config -> Int`, not a
//!   definition. That is why the reported location is line 10 (0-based) and
//!   why the symbol kind comes back `Function`.
//!
//! Note the module symbol is matched by `class_node_type == "module"` yet still
//! does not surface, so the string mapping is wrong in more than one place.
//!
//! This matters beyond Haskell: the same "declare a node-type string, then match
//! on it" mechanism is how every language is wired, so a wrong string produces
//! silently wrong answers rather than an error.
//!
//! It also explains why the catalog's advertised evidence did not catch it.
//! `product/languages.json` claims for `haskell`:
//!
//!   "Focused parser acceptance coverage in m06_acceptance.rs or m10_acceptance.rs"
//!
//! and `m06_acceptance.rs` contains exactly one Haskell line, inside a table of
//! `("name", Language, source)` cases that only assert `root_node().child_count()
//! != 0`. That proves the grammar loads. It asserts nothing about symbols, so it
//! passes with extraction entirely broken.
//!
//! Haskell is advertised as `support_level: "supported"` with `extensions:
//! ["hs"]`, and the evidence cites "LanguageConfig extension and symbol mapping
//! in language_config.rs" — the symbol mapping is exactly what is broken.
//!
//! These assertions pin the CURRENT, WRONG behaviour so the defect is visible
//! in the gate instead of being an unrecorded surprise. When this is fixed,
//! every assertion below has to be inverted in the same commit that fixes it;
//! that inversion is the point, not an inconvenience.
//!
//! UPDATED 2026-10-14, after `find_identifier_name` learned to read the
//! grammar's `name` FIELD before falling back to child node kinds (the Bash
//! fix, commit below). That change is a strict superset of the old lookup and
//! it partially improved Haskell, measured against the parse tree rather than
//! assumed:
//!
//! ```text
//! function @row11 field(name)=Some("use")   <- the definition, correctly named
//! function @row10 field(name)=None  text="Config -> Int"  <- signature body
//! module   @row0  field(name)=None  text="Small"
//! module   @row0  field(name)=None  text="module"
//! ```
//!
//! So `use` is now correctly extracted, and the state moved from **1** symbol
//! to **2**: `[("Config", 10), ("use", 11)]`. `mkAll` is still missing, `Config`
//! is still reported at the signature body instead of its `data` definition,
//! and the module still does not surface. That is progress, not a fix, and the
//! assertions below record precisely which half is still wrong.

use cognicode_core::domain::value_objects::Language;
use cognicode_core::infrastructure::parser::TreeSitterParser;

const SMALL_HS: &str = "module Small where

data Config = Config
  { cfgName :: String
  , cfgCount :: Int
  }

mkAll :: [Config]
mkAll = [Config \"a\" 1]

use :: Config -> Int
use c = cfgCount c
";

fn haskell_symbols() -> Vec<(String, u32)> {
    let parser = TreeSitterParser::new(Language::Haskell).expect("haskell grammar must load");
    parser
        .find_all_symbols_with_path(SMALL_HS, "Small.hs")
        .expect("parse must succeed")
        .iter()
        .map(|s| (s.name().to_string(), s.location().line()))
        .collect()
}

#[test]
fn haskell_data_type_is_not_extracted_from_its_own_definition_node() {
    let symbols = haskell_symbols();
    // The whole file yields ("Config", 10) and ("use", 11). `Config` therefore
    // does appear, but NOT from `data_type @line2` — it comes from the
    // `function` node whose text is the signature body `Config -> Int`, which
    // starts on 1-based line 11 (0-based 10). That node has no `name` field,
    // which is why the field-first lookup did not rescue it either.
    let definition_line = SMALL_HS
        .lines()
        .position(|l| l.starts_with("data Config"))
        .unwrap() as u32;
    let config: Vec<_> = symbols.iter().filter(|(n, _)| n == "Config").collect();
    let at_definition = config.iter().any(|(_, l)| *l == definition_line);
    assert!(
        !at_definition,
        "expected the CURRENT defect: Config is not reported at its definition \
         (line {definition_line}); symbols found: {symbols:?}. If this assertion \
         starts failing because extraction was FIXED, invert it to assert the \
         real location rather than deleting the coverage."
    );
}

#[test]
fn haskell_module_is_not_extracted() {
    let symbols = haskell_symbols();
    // `class_node_type` is declared as "module" and the grammar does emit
    // `module` nodes, yet nothing surfaces for `Small`. Measured: both module
    // nodes have `field(name) = None` and their text is "Small" / "module",
    // so neither the field lookup nor the node-kind whitelist can name them.
    // Fixing this needs a Haskell-specific mapping, not a lookup change.
    assert!(
        !symbols.iter().any(|(n, _)| n == "Small"),
        "expected the CURRENT defect (module name not extracted); found {symbols:?}"
    );
}

#[test]
fn haskell_bind_bindings_are_not_extracted() {
    let symbols = haskell_symbols();
    // Measured now: ("Config", 10) and ("use", 11). `use` is recovered because
    // the grammar models `use c = ...` as a `function` node that carries a
    // `name` field. `mkAll = [...]` is modelled as a `bind`, which the filter
    // does not match at all, so it is still missing.
    assert_eq!(
        symbols.len(),
        2,
        "expected the CURRENT defect: mkAll still unindexed, and only the two \
         symbols measured after the field-first name lookup; found {symbols:?}. \
         If extraction was fixed, invert this to the real count."
    );
    assert!(
        !symbols.iter().any(|(n, _)| n == "mkAll"),
        "expected the CURRENT defect (bind not extracted); found {symbols:?}"
    );
}

#[test]
fn haskell_use_binding_is_extracted_at_its_definition() {
    // The one part that IS correct today, asserted positively so the fix for
    // the rest cannot regress it into the previous "extracts nothing" state.
    let symbols = haskell_symbols();
    let definition_line = SMALL_HS
        .lines()
        .position(|l| l.starts_with("use c ="))
        .unwrap() as u32;
    assert!(
        symbols
            .iter()
            .any(|(n, l)| n == "use" && *l == definition_line),
        "`use c = ...` is a real definition and is currently extracted at its \
         own line ({definition_line}); symbols found: {symbols:?}"
    );
}
