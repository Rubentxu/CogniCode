# Delta for explorerql-filters

## ADDED Requirements

### Requirement: Typed property predicates

Pattern `WHERE` clauses MUST accept bound node or edge properties using `<binding>.<property> <operator> <typed-value>`. String, number, Boolean, and null values MUST retain their types. Supported comparison operators SHALL be `=`, `!=`, `<`, `<=`, `>`, and `>=`.

#### Scenario: Node and edge properties filter a pattern
- GIVEN `MATCH (f:Function)-[c:Calls*1..2]->(g:Function) WHERE f.visibility = "public" AND c.weight >= 2 RETURN g`
- WHEN the query executes
- THEN only matches satisfying both typed predicates MUST participate

#### Scenario: Incompatible comparison rejected
- GIVEN a numeric property is compared with the string `"high"` using `>=`
- WHEN the query is lowered
- THEN a typed predicate error MUST identify the incompatible property and value types

### Requirement: Pattern provenance and confidence predicates

Pattern predicates MUST support `<edge>.provenance.<source>` and `<edge>.confidence`. Confidence MUST be numeric within `[0.0, 1.0]`; unknown provenance sources and out-of-range values MUST be rejected before execution.

#### Scenario: Provenance and confidence constrain edges
- GIVEN `WHERE c.provenance.tree_sitter = "call" AND c.confidence >= 0.8`
- WHEN the pattern executes
- THEN only edges satisfying both predicates MUST be traversed
- AND returned values MUST preserve provenance and confidence types

#### Scenario: Invalid confidence rejected
- GIVEN `WHERE c.confidence > 1.1`
- WHEN the query is parsed or lowered
- THEN the diagnostic MUST state that confidence must be within `[0.0, 1.0]`

### Requirement: Missing-property semantics

A missing property MUST compare as typed null. It MUST NOT equal a non-null value, and explicit equality with null MUST match it.

#### Scenario: Missing property filtered safely
- GIVEN one matching node lacks `module`
- WHEN `WHERE f.module = "api"` is evaluated
- THEN that node MUST be excluded without an error

#### Scenario: Null predicate selects missing property
- GIVEN one matching node lacks `module`
- WHEN `WHERE f.module = null` is evaluated
- THEN that node MUST be included
