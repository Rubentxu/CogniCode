# Registro de riesgos

| ID | Riesgo | Prob. | Impacto | Acciones afectadas | Mitigación |
|---|---|---:|---:|---|---|
| R-01 | Recertificar C8 revela más artefactos no trackeados | M | 5 | QW-03/04, CR-01 | clean clone como primer paso; no corregir ad hoc durante la misma certificación sin nuevo SHA |
| R-02 | Nueva rule `application_no_infrastructure` genera gran RED inicial | H | 4 | CR-06 | tratar primer run como inventario; baseline/allowlist temporal con fecha de retirada |
| R-03 | Refactor hexagonal cambia comportamiento | M | 5 | ST-01..05 | characterization tests + migración de un consumer por slice |
| R-04 | Optimización e91 cambia comunidades/resultados | M | 5 | CR-04 | golden fixture y semantic-equivalence test antes/después |
| R-05 | Perf gate flaky por ruido del runner | H | 3 | CR-05 | usar rango/percentile y runner estable; fallar por regresión significativa, no por ±5% |
| R-06 | OTel 0.28 rompe `/metrics` o OTLP | M | 4 | CR-07 | contract tests para exposition format y startup sin collector |
| R-07 | CI adaptativa omite suite relevante | M | 5 | CR-08 | `unknown => safe fallback`; test del selector con paths plantados |
| R-08 | Coverage gate incentiva tests de bajo valor | M | 3 | CR-09 | no imponer porcentaje global arbitrario; no-regression + zonas críticas |
| R-09 | División de mega-módulos aumenta superficie pública | M | 4 | ST-02..04 | medir public API count; aprobar solo interfaces más pequeñas que implementación ocultada |
| R-10 | Unificación graph-build elimina comportamiento legítimo | M | 5 | ST-05 | matriz de equivalencia y diferencias intencionales antes de borrar una ruta |
| R-11 | Pinning SHA de Actions se queda obsoleto | M | 2 | QW-05/06 | updater automático de Actions después de pinning |
| R-12 | v0.99.0 se publica antes de C8-R | L/M | 5 | CR-01 | release admission debe exigir referencia a certificado C8-R válido |

## STOP criteria

Detener la fase y abrir incidente si:

- un cambio de arquitectura modifica outputs públicos sin especificación;
- e91 mejora latencia degradando semantic equivalence;
- CR-08 permite merge con una suite que el mapping debía seleccionar;
- un certificado se genera con working tree sucio;
- aparece una vulnerabilidad nueva `deny` no documentada durante release.
