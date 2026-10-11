# B2.W4 — Evidence Export y análisis reproducible

> **Estado**: ACTIVE 2026-10-11. Desbloqueado por B2.W2.
> Padre: [`b2-shared-analysis-capability-contracts.md`](b2-shared-analysis-capability-contracts.md).
> Spec autoridad: [`docs/roadmap/2026-Q4-CLI-MCP-ROADMAP.md`](../docs/roadmap/2026-Q4-CLI-MCP-ROADMAP.md) §B2.W4.

## Valor

El productor `assurance-evidence/v1` deja de mentir al consumidor
(pipelinek-assurance) y de romperse en escenarios realistas del AAT
(`>1024 entidades` → consumer reject inesperado, workspace sin archivos
→ Complete vacío, reorden de flags → resultados distintos). El envelope
que se firma con un digest M-D01 sigue siendo byte-estable cuando el
input no cambia; lo que cambia es el conjunto de cosas que el productor
rechaza producir porque sabe que el consumer lo rechazaría.

## Work units

- [ ] **T1** — TDD red: `tests/b2_w4_export_invariants.rs` con 6 tests
  que pinean el contrato. RED antes de tocar el producer.
- [ ] **T2** — Refactor `extract_from_workspace` para que:
  - (a) el walk se haga una sola vez y antes de procesar capacidades;
  - (b) el orden de las capacidades en `requested` no afecte al
    envelope (commutative);
  - (c) las colecciones se truncen a `MAX_COLLECTION_SIZE` con
    `Partial { gaps }` honesto;
  - (d) `Complete` requiera colección no vacía (gaps honestos);
  - (e) `Unsupported { reason }` ya existente siga siendo no-vacío.
- [ ] **T3** — Re-verificar: golden SHA cambia materialmente (CogniCode
  workspace tiene 4671 `.rs` > `MAX_COLLECTION_SIZE=1024`). Regenerar
  golden y bumpear pin. Documentar el delta en el commit message.
- [ ] **T4** — Gate: clippy `-D warnings`, fmt, fast + full.

## Invariantes pineadas por T1

1. **Order independence**: `[entities, architecture, relations]` y
   `[architecture, relations, entities]` producen entities/facts
   byte-iguales (orden canónico por `id`).
2. **Single extraction**: una sola pasada del walk. Test indirecto:
   `[facts, entities]` produce entities no-vacías Y facts no-vacíos
   consistentes.
3. **Symmetric limits**: el producer trunca sus colecciones a
   `MAX_COLLECTION_SIZE`; consumer las rechaza al mismo número.
4. **Effective worktree identity**: `subject.revision` y
   `provenance.subject_revision` ambos iguales a `git rev-parse HEAD`
   (o al nombre del directorio si no es git repo).
5. **Honest coverage**: para cada capability del envelope:
   - `Complete` ⟹ colección no-vacía;
   - `Partial` ⟹ `gaps` no-vacío;
   - `Unsupported` ⟹ `reason` no-vacío.
6. **Reference consistency**: cada `Fact.source_anchor_ref` resuelve
   a un `SourceAnchor`; cada `SourceAnchor.file` es un path real bajo
   el workspace al tiempo de emisión.

## Acceptance criteria

- Los 6 tests en `b2_w4_export_invariants.rs` pasan.
- Los 17 tests en `assurance_export_roundtrip.rs` siguen verdes (no
  regresión de roundtrip).
- `bash scripts/test-full.sh` exit 0.
- `cargo clippy -p cognicode-core --tests -- -D warnings` limpio.
- `cargo fmt --all --check` limpio.
- `python3 scripts/ci/test_assurance_export_golden.py` exit 0
  (con pin actualizado tras cambio material del producer).

## Restricciones (heredadas)

- Sin nuevas dependencias (sólo `serde`, `ciborium`, `sha2`).
- Forma del envelope no cambia (orden de campos, nombres, discriminadores).
- AAT-B2-04 (consumidor Kotlin real) queda fuera de scope; los
  cambios estructurales deben facilitar esa validación futura pero
  no la implementan.
- Capabilities declaradas `Unsupported` (relations, signals,
  architecture) NO se promocionan a Complete sin extracción
  verificable (regla del goal doc).

## Recibo (append-only)

| Fecha | WU | SHA | Notas |
|---|---|---|---|
| 2026-10-11 | T1 (RED) | _pendiente_ | `b2_w4_export_invariants.rs` con 6 tests rojos. |
| _pendiente_ | T2 | _pendiente_ | Refactor `extract_from_workspace`. |
| _pendiente_ | T3 | _pendiente_ | Golden regenerado y pin actualizado. |
| _pendiente_ | T4 | _pendiente_ | Gate full verde. |

## Hallazgos preliminares

- **CogniCode workspace tiene 4671 `.rs`** ≫ `MAX_COLLECTION_SIZE=1024`.
  El producer actual emite los 4671 entities y el consumer los rechaza
  silenciosamente con `Bounded`. El golden SHA pre-W4
  (`5cc8fdf3...`) representa el envelope "honesto" en cuanto a su
  contenido pero "peligroso" en cuanto a la paridad producer/consumer.
  B2.W4 corrige eso y el SHA cambia materialmente.

- **El walk se hace después de procesar la primera capability que lo
  pide.** Si `facts` viene antes que `entities` en `--capability`,
  `facts` ve `entities=[]` y produce 0 facts. El envelope depende del
  orden: bug de no-conmutatividad. El refactor lo pone antes.

- **`Complete` no verifica no-vacío.** Si el workspace tiene 0 `.rs`,
  el envelope emite `entities=[]` con `Complete`. Falso positivo. El
  refactor usa `Partial { gaps }` con motivo "no se encontraron
  archivos Rust" cuando aplica.

## Riesgos

- **R-B2-W4-1**: regenerar el golden requiere coordination con el
  consumer (pipelinek-assurance). El pin actual
  (`5cc8fdf3ed36bc4d3bdb615ac85530f901e23991a9524c2f7d928395c37350d5`)
  cambia. Documentar el delta y avisar al consumer en el PR.
- **R-B2-W4-2**: el fixture de tests con >1024 archivos tarda ~0.1s en
  crearlos. Aceptable pero medible. Si se hace lento, usar un mock en
  lugar de filesystem real.
- **R-B2-W4-3**: la conmutatividad exige canonicalizar `entities` por
  `id` antes de comparar. Si los ids no son estables (dependen del
  timestamp del filesystem), la canonicalización falla. En la
  práctica, los ids son paths absolutos canonicalizados y no
  dependen de timestamps.

## Out of scope

- AAT-B2-04 (consumidor Kotlin real).
- Promoción de capabilities Unsupported a Complete.
- Cambios en `envelope.rs` schema (orden de campos, discriminadores).
- Cambios en `serialize.rs` (decode ya pineado por roundtrip tests).