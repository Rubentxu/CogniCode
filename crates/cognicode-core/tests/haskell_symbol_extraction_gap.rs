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
    // Measured: the whole file yields exactly one symbol, ("Config", 10).
    // `Config` therefore does appear, but NOT from `data_type @line3` — it
    // comes from the `function` node whose text is the signature body
    // `Config -> Int`, which starts on 1-based line 11 (0-based 10).
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
fn haskell_function_binds_are_not_extracted() {
    let symbols = haskell_symbols();
    // Measured: only ("Config", 10) comes back. `mkAll` and `use` are both
    // real definitions in the file and neither is indexed.
    assert_eq!(
        symbols.len(),
        1,
        "expected the CURRENT defect: 3 of 4 definitions unindexed; found {symbols:?}. \
         If extraction was fixed, invert this to the real count."
    );
    assert!(
        !symbols.iter().any(|(n, _)| n == "mkAll"),
        "expected the CURRENT defect (bind not extracted); found {symbols:?}"
    );
    assert!(
        !symbols.iter().any(|(n, _)| n == "use"),
        "expected the CURRENT defect (`use c = ...` bind not extracted); found {symbols:?}"
    );
}
