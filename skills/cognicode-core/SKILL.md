---
name: cognicode-core
description: >
  Core CogniCode workflow — analyze, build, test, deploy.
  Trigger: When a task involves understanding or modifying the
  CogniCode codebase (graph store, MCP server, Explorer UI).
  Applies to: exploration, impact analysis, refactoring, E2E testing.
license: MIT
metadata:
  version: "0.92.0"
  maturity: stable
  author: CogniCode Team
  homepage: https://github.com/Rubentxu/CogniCode
---

# Objective

Provide a portable entry point to the CogniCode workflow. This skill
is IDE-agnostic — drop the `compatibility: opencode` field from
forked copies.

# Required process

1. Resolve the workspace: `cd /var/home/rubentxu/Proyectos/rust/CogniCode`
2. Inspect the ROADMAP: `cat docs/ROADMAP.md | head -30`
3. Run the post-e31 audit: `just post-e31-audit`
4. Find the right ADRs: `ls docs/adr/`

# Skills layered on top

- `cognicode-mcp-driven` — uses the MCP server
- `cognicode-sandbox` — runs sandbox scenarios
- `cognicode-uat` — runs UAT plan
