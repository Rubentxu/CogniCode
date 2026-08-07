# Narrative Store Specification

**Version**: 0.1.0
**Date**: 2026-08-05
**Status**: draft
**Change**: `e14-narrative-runtime-cycle-2`
**Tags**: [port, persistence, ladybugdb, narrative-view]

## Purpose

Defines the `NarrativeStore` port trait for persisting rendered `ContextualView` outputs as a snapshot cache. Shapers remain synchronous; I/O lives behind the port. Backed by LadybugDB's `NarrativeView` node table.

## Requirements

### Requirement: Snapshot Save with Upsert

The system MUST persist a rendered `ContextualView` snapshot to the `NarrativeView` table. Subsequent saves for the same (workspace, view_id, object_id) tuple SHALL update the existing row.

#### Scenario: Save snapshot creates new row

- GIVEN a rendered `ContextualView` for workspace "ws1", view_id "project-diary", object_id "obj-A"
- WHEN `NarrativeStore::save_snapshot` is called
- THEN the snapshot is persisted to the `NarrativeView` table
- AND the row contains the serialized `ContextualView` payload, view_kind, and source_rev

#### Scenario: Save snapshot upserts on duplicate key

- GIVEN a snapshot already exists for ("ws1", "project-diary", "obj-A")
- WHEN `save_snapshot` is called again with updated content for the same tuple
- THEN the existing row is updated with the new payload and source_rev
- AND no duplicate row is created

### Requirement: Snapshot Load with Cache Hit

The system SHALL return a cached `ContextualView` when a snapshot exists for the given lookup key, without invoking any shaper.

#### Scenario: Cache hit returns stored snapshot

- GIVEN a snapshot exists for ("ws1", "example-object", "obj-B")
- WHEN `NarrativeStore::load_snapshot("ws1", "example-object", "obj-B")` is called
- THEN the stored `ContextualView` is returned
- AND no shaper is invoked

### Requirement: Snapshot Load with Cache Miss

The system SHALL return `Ok(None)` when no snapshot exists for the given lookup key.

#### Scenario: Cache miss returns None

- GIVEN no snapshot exists for ("ws1", "composed-narrative", "obj-C")
- WHEN `load_snapshot("ws1", "composed-narrative", "obj-C")` is called
- THEN `Ok(None)` is returned

### Requirement: List Snapshots for Workspace

The system MUST return all snapshots for a workspace, with an optional filter by view_kind.

#### Scenario: List all snapshots for a workspace

- GIVEN snapshots exist for "ws1" across multiple view_kinds
- WHEN `list_for_workspace("ws1", None)` is called
- THEN all snapshots for "ws1" are returned

#### Scenario: List snapshots filtered by view_kind

- GIVEN snapshots exist for "ws1" with view_kinds "project-diary" and "example-object"
- WHEN `list_for_workspace("ws1", Some("project-diary"))` is called
- THEN only snapshots with view_kind "project-diary" are returned

### Requirement: Cache Invalidation by Source Revision

The system MUST delete all snapshots for a workspace whose `source_rev` is less than or equal to a given threshold, and SHALL return the count of deleted rows.

#### Scenario: Invalidate stale snapshots

- GIVEN workspace "ws1" has 3 snapshots with source_rev values 3, 5, and 7
- WHEN `invalidate("ws1", 5)` is called
- THEN snapshots with source_rev 3 and 5 are deleted
- AND the snapshot with source_rev 7 remains
- AND the method returns count 2

#### Scenario: Invalidate with no matching rows

- GIVEN workspace "ws1" has snapshots, all with source_rev >= 10
- WHEN `invalidate("ws1", 5)` is called
- THEN no snapshots are deleted
- AND the method returns count 0

### Requirement: Graceful Degradation on Missing Table

The system SHALL NOT panic or produce undefined behavior when the underlying `NarrativeView` table does not exist. All operations MUST return a descriptive error.

#### Scenario: Save fails on missing table

- GIVEN the `NarrativeView` table does not exist in the database
- WHEN `save_snapshot` is called
- THEN an error is returned indicating the table is missing

#### Scenario: Load fails on missing table

- GIVEN the `NarrativeView` table does not exist
- WHEN `load_snapshot` is called
- THEN an error is returned indicating the table is missing
