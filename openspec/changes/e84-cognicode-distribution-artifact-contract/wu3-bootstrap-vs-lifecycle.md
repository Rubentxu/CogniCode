# e84 WU3 — Bootstrap (Layer 0) vs Product Lifecycle (Layer 1)

Two layers, deliberately separated.

```text
LAYER 0 — install `cogh` itself
  provided by an EXTERNAL channel: install.sh | mise | aqua | Homebrew | asdf | Nix

LAYER 1 — `cogh` installs and manages CogniCode
  cogh install | cogh update | cogh uninstall | cogh doctor
  → ~/.cognicode/{install,shims,bundles,plugins,skills,...}
```

## Why the separation is not cosmetic

Today `cogh` is one binary that would have to solve both problems, and the
`Cogh` `ComponentKind` exists as a vestigial marker for the self-install it
deliberately does not do ("reserved for forward compat — cogh doesn't install
itself"). That reservation is the right instinct recorded in the wrong place.

Making the boundary explicit yields three properties:

1. **`cogh` need not compete with version managers.** Layer 0 is a problem
   already solved, well, by tools with larger ecosystems (mise and aqua in
   particular verify checksums and attestations themselves).
2. **The bootstrap is a single static binary.** Layer 0 has one artifact, one
   platform token, one digest. It is the only thing that must be trivially
   installable.
3. **Layer 1 stays CogniCode-specific.** The bundle/profile/manifest semantics
   have no analogue in a general tool manager and must not be generalized away.

## `cogh` is a lifecycle manager, not a version manager

Explicit non-goal: `cogh` does **not** become a generic language/tool version
manager. It does not gain plugin backends for arbitrary tools, does not manage
language runtimes, and does not reimplement asdf's plugin protocol.

What it keeps from the version-manager pattern is *mechanics*, not *scope*:
immutable per-version directories, shims on `PATH`, atomic install with rollback,
and a per-project pin. Those mechanics are what make it safe to install what it
installs.

## Consequence for the CLI surface

Layer 1 is the whole product surface (`install`, `update`, `uninstall`, `doctor`,
`current`, `list`, `where`, `reshim`, `ide`, `skill`). Layer 0 has no CLI at all
from our side — it is an artifact plus an installer script.

`cogh version` must therefore report **both** layers:
`cogh <bootstrap version>` and the installed CogniCode runtime version, since the
two can legitimately differ.
