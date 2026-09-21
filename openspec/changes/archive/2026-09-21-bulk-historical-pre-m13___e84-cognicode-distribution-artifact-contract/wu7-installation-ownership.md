# e84 WU7 — Installation Ownership

## The decision

```text
the external installation channel  OWNS `cogh` (Layer 0)
`cogh`                             OWNS the installed CogniCode runtime (Layer 1)
```

Neither owns the other's artifacts. Ownership is exclusive and unambiguous, so
there is never a question of who may replace what.

## Therefore `cogh update` means

```text
update the installed CogniCode runtime / bundle     ← Layer 1
```

It does **NOT** mean:

```text
replace the currently running `cogh` executable     ← Layer 0
```

This resolves the gap between the two layers cleanly: if `cogh` were to replace
itself, it would be overwriting a file owned by mise/Homebrew/aquaproj — which is
exactly the class of bug that makes a tool hostile to package managers.

## `cogh self-update` (future, conditional)

If a self-update is ever added it must be **provenance-aware**:

```text
detect how the running cogh was installed
  ├── external channel detected (mise / brew / aqua / asdf / nix / system pkg)
  │     → REFUSE, and print the channel's own upgrade command
  └── standalone install.sh / manual
        → may self-update
```

Rules:
1. It must never fight a package manager for ownership of the same file.
2. When it refuses, it must tell the user the exact command for their channel.
3. It must be able to say "I do not know how I was installed" and default to
   refusing.
4. Silent self-replacement is forbidden.

## Layer 1 lifecycle is where the work is

The commands that must be real (e86):

```text
cogh install            → install a bundle version for a profile
cogh current            → the pinned/active runtime version
cogh latest             → the newest available runtime version
cogh update             → move the runtime to a newer version, transactionally
cogh rollback           → return to the previous runtime version
cogh uninstall          → remove an installed runtime version (and its shims)
cogh doctor             → diagnose both layers
```

`cogh rollback` is the command today's surface does not have and that the
transactional engine already makes possible: the journal and per-version install
directories are exactly the substrate needed.

Note this makes `cogh uninstall` a real defect today, not merely a stub: it prints
a line and never removes `~/.cognicode/install/<version>/`. An uninstall that
leaves the runtime installed is worse than an unimplemented one.
