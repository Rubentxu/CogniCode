#!/usr/bin/env python3
"""Where `Language` is declared, and where its tree-sitter mappings are.

Both product generators read this enumeration, and both used to read it out of
`crates/cognicode-core/src/infrastructure/parser/tree_sitter_parser.rs` with
the same six lines of regex. That stopped matching on 2026-10-03, and the
failure was silent in the worst way available: the generators raise
`Language enum is missing`, four manifest checks and four support-matrix checks
went red, and nobody had run them since the move.

The move is the correct one and is not what this module is about. `Language`
was lifted out of `infrastructure/parser` into
`domain/value_objects/language.rs` because a closed enumeration of the
languages the product supports, the extensions that select them and the AST
node kinds each uses is the vocabulary of the problem, not tree-sitter. Its
own doc comment says so. The two generators were left pointing at the old
file.

So there are two facts here, and they are in two places on purpose:

- the *declaration* is `domain/value_objects/language.rs`, and that file
  deliberately contains no reference to tree-sitter;
- the *mapping* from a variant to a compiled `tree_sitter_*` grammar stayed in
  `infrastructure/parser/tree_sitter_parser.rs`, because those are Rust crates
  only the infrastructure layer links.

A generator that needs both has to read both. Reading the enum from the parser
file was never a layering decision, it was a stale path, and pretending
otherwise by searching both files for whichever has the enum would let the
declaration drift back into the adapter without anything failing.

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
