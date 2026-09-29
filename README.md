<p align="center"><img src="assets/icon-1024.png" width="120" alt="Dilo"></p>

# Dilo

**Mantén una tecla, habla, suelta. Tus palabras aparecen donde está tu cursor.**
Gratis, open source y 100 % local: tu voz nunca sale de tu computadora.

*Hold a key, talk, let go — your words land wherever your cursor is. Free, open source, fully offline.*

▶️ [Video de presentación (30 s)](video/Dilo_presentacion.mp4)

## Descargar

1. Baja **Dilo.dmg** de la [última versión](../../releases/latest) y arrastra Dilo a Aplicaciones.
2. La primera vez macOS dirá que no puede verificar al desarrollador (la app aún no está notarizada por Apple).
   Ábrela con **Ajustes del Sistema → Privacidad y seguridad → "Abrir de todos modos"**, o corre esto una vez en Terminal:
   ```bash
   xattr -dr com.apple.quarantine /Applications/Dilo.app
   ```
3. Dale permiso de **Accesibilidad** y **Micrófono** cuando lo pida, y deja que descargue el modelo de voz (~670 MB, una sola vez).

Requiere una Mac con **Apple Silicon (M1 o más nuevo)** y macOS 13 o superior.

### Actualizaciones

Desde la versión 0.3, Dilo se actualiza sola: cuando hay una versión nueva verás un aviso en **Inicio** y en el menú de la barra;
pulsa **Actualizar** (o ve a **Ajustes → Actualizaciones**) y se descarga, se verifica su firma, se instala y Dilo se reinicia. No vuelve
a pedir autorización a macOS ni tus permisos. Si vienes de la 0.2, instala la 0.3 a mano una última vez.

## Cómo funciona

1. Mantén presionada **Fn** (Mac) o **Control derecha** (Windows/Linux).
2. Habla. Un monito aparece junto a tu mouse: su boca es la onda de tu voz.
3. Suelta. El texto se pega en la app donde estés escribiendo y el monito se traga a sí mismo.

Desde el ícono de la barra de menú abres:
- **Historial** — todo lo que has dictado, con búsqueda (ignora acentos y mayúsculas), edición, copiar y borrar con "Deshacer".
- **Diccionario** — correcciones ("cloud" → "Claude") y atajos ("mi correo" → tu email).
- **Estadísticas** — palabras totales, de hoy y de la semana, promedio por día y por dictado, palabras por minuto, racha, tiempo ahorrado, gráfico de 30 días e idiomas.
- **Monito** — cinco estilos (Wave, Glass, Jolly, Dot, Aura) y tres tamaños.
- **Ajustes** — idioma, tecla, micrófono, memoria, iniciar con la Mac.

## Motor de voz

[Parakeet TDT 0.6B v3](https://huggingface.co/nvidia/parakeet-tdt-0.6b-v3) de NVIDIA (int8, ONNX), corriendo en tu equipo.
Detecta solo 25 idiomas europeos (español, inglés, francés, alemán, portugués, italiano…) y puedes mezclarlos en la misma frase.
Descarga única de ~670 MB, verificada con SHA-256.

Medido en un MacBook con Apple M5:

| | |
|---|---|
| Arrancar el motor | ~0,9 s (ocurre mientras hablas, no lo notas) |
| Transcribir 4–5,5 s de audio | ~0,17–0,23 s |
| RAM de la app en reposo | ~85–95 MB |
| RAM del motor mientras está activo | ~1,3 GB (proceso aparte) |

El motor corre en un proceso separado que se cierra tras 2 minutos sin uso (configurable),
así que esa memoria vuelve completa al sistema. Mientras no dictas, Dilo ocupa menos de 100 MB.

## Permisos en Mac

- **Accesibilidad**: para detectar la tecla y pegar el texto.
- **Micrófono**: para escucharte (solo mientras mantienes la tecla).
- Recomendado: Ajustes del Sistema → Teclado → "Al pulsar la tecla 🌐" → **No hacer nada**.

## Publicar una versión (para quien mantiene el proyecto)

1. Sube el número de versión en `src-tauri/tauri.conf.json`, `src-tauri/Cargo.toml` y `package.json`, y escribe las notas en `release-notes.md`.
2. `./scripts/release.sh` compila, firma (app y paquete de actualización), y deja todo en `release/`; con `--publish` crea la release en GitHub.
3. Necesitas la llave de actualizaciones en `~/.tauri/dilo-updater.key` (se genera con `bun tauri signer generate -w ~/.tauri/dilo-updater.key`;
   **no la subas al repo ni la pierdas**: sin ella no se pueden firmar actualizaciones) y el mismo certificado de firma de siempre.
   La llave pública va en `tauri.conf.json`.

## Compilar

Requisitos: [Rust](https://rustup.rs), [Bun](https://bun.sh), Xcode Command Line Tools (Mac).

```bash
bun install
bun tauri dev                    # desarrollo
./scripts/build-local.sh         # Dilo.app firmada con tu certificado local
# o simplemente: bun tauri build --bundles app
cd src-tauri && cargo test --lib # pruebas
```

## Estado

- ✅ macOS
- 🚧 Windows y Linux: el código compila para ellos, pero falta probarlos y pulir el pegado.

## Créditos

Dilo se apoya en el trabajo de [Handy](https://github.com/cjpais/Handy) (MIT) de CJ Pais, y usa sus librerías
[`transcribe-rs`](https://github.com/cjpais/transcribe-rs) y [`handy-keys`](https://github.com/handy-computer/handy-keys).
Dilo es un proyecto independiente, sin afiliación con Handy.
También usa [Tauri](https://tauri.app), [cpal](https://github.com/RustAudio/cpal), [enigo](https://github.com/enigo-rs/enigo),
y el modelo Parakeet de NVIDIA (exportado a ONNX por [istupakov](https://huggingface.co/istupakov/parakeet-tdt-0.6b-v3-onnx)).

Inspirado por la función de dictado de Meta AI para Mac.

## Licencia

MIT
