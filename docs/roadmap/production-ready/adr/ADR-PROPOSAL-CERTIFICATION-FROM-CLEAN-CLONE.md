# ADR Proposal — Certifications are produced from a clean clone

**Estado:** Proposed

## Contexto
C8 se ejecutó sobre un working tree que contenía un source file ignorado por Git. El build local podía pasar aunque el SHA certificado no contenía el binario.

## Decisión propuesta
Toda certificación C# que afirme build/test de un artefacto MUST ejecutarse sobre un fresh clone o un checkout materializado desde el objeto Git, nunca sobre el working tree habitual.

## Reglas
- SHA congelado antes de empezar.
- tree limpio.
- todos los inputs trackeados.
- cada corrección cambia el SHA y reinicia los gates relevantes.
- el expediente registra toolchain y comandos.

## Consecuencias
Positivas: elimina dependencia de archivos locales y hace la evidencia transferible.  
Negativas: aumenta el tiempo de certificación; se mitiga con caches externas al checkout, no copiando working-tree state.
