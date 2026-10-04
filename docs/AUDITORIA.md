# Auditoría de Dilo — bugs corregidos y mejoras pendientes

Rama: `claude/dilo-subagentes-audit-bugs-qh0a20` · Revisión hecha con 5 subagentes en paralelo
(audio/motor/pegado, datos, sistema, interfaz React, scripts/producto).

**Verificación:** `cargo test --lib` del crate completo pasa 95 tests; `tsc --noEmit` y `vite build` limpios.
**No verificado** (necesita Mac): todo el código `cfg(target_os = "macos")` (CoreGraphics, AVFoundation, Fn,
bandeja, portapapeles), el CSP nuevo en la webview y el flujo real de dictado de punta a punta.

---

## 1. Bugs corregidos

### Audio, motor y pegado (`recorder`, `engine`, `paste`, `coordinator`, `models`)
| Sev. | Problema | Arreglo |
|---|---|---|
| Alta | Muestras NaN/inf contaminaban RMS y pico, llegaban al modelo y dejaban el monito al máximo | `clean()` las convierte a 0 y recorta a [-1, 1] |
| Alta | Panic dentro del modelo mataba el proceso del motor (recarga de varios segundos) | `catch_unwind` en el worker, devuelve error |
| Alta | `lock().unwrap()` en el callback de audio hacía panic con lock envenenado | Helper `lock()` tolerante |
| Alta | Descarga cortada dejaba el `.part` (hasta 650 MB) en disco | Se borra ante cualquier fallo |
| Alta | Linux/Windows: el texto del portapapeles desaparecía antes de pegar (handle se soltaba) | Handle persistente |
| Alta | Si `complete` hacía panic, `jobs` no bajaba y el modelo nunca se descargaba por inactividad | `JobGuard` (RAII) |
| Media | Ventana de nivel con atómicos sueltos → RMS falso | Un solo `Mutex`, suma en f64 |
| Media | Prefijo de longitud sin límite en el protocolo del worker (reserva de hasta 16 GB) | Máximo 20 min |
| Media | Logs extra en stdout o bytes no UTF-8 rompían el protocolo | `read_protocol` ignora ruido; lectura lossy |
| Media | Texto vacío pegaba un espacio y pisaba el portapapeles; el diccionario podía dejar texto vacío y se guardaba en historial | Se ignora sin tocar nada |
| Media | macOS: si `set_text` fallaba, el portapapeles quedaba vacío | Se restaura |
| Media | Descarga sin tope de bytes, sin timeout de conexión, y dos descargas simultáneas pisaban el `.part` | Tope, `connect_timeout(30s)`, Mutex |
| Baja | `simulate` reproducía WAV no 16 kHz mono a velocidad errónea | Valida formato |

### Datos (`store`, `rules`, `stats`, `settings`, `commands`, `updater`)
| Sev. | Problema | Arreglo |
|---|---|---|
| Alta | Regla larga que fallaba el límite de palabra bloqueaba a la corta ("code review" + "code" sobre "code reviewed") | Un regex anclado por regla, probadas por prioridad; límites de longitud |
| Alta | `dilo.db` corrupta impedía arrancar | Se mueve a `dilo.db.corrupt-<ts>` y se crea otra |
| Media | Un campo mal tipado reseteaba todo `settings.json`; JSON roto se pisaba sin copia; BOM rompía el parseo | Parseo campo a campo, copia `.corrupt`, tolera BOM, fsync al guardar |
| Media | Desbordes (`storage_warn_mb`, `days * 86_400_000`), `mic: ""`, `language` arbitrario | Topes, `saturating_*`, validación |
| Media | `created_at` corrupto podía hacer panic en stats; entradas futuras inflaban rachas; texto CJK/tailandés contaba 1 palabra por frase | Se omiten timestamps fuera de rango, sumas saturantes, conteo por carácter |
| Media | `word_count` negativo rompía el listado completo | Lectura i64 acotada |
| Media | Lock envenenado propagaba panics; búsqueda no encontraba acentos descompuestos | `conn()` recupera el lock; `fold` quita marcas combinantes |
| Media | Updater: flag `installing` no se limpiaba si fallaba/cancelaba; error confuso | `BusyGuard` (Drop), re-consulta si no hay update pendiente |
| Baja | `download_model` con panic dejaba `downloading=true`; `update_settings` desincronizaba autostart | `catch_unwind`, rollback del autostart |

Sin bug: no hay inyección SQL, `%`/`_` en búsqueda son literales, los metacaracteres regex se escapan,
las rachas usan días de calendario (DST ok), no hay path traversal, la comparación de versiones la hace el plugin de Tauri.

### Sistema (`hotkey`, `mascot`, `tray`, `lib`, `main`, `permissions`, config)
| Sev. | Problema | Arreglo |
|---|---|---|
| Alta/Media | Cambiar de tecla desregistraba la anterior antes de registrar la nueva: si fallaba, sin hotkey y sin aviso | Registra primero, conserva la anterior si falla |
| Media | Hilo del hotkey vivía para siempre; pulsación podía quedar colgada | Termina al cerrar el canal y emite release |
| Media | Shift+letra / Option+letra aceptados como hotkey (bloquean escribir mayúsculas/acentos) | Teclas de escritura exigen Ctrl, Cmd o Fn |
| Alta (Win/Linux) | Mascota fuera de pantalla (sin clamp derecho/inferior), multi-monitor, HiDPI (píxeles físicos vs lógicos), `available_monitors()` 60 veces/s | Clamp a 4 bordes, monitor más cercano, escalado por plataforma, consulta 1/s |
| Media | Migración Dicta→Dilo dejaba atrás `-wal`/`-shm`/`-journal` (filas recientes perdidas) | Se mueven también |
| Media | Capabilities (`core:default`) y CSP más abiertos de lo necesario | Permisos mínimos; CSP con `object-src/base-uri/form-action/frame-src/frame-ancestors` |
| Baja | `expect` en `main.rs` y `permissions.rs` (panic); tray mostraba check falso en idioma activo | Manejo de errores / `refresh` tras clic |

### Interfaz (`src/panel`, `src/shared`, `src/mascot`, `vite.config.ts`)
- `vite.config.ts` rompía el build en Windows (`/C:/...`).
- `useUpdate`: al cambiar de pestaña durante la descarga permitía un segundo install.
- `useModelStatus`: la respuesta inicial podía pisar un evento más nuevo.
- Mascota: el nivel de voz no se reseteaba entre dictados, el payload no se validaba, cleanup con rechazos sin capturar.
- Home: respuestas de stats/storage desordenadas (guard de secuencia), eje del gráfico mostraba "1" en vez de "0,5", "1 palabras".
- Diccionario: reglas duplicadas, sin buscador en listas grandes, inputs sin `aria-label`, switch con nombre cambiante.
- Historial/Ajustes: nombres accesibles, Esc cierra el menú, `ConfirmDialog` atrapa foco, polling de permisos pausado con ventana oculta, idioma guardado fuera de lista salía en blanco.

### Scripts y docs
- `release.sh` aborta si las versiones de `tauri.conf.json`/`Cargo.toml`/`package.json` no coinciden o si el tag ya existe; avisa si las release notes no mencionan la versión.
- `make-manifest.py`: error claro si falta el `.sig`.
- `HANDOFF.md` actualizado (idle 2 min, 5 skins, auto-update e inicio al arrancar ya hechos).

---

## 2. Mejoras pendientes (priorizadas)

### Alta
1. **Notarización de Apple.** Hoy hay que saltarse Gatekeeper a mano o usar `xattr`. Requiere Developer ID.
2. **Llave del updater** (`~/.tauri/dilo-updater.key`) en una sola Mac y sin contraseña: si se pierde, nadie puede actualizar. Guardarla en un gestor de secretos y ponerle contraseña.
3. **Sin CI** (`.github/` no existe). Workflow con `cargo test --lib`, `cargo clippy`, `bun run build`. Los tests con modelo están `#[ignore]`, el motor nunca se prueba solo.
4. **Windows y Linux no usables hoy:** `latest.json` solo publica `darwin-aarch64`; pegado en Windows, bandeja, Wayland (`enigo` probablemente falla) y restauración de portapapeles (solo texto) sin probar.
5. **`engine_real.rs` solo funciona en macOS** (rutas fijas de `HOME`/`Library`). Usar `dirs` o `DILO_MODEL_DIR`.
6. **Onboarding:** abrir con quitar cuarentena + Accesibilidad + Micrófono + 🌐 en "No hacer nada" + descarga de 670 MB, todo manual. Falta asistente de primer arranque con checklist y prueba de dictado. Es la causa nº 1 de "no funciona".
7. **Errores solo en el log:** "Accessibility permission not granted", fallo de micrófono, descarga o pegado no se muestran al usuario (el mascot tiene estados sad/confused pero no hay mensaje en el panel).

### Media
8. **i18n inexistente:** todo el texto y los `Intl` están fijos en español; "español por defecto + English" no se cumple. Falta diccionario `t()` y detección de locale.
9. **Privacidad:** el historial en SQLite guarda el texto en claro, sin retención configurable. Añadir "no guardar historial" y borrado automático; documentar en README el uso del portapapeles y la consulta a GitHub del updater.
10. **`hotkey.ts` solo con símbolos de Mac** (⌘⌥⌃, "de este Mac", "Ajustes del Sistema → 🌐"); `HotkeyHint` muestra "Mantén  y habla" con hotkey vacío; la bandeja muestra el string crudo ("CtrlRight"). Captura de hotkey sin botón Cancelar ni aviso de conflictos.
11. **Historial sin virtualización** ("Cargar más" hace crecer el DOM); filtros Hoy/Semana en cliente sobre lo ya cargado; paginación atada a páginas de 50. `Stats::compute` carga todo el historial en memoria (agregar en SQL).
12. **Modelo descargado solo validado por tamaño** (un archivo corrupto del tamaño correcto pasa); la descarga no se reanuda (sin `Range`) ni tiene timeout de lectura entre chunks.
13. **Sin watchdog de duración** si se pierde el release de la tecla (secure input); necesario en `coordinator.rs`.
14. **Dependencias de terceros** (`transcribe-rs 0.3.11`, `handy-keys 0.3.4`) con versiones laxas: fijar con `=` o vendorizar; `cargo audit`/dependabot para `cpal`, `rusqlite`, `reqwest`.
15. **Entitlements:** solo `audio-input`. Si se notariza con Hardened Runtime y ONNX carga un dylib dinámico, hará falta `com.apple.security.cs.disable-library-validation` (revisar también el worker `--engine-worker`).
16. **Tests de integración** del flujo (coordinator, paste, hotkey): los unit tests cubren datos y lógica pura.

### Baja
17. Animación del monito: `setState` a 60 fps incluso en estados estáticos (`done`, `sad`); pausar o mover a refs.
18. Tray: handler `DoubleClick` es código muerto; id `open:settings` duplicado; `icon_as_template` solo macOS (icono negro sobre fondo oscuro en Windows/Linux). Bucle del hotkey duerme 5 ms siempre (~200 despertares/s en reposo).
19. `lib.rs` repite `prompt_accessibility` y abre el panel en cada arranque si falta el permiso (también con `--hidden`).
20. Reglas del diccionario: mismo largo conserva el orden de la lista sin control en la UI; `match_case` solo preserva la mayúscula inicial ("iOS" → "IOS"); sin límite en `update_dictation`; restricción UNIQUE en `store.rs` en vez de comprobar duplicados en cliente; sin import/export.
21. Radiogroups (skins, filtros) sin navegación con flechas; gráfico de 30 días con tooltip solo con ratón; estados de carga vacíos ("–").
22. `MIN_PEAK_RMS = 0.01` (-40 dBFS) puede descartar micrófonos muy bajos o susurros: hacerlo configurable. Buffer a 192 kHz puede llegar a ~230 MB. `recorder.start` mantiene el lock mientras abre el dispositivo.
23. `video/Dilo_presentacion.mp4` (8,8 MB) inflando el repo: mover a Releases. `freezePrototype: true` en Tauri como endurecimiento extra (probar el frontend primero). El nombre "Dilo" sigue provisional y está repetido en muchos textos: centralizarlo.

### Oportunidades de producto (vs. Wispr Flow)
- **Limpieza de muletillas y puntuación automática** (lo más valioso a corto plazo; ya hay base en `rules.rs`) y reescritura con LLM local ("hazlo más formal").
- Modo manos libres (toggle) y atajo para deshacer el último pegado.
- Comandos de voz ("nueva línea", "borra eso").
- Modos por app (tono formal en Mail, código en el editor).
- Idioma forzado (Parakeet no lo permite), sincronización entre equipos, versiones móviles.

---

## 3. Orden sugerido
1. Probar la rama en el Mac (dictado completo, cambio de tecla, mascota con 2 monitores).
2. CI + arreglar `engine_real.rs` (barato y protege todo lo demás).
3. Onboarding con checklist + errores visibles en el panel.
4. i18n y modo "no guardar historial".
5. Notarización y publicación para Windows/Linux.
