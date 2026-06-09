# Help and Onboarding

This file proposes how CogniCode Explorer teaches itself to new users.
The product is a navigation tool with a rich model, and a rich model
without an on-ramp is a tool only its authors can use. The strategy
is borrowed from the Glamorous Toolkit's moldable philosophy:
contextual help bound to the focused object, progressive disclosure
driven by the user's moves, and a glossary that is a first-class
artifact, not a footnote.

Everything in this file is content, not code. Help text, glossary
entries, and suggested questions live in the docs/help layer
described in `core-mcp-boundaries.md`. They are versioned, trans-
latable, and editable by people who are not engineers.

## Glossary Strategy

The glossary in `glossary.md` is the single source of truth. Every
term the user encounters in the product is defined there, and the
help surfaces link to it.

The rules:

- The glossary is content. It is stored in a structured format
  (one file per term, or one file with a stable schema; the
  decision is an implementation detail) and loaded by the
  explorer and the MCP server.
- A new public term lands in the glossary in the same change that
  introduces it. No term ships without a definition.
- A term's definition has a stable id. The help surfaces link to
  it by id, not by URL, so renames do not break links.
- The glossary has examples. A term without an example is a
  definition the user cannot use.

The glossary is not a marketing document. It is precise, concise,
and shaped for lookup, not for reading cover to cover.

## Contextual Help Strategy

Contextual help is help that knows what the user is looking at.
Three rules drive it.

- Help is bound to the focused object. When the user focuses a
  symbol, the help panel shows the symbol's help: what it is, what
  the lens options mean, what the suggested questions are.
- Help is bound to the current view. When the user opens the
  community map, the help panel explains what a community is, how
  the algorithm works at a high level, and what the user is
  supposed to take away.
- Help is bound to the user's level. The same question answered
  at the code level and at the component level is a different
  answer. The help text changes to match.

The implementation is a content table keyed by `(object_kind,
view_id)`. The keys are stable ids. Adding a new view is also
adding the help text that goes with it.

## "What Can I Do Here?" Prompts

Every focused object in the explorer has a small panel of
prompts. The panel is the user-facing form of the curated question
set in `target-product-model.md`. The prompts are not generic;
they are bound to the object's kind.

The rules:

- The panel shows three to five prompts. More is a wall; fewer
  is unhelpful.
- The first prompt is always the one most likely to be useful
  for a user new to the kind. It is the "where do I start"
  prompt.
- Each prompt names a verb, names an object, and gives a one-line
  expectation of the result. The expectation is honest: a prompt
  that promises a miracle and delivers a list is a credibility
  loss.
- A prompt that does not apply to the current focus is hidden,
  not greyed out. Greyed-out prompts are noise.

The panel updates as the user moves. A symbol that becomes a
hotspot gains a "what is risky about this" prompt. A symbol that
loses its hotspot status loses the prompt. The panel reflects the
graph, not a static list.

## Suggested Questions Per Object Kind

The prompts above are driven by a per-kind list of suggested
questions. The lists are content, stored in the docs/help layer.
The seed list below is the v1 contract; it grows in step with the
verb set in `query-and-navigation.md`.

### Symbol

- "What does this symbol do?" - the definition, signature, and a
  short summary of its callers and callees.
- "Who calls this?" - direct and transitive callers, sorted by
  fan-in.
- "What is risky to change here?" - the risk overlay on this
  symbol.
- "Where does this belong?" - the climb-up path to the
  component, container, and system.
- "What justifies this?" - the decisions and issues that back
  this code.

### File

- "What is in this file?" - the symbols defined here, sorted by
  fan-in.
- "What is risky in this file?" - the per-file risk overlay.
- "What changed in this file?" - the time-windowed diff.
- "Where does this file belong?" - the climb-up path to scope,
  component, container, and system.

### Scope

- "What lives in this scope?" - the symbols in the scope, grouped
  by sub-scope.
- "What depends on this scope?" - the inverse of the previous
  question.
- "What changed in this scope?" - the time-windowed diff at the
  scope level.

### Component

- "What is in this component?" - the files and scopes that
  compose the component.
- "What depends on this component?" - the consumers across
  components.
- "What is risky in this component?" - the component-level risk
  overlay.
- "What justifies this component?" - the decisions and issues
  that back its existence.

### Container

- "What runs in this container?" - the components deployed here.
- "What does this container talk to?" - the cross-container
  dependencies.
- "What changed in this container?" - the time-windowed diff at
  the container level.

### System

- "What are the moving parts?" - the container-level view.
- "What is the shape?" - the community map at the system level.
- "Where do I start?" - the graph-generated trail for the
  system.

### Decision

- "What does this decision justify?" - the code nodes
  downstream of the decision.
- "What contradicts this decision?" - the edges and evidence
  that disagree with the decision's premise.

### Issue

- "What does this issue resolve?" - the code nodes that close
  the issue.
- "What cites this issue?" - the docs and decisions that
  reference it.

### Doc

- "What does this doc cite?" - the code nodes referenced from
  the doc.
- "What is this doc a rationale for?" - the code and decisions
  the doc supports.

## Progressive Disclosure Strategy

Progressive disclosure is the discipline of showing the user only
what they need at the moment, and giving them a path to the rest
when they are ready. The strategy has three layers.

### Layer 1: The focused object

The first thing a new user sees is the Object Inspector for the
focused object. It is the entry point. It is dense but
disciplined: definition on top, callers and callees next, evidence
last. The user can do useful work without leaving this panel.

The contextual help for the focused object is one click away. The
suggested questions are visible without a click. The glossary is
one more click.

### Layer 2: The contextual view

When the user clicks a suggested question, the contextual view
opens. The view shows the focused node, its same-level
neighbors, and its parents and children at adjacent levels, in
one panel. The user is now seeing the model.

The contextual help for the view is bound to the view id and
opens in the same panel. The lens selector appears when the user
is ready for it. The user does not see the lens selector until
they are in a view.

### Layer 3: The full graph

When the user is ready to navigate freely, the named view
mechanism, the ExplorerQL field (evolved from MoldQL), and the
cross-level climb-up are all available. The user has earned
the model. None of this is hidden; it is just not surfaced
until the user is moving. The language and grammar decisions
behind the query field are in `query-language-decision.md`.

### Triggers for layer promotion

The product does not gate layers behind settings. It promotes the
user as they act.

- First time the user focuses a node: layer 1 only.
- First time the user clicks a suggested question: layer 2
  surfaces stay; the lens selector appears.
- First time the user changes a lens: the named view mechanism
  becomes available, with a one-time explanation of what it is.
- First time the user opens the community map: the system-level
  projections become available.
- First time the user types in the ExplorerQL field (the
  evolved-from-MoldQL query field): the autocomplete surfaces
  and the error model become load-bearing.

The triggers are observable in telemetry and recoverable in
support. A user who skips a trigger is not punished; the trigger
re-fires when the user does the corresponding action.

## Onboarding Flows

The product ships with two onboarding flows. Both are content;
both are skippable; both are recoverable from the help menu.

### Flow 1: Open the example brain

The product ships with a small example brain. The first-run
flow opens it, focuses a hotspot, and walks the user through
the three layers above using the example's own nodes. The
user learns on data they did not have to bring.

### Flow 2: Open your own repo

The user points CogniCode at a local repository. The first-run
flow waits for the first snapshot, then surfaces a small set
of "what is the shape of this" prompts built from that
specific graph. The user learns on their own data, with
prompts that are honest about what the graph knows.

The two flows are not mutually exclusive. A new user can run
flow 1, close the example brain, and run flow 2 against their
own code, and the product state is consistent across both.

## What Help Is Not

Some things look like help and are not. The list below is what
the help strategy is not.

- A tour. Tours are scripted and stop being useful when the
  user goes off-script. The contextual help is reactive; it is
  always about what the user is looking at.
- A documentation site. The help is in the product. The
  documentation site is for the model itself (this set of
  docs) and for the API (the MCP reference).
- A tooltip factory. Tooltips are fine for short labels, not
  for explanations. Explanations live in the contextual help
  panel.
- A chat with an AI. The help is content. It does not invent;
  it does not hallucinate. It is the curated answer to the
  question the user is actually asking.
