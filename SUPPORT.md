# Support

This project has **no paid support, no support SLA, and no support contract**.
Anyone promising you an SLA on behalf of CogniCode is not the maintainer. What
follows is what is actually available.

## Where to ask, and where to report — these are different

| You want to | Go to |
|---|---|
| **Ask a usage question** ("how do I configure X?") | A GitHub issue, or Discussions if enabled on the repository |
| **Report a bug** ("this is broken, here is how to reproduce") | A GitHub issue using the **bug report** template |
| **Propose a feature** | A GitHub issue using the **feature request** template |
| **Report a security vulnerability** | **GitHub Security Advisories** — never a public issue. See `SECURITY.md` |
| **Report unacceptable conduct** | The same private channel. See `CODE_OF_CONDUCT.md` |
| **Contribute a change** | Read `CONTRIBUTING.md` first, then open a pull request |

Using the right channel matters: a security report posted publicly is a
disclosure, and a conduct report posted publicly names the person you are
reporting before anyone has looked at it.

## Before you file an issue

Most questions are answerable from the versioned artifacts, which are kept
current on purpose:

- `product/README.md` — the product contract: what ships, what does not, and
  what each MCP tool actually returns.
- `product/tools.json` — the generated tool surface. If a tool is not in here,
  it is not a shipped tool, whatever the README says.
- `product/languages.json` — language support with evidence. Languages are
  marked `supported` or `experimental`; an `experimental` language is parsed and
  indexed, not fully analysed. Do not file a bug about an experimental language
  behaving experimentally.
- `product/profiles.json` — the install profiles and their stability.
- `docs/distribution/INSTALL.md` — installation, upgrade, rollback, uninstall,
  and platform support.

If the answer is in one of those, you found it faster than a maintainer can
respond. If the artifacts are *wrong*, that is a real bug: report the
discrepancy between the artifact and the behaviour, with both.

## What makes an issue answerable

For a bug, the four facts that decide whether anyone can reproduce it:

1. **Version** — `cognicode --version`, or the commit SHA if built from source.
2. **Platform** — OS, and architecture if not the obvious one.
3. **Install channel** — `cogh`, the `install.sh` bootstrap, `mise`, or from
   source. Behaviour differs between channels, especially around profiles.
4. **A minimal reproduction** — a repository or a snippet, the command you ran,
   and the output you got. Not "it doesn't work".

Include logs when you have them. Note that logs go to **stderr**; if you were
expecting to find something on stdout, that itself is worth reporting, because
stdout is the MCP protocol channel.

## What to expect

Responses are best-effort. Issues get triaged, not answered on a schedule.
A report that includes a minimal reproduction is far more likely to be acted on
than one that says something is broken.

If your issue is a disagreement with a design decision rather than a defect,
say so. You will get a real answer, and possibly a pointer to the ADR or roadmap
entry that explains the decision. Check `docs/roadmap/ROADMAP.md` and
`docs/adr/` before assuming the decision was arbitrary.

## Reporting something you found that is not a bug

You do not need to be sure. "I expected X and got Y" is useful even when the
answer is "that is intended, and here is why". What is not useful is a report
that asserts a defect without a reproduction; that costs maintainer time without
adding information.

## License

Support and fixes are provided under the project's license, `MIT OR Apache-2.0`.
See `LICENSE`.
