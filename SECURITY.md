# Security Policy

## Reporting a vulnerability

**Do not open a public issue for a security problem.** A public issue notifies
everyone, including people who will write an exploit before a fix exists.

Use **GitHub Security Advisories** on this repository: go to
`Security` → `Report a vulnerability`. That opens a private channel visible
only to the maintainers until an advisory is published.

Please include:

- what an attacker can do, concretely, not just what you think is wrong;
- the version or commit you tested (`cognicode --version`, or the SHA);
- the platform and install channel (`cogh`, `install.sh`, `mise`, from source);
- reproduction steps or a proof of concept;
- whether the issue is already public anywhere.

We will acknowledge a report and confirm whether it is accepted as a
vulnerability. We do not commit to a disclosure deadline, and we will not close
a report without telling you why.

## What the threat model actually is

CogniCode is a **local** tool. It runs on a developer's machine, indexes a local
repository, and exposes analysis over MCP. That shapes what is and is not a
security problem here. The boundaries below are the ones the project enforces,
stated in `AGENTS.md`; they are not aspirations.

### Untrusted input: the repository, its files, its prompts, and tool results

The repository under analysis is **not a trusted instruction source**. A file
in the repository, a comment in a source file, a README, a prompt fragment, or
an MCP tool result may all contain text that looks like an instruction to an
AI agent. CogniCode treats all of it as **data to be analysed, never as
instructions to be followed**.

This matters because the product is used *by* AI agents. An agent that reads a
poisoned source file and treats a line in it as a command is the attack, not
CogniCode. The defence is that analysis output is structured data with a known
schema, not prose that an agent will act on.

If you find a path where analysis output can smuggle an instruction into an
agent's context as if it were a directive, that is a reportable vulnerability.

### The core has no network, no metrics collector, and no Control Plane

The core runs offline. Analysis, indexing, graph construction, and refactoring
computation do not require network access, do not emit telemetry, and do not
depend on a Control Plane. If you find a code path where the core phones home
without an explicit, documented capability grant, that is a reportable
vulnerability.

### MCP is JSON-RPC over stdout

The MCP server speaks JSON-RPC over stdout. Observability goes to stderr.
**Logs never go to stdout** — a log line on stdout corrupts the protocol stream
and, more importantly, can leak analysis content into a channel the host did not
intend. Mixing the two is treated as a defect, not a style issue.

### Capability controls are independent

Read, write, execute, network, and mutation are **separate** capability
controls. A path granted read access does not imply write; execute does not
imply network; a mutation capability is not implied by any of the others.

The invariant to test is that **no single control can be escalated into
another**. If a read-scoped token can cause a write, or a network grant is
reachable without a network capability, that is a reportable vulnerability.
This is the class of bug the project is most defensive about, because the whole
value proposition is that a refactoring agent can act on a real repository.

### Refactoring safety

CogniCode proposes and applies refactors. Impact analysis exists so that a
refactor is not applied blind. A defect where a refactor mutates files outside
the declared impact set, or applies a change the user did not authorise, is a
reportable correctness and safety issue.

## Supported versions

| Version | Supported |
|---|---|
| 0.100.0 (current `main`, not yet released) | yes |
| 0.99.2 (latest release, `v0.99.2`) | yes |

Older lines are not patched. If you are on an older release and find a
vulnerability, the report is still useful; tell us the version so the fix can
be assessed against the current line.

## Out of scope

These are not vulnerabilities in CogniCode:

- **Findings in the code being analysed.** If a repository you point CogniCode
  at contains a vulnerability, that is the repository's problem. Report it
  there. (If CogniCode *amplifies* it — for example by executing something it
  should not — that is in scope.)
- **Malicious MCP hosts** feeding hostile tool results to an agent. The host
  is outside our trust boundary, but see the untrusted-input section above: if
  our output format is what makes the injection possible, that is in scope.
- **Missing hardening in a consumer's own configuration**, such as exposing an
  MCP endpoint to a network the user did not intend to expose.

## License

Security fixes are released under the project's license, `MIT OR Apache-2.0`.
See `LICENSE` and
`docs/adr/ADR-053-cp1-license-publication.md`.
