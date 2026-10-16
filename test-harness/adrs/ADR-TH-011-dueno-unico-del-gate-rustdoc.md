# ADR-TH-011 — El gate de rustdoc se paga una vez: dueño único en integración

**Estado:** ACCEPTED. **Fecha:** 2026-10-15. **Contexto:** mandato del operador de
2026-10-15 (segundo bloque) para bajar el coste de iteración del `merge-gate` sin
perder garantías. **Autoridad del gate:** cambia el dueño de una obligación, con
contrato que lo verifica.

## Contexto

Perfil medido del gate de merge en el ledger durable de PipelineK (run `1b37e37b`:
58 etapas, 1 630 s): las diez primeras etapas son el 94% del tiempo y
`rustdoc-gate` sola es el 41% (667 s). El target `m011_rustdoc_gate` ejecuta un
`cargo doc` completo del workspace y comprueba que no haya avisos de las
categorías de M0.11.

Ese mismo target lo ejecutaba **también** la lane de integración, dentro de
`cargo test --workspace --all-targets`. Es decir: cada cambio pagaba dos veces una
comprobación que solo puede emitir un veredicto. No es una etapa lenta sin
alternativa: es trabajo duplicado.

## Decisión

1. **Dueño único: la lane de integración.** La stage se retira de
   `merge-gate.pipeline.kts` y se nombra explícitamente en
   `integration.pipeline.kts` (antes era implícita dentro de `--all-targets`, que
   la hace ejecutar sin que el contrato pueda verificar quién es el dueño).

2. **La garantía no se retira, cambia de momento.** El workspace sigue teniendo que
   mantener cero avisos de rustdoc en las categorías gateadas, verificado por el
   mismo target, en una lane que bloquea `main`. Lo que cambia es que un enlace
   roto deja de bloquear el PR y bloquea la integración posterior (fix-forward).

3. **El contrato pasa de «el merge lo ejecuta» a «exactamente una autoridad lo
   ejecuta, y no es el merge».**
   `core_gate_coverage_contract::the_rustdoc_gate_runs_in_exactly_one_authority`
   falla si el gate desaparece de todas las lanes («runs in no pipeline, so it
   gates nothing») y si vuelve a aparecer en el merge («must run in exactly one
   authority», con la lista de runners). La aserción anterior no detectaba la
   duplicación; esta detecta las dos direcciones del error.
   El contrato sigue viviendo en el crate de core, que el merge ejecuta sin
   restricciones: si alguien borra el wiring entero, el contrato falla igualmente.

4. **Nada se movió por ser lento.** Solo se movió la etapa que ya se ejecutaba dos
   veces, y la medición lo acompaña (`docs/07-auditoria-merge-gate.md` §7 y §8).

## Consecuencias

- El gate de merge deja de pagar la etapa: 667 s medidos in-gate en la corrida
  histórica (run `1b37e37b`) y 737 s standalone en esta máquina; el ahorro por
  iteración se declara como rango (680–737 s, ≈55%) porque sumar la medición
  standalone al total del gate sobreestima el «antes».
- El presupuesto de 6–7 minutos **no se alcanza** con este cambio: quedan ~963 s.
  El camino medido es que el merge ejecute las pruebas **afectadas** (selección
  por impacto, ya medida contra CR-08 en `parity`) y que la batería completa siga
  en integración. Ese siguiente paso exige wire the harness into the lane y su
  propio ADR.
- Un fallo de rustdoc se descubre después del merge, en la lane de integración.

## Lo que este ADR no decide

- Mover más etapas de merge a integración o candidato. Cada una necesita su
  medición y su razón de dueño.
- La selección por impacto dentro del merge (siguiente bloque, W6 del roadmap del
  harness).
- El destino de los advisories de `cargo deny`, que hoy viven en merge y dependen
  de una base externa.
