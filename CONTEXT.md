# CogniCode

CogniCode exists to make codebases operable by AI agents through structured
code intelligence, evidence, and safe action surfaces.

## Language

**Investigador agéntico de código**:
Sistema que formula hipótesis sobre un codebase, reúne evidencia estructural y
produce investigaciones accionables para agentes o humanos.
_Avoid_: scanner, clon de SonarQube, clon de CodeQL

**Señal**:
Observación barata y parcial que puede contribuir a una investigación, pero que
no constituye una conclusión por sí sola.
_Avoid_: finding definitivo, alerta final

**Patrón sospechoso**:
Señal estática que coincide con una forma de código asociada a riesgo, pero que
por sí sola no demuestra un defecto.
_Avoid_: bug confirmado, vulnerabilidad encontrada, finding definitivo

**Hipótesis**:
Proposición investigable sobre el codebase que puede ser sustentada o refutada
con evidencia.
_Avoid_: conclusión, diagnóstico final

**Teoría provisional**:
Hipótesis sustentada por evidencia suficiente para guiar acción, pero todavía
abierta a refutación por nueva evidencia.
_Avoid_: verdad definitiva, fallo confirmado permanente

**Refutación**:
Evidencia o argumento que invalida, debilita o acota una hipótesis.
_Avoid_: ignorar una señal, borrar evidencia incómoda

**Conocimiento histórico**:
Estado versionado de las hipótesis, refutaciones y casos que CogniCode sostenía
con la evidencia disponible en un momento dado.
_Avoid_: verdad global, estado actual sin procedencia

**Caso argumentado**:
Conclusión de calidad o seguridad acompañada de cadena causal, evidencia,
validación posible y remediación probable.
_Avoid_: issue aislado, finding plano

## Relationships

- Un **Investigador agéntico de código** combina múltiples **Señales** para
  producir un **Caso argumentado**.
- Un **Patrón sospechoso** es un tipo de **Señal** que requiere validación antes
  de convertirse en un **Caso argumentado**.
- Una **Hipótesis** puede evolucionar a **Teoría provisional** cuando acumula
  evidencia suficiente, o debilitarse mediante una **Refutación**.
- Un **Caso argumentado** representa **Conocimiento histórico**: es refutable y
  queda ligado a la evidencia disponible en un momento concreto.
- Una regla estática produce una **Señal**, no necesariamente un **Caso
  argumentado**.

## Example dialogue

> **Dev:** "¿CogniCode compite con SonarQube añadiendo más reglas?"
> **Domain expert:** "No. CogniCode usa reglas como señales dentro de una
> investigación agéntica que debe explicar la cadena causal y cómo validarla."
>
> **Dev:** "Entonces una concatenación SQL con entrada externa es una vulnerabilidad?"
> **Domain expert:** "No necesariamente. Es un patrón sospechoso; se convierte
> en caso argumentado cuando conectamos origen de datos, ruta de ejecución,
> sanitización ausente y una validación reproducible. Incluso entonces queda
> como teoría provisional refutable si aparece nueva evidencia."
>
> **Dev:** "¿Entonces nunca decimos que algo es verdad?"
> **Domain expert:** "Decimos qué conocimiento histórico sostiene CogniCode con
> la evidencia disponible, qué lo refutaría y qué acción recomienda."

## Flagged ambiguities

- "scanner" se usó como posible descripción del producto, pero se resolvió que
  el concepto central es **Investigador agéntico de código**.
- "bug", "vulnerabilidad" y "finding" no deben usarse para señales no
  validadas; el término correcto es **Patrón sospechoso** cuando solo existe
  coincidencia estática.
