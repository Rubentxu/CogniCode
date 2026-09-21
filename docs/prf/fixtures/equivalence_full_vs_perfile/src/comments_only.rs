// Equivalence oracle fixture (F2.W3, R4) — comments only, no symbols.
// Scenario 5.
//
// Block comments, line comments, doc comments, none of them should
// produce a symbol.

// A single-line comment is not a symbol.

/*
   A block comment is not a symbol either.
*/

/// Doc comments are not symbols either.

/*! Inner doc comments are also not symbols. */
