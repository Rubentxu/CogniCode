# DISTRIBUTION-FIRST RELEASE POLICY — Obligatoria para aplicaciones

> **Estado:** AUTORIDAD VIGENTE. Aplica a toda release de **aplicación**
> de CogniCode. Las bibliotecas o SDK publicados en Maven/Nexus se
> rigen por sus propias especificaciones públicas (ver §7 al pie).

## 1. Principio de distribución

Una release de una aplicación es un **producto instalable y utilizable
por el consumidor**. No es un backup del repositorio, un volcado del
workspace ni un paquete de evidencias del agente.

**PROHIBIDO publicar código fuente o artefactos internos como assets
de una release de aplicación.**

Solo podrán publicarse distribuciones ejecutables reales y los
metadatos mínimos exigidos para verificar su integridad, procedencia
y cumplimiento de los contratos de distribución vigentes.

## 2. Artefactos permitidos

**Producto:**

- Binarios ejecutables nativos: ELF, EXE, app bundles, etc.
- Distribuciones instalables: ZIP, tar.gz, deb, rpm, msi, dmg, etc.
- Distribuciones JVM con lanzadores, bytecode JAR, dependencias y
  runtime requerido o documentado.
- SDK compilado independiente, solo cuando sea un producto
  distribuible con consumidores y criterios de instalación
  verificados.

**Integridad obligatoria cuando lo establezca el contrato:**

- `SHA256SUMS` o mecanismo equivalente autorizado.
- SBOM y manifiesto de integridad si la especificación o los ADR
  vigentes los exigen.

No publicar un artefacto auxiliar si carece de una función de
distribución o verificación demostrable. No duplicar los mismos
bytes bajo nombres diferentes sin una necesidad de consumidor
acreditada.

## 3. Artefactos prohibidos como assets

- Archivos fuente del repositorio, incluidos source ZIP y source
  tarball creados por los agentes.
- Paquetes `-sources.jar` de aplicaciones.
- Código de tests, UAT, E2E, AAT, fixtures y corpus interno.
- `HANDOFF.md`, reportes de agentes, logs, trazas, resultados XML,
  estadísticas de tests o inventarios internos.
- `git-head.txt`, clasificaciones, informes de impacto y otros
  recibos que no sean contratos públicos de distribución.
- Directorios `src/`, `test/`, `.git/`, `.worktrees/`, `build/` o
  equivalentes, salvo los componentes compilados expresamente
  requeridos para ejecutar la aplicación.
- Archivos temporales, caches y artefactos intermedios.

Los datos necesarios para auditar internamente una release se
conservan en el repositorio, su almacenamiento de evidencias o el
certifier propietario. **No se suben indiscriminadamente a GitHub
Releases.**

Las notas públicas de la release describen cambios funcionales,
compatibilidad, instalación, digest y limitaciones conocidas.

## 4. Separación de autoridades

- **Repositorio de desarrollo:** código, tests, ADR, roadmap e
  historial Git.
- **Release pública:** distribuciones consumibles y metadatos
  normativos mínimos.
- **Certifier/harness:** ejecución externa, resultados UAT/E2E,
  trazas y recibos de certificación.
- **Instaladores / package managers:** consumen los artefactos ya
  publicadas; nunca reconstruyen el producto.

No confundir un source archive automático de GitHub con un binario
de distribución.

Los agentes nunca usarán rutas `/archive/refs/tags/` ni `/zipball/`
como enlaces de instalación de la aplicación.

## 5. Release gate obligatorio

Antes de publicar:

1. Construir la distribución desde el commit autorizado y verificar
   que contiene una aplicación realmente ejecutable.
2. Instalar el artefacto en un entorno limpio y ejecutar las
   comprobaciones de identidad y funcionamiento pertinentes.
3. Comprobar que **no** contiene fuentes, tests, fixtures o archivos
   internos innecesarios.
4. Comprobar nombre, versión, plataforma, arquitectura, contenido,
   SHA-256 y dependencias de runtime.
5. Generar el inventario exacto de assets que van a publicarse.
6. Rechazar todo asset que no pertenezca a la allowlist de
   distribución vigente.
7. Ejecutar los gates normativos correspondientes al riesgo y a la
   candidata.
8. Publicar la release, preferentemente desde draft con el conjunto
   completo de assets verificado.
9. Consultar GitHub Releases y comprobar nombres, cantidades,
   digests y enlaces de descarga reales.

**FAIL-CLOSED:** release sin binario usable, con binarios
incompletos o con fuentes añadidos por el agente =
`PUBLICATION_REFUSED`.

Un ZIP que solo contiene el código del repositorio nunca satisface
la condición de distribución ejecutable.

## 6. Inmutabilidad y trazabilidad

- Los tags de release deben cumplir la autoridad Git vigente.
- La identidad del producto debe coincidir entre nombre del asset,
  contenido, metadatos y ejecución.
- Los artefactos publicadas son **inmutables**.
- **No modificar ni reconstruir los bytes después de obtener el
  digest aprobado.**
- La evidencia de certificación permanece accesible mediante
  referencia estable, sin necesidad de incorporarla como asset
  público.
- Una release candidate publicada **no equivale** a certificación
  externa.

## 7. Regla de aceptación final

Una entrega de aplicación solo puede declararse `PUBLISHED` cuando
existen artefactos distribuibles reales, accesibles y verificables
por sus consumidores.

**Publicar un tag, un source archive o un conjunto de documentos NO
constituye una entrega del producto.**

---

> **Excepción declarada:** Esta política **no modifica
> automáticamente** los contratos de bibliotecas o SDK publicadas en
> Maven/Nexus. Sus artefactos y metadatos se rigen por sus propias
> especificaciones públicas.

## Relación con el resto del roadmap

- `AGENTS.md` — autoridad de gobernanza; sección "Resultados
  production-ready" refuerza esta política.
- `docs/roadmap/ROADMAP.md` — única autoridad de agenda; la
  iniciativa B0 (VDH) implementa esta política en su gate de
  release-candidate.
- `docs/prf/specs/SPEC-DISTRIBUTION.md` — especificación técnica
  PRF (CERRADA) que precede a esta política de orden superior.
- `docs/cognicode_vdh_roadmap_fp_aceptacion_efimera_2026-10-15/`
  — overlay VDH; `RELEASE-PROTOCOL.md` y `B0-UAT-01/AAT-01/E2E-01`
  la aplican al ciclo de B0.
- `docs/adr/ADR-034-cognicode-distribution-package.md` — ADR de
  distribución de CogniCode; subordinado a esta política.
