#!/usr/bin/env python3
"""Where `Language` is declared, and where its tree-sitter mappings are.

Two facts live in two places on purpose:

- the *declaration* is `domain/value_objects/language.rs`, and that file
  deliberately contains no reference to tree-sitter: a closed enumeration of
  the languages the product supports, the extensions that select them and the
  AST node kinds each uses is the vocabulary of the problem;
- the *mapping* from a variant to a compiled `tree_sitter_*` grammar stayed in
  `infrastructure/parser/tree_sitter_parser.rs`, because those are Rust crates
  only the infrastructure layer links.

A generator that needs both has to read both. Reading the enumeration from the
parser file would be reading a stale path, and pretending otherwise by
searching both files for whichever has the enum would let the declaration
drift back into the adapter without anything failing.

Deliberately not a Rust parser. One `enum` with a known shape is read by
shape, for the same reason `release_lane.py` reads two Kotlin declarations by
shape rather than being a second implementation of Kotlin's grammar. When a
rule here stops matching, the failure names the file it was reading.
"""

from __future__ import annotations

import re
from pathlib import Path

# The declaration. Domain vocabulary, free of any tree-sitter reference.
LANGUAGE_ENUM_REL = "crates/cognicode-core/src/domain/value_objects/language.rs"

# The mapping from variant to compiled grammar. Infrastructure, because the
# grammars are crates only that layer links.
TREE_SITTER_MAPPING_REL = (
    "crates/cognicode-core/src/infrastructure/parser/tree_sitter_parser.rs"
)

# The adapter that binds each language to a compiled grammar. It says which
# languages have a reader of their own; it no longer says what a language
# declares, because that lives in the domain next to the enumeration.
LANGUAGE_CONFIG_REL = (
    "crates/cognicode-core/src/infrastructure/parser/language_config.rs"
)

_ENUM = re.compile(
    r"pub enum Language\s*\{(?P<body>.*?)^\}", re.MULTILINE | re.DOTALL
)
_VARIANT = re.compile(r"^\s*([A-Z][A-Za-z0-9_]*)\s*,", re.MULTILINE)
_MAPPING = re.compile(r"Language::([A-Z][A-Za-z0-9_]*)\s*=>")


def read_required(root: Path, relative: str) -> str:
    """File contents, or a failure that names the path it wanted."""
    path = root / relative
    try:
        content = path.read_text(encoding="utf-8")
    except OSError as exc:
        raise ValueError(f"required source is unreadable: {relative}") from exc
    if not content.strip():
        raise ValueError(f"required source is empty: {relative}")
    return content


def language_variants(root: Path) -> list[str]:
    """The variants of the `Language` enum, in declaration order.

    The order is the declaration's, not sorted, because the generated
    documents list languages in that order and a sort would rewrite them
    without any change in meaning — which is the kind of diff that hides the
    change that did happen.
    """
    source = read_required(root, LANGUAGE_ENUM_REL)
    match = _ENUM.search(source)
    if not match:
        raise ValueError(
            f"Language enum is missing from {LANGUAGE_ENUM_REL}. If the "
            f"declaration moved, update LANGUAGE_ENUM_REL here rather than "
            f"searching for it: a search that accepts either file is how it "
            f"drifted into the adapter layer in the first place"
        )
    variants = _VARIANT.findall(match.group("body"))
    if not variants:
        raise ValueError(f"Language enum in {LANGUAGE_ENUM_REL} has no variants")
    return variants


def unmapped_variants(root: Path, variants: list[str]) -> list[str]:
    """Variants with no tree-sitter grammar behind them.

    Every variant the product claims to support needs a compiled grammar. A
    variant added to the enum without a mapping arm would otherwise be
    published in the support matrix and fail at parse time, in a user's run
    rather than in a review.
    """
    source = read_required(root, TREE_SITTER_MAPPING_REL)
    mapped = set(_MAPPING.findall(source))
    return sorted(set(variants) - mapped)


_EXTENSIONS_DECLARATION = re.compile(
    r"pub const fn extensions\(self\).*?\{(?P<body>.*?)\n    \}", re.DOTALL
)
_EXTENSION_ENTRY = re.compile(
    r"Language::(?P<variant>[A-Z][A-Za-z0-9_]*)\s*=>\s*&\[(?P<extensions>[^\]]*)\]"
)


def language_extensions(root: Path) -> dict[str, list[str]]:
    """The extensions that select each language, read from the declaration.

    The lists live in the domain, next to the enumeration they belong to,
    and this reads them there. Reading them from the adapter instead would
    be reading a second copy, and the copy is the one that drifts.
    """
    source = read_required(root, LANGUAGE_ENUM_REL)
    match = _EXTENSIONS_DECLARATION.search(source)
    if not match:
        raise ValueError(
            f"the extension lists are missing from {LANGUAGE_ENUM_REL}. If the "
            "declaration moved or changed shape, update LANGUAGE_ENUM_REL here "
            "rather than searching for it: a search that accepts either file is "
            "how the declaration drifts in the first place"
        )
    result: dict[str, list[str]] = {}
    for entry in _EXTENSION_ENTRY.finditer(match.group("body")):
        result[entry.group("variant")] = re.findall(
            r'"([^"]*)"', entry.group("extensions")
        )
    missing = sorted(set(language_variants(root)) - set(result))
    if missing:
        raise ValueError(
            f"languages without extension lists in {LANGUAGE_ENUM_REL}: {missing}"
        )
    return result


_SYMBOLS_ENTRY = re.compile(
    r"Language::(?P<variant>[A-Z][A-Za-z0-9_]*)\s*=>\s*LanguageSymbols\s*\{"
    r"(?P<body>.*?)\n            \}",
    re.DOTALL,
)
_SYMBOLS_FIELD = re.compile(
    r"(?P<field>[a-z_]+):\s*&\[(?P<values>.*?)\]", re.DOTALL
)
_SEMANTIC_CONFIG = re.compile(
    r"pub const \w+_CONFIG: LanguageConfig = LanguageConfig \{(?P<body>.*?)\n\};",
    re.DOTALL,
)


def language_symbols(root: Path) -> dict[str, dict[str, list[str]]]:
    """What each language declares, read from the domain.

    The declaration is the truth about a language: the node kinds it uses
    for functions, classes, variables, calls and imports, and the field
    that names a callee. It is read here so the published matrix says what
    the product can really assert, instead of saying it by hand.
    """
    source = read_required(root, LANGUAGE_ENUM_REL)
    declarations: dict[str, dict[str, list[str]]] = {}
    for entry in _SYMBOLS_ENTRY.finditer(source):
        fields = {
            field.group("field"): re.findall(r'"([^"]*)"', field.group("values"))
            for field in _SYMBOLS_FIELD.finditer(entry.group("body"))
        }
        declarations[entry.group("variant")] = fields
    missing = sorted(set(language_variants(root)) - set(declarations))
    if missing:
        raise ValueError(
            f"languages without a declaration in {LANGUAGE_ENUM_REL}: {missing}"
        )
    return declarations


def semantic_reader_languages(root: Path) -> set[str]:
    """The languages a reader of their own answers for.

    A configuration language is not a program, and the reader that knows
    what a Terraform resource is not the generic walker. The registry says
    which languages have such a reader.
    """
    source = read_required(root, LANGUAGE_CONFIG_REL)
    readers: set[str] = set()
    for config in _SEMANTIC_CONFIG.finditer(source):
        variant = re.search(
            r"language:\s*Language::([A-Z][A-Za-z0-9_]*)", config.group("body")
        )
        handler = re.search(r"semantic_handler:\s*Some\(", config.group("body"))
        if variant and handler:
            readers.add(variant.group(1))
    return readers


_LANGUAGE_ID_BODY = re.compile(
    r"pub const fn id\(self\).*?\{(?P<body>.*?)\n    \}", re.DOTALL
)
_LANGUAGE_NAME_BODY = re.compile(
    r"pub fn name\(self\).*?\{(?P<body>.*?)\n    \}", re.DOTALL
)
_LANGUAGE_ID = re.compile(
    r"Language::(?P<variant>[A-Z][A-Za-z0-9_]*)\s*=>\s*\"(?P<id>[a-z0-9]+)\""
)
_LANGUAGE_NAME = re.compile(
    r"Language::(?P<variant>[A-Z][A-Za-z0-9_]*)"
    r"(?:\s*\|\s*Language::[A-Z][A-Za-z0-9_]*)?"
    r"\s*=>\s*\"(?P<name>[^\"]+)\""
)


def _language_spellings(
    root: Path,
    body: re.Pattern[str],
    entry: re.Pattern[str],
    group: str,
) -> dict[str, str]:
    """One spelling per language, read from the declaration.

    The body is isolated before it is read: the same file spells a language
    in more than one way — its identifier, its display name, its node
    kinds — and a search that accepts any of them is a search that cannot
    say which one it read.
    """
    source = read_required(root, LANGUAGE_ENUM_REL)
    match = body.search(source)
    if not match:
        raise ValueError(
            f"the language spelling is missing from {LANGUAGE_ENUM_REL}. If the "
            "declaration moved or changed shape, update LANGUAGE_ENUM_REL here "
            "rather than searching for it: a search that accepts either file is "
            "how the declaration drifts in the first place"
        )
    result: dict[str, str] = {}
    for found in entry.finditer(match.group("body")):
        result[found.group("variant")] = found.group(group)
    missing = sorted(set(language_variants(root)) - set(result))
    if missing:
        raise ValueError(
            f"languages without a public spelling in {LANGUAGE_ENUM_REL}: {missing}"
        )
    return result


def language_ids(root: Path) -> dict[str, str]:
    """The public identifier of each language."""
    return _language_spellings(root, _LANGUAGE_ID_BODY, _LANGUAGE_ID, "id")


def language_names(root: Path) -> dict[str, str]:
    """The display name of each language."""
    return _language_spellings(root, _LANGUAGE_NAME_BODY, _LANGUAGE_NAME, "name")
