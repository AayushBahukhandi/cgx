

<div align="center">

<a href="https://aayushbahukhandi.github.io/cgx/">
  <img src="https://raw.githubusercontent.com/AayushBahukhandi/cgx/main/assets/cgx-web-hero.gif" alt="cgx web graph demo" width="100%" />
</a>

<br /><br />

# cgx

**Convierte cualquier repositorio de Git en un grafo de conocimiento consultable.**

[![CI](https://github.com/AayushBahukhandi/cgx/actions/workflows/ci.yml/badge.svg)](https://github.com/AayushBahukhandi/cgx/actions)
[![crates.io](https://img.shields.io/crates/v/cgx-cli.svg)](https://crates.io/crates/cgx-cli)
[![License: MIT](https://img.shields.io/badge/license-MIT-green.svg)](LICENSE)
[![Graph](https://img.shields.io/badge/cgx-live%20graph-blue)](https://aayushbahukhandi.github.io/cgx/)

[**Demo en Vivo**](https://aayushbahukhandi.github.io/cgx/) · [**Documentación**](https://docs.rs/cgx-cli) · [**Lanzamientos**](https://github.com/AayushBahukhandi/cgx/releases)

</div>

---

> 🚀 **v0.5.3** — corrección de propiedad: `cgx analyze` estaba creando un nodo `Author` inválido por archivo (nombrado con la ruta del archivo), lo que contaminaba el grafo con ~una comunidad singleton por archivo. La autoría ahora se indexa por contribuidor, por lo que la propiedad de `cgx docs` es correcta y el recuento de comunidades Louvain disminuye en consecuencia. Pulido del docs-vault: el Glossary colapsa las comunidades singleton y elimina los tipos de nodos con recuento cero, las notas de módulos no documentadas pierden la columna vacía "Description", `CrossClusterDeps` ya no emite enlaces wiki huérfanos, y `Owners.md` muestra una tabla real de contribuidor → archivos propiedad. `cgx update --auto` ahora verifica que la versión realmente cambió en lugar de informar éxito siempre. [v0.5.3 →](https://github.com/AayushBahukhandi/cgx/releases/tag/v0.5.3)
>
> **v0.5.2** — `cgx todos` ya no marca prosa JSDoc (`Note:`, `**Warning:**`, la palabra "bugs") como anotaciones: la coincidencia ahora es sensible a mayúsculas/minúsculas con límites de palabra, y el texto mostrado es la línea coincidente en lugar del abre-bloque `/**`. `cgx todos` en un repositorio indexado sin anotaciones ahora dice "No annotation comments found." en lugar de sugerir `cgx analyze`. El diseño del grafo en terminal cae en una cuadrícula determinista cuando la simulación de fuerza dirigida diverge, corrigiendo un error raro de NaN/Inf en la TUI. Aclaraciones en README: diagrama de layout del vault, predicado `node_count_max`. [v0.5.2 →](https://github.com/AayushBahukhandi/cgx/releases/tag/v0.5.2)
>
> **v0.5.1** — ganchos de git compatibles con bisect: los ganchos `post-checkout` / `post-commit` gestionados por cgx ahora se omiten durante `git bisect`, `git rebase` y `git merge`, por lo que `cgx bisect-script` funciona dentro de `git bisect run` sin ensuciar `AGENTS.md` / `CGX_SKILL.md`. Bisect-script ahora también sale con `125` (omitir) en archivos de predicados vacíos y cuando `rule_violations_max` está configurado sin `--rule-violations N`, en lugar de pasar silenciosamente. [v0.5.1 →](https://github.com/AayushBahukhandi/cgx/releases/tag/v0.5.1)
>
> **v0.5.0** — `cgx docs generate --vault` convierte tu repositorio indexado en un vault de documentación listo para Obsidian (visión general del proyecto, propósitos de dependencias, detección de dependencias no usadas, TL;DR por archivo, clasificación de roles) · la extracción de docstrings ahora cubre Rust, Go, Java, PHP, Python (antes solo TypeScript) · `cgx bisect-script` se integra en `git bisect run` para encontrar commits que rompen predicados de grafo declarativos · nuevas consultas `GraphDb`: `get_file_summary`, `get_public_api`, `list_entry_points`, `get_cross_cluster_deps`. [Notas de lanzamiento →](https://github.com/AayushBahukhandi/cgx/releases/tag/v0.5.0)
>
> Anterior: [v0.4.0](https://github.com/AayushBahukhandi/cgx/releases/tag/v0.4.0) — `cgx watch`, `cgx query context`, gancho Claude Code PreToolUse, gestión de caché.

---

## Una base de código tiene dos grafos

```
   ESTRUCTURAL                          TEMPORAL
   qué llama a qué                      qué cambia junto

   AuthService                        payments.ts ─┐
       │                                            │ 87% co-cambio
       ├── login()                                  │ (¡sin arista de import!)
       │     └── db.query()           flags.ts  ────┘
       └── logout()                          ▲
             └── session.clear()             │
                                       acoplamiento oculto
                                       vive aquí
```

Cada otra herramienta de análisis de código solo te muestra el lado izquierdo. cgx construye ambos: analizando tu AST con Tree-sitter **Y** superponiendo todo tu historial de Git. El grafo temporal es donde realmente residen la mayoría de los errores y los riesgos de refactorización.

---

## Inicio Rápido

```bash
# 1. Instalar
brew install aayushbahukhandi/cgx/cgx       # o: cargo install cgx-cli

# 2. Indexa tu repositorio (funciones, imports, historial de git — todo en segundos)
cd your-project && cgx analyze

# 3. Abre el grafo WebGL en tu navegador
cgx view --web

# 4. Haz preguntas arquitectónicas en milisegundos
cgx query blast-radius "AuthService"        # ¿qué se rompe si modifico esto?
cgx hotspots                                # alto churn × acoplamiento = zona de peligro

# 5. Conecta tu editor con IA (Cursor, Claude Code, Windsurf, Codex)
cgx setup
```

Después de `cgx analyze`, se generan dos archivos en la raíz de tu repositorio: `CGX_SKILL.md` (instruye a tu IA para consultar el grafo en lugar de abrir archivos) y `AGENTS.md` (resumen arquitectónico en prosa). Ambos se regeneran automáticamente en cada commit.

---

## Generando un vault de documentación (nuevo en v0.5)

```bash
cgx docs generate --vault           # escribe en tu vault de Obsidian (detectado automáticamente o configurado)
cgx docs generate --local           # escribe en ./cgx-docs/
cgx docs generate --vault --force   # reconstrucción completa
cgx docs generate --vault --incremental   # regenera solo archivos cuya porción del grafo cambió
cgx docs status                     # muestra qué se regeneraría
cgx docs prompts --next             # transmite estubos de prosa IA sin completar a Claude/Cursor
```

El vault tiene una estructura en capas construida directamente desde tu grafo:

```text
cgx-docs/
├── .obsidian/                          ← Configuración del espacio de trabajo Obsidian (creado automáticamente)
├── README.md                           ← descripción del proyecto + resumen de deps + navegación
├── 00-Overview/
│   ├── Architecture.md                 ← stack, lenguajes, propósitos de deps + detección de deps no usadas,
│   │                                     archivos por rol, grupos más grandes, puntos de entrada
│   ├── HowToNavigate.md                ← ruta de lectura para nuevos contribuidores
│   └── Glossary.md                     ← tipos de nodos + comunidades
├── 10-PublicAPI/<group-slug>.md        ← símbolos exportados por directorio de origen
│                                         (p. ej., source-core.md, test-helpers.md)
├── 20-Architecture/
│   ├── Groups.md                       ← navegación principal: por directorio de origen
│   ├── Communities.md                  ← clústeres Louvain crudos
│   ├── CrossClusterDeps.md
│   └── EntryPoints.md
├── 30-Modules/<role>/<file>.md         ← por archivo: TL;DR + insignia de rol + tabla de estructura
│                                         con descripciones inline de docstrings + llamantes/
│                                         llamados + tests + propiedad + `<!-- cgx-prompt -->`
│                                         estub IA. <role> es `source` o `test`.
├── 40-Risk/
│   ├── Hotspots.md
│   ├── ComplexityHigh.md
│   ├── DeadCode.md
│   └── Duplicates.md
└── 50-Ownership/
    ├── Owners.md
    └── BlameGraph.md
```

cgx en sí nunca llama a un LLM. Cada nota de módulo termina con un bloque autónomo `<!-- cgx-prompt -->` que agrupa cada hecho que tu IA necesita para escribir prosa (símbolos exportados, llamantes/llamados, tests, propietarios, métricas, docstrings existentes). Pégalo en Claude/Cursor: tú decides el horario, la clave API y el costo.

Configura el vault por defecto en `.cgx/config.toml`:

```toml
[docs]
vault_path = "/Users/you/Documents/Obsidian Vault"   # usado por --vault
output_dir = "./cgx-docs"                             # usado por --local
wiki_links = "obsidian"                               # o "markdown"
prompt_packets = true
frontmatter = true
```

---

## git bisect sobre el grafo (nuevo en v0.5)

`cgx bisect-script` se integra en `git bisect run` y evalúa predicados declarativos sobre el grafo recién indexado en cada commit:

```bash
# 1. Genera un archivo de predicado inicial
cgx bisect-script --example > .cgx/bisect.toml

# 2. Edita .cgx/bisect.toml — cualquier cosa que puedas expresar contra el grafo:
#    node_count_min, node_count_max, nodes_exist, nodes_missing, nodes_alive,
#    rule_violations_max

# 3. Bisect
git bisect start
git bisect bad HEAD
git bisect good v0.4.1
git bisect run cgx bisect-script --analyze
```

Códigos de salida: `0` = bueno, `1` = malo, `125` = omitir — exactamente lo que `git bisect run` espera. Usa `--analyze` para reindexar en cada commit visitado.

---

## Por qué importa

| | cgx | Leer archivos fuente |
|---|---|---|
| Responde "¿qué se rompe si modifico `AuthService`?" | `cgx query blast-radius AuthService` → ~50 tokens, 0.3s | abrir 40 archivos → 15.000–50.000 tokens |
| Resumen arquitectónico para un agente IA | `get_repo_summary` → **~150 tokens** | ls recursivo + lecturas → 50.000+ tokens |
| Encontrar acoplamientos ocultos | `cgx hotspots` → puntuaciones de co-cambio | grep-and-pray (buscar con grep y rezar) |
| Resumen de símbolo en un paso | `cgx query context login` → **~400 tokens** | 2–15K por archivo |

---

## Características Destacadas

| Característica | Qué hace |
|---|---|
| **`cgx docs generate --vault`** | Genera un vault de documentación en capas y listo para Obsidian desde el grafo: visión general del proyecto, TL;DR por archivo + insignia de rol, APIs públicas, hotspots, propósitos de dependencias con detección de no usadas, más estubos de prosa IA que completas a tu ritmo |
| **`cgx bisect-script`** | Integra `git bisect run` para hacer bisect en predicados de grafo declarativos (nodo existe, límites de conteo, sin código muerto, ...). Sale 0/1/125 — git hace la búsqueda binaria |
| **Análisis AST con Tree-sitter** | TS/TSX, JS/JSX, Python, Rust, Go, Java, PHP — analizado en paralelo, con extracción de docstrings en los seis idiomas (TypeScript, Rust, Go, Java, PHP, Python) |
| **Superposición de historial Git** | Puntuaciones de churn, aristas de co-cambio, propiedad — el grafo temporal |
| **`cgx query blast-radius`** | Llamantes directos y transitivos con puntuación de riesgo |
| **`cgx watch`** | Reindexación en vivo con debounce en cada guardado |
| **`cgx query context <sym>`** | Llamantes + deps + comunidad + riesgo en un bloque de ~400 tokens |
| **Servidor MCP (10 herramientas)** | Cursor, Claude Code, Windsurf, Codex — campo `_summary` en cada respuesta |
| **Gancho Claude Code PreToolUse** | Inyecta automáticamente el contexto del archivo antes de cada Edit/Write |
| **Grafo WebGL + enlaces para compartir** | Sigma.js renderiza miles de nodos; `cgx share` → visor de gist sin instalación |

<details>
<summary><strong>Ver lista completa de características (30+)</strong></summary>

| Característica | Descripción |
|---|---|
| **Vault de Documentación** | `cgx docs generate --vault` escribe un vault de Obsidian en capas con visión general del proyecto, TL;DR por archivo + clasificación de roles, APIs públicas agrupadas por directorio, hotspots/código muerto, tabla de dependencias con propósitos curados y banderas de no usadas, y estubos de prosa IA que completas bajo demanda |
| **Script Bisect** | `cgx bisect-script` evalúa predicados de grafo declarativos (nodo existe/falta, límites de conteo, código muerto) y sale 0/1/125 para `git bisect run` |
| **Extracción de docstrings multiidioma** | Rust (`///`, `/** */`, incluyendo en `#[derive]`), Go (`// SymbolName`), Java/PHP (`/** */`, omitiendo anotaciones), Python (cuerpo docstring con triples comillas), TypeScript — todos extraídos en tiempo de análisis |
| **Análisis AST** | Tree-sitter analiza TS/TSX, JS/JSX, Python, Rust, Go, Java, PHP en paralelo |
| **Seguimiento de Llamantes JSX** | Los usos de componentes React (`<MyComp />`) se rastrean como aristas de llamada |
| **Extracción de Comentarios JSX** | Los comentarios de expresión `{/* TODO */}` y el código JSX comentado se extraen y etiquetan por separado de los comentarios de código |
| **Índice de Anotaciones** | `cgx todos` lista todas las etiquetas TODO/FIXME/HACK/NOTE/BUG/OPTIMIZE/WARN/XXX con `comment_type` (código vs jsx) |
| **Cobertura de Docs** | `cgx docs coverage` informa qué % de funciones exportadas tienen comentarios de documentación, por comunidad |
| **Puntuación de Complejidad** | `cgx complexity` clasifica funciones por puntuación de complejidad cognitiva (anidación if/for/switch/ternaria) |
| **Superposición de Cobertura de Tests** | `cgx test coverage` / `cgx test gaps` mapea archivos de test → funciones de origen vía aristas TESTS |
| **Salud de Dependencias** | `cgx deps health` analiza package.json / Cargo.toml / requirements.txt / go.mod, referencia cruzada con OSV para CVEs |
| **Asistente de Revisión PR** | `cgx review` genera un breve estructurado: radio de explosión, alertas de hotspot, tests faltantes, revisores sugeridos |
| **Reglas de Arquitectura** | `cgx rules check` ejecuta reglas SQL o incorporadas (`no_cycles`, `max_coupling`, etc.) con salida GitHub Actions |
| **Detección de Duplicados** | `cgx dupes` encuentra cuerpos de función casi idénticos vía huellas AST normalizadas + similitud Jaccard |
| **Explicador de Arquitectura** | `cgx explain AuthService` / `cgx explain --onboard` genera documentos Markdown estructurados desde el grafo |
| **Línea de Tiempo** | `cgx timeline` toma snapshots del grafo en cada commit; el controlador en la interfaz web te permite ver evolucionar la arquitectura |
| **Inteligencia Git** | Puntuaciones de churn, aristas de co-cambio, propiedad — el grafo temporal |
| **Almacenamiento DuckDB** | Base de datos de grafo embebida sin servidor. Consultas instantáneas. |
| **Detección de Comunidades** | El algoritmo Leiden agrupa automáticamente tu base de código en módulos |
| **TUI de Terminal** | Grafo de fuerza dirigida en Ratatui. Funciona sobre SSH. |
| **Grafo en Navegador WebGL** | Sigma.js renderiza miles de nodos a 60fps |
| **Chat IA** | Haz preguntas sobre tu código en lenguaje natural. Ollama soportado. |
| **Servidor MCP** | 10 herramientas tipadas para Cursor, Claude Code, Windsurf, Codex/OpenCode |
| **Sistema de Skills** | `CGX_SKILL.md` generado automáticamente — funciona en cualquier asistente IA; comando slash `/cgx` en Claude Code vía `cgx setup` |
| **Exclusión de artefactos de build** | `web-ui-dist/`, `.next/`, `coverage/`, `*.min.js` etc. excluidos automáticamente; personalizable vía `.cgxignore` |
| **Enlaces para Compartir** | `cgx share` sube tu grafo a un Gist — cualquiera lo ve en un navegador, sin instalación |
| **Publicación GitHub Pages** | `cgx publish` empuja un sitio de grafo autónomo a tu rama `gh-pages` |
| **Diff de Grafo** | Ve cómo cambió tu arquitectura entre commits |
| **Detección de Código Muerto** | Cinco categorías: exports sin referencia, funciones privadas inalcanzables, variables sin usar, nodos desconectados, archivos zombie — con confianza Alta/Media/Baja y pistas de falsos positivos |
| **Binario Autónomo** | La interfaz web está embebida en el binario — Homebrew y `cargo install` funcionan out-of-the-box |
| **Reindexación en Vivo** | `cgx watch` monitorea tu repo y ejecuta análisis incremental con debounce en cada cambio de archivo |
| **Briefing de Contexto para Agentes** | `cgx query context <symbol>` devuelve llamantes + deps + comunidad + riesgo en un bloque de ~400 tokens (vs 2–15K para leer un archivo). Soporta `--json`. |
| **Gancho Claude Code** | `cgx setup --hooks` instala un gancho PreToolUse que inyecta automáticamente el contexto del archivo antes de cada Edit/Write |
| **Gestión de Caché** | `cgx clean --orphaned` barre archivos db obsoletos; `cgx clean --budget 2G` hace evicción LRU. Evicción automática opt-in vía `CGX_MAX_CACHE_BYTES` |

</details>

---

## Instalación

### Homebrew (macOS / Linux) — recomendado

```bash
brew install aayushbahukhandi/cgx/cgx
```

> Tap: [AayushBahukhandi/homebrew-cgx](https://github.com/AayushBahukhandi/homebrew-cgx)

Para actualizar:

```bash
brew upgrade aayushbahukhandi/cgx/cgx
```

### cargo

```bash
cargo install cgx-cli
```

El binario instalado se llama `cgx`. Si `cgx --version` imprime `command not found`, agrega el directorio bin de Cargo a tu PATH:

```bash
# zsh (~/.zshrc) o bash (~/.bashrc / ~/.bash_profile)
export PATH="$HOME/.cargo/bin:$PATH"
```

```fish
# fish (~/.config/fish/config.fish)
fish_add_path "$HOME/.cargo/bin"
```

> `cargo install` compila desde el fuente y puede tardar unos minutos. Ejecútalo de nuevo para actualizar.

### Binario precompilado (Windows, macOS, Linux)

Descarga el lanzamiento más reciente desde [GitHub Releases](https://github.com/AayushBahukhandi/cgx/releases/latest). Reemplaza `VERSION` con la etiqueta que se muestra en esa página (p. ej., `v0.4.0`).

```bash
# macOS arm64 (Apple Silicon)
curl -L https://github.com/AayushBahukhandi/cgx/releases/latest/download/cgx-VERSION-aarch64-apple-darwin.tar.gz | tar xz
sudo mv cgx /usr/local/bin/

# macOS x86_64 (Intel)
curl -L https://github.com/AayushBahukhandi/cgx/releases/latest/download/cgx-VERSION-x86_64-apple-darwin.tar.gz | tar xz
sudo mv cgx /usr/local/bin/

# Linux x86_64
curl -L https://github.com/AayushBahukhandi/cgx/releases/latest/download/cgx-VERSION-x86_64-unknown-linux-gnu.tar.gz | tar xz
sudo mv cgx /usr/local/bin/
```

Windows: descarga el `.zip` desde la página de lanzamientos y coloca `cgx.exe` en un directorio de tu `%PATH%`.

### Verificar

```bash
cgx --version
cgx doctor      # verifica tu configuración e integraciones de editor
```

### Mantenerse actualizado

A partir de **v0.1.6**, cgx verifica actualizaciones una vez al día e imprime un aviso cuando hay una versión más reciente disponible. También puedes verificar y actualizar manualmente:

```bash
cgx update          # muestra versión instalada, versión más reciente e instrucciones de actualización
cgx update --auto   # detecta tu método de instalación y actualiza automáticamente
```

Establece `CGX_NO_UPDATE_CHECK=1` para deshabilitar la verificación en segundo plano.

<details>
<summary>Notas de actualización para versiones anteriores</summary>

> **¿En v0.5.2?** Ejecuta `cgx update --auto` o reinstala. v0.5.3 corrige un error de ingestión de grafo donde `cgx analyze` creaba un nodo `Author` inválido por archivo (nombrado con la ruta), inflando el recuento de comunidades con singletons por archivo y produciendo una vista de propiedad sin sentido. **Vuelve a ejecutar `cgx analyze --force` después de actualizar** para que la autoría se re-indexe por contribuidor: verás caer el recuento de comunidades y la propiedad de `cgx docs` será correcta. También pule el vault de docs (Glossary, tablas de estructura de notas de módulo, enlaces inter-clúster, tabla de propietarios) y hace que `cgx update --auto` verifique que la actualización realmente cambió la versión. Sin cambios de esquema.
>
> **¿En v0.5.1?** Ejecuta `cgx update --auto` o reinstala. v0.5.2 es una versión de pulido: `cgx todos` ya no reporta prosa JSDoc (`Note:`, `**Warning:**`, la palabra "bugs") como etiquetas de anotación, y el texto mostrado ahora es la línea coincidente en lugar del abre-bloque `/**` — vuelve a ejecutar `cgx analyze --force` para re-extraer etiquetas. La TUI también obtiene una cuadrícula determinista como fallback cuando la simulación de fuerza dirigida diverge, previniendo un error raro de NaN/Inf. Sin cambios de esquema.
>
> **¿En v0.5.0?** Ejecuta `cgx update --auto` o reinstala. v0.5.1 corrige los ganchos de git gestionados por cgx para que hagan no-op durante `git bisect` / `rebase` / `merge` (antes `git bisect run cgx bisect-script` ensuciaba `AGENTS.md` y `CGX_SKILL.md` en cada paso y rompía el siguiente checkout). `cgx bisect-script` ahora también sale con `125` (omitir) en lugar de pasar silenciosamente cuando el archivo de predicado está vacío o cuando `rule_violations_max` está configurado sin `--rule-violations N`. **Vuelve a ejecutar `cgx analyze` una vez después de actualizar** para que se instale la nueva plantilla de gancho.
>
> **¿En v0.4.x?** Ejecuta `cgx update --auto` o reinstala. v0.5.0 añade `cgx docs generate --vault` (vault de documentación Obsidian con visión general, TL;DR por archivo, clasificación de roles, propósitos de deps + detección de no usadas, estubos de prosa IA), `cgx bisect-script` (integra `git bisect run`), extracción de docstrings para Rust/Go/Java/PHP/Python (antes solo TypeScript), y cuatro nuevos métodos de consulta `GraphDb`. Vuelve a ejecutar `cgx analyze` después de actualizar para poblar los nuevos datos `doc_comment`.
>
> **¿En v0.3.0?** Ejecuta `cgx update --auto` o reinstala. v0.3.1 corrige `cgx complexity --combined` (ahora usa churn a nivel de archivo), añade `cgx test coverage --by=community`, mejora los mensajes de resultado vacío de `cgx todos`, muestra reglas incorporadas disponibles en `cgx rules list`, y añade un aviso de índice obsoleto a `cgx complexity`. También añade comandos antes no documentados: `cgx impact`, `cgx init`, `cgx list`, `cgx query deps`, `cgx query community`.
>
> **¿En v0.2.x?** Ejecuta `cgx update --auto` o reinstala. v0.3.0 añade `cgx todos`, `cgx docs coverage`, `cgx complexity`, `cgx test coverage/gaps`, `cgx deps health`, `cgx review`, `cgx rules check`, `cgx dupes`, `cgx explain`, y `cgx timeline` — un conjunto completo de comandos de inteligencia de código avanzada. Vuelve a ejecutar `cgx analyze --force` después de actualizar para refrescar el grafo con nuevas columnas (complexity, doc_comment, is_tested, test_count).
>
> **¿En v0.1.9 o anterior?** Ejecuta `cgx update --auto` o reinstala. v0.2.0 añadió detección de código muerto (`cgx query dead-code`) y corrigió el seguimiento de llamadas inter-archivo: `new ClassName()`, llamadas a métodos estáticos y llamadas a funciones a nivel de módulo ahora crean aristas apropiadas, eliminando la mayoría de los falsos positivos.

</details>

---

## Integración con IA

### Método 1 — Skills (Habilidades) (funciona en todas partes, configuración cero)

Después de `cgx analyze`, aparece un archivo `CGX_SKILL.md` en la raíz de tu repositorio. Cualquier asistente IA que pueda leer archivos y ejecutar comandos de terminal — Claude Code, Cursor, GitHub Copilot Chat, Gemini CLI — lo usará automáticamente.

El archivo de habilidad le indica a tu IA:
- Cuándo llamar a `cgx query` en lugar de leer archivos fuente
- Tanto nombres de herramientas MCP (`get_blast_radius`) como equivalentes CLI (`cgx query blast-radius`) en un solo lugar
- Estadísticas en vivo sobre tu base de código (hotspots, comunidades, puntos de entrada)
- Lenguaje de disparador obligatorio para que la IA active cgx sin que se le pregunte

**Resultado:** Tu IA deja de leer archivos fuente para responder una pregunta arquitectónica y ejecuta un solo comando `cgx query` en su lugar. `get_repo_summary` cuesta ~150 tokens; leer un único archivo fuente grande cuesta 15.000–50.000. Misma respuesta, sin abrir un archivo.

### Método 1b — Comando slash `/cgx` en Claude Code

```bash
cgx setup   # instala ~/.claude/skills/cgx/SKILL.md + registra /cgx
```

Después de ejecutar `cgx setup`, escribe `/cgx` en Claude Code para analizar cualquier repositorio de forma interactiva — incluso repos que no han sido pre-indexados. Funciona como `/graphify` pero para consultas estructurales de código en lugar de construcción de grafo de conocimiento.

### Método 2 — Servidor MCP (Cursor, Claude Code, Windsurf, Codex/OpenCode)

```bash
cgx setup    # auto-detecta tus editores y escribe sus configs MCP
```

Reinicia tu editor. cgx ahora expone 10 herramientas tipadas que tu IA puede llamar directamente:

| Herramienta | Qué responde | Tokens típicos |
|---|---|---|
| `get_repo_summary` | Visión general arquitectónica: nodos, comunidades, hotspots, nodos god | ~150 |
| `find_symbol` | ¿Dónde está definido X? Archivo + línea | ~50 |
| `get_neighbors` | ¿De qué depende X? ¿Qué depende de X? | ~50 |
| `get_blast_radius` | ¿Qué se rompe si modifico X? Nivel de riesgo + conteo afectado | ~50 |
| `get_call_chain` | Traza de A a B a través del grafo de llamadas | ~100 |
| `get_community` | Todos los nodos en el clúster auth/db/payments | ~200 |
| `search_graph` | Búsqueda de texto completo sobre todos los nombres de símbolo | ~100 |
| `get_hotspots` | Archivos con mayor churn × acoplamiento | ~100 |
| `get_file_owners` | Propiedad git blame para cualquier archivo | ~50 |
| `get_dead_code` | Exports sin referencia, funciones inalcanzables, archivos zombie — con confianza + pistas de falsos positivos | ~100 |
| `run_query` | SELECT SQL crudo contra el grafo (solo lectura) | varía |

Cada respuesta incluye un campo `_summary` — una oración en texto plano que el modelo lee primero antes de parsear JSON, para que pueda omitir la inspección más profunda cuando no sea necesaria.

**Ejemplo:** Pregunta "refactoriza la función de login para añadir rate limiting" en Claude Code. Llama `get_blast_radius`, `get_neighbors` y `get_file_owners` — 3 llamadas de herramienta, menos de **200 tokens en total** — y luego escribe el código sabiendo exactamente qué necesita actualizar.

---

## Cómo se compara cgx

|  | cgx | GitNexus | Graphify |
|---|---|---|---|
| Análisis Tree-sitter | ✅ | ✅ | ✅ |
| Seguimiento de llamantes JSX/TSX | ✅ | ❌ | ❌ |
| Resolución inter-archivo | ✅ | ✅ | ❌ |
| Historial Git (churn/blame) | ✅ | ❌ | ❌ |
| Grafo de co-cambio | ✅ | ❌ | ❌ |
| Detección de código muerto | ✅ | ❌ | ❌ |
| Puntuación de complejidad cognitiva | ✅ | ❌ | ❌ |
| Índice de anotaciones TODO/FIXME | ✅ | ❌ | ❌ |
| Reporte de cobertura de docs | ✅ | ❌ | ❌ |
| Superposición de cobertura de tests | ✅ | ❌ | ❌ |
| Salud CVE de dependencias | ✅ | ❌ | ❌ |
| Generador de brief de revisión PR | ✅ | ❌ | ❌ |
| Reglas de fitness de arquitectura | ✅ | ❌ | ❌ |
| Detección de duplicados/clones | ✅ | ❌ | ❌ |
| Explicador de arquitectura | ✅ | ❌ | ❌ |
| Snapshots de línea de tiempo de commits | ✅ | ❌ | ❌ |
| TUI de terminal | ✅ | ❌ | ❌ |
| Grafo en navegador WebGL | ✅ | ❌ | ✅ |
| Chat IA (multi-proveedor) | ✅ | ❌ | ❌ |
| Ollama / LLM local | ✅ | ❌ | ❌ |
| Servidor MCP | ✅ | ✅ | ❌ |
| Sistema de Skills | ✅ | ❌ | ✅ |
| Enlaces para compartir (sin instalación) | ✅ | ❌ | ❌ |
| Publicación GitHub Pages | ✅ | ❌ | ❌ |
| Binario autónomo | ✅ | ❌ | ❌ |
| Se requiere LLM para indexar | ❌ Nunca | ❌ Nunca | ✅ Siempre |
| Sobrecarga de contexto de sesión | ~1.300 tokens | desconocido | ~15.000 tokens |
| Costo de `get_repo_summary` | ~150 tokens | n/a MCP | sin MCP |
| Licencia | MIT | No comercial | MIT |

---

## Videos de Demo

<table>
<tr>
<td align="center" width="50%">

**CLI**

[![cgx CLI demo](https://aayushbahukhandi.github.io/cgx/thumb-cli.jpg)](https://aayushbahukhandi.github.io/cgx/cgx-cli.mp4)

</td>
<td align="center" width="50%">

**Interfaz Web**

[![cgx Web UI demo](https://aayushbahukhandi.github.io/cgx/thumb-web.jpg)](https://aayushbahukhandi.github.io/cgx/cgx-web.mp4)

</td>
</tr>
</table>

---

## Comandos Principales

### Análisis

```bash
cgx analyze                        # indexa el repo actual
cgx analyze ./path                 # indexa cualquier ruta local
cgx analyze github:owner/repo      # clona desde GitHub e indexa (almacenado en ~/.cgx/clones/)
cgx analyze --watch                # recarga en vivo al guardar archivo
cgx analyze --incremental          # re-analiza solo archivos cambiados (usado por ganchos git)
cgx analyze --no-git               # omite capa de historial git
cgx analyze --force                # reindexación limpia completa
cgx analyze --verbose              # salida detallada durante el análisis
cgx analyze --no-cluster           # omite detección de comunidades
cgx analyze --quiet                # suprime salida
```

### Visualizar

```bash
cgx view                       # TUI de terminal (funciona sobre SSH)
cgx view --web                 # grafo WebGL en navegador — auto-analiza si no está indexado
cgx view --community=3         # limita vista TUI a un clúster
```

> En la TUI de terminal, presiona `e` en un nodo seleccionado para ver su grafo ego (vecinos hasta 2 saltos).

### Compartir

```bash
cgx share                      # sube grafo a un GitHub Gist → URL de visor hospedado
cgx share --token ghp_xxx      # usa un token GitHub específico
cgx share --public             # hace el Gist público (por defecto: secreto)
# Para dejar de compartir: gh gist delete <gist-id>   (mostrado en la salida de cgx share)
```

`cgx share` requiere un token GitHub con ámbito `gist`. Usa (en orden): `--token`, variable de entorno `GITHUB_TOKEN`, o `gh auth token` si tienes la CLI de GitHub instalada.

La URL devuelta se ve así:
```
https://aayushbahukhandi.github.io/cgx/?data=https://gist.githubusercontent.com/...
```
Cualquiera puede abrir ese enlace en un navegador: no se necesita instalar cgx.

### Publicar en GitHub Pages

```bash
cgx publish                    # empuja sitio de grafo autónomo a rama gh-pages
cgx publish --dry-run          # previsualiza qué se empujaría
cgx publish --badge            # imprime markdown de badge para README
```

### Consultar

```bash
cgx query find "AuthService"            # localiza cualquier símbolo
cgx query find "login" --kind=Function  # filtra por tipo
cgx query deps "AppError"               # dependencias de un nodo
cgx query blast-radius "deleteUser"     # ¿qué se rompe si esto cambia?
cgx query chain "login -> AppError"     # traza una cadena de llamadas (formato: "desde -> a")
cgx query community 7                   # todos los nodos en comunidad #7
cgx query dead-code                     # exports sin referencia, funciones inalcanzables, archivos zombie
cgx query dead-code --kind=exports      # filtra: exports | functions | variables | files | disconnected
cgx query dead-code --confidence=high   # solo candidatos de alta confianza
cgx query dead-code --summary           # tabla de conteo por categoría y confianza
cgx query search "session"             # busca por nombre de símbolo
cgx query owners src/payments/index.ts  # propiedad git blame para un archivo
```

### Inteligencia Git

```bash
cgx hotspots                   # alto churn × alto acoplamiento = zona de peligro
cgx blame-graph                # propiedad por contribuidor
cgx diff HEAD~5                # diff arquitectónico entre commits
cgx impact                     # impacto aguas abajo de cambios en los últimos 7 días
cgx impact --since=14          # mira hacia atrás N días
```

### Documentación y Anotaciones

```bash
cgx todos                                    # lista todas las etiquetas TODO/FIXME/HACK/NOTE
cgx todos --kind=FIXME                       # filtra por tipo de etiqueta
cgx todos --comment-type=jsx                 # solo comentarios JSX {/* */}
cgx todos --comment-type=jsx_commented_code  # bloques de código JSX comentados
cgx todos --json                             # salida como JSON
cgx docs coverage                            # % de funciones exportadas con comentarios de doc
```

### Complejidad

```bash
cgx complexity                     # top 20 funciones por complejidad cognitiva
cgx complexity --threshold=0.15    # funciones con score > 0.15
cgx complexity --combined          # ordena por riesgo combinado complejidad × churn
```

### Cobertura de Tests

```bash
cgx test coverage                  # estadísticas generales de cobertura de tests
cgx test coverage --by=community   # desglose por clúster
cgx test gaps                      # funciones de alto acoplamiento sin testear
cgx test suggest                   # lista priorizada para escribir tests
```

### Salud de Dependencias

```bash
cgx deps health                    # reporte completo — paquetes, versiones, conteos CVE
cgx deps health --critical         # solo paquetes afectados por CVE
cgx deps audit                     # conteo rápido CVE por paquete
cgx deps outdated                  # paquetes con versiones más recientes disponibles
```

### Asistente de Revisión PR

```bash
cgx review                         # rama actual vs main/master
cgx review feature/my-branch       # rama específica vs main
cgx review HEAD~5                  # últimos 5 commits
cgx review --format=markdown       # formato de comentario GitHub PR
cgx review --format=github-actions # formato de anotación GitHub Actions
```

### Reglas de Arquitectura

```bash
cgx rules check                    # ejecuta todas las reglas en .cgx/rules.toml
cgx rules check --rule=no-cycles   # ejecuta una regla específica
cgx rules list                     # lista todas las reglas definidas
```

Ejemplo `.cgx/rules.toml`:

```toml
[[rules]]
name = "no-circular-dependencies"
built_in = "no_cycles"
severity = "error"

[[rules]]
name = "no-direct-db-access-outside-repository-layer"
description = "Solo archivos de repository deben importar desde db/"
severity = "error"
query = """
SELECT n.path, e.dst FROM edges e
JOIN nodes n ON n.id = e.src
WHERE e.kind = 'IMPORTS' AND e.dst LIKE '%/db/%'
  AND n.path NOT LIKE '%/repository/%'
"""
```

Reglas incorporadas: `no_cycles`, `max_coupling`, `max_complexity`, `require_docs_for_public`.

### Detección de Duplicados

```bash
cgx dupes                          # todos los pares clon (umbral 80%)
cgx dupes --threshold=0.9          # solo clones casi idénticos
cgx dupes --kind=exact             # solo duplicados exactos
```

### Explicador de Arquitectura

```bash
cgx explain AuthService            # explica un símbolo específico
cgx explain src/auth/              # explica una carpeta
cgx explain --onboard              # guía de onboarding completa para el repo
cgx explain --onboard --out=ARCHITECTURE.md  # escribe a archivo
```

### Línea de Tiempo

```bash
cgx timeline                       # snapshot últimos 20 commits
cgx timeline --commits=50          # últimos N commits
cgx timeline --since=2024-01       # desde una fecha
cgx timeline --json                # salida como JSON
```

### Exportar

```bash
cgx export --format=json       # grafo JSON completo
cgx export --format=mermaid    # pega en cualquier README
cgx export --format=graphml    # importa en Gephi / yEd
```

### Mantenimiento

```bash
cgx summary                    # estadísticas del repo: nodos, aristas, lenguajes, comunidades
cgx doctor                     # ejecuta diagnósticos sobre tu instalación
cgx clean                      # elimina datos indexados para el repo actual
cgx clean --all                # elimina TODOS los repos indexados
cgx status                     # muestra estado del índice para repos registrados
cgx init                       # crea .cgx/config.toml con valores por defecto
cgx init --yes                 # sin interacción con valores por defecto
cgx list                       # lista todos los repos indexados con conteos de nodos/aristas
```

---

## Chat IA

La interfaz web (`cgx view --web`) incluye un panel de chat integrado. Haz preguntas en lenguaje natural sobre tu base de código.

### Proveedores de IA Compatibles

#### OpenAI
```bash
export OPENAI_API_KEY=sk-...
cgx serve
```

#### Anthropic
```bash
export ANTHROPIC_API_KEY=sk-ant-...
export CGX_CHAT_PROVIDER=anthropic
export CGX_CHAT_MODEL=claude-haiku-4-5
cgx serve
```

#### Ollama (totalmente local, sin clave API)
```bash
ollama pull codellama
export CGX_CHAT_PROVIDER=ollama
export CGX_CHAT_MODEL=codellama
cgx serve
```

#### Cualquier API compatible con OpenAI
```bash
export CGX_CHAT_PROVIDER=openai-compatible
export CGX_CHAT_BASE_URL=https://api.together.ai/v1
export CGX_CHAT_API_KEY=your-key
export CGX_CHAT_MODEL=meta-llama/Llama-3-70b-chat-hf
cgx serve
```

```bash
cgx serve                              # inicia servidor (abre navegador automáticamente)
cgx serve --no-open                    # inicia servidor sin abrir navegador
```

> **Nota de privacidad:** el chat de cgx envía solo metadatos del grafo a la IA: nombres de nodos, rutas de archivos, puntuaciones de churn, etiquetas de comunidad. Nunca envía tu código fuente. Con Ollama, nada sale de tu máquina.

---

## Lenguajes Soportados

| Lenguaje | Parser | Estado |
|---|---|---|
| TypeScript / TSX | tree-sitter-typescript | ✅ Estable — incl. extracción de comentarios JSX |
| JavaScript / JSX | tree-sitter-javascript | ✅ Estable — incl. extracción de comentarios JSX |
| Python | tree-sitter-python | ✅ Estable |
| Rust | tree-sitter-rust | ✅ Estable |
| Go | tree-sitter-go | ✅ Estable |
| Java | tree-sitter-java | ✅ Estable |
| PHP | tree-sitter-php | ✅ Estable |
| C# | tree-sitter-java (fallback) | 🔧 Beta |
| C / C++ | — | 📋 Planificado |
| Swift | — | 📋 Planificado |
| Ruby | — | 📋 Planificado |

¿Quieres que se agregue un idioma? [Abre un issue](https://github.com/AayushBahukhandi/cgx/issues/new) o envía un PR — los nuevos parsers son un solo archivo en `crates/cgx-engine/src/parsers/`.

---

## Configuración

cgx no requiere archivo de configuración para uso básico. Todo tiene valores por defecto sensatos.

### Excluir rutas del análisis

cgx omite automáticamente los artefactos de compilación: `node_modules/`, `target/`, `dist/`, `*-dist/` (p. ej., `web-ui-dist/`), `.next/`, `coverage/`, `vendor/`, `venv/`, `*.min.js`, `*.bundle.js`, y similares.

Para exclusiones personalizadas, crea `.cgxignore` en la raíz de tu repositorio — misma sintaxis que `.gitignore`:

```gitignore
# .cgxignore
generated/
proto/
vendor/
*_pb.ts
```

Los archivos `.cgxignore` en subdirectorios también funcionan (alcance limitado a ese subárbol).

### Configuración avanzada

Para personalización adicional, crea `.cgx/config.toml` en la raíz de tu repositorio:

```toml
[analyze]
# Lenguajes a parsear (por defecto: todos soportados)
languages = ["typescript", "javascript", "python"]

# Directorios a omitir (prefiere .cgxignore para exclusiones por repo)
exclude = ["vendor/", "generated/", "*.pb.go"]

# Ventana de historial git para cálculo de churn (días)
churn_window_days = 90

# Conteo mínimo de co-cambios para crear una arista CO_CHANGES
co_change_threshold = 2

[chat]
# Proveedor por defecto (openai | anthropic | ollama | openai-compatible)
provider = "ollama"
model = "codellama"
ollama_host = "http://localhost:11434"

[serve]
port = 7373
auto_open = true

[skill]
auto_generate = true
```

---

## Arquitectura

cgx está construido en Rust (motor principal) y TypeScript (interfaz web).

```
cgx-engine  — Análisis Tree-sitter, almacenamiento DuckDB, análisis git,
              clustering Leiden, exportación, generación de skills
cgx-cli     — Todos los comandos de usuario, TUI (Ratatui), servidor HTTP (Axum),
              interfaz web embebida vía rust-embed (binario autónomo)
cgx-mcp     — Servidor stdio MCP (JSON-RPC 2.0)
web-ui      — Vite + React + grafo WebGL Sigma.js
```

El grafo se almacena localmente en `~/.cgx/repos/<hash>.db` — un archivo DuckDB por repo. Sin servicios externos, sin cloud, sin red necesaria para uso local.

---

## Contribuir

cgx tiene licencia MIT y welcomes contribuciones.

```bash
git clone https://github.com/AayushBahukhandi/cgx
cd cgx
cargo build --workspace
npm install && npm run build

# Ejecutar tests
cargo test --workspace

# Lint
cargo clippy --workspace -- -D warnings -D clippy::unwrap_used
cargo fmt --all
```

**Mejores lugares para contribuir:**
- Nuevos parsers de lenguaje — un solo archivo en `crates/cgx-engine/src/parsers/`
- Nuevos formatos de exportación
- Mejoras en la TUI
- Funciones de la interfaz web

Consulta [CONTRIBUTING.md](CONTRIBUTING.md) para la guía completa.

---

## Hoja de Ruta

**v0.4.0 — Indexación en Vivo y Flujo de Trabajo de Agentes (lanzada)**
- [x] `cgx watch` — comando watch de alto nivel con re-análisis incremental con debounce en cambios de archivo (`--debounce-ms` ajustable)
- [x] `cgx query context <symbol>` — briefing de agente en un paso: llamantes + deps + comunidad + riesgo en ~400 tokens, con `--json` para consumo de herramientas
- [x] `cgx hook` + `cgx setup --hooks` — gancho opt-in Claude Code PreToolUse que inyecta contexto de archivo en Edit/Write/MultiEdit
- [x] `cgx clean --orphaned` — barra entradas de registro obsoletas Y archivos `.db` sin referencia en `~/.cgx/repos/`
- [x] `cgx clean --budget <SIZE>` — evicción LRU (p. ej., `--budget 2G`); evicción automática opt-in vía variable de entorno `CGX_MAX_CACHE_BYTES`
- [x] `RepoEntry.last_used_at` — actualizado en cada consulta para seguimiento LRU
- [x] UX Spinner durante fases de análisis (walk / parse / resolve / store / git / cluster) vía indicatif
- [x] Aviso coloreado "⚡ Update available" vía `console::style`
- [x] Corrección de error: análisis incremental ya no hace double-upsert de aristas (resolvió error de índice ART de DuckDB en repos grandes)
- [x] Corrección de error: `cgx doctor` sugiere `cgx clean --orphaned` cuando se detectan entradas de registro huérfanas

<details>
<summary><strong>Lanzamientos anteriores (v0.3.x y anteriores)</strong></summary>

**v0.3.2 — Correcciones de Errores (lanzada)**
- [x] `cgx share` — corregido: el visor ahora carga el grafo compartido en lugar del grafo cgx publicado cuando está presente el parámetro URL `?data=`
- [x] `cgx publish` — corregido: crash en repos con subdirectorios en `assets/` (entrada de árbol git inválida)
- [x] `cgx share` — ahora imprime `gh gist delete <id>` en la salida para que los usuarios sepan cómo dejar de compartir

**v0.3.1 — Correcciones de Errores y Documentación (lanzada)**
- [x] `cgx complexity --combined` — corregido: ahora usa correctamente churn a nivel de archivo (no churn de función siempre-cero)
- [x] `cgx test coverage --by=community` — implementado: tabla de desglose de cobertura de tests a nivel de comunidad
- [x] Mensajes de resultado vacío de `cgx todos` — corregido: "No FIXME annotation comments found." en lugar de "Run cgx analyze"
- [x] Advertencia obsoleta de `cgx complexity` — advierte cuando todos los scores son 0.00 y sugiere reindexación `--force`
- [x] `cgx rules list` — ahora muestra las 4 reglas incorporadas disponibles incluso sin un `.cgx/rules.toml`
- [x] Formato `cgx query chain` — README corregido a formato `"A -> B"`
- [x] `cgx query search` — aclarado como búsqueda por nombre de símbolo (no búsqueda de código texto completo)
- [x] `cgx impact`, `cgx init`, `cgx list`, `cgx query deps`, `cgx query community` — documentados en README

**v0.3.0 — Inteligencia de Código Avanzada (lanzada)**
- [x] `cgx todos` — índice de anotaciones (TODO/FIXME/HACK/NOTE/BUG/OPTIMIZE/WARN/XXX, comentarios JSX)
- [x] `cgx docs coverage` — cobertura de documentación por comunidad
- [x] `cgx complexity` — puntuación de complejidad cognitiva por función
- [x] `cgx test coverage` / `cgx test gaps` — superposición de cobertura de tests vía aristas TESTS
- [x] `cgx deps health` / `cgx deps audit` — salud CVE de dependencias (API OSV)
- [x] `cgx review` — brief de revisión PR (radio de explosión, hotspots, tests faltantes, revisores)
- [x] `cgx rules check` — funciones de fitness de arquitectura (SQL + reglas incorporadas)
- [x] `cgx dupes` — detección de duplicados/clones vía huellas AST normalizadas
- [x] `cgx explain` — explicador de arquitectura para símbolos, carpetas y onboarding completo
- [x] `cgx timeline` — snapshots de línea de tiempo de commits git

</details>

**Próximos pasos**
- [ ] `cgx changelog` — genera changelogs desde diffs de grafo
- [ ] Extensión VS Code
- [ ] Presupuesto de tokens por consulta (`cgx query <cmd> --budget=tokens` con truncamiento basado en importancia)
- [ ] Auto-commit de diagrama Mermaid a docs/ en cada push (GitHub Action)
- [ ] Parsers Ruby, Swift, C/C++
- [ ] Búsqueda semántica de código (`cgx query search --semantic` con resúmenes LLM opcionales)
- [ ] cgx cloud — grafos compartidos para equipos (hospedado)

---

## Licencia

MIT — úsalo en cualquier cosa, comercial o de otro tipo.

---

<div align="center">

Construido con Rust 🦀 · Tree-sitter · DuckDB · Sigma.js

**[Dale estrella a este repositorio](https://github.com/AayushBahukhandi/cgx) si cgx te ahorra tiempo.**

</div>
