# Spec: diagram-snapshot-export

**Domain**: explorer
**Change**: e20-4-svg-snapshot
**Source**: SDD spec — synced from `sddk/e20-4-svg-snapshot/spec.md` (engram)
**Status**: ACTIVE

---

## Capability: diagram-snapshot-export

Renders Mermaid text to PNG/SVG for static documentation output. Final leg of ADR-003 Rule 1 pipeline: Mermaid → SVG/PNG.

---

## Requirements

### Requirement: PNG Rendering

The system SHALL render Mermaid text to PNG format and return it with `Content-Type: image/png`.

**Scenario: PNG render success**
- Given a valid Mermaid diagram text
- When `SnapshotService::render(mermaid, Png)` is called
- Then the result is a valid PNG image byte sequence
- And `Content-Type: image/png` is set in the response

---

### Requirement: SVG Rendering

The system SHALL render Mermaid text to SVG format and return it with `Content-Type: image/svg+xml`.

**Scenario: SVG render success**
- Given a valid Mermaid diagram text
- When `SnapshotService::render(mermaid, Svg)` is called
- Then the result is a valid SVG image byte sequence
- And `Content-Type: image/svg+xml` is set in the response

---

### Requirement: Format Dispatch

The system SHALL dispatch to the correct renderer based on the `format` parameter (`png` or `svg`).

**Scenario: Format = png**
- Given `format=png` in the request
- When the request is processed
- Then `SnapshotFormat::Png` is used
- And PNG output is returned

**Scenario: Format = svg**
- Given `format=svg` in the request
- When the request is processed
- Then `SnapshotFormat::Svg` is used
- And SVG output is returned

**Scenario: Invalid format**
- Given an invalid `format` value
- When the request is validated
- Then HTTP 400 Bad Request is returned

---

### Requirement: ViewKind Whitelist

The system SHALL only accept snapshot requests for approved ViewKinds.

**Allowed ViewKinds**: `c4_context`, `c4_container`, `c4_component`, `call_graph`, `impact_radius`, `vertical_slice`

**Scenario: Allowed ViewKind**
- Given a `view_kind` in the whitelist
- When the request is validated
- Then the request proceeds to rendering

**Scenario: Disallowed ViewKind**
- Given a `view_kind` NOT in the whitelist
- When the request is validated
- Then HTTP 400 Bad Request is returned

---

### Requirement: REST Endpoint

The system SHALL expose `GET /api/workspaces/:workspace_id/snapshot` with query parameters `view_kind`, `target`, and `format`.

**Scenario: Snapshot endpoint returns 200**
- Given valid `workspace_id`, `view_kind`, `target`, and `format`
- When GET `/api/workspaces/:id/snapshot?view_kind=&target=&format=png|svg` is called
- Then HTTP 200 is returned with the image
- And `Content-Disposition: attachment` header is set with a sanitized filename

**Scenario: Workspace not found**
- Given a non-existent `workspace_id`
- When the request is made
- Then HTTP 404 Not Found is returned

---

### Requirement: MCP Tool `export_snapshot`

The system SHALL provide an MCP tool `export_snapshot` (gated on `multimodal` feature) that accepts `view_kind`, `target`, and `format` parameters.

**Scenario: MCP tool returns base64 image**
- Given a valid `view_kind`, `target`, and `format`
- When `export_snapshot` is called via MCP
- Then the result is `{ image: base64_string, format: "png"|"svg" }`

**Scenario: MCP tool gated without multimodal**
- Given the binary is compiled without `--features multimodal`
- When MCP tools are listed
- Then `export_snapshot` is NOT present

---

### Requirement: C4 Toolbar Download Buttons

The system SHALL provide "Download PNG" and "Download SVG" buttons in the C4 Toolbar component.

**Scenario: C4 Toolbar renders download buttons**
- Given the C4 Toolbar is displayed
- Then "Download PNG" and "Download SVG" buttons are visible
- And clicking calls `fetchSnapshot` with the correct parameters

---

### Requirement: Inspector Export Menu

The system SHALL provide "Download as PNG" and "Download as SVG" options in the ExportMenu dropdown.

**Scenario: ExportMenu shows snapshot options**
- Given the ExportMenu dropdown is open
- Then "Download as PNG" and "Download as SVG" menu items are present
- And clicking calls `fetchSnapshot` with the correct workspace ID and parameters

---

## Error Handling

| Error Condition | HTTP Status | Error Code |
|-----------------|-------------|------------|
| Empty Mermaid input | 400 | `empty_input` |
| Input size > 1 MB | 413 | `size_limit_exceeded` |
| `mmdc` binary not found | 503 | `mmdc_not_found` |
| Render failure | 500 | `render_failed` |
| Render timeout (>30s) | 504 | `timeout` |
| Invalid `view_kind` | 400 | `invalid_view_kind` |
| Invalid `format` | 400 | `invalid_format` |

---

## Edge Cases

| Condition | Expected Behavior |
|-----------|------------------|
| `mmdc` not installed | 503 `FeatureDisabled` on REST; tool absent on MCP |
| Puppeteer/Chromium crash | 500 `render_failed` with stderr |
| Empty Mermaid (whitespace only) | 400 `BAD_REQUEST` |
| Diagram size > 1 MB | 413 `PAYLOAD_TOO_LARGE` |
| Unknown ViewKind | 400 `BAD_REQUEST` |
| Target symbol not found | 404 `NOT_FOUND` |
| `Content-Disposition` injection attempt | Filename sanitized (alphanumerics only) |
