# ExampleObject View Specification

## Purpose

Define the `ExampleObject` ViewExecutor contract. An `ExampleObject` view renders code usage examples for a symbol inline with documentation as a navigable narrative composed of `ViewBlock` entries.

## Requirements

### Requirement: ExampleObject ViewExecutor

The system SHALL provide an `ExampleObject` ViewExecutor implementing the `ViewExecutor` trait. The executor SHALL accept a `Symbol` target via `InspectionTarget` and SHALL produce a `ContextualView` with `view_kind: ViewKind::ExampleObject` and `renderer_kind: RendererKind::Composite`.

The executor MUST NOT perform I/O directly. It SHALL delegate to a pure shaper function that receives the pre-resolved `ResolvedSymbol` and any example data from the graph repository.

#### Scenario: Symbol with usage examples produces narrative view

- GIVEN a resolved symbol that has three usage examples stored in the graph
- WHEN the `ExampleObject` executor's `build()` is called with the symbol target
- THEN a `ContextualView` is returned with `view_kind: ViewKind::ExampleObject`
- AND the view contains one `ViewBlock` per example, each carrying the example code and source location

#### Scenario: Symbol without examples produces empty placeholder view

- GIVEN a resolved symbol that has zero usage examples in the graph
- WHEN the `ExampleObject` executor's `build()` is called with the symbol target
- THEN a `ContextualView` is returned with a single `ViewBlock` indicating no examples found
- AND the view SHALL NOT return an error

#### Scenario: Executor applies only to Symbol targets

- GIVEN a `ViewContext` whose `target` is not `InspectionTarget::Symbol`
- WHEN the `ExampleObject` executor's `build()` is called
- THEN the executor SHALL return `ExplorerError::ViewNotAvailable`

### Requirement: ExampleObject shaper signature

The pure shaper function for `ExampleObject` SHALL follow the `ComposedNarrative` pattern: synchronous, deterministic, consuming pre-resolved data and producing a `ContextualView`. The shaper MUST NOT access ports directly.

#### Scenario: Shaper is deterministic

- GIVEN the same symbol data and example set
- WHEN the shaper is called twice with the same input
- THEN both invocations SHALL produce identical `ContextualView` outputs
- AND both SHALL have the same `blocks`, `title`, and `view_kind`

### Requirement: ExampleObject descriptor

The `ExampleObject` executor SHALL implement `ViewDescriptor` with:
- `id`: `"example-object"`
- `applies_to`: `[InspectableObjectType::Symbol]`
- `view_kind`: `ViewKind::ExampleObject`
- `renderer_kind`: `RendererKind::Composite`

#### Scenario: Descriptor metadata is consistent

- GIVEN the `ExampleObject` executor's `ViewDescriptor` implementation
- WHEN its `id()`, `applies_to()`, `view_kind()`, and `renderer_kind()` are queried
- THEN each method SHALL return the value declared in the requirement above
