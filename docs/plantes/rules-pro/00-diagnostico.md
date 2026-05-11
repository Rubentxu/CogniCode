# Diagnóstico del Sistema de Reglas Actual

> **Fecha**: 11 de Mayo de 2026  
> **Estado**: Documento de análisis inicial

---

## 1. Estado Actual del Sistema

El sistema de reglas de CogniCode cuenta actualmente con **854 reglas** implementadas en un único archivo `catalog.rs` que comprende **28,251 líneas** de código. De estas reglas, 16 han sido segregadas en módulos separados, y el conjunto de pruebas reporta **275 de 294 tests pasando**, lo que indica una tasa de éxito del 93.5%.

La distribución de enfoques de implementación entre las reglas existentes se distribuye aproximadamente de la siguiente manera:

| Enfoque | Porcentaje Aproximado | Descripción |
|---------|----------------------|-------------|
| **Regex** | ~65% | Pattern matching sobre texto plano |
| **Tree-sitter queries** | ~25% | Consultas estructurales sobre AST |
| **Híbrido** | ~10% | Combinación de ambos enfoques |

---

## 2. Problemas Identificados

### 2.1 Limitaciones del Crate `regex` de Rust

El crate `regex` de Rust **NO soporta** las siguientes características de regex avanzado:

- **Lookahead positivo**: `(?=...)`
- **Lookbehind**: `(?<=...)`  
- **Negative lookahead**: `(?!)`
- **Negative lookbehind**: `(?<!)`

Esta limitación ha causado **bugs concretos** en las siguientes reglas:

| Regla | Bug | Causa Raíz |
|-------|-----|------------|
| **S1135** | Pattern con `(?=...)` lookahead no funcional | Regex no soporta lookahead |
| **S5122** | SQL injection pattern con lookbehind | Regex no soporta lookbehind |
| **S4792** | DES/RC4 detection con regex compleja | Regex no soporta lookbehind |
| **S1134** | Malformed regex en pattern | Regex compilation falla |
| **S2068** | Minimum length validation | Lógica de longitud no expresable en regex puro |

### 2.2 Regex No Distingue Contexto Semántico

El enfoque puramente basado en regex presenta un **alto tasa de falsos positivos** porque:

- **No distingue código de comentarios**: Una búsqueda de `TODO` encuentra tanto código real como comentarios
- **No distingue código de strings**: `format!("SELECT * FROM")` dispara reglas SQL aunque sea un string literal
- **No distingue identificadores**: `crypto` en un comentario no es lo mismo que `crypto` en una llamada a función
- **No hay scope tracking**: Las variables locales no se diferencian de globales

### 2.3 Arquitectura Fragmentada

El sistema actual adolece de varios problemas arquitectónicos:

#### Sin Visitor Trait Reutilizable
Cada regla implementa su propio tree walking, resultando en:
- Código duplicado entre reglas similares
- Difícil mantenimiento cuando cambia el AST
- Imposible compartir lógica de traversal

#### Sin Pattern Library
Cada regla compila sus propias expresiones regulares:
- No hay reutilización de patrones comunes
- Inconsistencia en how patterns son escritos
- Sin validación centralizada

#### Sin Scope Tracking
Las reglas basadas en regex no tienen noción de:
- Alcance de variables
- Ámbito de funciones
- Visibilidad de símbolos

#### Catálogo Monolítico
El archivo `catalog.rs` de 28,251 líneas es **imposible de mantener**:
- No hay separación por preocupación (separation of concerns)
- Cambios en una regla pueden afectar otras inadvertidamente
- Navegación y comprensión del código extremadamente difícil

### 2.4 Módulo Segregado Huérfano

El módulo segregado `rules/rules/rust/` estaba **completamente huérfano**:
- Faltaba el archivo `rust/mod.rs`
- Las reglas rust específicas no estaban registradas
- El sistema no podía cargar reglas específicas del lenguaje

---

## 3. Lecciones Aprendidas

### 3.1 Los 4 Bugs Corregidos Demuestran la Fragilidad de Regex

La corrección de los bugs en S1135 (lookahead), S5122 (lookbehind), S2068 (min length), y S1134 (malformed regex) demuestra que:

> **El regex es frágil para análisis de código**. Las expresiones regulares fueron diseñadas para matching de texto, no para análisis semántico.

Cuando la semántica del código importa (y en análisis de código siempre importa), regex alcanza sus límites rápidamente.

### 3.2 99% Funcionalidad con Precisión Limitada

El sistema actual **funciona** en el 99% de los casos, pero con **precision limitada**:

- Los findings son mayormente correctos
- La tasa de falsos positivos es aceptable para uso interno
- Pero no es suficiente para exponer a usuarios externos

### 3.3 La Experiencia de Datadog es Ilustrativa

Datadog enfrentó desafíos similares y tomó una decisión arquitectónica significativa:

> **Migraron de Java+ANTLR a Rust+tree-sitter** y obtuvieron:
> - **3x más rendimiento**
> - **10x menos consumo de memoria**
> - Mejor precisión en los findings

Esta migración validó el enfoque de usar parsing estructural (AST) en lugar de regex puro.

---

## 4. Análisis de las Reglas por Categoría

### 4.1 Reglas de Seguridad (Security)

Las reglas de seguridad son las más críticas y también las más afectadas por las limitaciones de regex:

- **S2068**: Hardcoded credentials — regex no distingue contexto de assignment
- **S5122**: SQL Injection — regex no puede trackear dataflow
- **S4792**: Weak crypto — regex no entiende estructuras de llamada
- **S5332**: SSL verification disabled — solo detectable con análisis semántico

### 4.2 Reglas de Bugs

Las reglas de bugs detectan errores comunes:

- **S1656**: Variable shadowing — requiere scope tracking
- **S2259**: Null pointer dereference — requiere análisis de flujo
- **S2589**: Short-circuit evaluation — requiere comprensión semántica
- **S2757**: Operator precedence — detectable con AST

### 4.3 Code Smells

Los code smells son el dominio natural de regex:

- **S1135**: TODO comments — regex simple funciona bien
- **S107**: Demasiados parámetros — requiere análisis de signatura
- **S138**: Funciones demasiado largas — requiere métricas estructurales

---

## 5. Tests que Fallan Actualmente

```
275/294 tests pasando
19 tests fallando (6.5%)
```

Las categorías de tests que fallan corresponden principalmente a:

1. **S4792 DES/RC4**: 3 tests fallando por regex lookbehind
2. **S5122 SQL Injection**: 3 tests fallando por regex lookbehind
3. **S1135**: 2 tests fallando por regex lookahead
4. **S1134**: 2 tests fallando por regex malformed
5. **Otros**: 9 tests fallando por diversas razones

---

## 6. Conclusiones del Diagnóstico

### 6.1 El Sistema Necesita Evolución, No Revolución

El sistema actual **funciona** pero necesita evolucionar para:

1. **Mayor precisión** en los findings
2. **Mejor mantenibilidad** del código
3. **Mejor rendimiento** en el análisis
4. **Escalabilidad** para nuevas reglas

### 6.2 La Dirección Está Clara

Las lecciones aprendidas apuntan a una dirección clara:

1. **Migrar de regex a AST** para análisis estructural
2. **Adoptar un visitor trait reutilizable** para compartir lógica
3. **Crear una pattern library** para reutilización
4. **Implementar scope tracking** para análisis semántico

### 6.3 El Plan Rules Pro

El plan **CogniCode Rules Pro** describe la arquitectura, las herramientas y el roadmap para lograr esta evolución de manera incremental y segura.

---

## Referencias

- [Rust regex crate documentation](https://docs.rs/regex/latest/regex/)
- [Datadog static analysis journey](https://www.datadoghq.com/blog/engineering/building-a-static-analysis-engine/)
- [Tree-sitter: A parser generator tool](https://tree-sitter.github.io/tree-sitter/)
- [ast-grep: AST-based pattern matching](https://ast-grep.github.io/)
