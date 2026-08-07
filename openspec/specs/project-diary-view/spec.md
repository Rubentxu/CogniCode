# ProjectDiary View Specification

## Purpose

Define the `ProjectDiary` ViewExecutor contract. A `ProjectDiary` renders a workspace's exploration history as a navigable narrative document composed of `ViewBlock` entries.

## Requirements

### Requirement: ProjectDiary ViewExecutor

The system SHALL provide a `ProjectDiary` ViewExecutor implementing the `ViewExecutor` trait. The executor SHALL accept a `Workspace` target via `InspectionTarget` and SHALL produce a `ContextualView` with `view_kind: ViewKind::ProjectDiary` and `renderer_kind: RendererKind::Composite`.

The executor MUST NOT perform I/O directly. It SHALL delegate to a pure shaper function that receives pre-resolved workspace data.

#### Scenario: Workspace with exploration sessions produces navigable narrative

- GIVEN a workspace with three saved exploration sessions, each containing navigation events
- WHEN the `ProjectDiary` executor's `build()` is called with the workspace target
- THEN a `ContextualView` is returned with `view_kind: ViewKind::ProjectDiary`
- AND the view contains one `ViewBlock` per exploration session, each carrying the session id, title, and event count

#### Scenario: Empty workspace produces placeholder view

- GIVEN a workspace with zero exploration sessions
- WHEN the `ProjectDiary` executor's `build()` is called with the workspace target
- THEN a `ContextualView` is returned with a single `ViewBlock` indicating no sessions exist
- AND the view SHALL NOT return an error

#### Scenario: Executor applies only to Workspace targets

- GIVEN a `ViewContext` whose `target` is not `InspectionTarget::Workspace`
- WHEN the `ProjectDiary` executor's `build()` is called
- THEN the executor SHALL return `ExplorerError::ViewNotAvailable`

### Requirement: ProjectDiary shaper signature

The pure shaper function for `ProjectDiary` SHALL follow the `ComposedNarrative` pattern: synchronous, deterministic, consuming pre-resolved data and producing a `ContextualView`. The shaper MUST NOT access ports directly.

#### Scenario: Shaper is deterministic

- GIVEN the same workspace data containing two exploration sessions
- WHEN the shaper is called twice with the same input
- THEN both invocations SHALL produce identical `ContextualView` outputs
- AND both SHALL have the same `blocks`, `title`, and `view_kind`

### Requirement: ProjectDiary descriptor

The `ProjectDiary` executor SHALL implement `ViewDescriptor` with:
- `id`: `"project-diary"`
- `applies_to`: `[InspectableObjectType::Workspace]`
- `view_kind`: `ViewKind::ProjectDiary`
- `renderer_kind`: `RendererKind::Composite`

#### Scenario: Descriptor metadata is consistent

- GIVEN the `ProjectDiary` executor's `ViewDescriptor` implementation
- WHEN its `id()`, `applies_to()`, `view_kind()`, and `renderer_kind()` are queried
- THEN each method SHALL return the value declared in the requirement above
