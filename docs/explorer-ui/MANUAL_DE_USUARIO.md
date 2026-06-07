# CogniCode Explorer — Guía de Funcionalidades

> Recorrido completo por cada funcionalidad, con capturas de pantalla.

CogniCode Explorer es una aplicación web de tipo SPA pensada para navegar los datos de inteligencia de código que produce el pipeline de análisis de CogniCode. Expone scopes, archivos y símbolos como una jerarquía, y trae bajo demanda detalles sobre callers, callees, source slices, call graphs y calidad de código. Toda la interfaz es de solo lectura y se ejecuta por completo en el navegador.

Esta guía recorre cada funcionalidad visible con capturas de pantalla anotadas. Usala como referencia mientras aprendés la interfaz, o como checklist cuando necesites encontrar una capacidad puntual.

## 1. Primeros Pasos

Lanzás CogniCode Explorer desde el `justfile` del proyecto. Hay dos comandos disponibles, según quieras trabajar contra un backend real o un mock local:

- `just explorer-dev` levanta la app con fixtures de MSW (Mock Service Worker) cargados en el navegador. No necesita backend y la UI queda totalmente usable.
- `just explorer-full` levanta la app apuntando a un backend real `cognicode-explorer` por HTTP.

Ambos comandos levantan Vite en `http://127.0.0.1:5173/`. En la primera carga ves el shell vacío de tres paneles: un Navigator a la izquierda, un Object Inspector en el centro y un Lens Panel a la derecha. La aplicación viene con un tema oscuro: cada superficie, borde y color de texto está pensado para sesiones largas de lectura con poca luz.

![Shell vacío de la app con tres paneles placeholder](./screenshots/f01-shell-empty-state.png)

La barra de encabezado cruza la parte superior de la aplicación. Lleva el título del producto, un indicador de estado de conexión en vivo que muestra si el backend está alcanzable, y el botón disparador del Spotter (la pista del atajo de teclado `Cmd/Ctrl+K` se renderiza al lado). La barra de estado en la parte inferior de la pantalla refleja el mismo estado de conexión, así siempre sabés si estás mirando datos en vivo o fixtures mock.

![Detalle de la barra de encabezado mostrando el título, el estado de conexión y el botón del Spotter](./screenshots/f30-header-detail.png)

## 2. Búsqueda con Spotter

El Spotter es la forma más rápida de llegar a cualquier objeto del workspace. Es un diálogo tipo command-palette que se superpone a la aplicación y devuelve resultados mientras escribís.

### 2.1 Abrir el Spotter

Apretá `Cmd+K` (macOS) o `Ctrl+K` (Windows/Linux) desde cualquier lugar de la aplicación, o hacé clic en el disparador de búsqueda del encabezado. El diálogo se abre con un input vacío y una pista para arrancar a escribir. También podés acceder con el teclado: presioná `Tab` hasta que el disparador de búsqueda quede enfocado y después `Enter`.

![Diálogo del Spotter abierto en estado vacío con la pista "Type to search"](./screenshots/f03-spotter-dialog-empty.png)

### 2.2 Buscar objetos

Escribí una query en el input. Los resultados filtran en vivo a través de símbolos, archivos y scopes. Cada fila muestra un ícono de tipo (`ƒ` para funciones, `S` para scopes, y marcadores similares para otros tipos), el nombre fully qualified, la ruta del archivo y un saliency score a la derecha que rankea cuán relevante considera CogniCode al match.

![Spotter con resultados de búsqueda mostrando íconos de tipo, rutas de archivo y saliency scores](./screenshots/f04-spotter-with-results.png)

### 2.3 Filtrar por tipo

Una fila de pestañas de tipo se ubica arriba de la lista de resultados. Hacé clic en una pestaña para restringir los resultados a un único tipo de símbolo. La pestaña por defecto `All` muestra todos los matches; la pestaña `symbol` se restringe a funciones, clases, métodos y declaraciones similares. La pestaña activa queda resaltada.

![Spotter con la pestaña de filtro "symbol" activa](./screenshots/f05-spotter-filter-symbol.png)

### 2.4 Seleccionar un resultado

Usá `Arrow Up` y `Arrow Down` para mover el resaltado, y después presioná `Enter` (o hacé clic en la fila) para cargar el resultado. El Spotter se cierra, el Navigator y el Inspector se pueblan, y el Lens Panel queda disponible. Presioná `Escape` o hacé clic fuera del diálogo para descartar el Spotter sin seleccionar.

![Layout completo de tres paneles con datos cargados tras seleccionar un resultado](./screenshots/f06-full-layout-loaded.png)

## 3. Miller Columns Navigator

El panel izquierdo implementa navegación Miller Columns, un patrón de drill-down conocido de los file browsers. Cada columna representa un nivel de la jerarquía, y al hacer clic en un item se abre el siguiente nivel a su derecha. Un breadcrumb arriba de las columnas refleja la ruta actual.

### 3.1 Cómo funciona el drill-down

Los items que tienen hijos muestran una flecha `›` a la derecha. Hacé clic en el item (o presioná `Enter` cuando tenga foco) para expandir una columna hija. El breadcrumb se actualiza en el momento, y podés colapsar de vuelta a un nivel superior haciendo clic en cualquier item padre del breadcrumb o de una columna anterior.

### 3.2 Drill-down de dos niveles

El primer clic en un scope abre la columna de archivos. Desde ahí ves todos los archivos fuente contenidos en el scope seleccionado, ordenados por nombre. La columna recién abierta resalta el primer item por defecto.

![Miller Columns mostrando dos columnas tras el primer drill-down de scope a archivo](./screenshots/f07-miller-drill-down-2-levels.png)

### 3.3 Tres niveles de profundidad

Un segundo clic en un archivo abre la columna de símbolos. El Navigator muestra ahora tres columnas lado a lado: scope, archivo y símbolo. Ves cada declaración y definición dentro del archivo seleccionado, con sus tipos indicados por el ícono inicial.

![Miller Columns mostrando tres columnas en profundidad con scope, archivo y símbolo visibles](./screenshots/f08-miller-3-levels-deep.png)

### 3.4 Estado de foco del item

Los items seleccionados con clic o teclado reciben un anillo de foco visible. El anillo es un borde de acento fino que se queda en su lugar hasta que el foco se mueva a otro lado, y te da una indicación clara de qué item es la selección activa en ese momento. Este indicador de foco es también la señal que usa el Object Inspector para saber qué objeto renderizar.

![Item de Miller Column con el anillo de foco visible tras la selección](./screenshots/f09-miller-item-focused.png)

## 4. Object Inspector — Pestaña Overview

El panel central es el Object Inspector. Arranca por defecto en la pestaña **Overview**, que junta cada dato relevante del objeto seleccionado en un solo scroll. El strip de encabezado muestra el nombre fully qualified, el tipo, la ruta del archivo y el número de línea.

### 4.1 Bloque de Identidad

El bloque de Identidad se ubica al tope de la pestaña Overview. Indica el nombre, el tipo (función, clase, método, etc.), la ruta del archivo y el número de línea donde vive la declaración. Es la referencia canónica de «dónde está definido este bicho».

![Pestaña Overview: bloque Identity con nombre, tipo y file:line](./screenshots/f10-overview-identity-metrics.png)

### 4.2 Métricas de llamadas + Firma

Directamente debajo de Identity, el bloque de Call Metrics muestra fan-in (cuántos lugares llaman a este objeto) y fan-out (cuántos callees invoca este objeto). Le sigue el bloque Signature, con la declaración completa de la función, incluyendo los tipos de los parámetros y el tipo de retorno. Juntos responden «¿cuán conectado está este objeto y qué pinta tiene?».

### 4.3 Callers y Callees

Dos listas compactas enumeran los callers entrantes y los callees salientes. Cada entrada es un link clickeable que lleva el Navigator hasta ese símbolo. Si un caller vive en otro archivo u otro scope, la ruta se muestra para que de un vistazo sepas si la relación cruza un límite.

### 4.4 Source Slice + Quality

Más abajo al hacer scroll, el bloque Source Slice muestra las líneas relevantes del código fuente inline. El bloque Quality que está debajo lista las reglas y smells detectados en este objeto puntual, con badges de severidad y una descripción corta para cada hallazgo.

![Pestaña Overview scrolleada para mostrar los bloques de source slice y quality issues](./screenshots/f11-overview-source-quality.png)

### 4.5 Información de archivo y scope

La pestaña Overview también expone el archivo donde vive el objeto, con la cantidad de líneas del archivo, el lenguaje y el total de símbolos. Un desglose por tipos de símbolo parte el archivo según el tipo de declaración. Si estás inspeccionando un scope en lugar de un archivo, esta sección muestra la profundidad del scope y la cantidad de hijos en su lugar.

![Pestaña Overview: info de archivo con line count, symbol count y kinds breakdown](./screenshots/f12-overview-file-scope.png)

### 4.6 Relaciones cross-scope y hotspots

El fondo de la pestaña Overview contiene dos secciones más. La tabla de Cross-scope Relations lista cada llamada que cruza un límite de scope, ordenada por saliency. La lista Top Hotspots muestra los símbolos con mayor fan-in del proyecto: útil para identificar código que sostiene la estructura y merece una mirada cuidadosa.

![Pestaña Overview: tabla de cross-scope relations y lista de top hotspots](./screenshots/f13-overview-cross-scope-hotspots.png)

## 5. Object Inspector — Pestaña Call Graph

La pestaña Call Graph renderiza una visualización SVG interactiva de las relaciones de llamada alrededor del objeto seleccionado. El objeto seleccionado se ubica en el centro; los callers se abren hacia la izquierda y los callees hacia la derecha. Los edges son flechas dirigidas etiquetadas con el call site.

Podés panear el gráfico arrastrando el fondo y hacer zoom con la rueda del mouse o el trackpad. Cada nodo es un link clickeable que lleva el Navigator hasta ese símbolo, así que el gráfico funciona como superficie de navegación además de visualización.

![Pestaña Call Graph con el SVG interactivo mostrando nodos y aristas dirigidas](./screenshots/f14-call-graph-svg.png)

## 6. Object Inspector — Pestaña Source

La pestaña Source muestra el source completo del archivo que contiene al objeto seleccionado, con números de línea en el gutter y resaltado consciente de la sintaxis. La declaración del objeto seleccionado se marca en el gutter y se lleva a la vista automáticamente al cargar.

Usá esta pestaña cuando quieras leer código en contexto. Los números de línea son links clickeables: hacé clic en cualquier línea para copiar su referencia, y hacé clic en cualquier otro nombre de símbolo para saltar al Inspector de ese símbolo.

![Pestaña Source mostrando el source completo con números de línea y declaración resaltada](./screenshots/f15-source-view.png)

## 7. Object Inspector — Pestaña Quality

La pestaña Quality es un dashboard que resume la postura de calidad del objeto seleccionado y su archivo contenedor. Es el lugar indicado para mirar cuando querés saber «¿está sano este código?».

El dashboard muestra:

- Un estado de quality gate con un veredicto claro de pass o fail, y la lista de reglas que causaron el fallo.
- Ratings con letras (de la A a la E) para maintainability, reliability y complexity.
- Issues agrupados por severidad, desde Blocker hasta Info.
- Una estimación de deuda técnica en minutos: el tiempo que una persona developer necesitaría para remediar cada hallazgo.

![Pestaña Quality con ratings de la A a la E, grupos de severidad y estado del quality gate](./screenshots/f16-quality-dashboard.png)

## 8. Lens Panel

El panel derecho es el Lens Panel. Es un overlay contextual que trae un único aspecto de la selección al frente sin saturar al Inspector. Cada lens se enfoca en una pregunta distinta que podrías hacerte sobre el objeto seleccionado.

### 8.1 Lenses disponibles (estado idle)

En su estado idle, el Lens Panel muestra tres botones, uno por cada lens disponible: Call Graph, Hotspots y Quality. Hacé clic en cualquier botón para activar ese lens. El lens activo previamente se desactiva automáticamente.

![Lens Panel en estado idle con los tres botones de lens disponibles](./screenshots/f17-lens-panel-idle.png)

### 8.2 Lens Call Graph

El lens Call Graph muestra las relaciones de llamada entrantes y salientes de la selección actual, ordenadas por saliency. Es un primo compacto de la pestaña Call Graph, optimizado para una mirada rápida más que para navegación completa.

![Lens Call Graph activado, mostrando relaciones entrantes y salientes](./screenshots/f18-lens-call-graph-active.png)

### 8.3 Lens Hotspots

El lens Hotspots expone los símbolos con mayor fan-in del proyecto. Cada entrada lleva un confidence score derivado del modelo de saliency. Este lens es el lugar indicado para mirar cuando querés saber «qué partes de este codebase están haciendo el trabajo más pesado».

![Lens Hotspots activado, mostrando los símbolos top de fan-in con confidence scores](./screenshots/f19-lens-hotspots-active.png)

### 8.4 Lens Quality

El lens Quality agrupa cada issue de calidad que afecta a la selección actual por severidad: Blocker, Critical, Major (o Warning), Minor e Info. Cada entrada es un link clickeable al objeto afectado, así podés hacer triage de issues en una columna y saltar al fix en otra.

![Lens Quality activado, mostrando issues agrupados por severidad](./screenshots/f20-lens-quality-active.png)

### 8.5 Toggle de Solo Blockers

Un toggle en la parte superior del Lens Panel restringe el lens activo a hallazgos con severidad blocker únicamente. Activalo cuando quieras enfocarte exclusivamente en issues que bloquean un release. El toggle es sticky dentro de la sesión: se queda prendido entre cambios de lens hasta que lo apagues.

![Toggle de Blockers only prendido, filtrando el lens activo](./screenshots/f21-lens-blockers-only.png)

## 9. Diseño Responsive

El layout se adapta al ancho disponible. La interfaz fue auditada para accesibilidad de teclado en cada breakpoint, y el tema oscuro se mantiene consistente en todos los tamaños.

### 9.1 Mobile (390 px)

En un viewport de tamaño phone los tres paneles se apilan verticalmente. El Navigator colapsa a una sola columna, y el Object Inspector y el Lens Panel pasan a ser secciones full-width debajo. Los recorrés en orden de lectura, de arriba hacia abajo.

![Layout responsive mobile a 390 por 844 con paneles apilados verticalmente](./screenshots/f22-responsive-mobile.png)

Al hacer scroll hacia abajo aparece la sección del Object Inspector a ancho completo, con el Lens Panel siguiendo. El scroll táctil funciona exactamente igual que en desktop, y el Spotter sigue abriéndose en un overlay modal.

![Layout mobile scrolleado para mostrar la sección del Object Inspector](./screenshots/f23-mobile-scrolled.png)

### 9.2 Tablet (768 px)

En un viewport de tamaño tablet el layout pasa a dos columnas. El Navigator y el Object Inspector comparten la fila superior, y el Lens Panel cae debajo. Es un layout intermedio muy útil para code review en el regazo o en un stand.

![Layout responsive tablet a 768 por 1024 en modo dos columnas](./screenshots/f24-responsive-tablet.png)

### 9.3 Desktop (1440 px+)

En un monitor desktop se restaura el layout canónico de tres columnas: Navigator a la izquierda, Object Inspector en el centro, Lens Panel a la derecha. Los tres quedan visibles a la vez, que es el layout más eficiente para navegar codebases grandes.

![Layout desktop completo de tres paneles a 1440 px y por encima](./screenshots/f06-full-layout-loaded.png)

## 10. Navegación por Teclado

Cada elemento interactivo de la interfaz soporta navegación por teclado. La aplicación usa un patrón de roving tabindex: `Tab` se mueve entre regiones estructurales (skip link, botones del header, columnas, tabs, lens items), y `Arrow Up` y `Arrow Down` mueven el foco adentro de una región. Esto hace que la navegación sea predecible y evita el costo de tabular decenas de items.

### 10.1 Enlace para saltar al contenido

El primer elemento focuseable es un skip-to-content link. Presionar `Tab` desde la carga de la página trae el link al foco con un anillo visible. Presioná `Enter` para saltearte el header y saltar directo al Navigator.

![Skip-to-content link con anillo de foco en la primera presión de Tab](./screenshots/f02-skip-link-focus.png)

### 10.2 Foco del botón disparador del Spotter

Desde el skip link, `Tab` mueve el foco al botón disparador del Spotter en el header. El botón muestra un anillo de foco claro para que sepas exactamente qué se va a activar con `Enter`.

![Botón del Spotter con anillo de foco en la barra de encabezado](./screenshots/f26-keyboard-spotter-button.png)

### 10.3 Foco del input del Spotter

Después de abrir el Spotter, el campo de input queda auto-enfocado. El input lleva un anillo de foco, y una pista sutil te recuerda arrancar a escribir. Todos los atajos del Spotter (`Arrow Up/Down`, `Enter`, `Escape`) funcionan desde este estado.

![Campo de input del Spotter con anillo de foco tras abrir el diálogo](./screenshots/f27-keyboard-spotter-input-focus.png)

### 10.4 Foco de las pestañas de vista

Las cuatro pestañas del Object Inspector (Overview, Call Graph, Source, Quality) son alcanzables con `Tab`. La pestaña activa lleva un anillo de foco distinto del styling de "pestaña activa", así podés ver qué pestaña tiene foco incluso cuando no es la pestaña seleccionada en el momento.

![Anillo de foco de view tab en el strip de pestañas del Object Inspector](./screenshots/f28-keyboard-view-tab-focus.png)

### 10.5 Foco de los items del lens

Cada item del Lens Panel es alcanzable con `Tab` desde el header del panel. El item activo lleva un anillo de foco idéntico en estilo al anillo de foco de Miller Column, que mantiene el lenguaje visual consistente a lo largo de la aplicación.

![Anillo de foco de lens item en el panel derecho](./screenshots/f29-keyboard-lens-focus.png)

## 11. Atajos de Teclado

La lista completa de atajos de teclado:

| Atajo | Acción |
| --- | --- |
| `Cmd+K` / `Ctrl+K` | Abrir el Spotter |
| `Arrow Up` / `Arrow Down` | Navegar dentro del Spotter o las Miller Columns |
| `Enter` | Seleccionar el item resaltado |
| `Escape` | Cerrar el Spotter o descartar un diálogo |
| `Tab` | Mover el foco al siguiente panel o región |
| `Shift+Tab` | Mover el foco al panel o región anterior |

Tanto el Spotter como las Miller Columns implementan el patrón de roving tabindex, así que `Tab` siempre se mueve entre regiones estructurales en lugar de entre items individuales. Adentro de una región, usá `Arrow Up` y `Arrow Down` para mover el item activo.

## 12. Stack Tecnológico

La aplicación está construida con el siguiente stack:

- **React 19** con features concurrentes para el Inspector y el Lens.
- **TypeScript** en strict mode para type safety end-to-end.
- **Tailwind CSS 4** para el design system y el tema oscuro.
- **Vite 6** como build tool y dev server.
- **SWR** para data fetching, caching y revalidation.
- **MSW (Mock Service Worker)** para el mock backend de desarrollo que usa `just explorer-dev`.
- **cmdk** para el command palette del Spotter.
- **zod** para validación de schemas de la API.
- **Playwright** para tests end-to-end.

El build de producción se deploya de forma estática; la única dependencia de runtime es un backend `cognicode-explorer` alcanzable o una capa de mock compatible.
