"""One scanner for the comments of a source file.

Two consumers need to know where the comments are, and they have to agree:
one counts them as debt, the other has to blank them out before it counts
anything else. A second scanner would be a second truth, and the two would
drift the first time either one met a string literal that looked like a
comment.

So there is one state machine here, and the two views are derived from it:

    scan(text)                 the file as (is_comment, chunk) segments
    strip_rust_comments(text)  the file with the comments blanked out
    comment_lines(text)        the comment lines, with their line numbers

The scanner is string- and char-literal aware, because that is where the
failure modes live. A `/*` inside a string literal opens a block comment
that swallows the rest of the file; a `'` inside a character literal opens a
string that swallows the rest of the file. Both are silent: the code after
them simply stops being visible, and a count that comes out wrong has
nothing visibly broken to point at.

The subtle one is the lifetime. In Rust, `&'static str` and `<'a>` are
lifetimes, not character literals, and a scanner that cannot tell them apart
goes blind from the first lifetime to the end of the file. It has to keep
seeing the comments after it, because a scanner that stops seeing is a
scanner that silently under-reports.

Run:
    python3 -c "import sys; sys.path.insert(0, 'scripts/ci'); import comment_scan"
"""

from __future__ import annotations


def char_literal_end(text: str, index: int) -> int | None:
    """The index just past a character literal at `index`, or None.

    A character literal is a quote, one character or an escape sequence, and
    a closing quote. A lifetime is a quote that is not closed the way a
    literal is, and it is code.
    """
    length = len(text)
    if index + 1 >= length:
        return None
    following = text[index + 1]
    if following == "\\":
        # An escape sequence: the closing quote comes after it.
        cursor = index + 2
        while cursor < length and text[cursor] not in "\n":
            if text[cursor] == "'":
                return cursor + 1
            cursor += 1
        return None
    if following == "'":
        return index + 2
    if following == "\n":
        return None
    if index + 2 < length and text[index + 2] == "'":
        return index + 3
    return None


def scan(text: str) -> list[tuple[bool, str]]:
    """The file as a list of `(is_comment, chunk)` segments.

    The chunks are the original characters, unaltered. Whether a segment is
    code or a comment is the only thing decided here.
    """
    segments: list[tuple[bool, str]] = []

    def push(is_comment: bool, chunk: str) -> None:
        if not chunk:
            return
        if segments and segments[-1][0] == is_comment:
            segments[-1] = (is_comment, segments[-1][1] + chunk)
        else:
            segments.append((is_comment, chunk))

    index = 0
    length = len(text)
    # 0 = code, 1 = line comment, 2 = block comment
    state = 0
    quote: str | None = None
    # The closing sequence of a raw string, so `r#"..."#` closes correctly.
    raw_close = ""

    while index < length:
        char = text[index]

        if state == 1:  # line comment
            push(True, char)
            if char == "\n":
                state = 0
            index += 1
            continue

        if state == 2:  # block comment
            if char == "*" and index + 1 < length and text[index + 1] == "/":
                push(True, "*/")
                index += 2
                state = 0
                continue
            push(True, char)
            index += 1
            continue

        if quote is not None:
            if char == "\\" and not raw_close:  # escape, skipped in raw strings
                push(False, char)
                index += 1
                if index < length:
                    push(False, text[index])
                    index += 1
                continue
            if char == quote[0] and text.startswith(quote, index):
                push(False, quote)
                index += len(quote)
                quote = None
                raw_close = ""
                continue
            push(False, char)
            index += 1
            continue

        if char == "r" and index + 1 < length and text[index + 1] in ('"', "#"):
            end = index + 1
            while end < length and text[end] == "#":
                end += 1
            if end < length and text[end] == '"':
                closer = '"' + "#" * (end - index - 1)
                push(False, text[index : end + 1])
                index = end + 1
                quote = closer
                raw_close = closer
                continue

        if char == '"':
            quote = '"'
            raw_close = ""
            push(False, char)
            index += 1
            continue

        if char == "'":
            # A lifetime is code; a character literal is a literal.
            end = char_literal_end(text, index)
            if end is None:
                push(False, char)
                index += 1
                continue
            push(False, text[index:end])
            index = end
            continue

        if char == "/" and index + 1 < length:
            if text[index + 1] == "/":
                state = 1
                push(True, "//")
                index += 2
                continue
            if text[index + 1] == "*":
                state = 2
                push(True, "/*")
                index += 2
                continue

        push(False, char)
        index += 1
    return segments


def strip_rust_comments(text: str) -> str:
    """The file with every comment blanked out, keeping the line numbering.

    A comment character becomes a space and a newline stays a newline, so the
    result has the same length and the same lines as the original.
    """
    parts: list[str] = []
    for is_comment, chunk in scan(text):
        if is_comment:
            parts.append("".join("\n" if c == "\n" else " " for c in chunk))
        else:
            parts.append(chunk)
    return "".join(parts)


def comment_lines(text: str) -> list[tuple[int, str]]:
    """Every comment line of the file, with its line number.

    A line that carries no comment text — the blank line inside a block
    comment, for instance — is not reported: there is nothing on it to
    read.
    """
    found: list[tuple[int, str]] = []
    number = 1
    for is_comment, chunk in scan(text):
        if not is_comment:
            number += chunk.count("\n")
            continue
        pieces = chunk.split("\n")
        for offset, piece in enumerate(pieces):
            if offset > 0:
                number += 1
            if piece.strip():
                found.append((number, piece))
    return found
