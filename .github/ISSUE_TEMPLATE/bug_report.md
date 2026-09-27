---
name: Bug report
about: Something behaves incorrectly
title: "[bug] "
labels: bug
assignees: ''
---

<!--
Do NOT use this template for security vulnerabilities.
Report those through GitHub Security Advisories ("Security" -> "Report a vulnerability"),
not as a public issue. See SECURITY.md.
-->

## What happened (actual behaviour)

<!-- The wrong behaviour, in one or two sentences. -->

## What you expected (expected behaviour)

<!-- What should have happened instead. -->

## Reproduction

<!--
The part that decides whether anyone can help. A repository, or a snippet, plus
the exact commands. "It does not work" is not a reproduction.
-->

Steps:

1.
2.
3.

## Logs

<!--
Paste relevant output. Note that logs go to **stderr**; stdout is the MCP
JSON-RPC protocol channel. If you expected to find something on stdout, say so
in the "What happened" section — that is itself a reportable defect.
-->

```
```

## Environment

| | |
|---|---|
| **Version** | <!-- `cognicode --version`, or the commit SHA if built from source --> |
| **Platform** | <!-- OS and architecture, e.g. Linux x86_64, macOS arm64, Windows x86_64 --> |
| **Install channel** | <!-- `cogh` / `install.sh` / `mise` / from source --> |
| **MCP profile** | <!-- e.g. core, reviewer; see product/profiles.json --> |
| **Language(s) involved** | <!-- And whether the language is `supported` or `experimental` in product/languages.json --> |

## Anything else

<!-- Contract surfaces involved: an MCP tool name, a CLI flag, a published
contract. Check product/README.md and product/tools.json first. -->
