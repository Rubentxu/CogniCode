# U112 — T5 release snapshot (binarios release profile)

**Fecha**: 2026-09-23
**Source-commit**: `018f50f86fb58969cab8dcf3d0c950414780aca2`
**HEAD**: 018f50f8 docs(prf): §111 — u51-cli02-stdio-split-regen reduce §102 a 13 perdidos
**Profile**: release (`cargo build --release --workspace`)
**Tier-1**: linux-x86-64, linux-aarch64 GNU

## Binarios construidos

| Binario | Versión | SHA-256 | Tamaño (bytes) |
|---|---|---|---|
| `cogh` | `cogh 0.97.4` | `2d497782ffaa370a1ebe2221d5b45bddd2ad2f826738279c174bbff18c0e48f2` | 6973832 |
| `cognicode` | `cognicode 0.97.4` | `251ffd6cbc923fffdfd1f0e8e91a9bf80b0dc42fe2f68b47cd095c8030756123` | 95455856 |
| `cognicode-mcp` | `cognicode-mcp 0.97.4` | `582596cf2edd85a609b257455cf9569123a28d83f0f014277a8f7c5b1e93c3e3` | 104185008 |
| `cognicode-mcp-server` | `cognicode-mcp-server 0.97.4` | `711595f92e046b97cb533b25f21ceb74a89a06a4d45e39d5a77d162f12710c42` | 106296368 |
| `cognicode-release` | `cognicode-release 0.97.4` | `838f4d68fcf78cd0b511fcd69dfc56f0007bb7b46aadd615ce7f3bfca2973f41` | 1490808 |

## Tier-1 platforms (release.yml matrix)

```
  - x86_64-unknown-linux-gnu
  - aarch64-unknown-linux-gnu
```

## Reproducibilidad

Comandos:

```bash
CARGO_TARGET_DIR=/var/home/rubentxu/cargo-targets/release \
  cargo build --release --workspace
```

Los SHA-256 de los binarios de `release/` son válidos solo para el
source-commit anterior. Cualquier modificación posterior al
workspace invalidaría estos hashes sin cambiar la versión declarada
(`0.97.4`) hasta que se haga un bump deliberado.

## Limitación honesta

`cognicode-mcp-server` es el bin real para CI/release (no
`cognicode-mcp`, que es un shim del path del operador). Ambos se
construyen desde `cognicode-mcp` crate; el bin principal se llama
`cognicode-mcp-server`. El snapshot incluye ambos para auditoría.

## Próximo paso (regla #5 SDDK release)

Si el operador autoriza el push, ejecutar:
```
git push origin main && git push origin v0.97.4
```
que dispara release.yml → build en runners nativos (linux-x86-64,
linux-aarch64) → R1-R9 verificación en CI.
