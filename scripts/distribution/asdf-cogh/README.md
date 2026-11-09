# asdf-cogh

asdf plugin that installs `cogh`, the CogniCode installer, from the GitHub
Release assets. It checks the tarball against the release `SHA256SUMS` and
stops on a mismatch. `cogh` then installs `cognicode` and `cognicode-mcp`
(`cogh install mcp-server --profile reviewer`). Only Linux x86_64 GNU is published.

asdf installs plugins from a git repository whose root holds `bin/`, so this
directory is published as its own repository,
https://github.com/Rubentxu/asdf-cogh. This directory stays the source; copy
changes there to publish them.

```sh
asdf plugin add cogh https://github.com/Rubentxu/asdf-cogh.git
asdf list all cogh
asdf install cogh 0.103.0
asdf set cogh 0.103.0
asdf reshim cogh
"$(asdf which cogh)" --version   # check the path, not only the name
```

No provenance attestation exists for 0.103.0; the checksum proves integrity
against the release manifest only.
