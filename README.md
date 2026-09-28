<p align="center"><img src="assets/icon-1024.png" width="120" alt="Dicta"></p>

# Dicta

**Mantén una tecla, habla, suelta. Tus palabras aparecen donde está tu cursor.**
Gratis, open source y 100 % local: tu voz nunca sale de tu computadora.

*Hold a key, talk, let go — your words land wherever your cursor is. Free, open source, fully offline.*

> "Dicta" es un nombre provisional.

## Cómo funciona

1. Mantén presionada **Fn** (Mac) o **Control derecha** (Windows/Linux).
2. Habla. Un monito aparece junto a tu mouse: su boca es la onda de tu voz.
3. Suelta. El texto se pega en la app donde estés escribiendo y el monito se traga a sí mismo.

Desde el ícono de la barra de menú abres:
- **Historial** — todo lo que has dictado, con búsqueda, copiar y borrar.
- **Estadísticas** — palabras totales, de hoy y de la semana, promedio por día y por dictado, palabras por minuto, racha, tiempo ahorrado, gráfico de 30 días e idiomas.
- **Ajustes** — idioma, tecla, micrófono, memoria, monito.

## Motor de voz

[Parakeet TDT 0.6B v3](https://huggingface.co/nvidia/parakeet-tdt-0.6b-v3) de NVIDIA (int8, ONNX), corriendo en tu equipo.
Detecta solo 25 idiomas europeos (español, inglés, francés, alemán, portugués, italiano…) y puedes mezclarlos en la misma frase.
Descarga única de ~670 MB.

Medido en un MacBook con Apple M5:

| | |
|---|---|
| Arrancar el motor | ~0,9 s (ocurre mientras hablas, no lo notas) |
| Transcribir 4–5,5 s de audio | ~0,17–0,23 s |
| RAM de la app en reposo | ~85–95 MB |
| RAM del motor mientras está activo | ~1,3 GB (proceso aparte) |

El motor corre en un proceso separado que se cierra tras 5 minutos sin uso (configurable),
así que esa memoria vuelve completa al sistema. Mientras no dictas, Dicta ocupa menos de 100 MB.

## Permisos en Mac

- **Accesibilidad**: para detectar la tecla Fn y pegar el texto.
- **Micrófono**: para escucharte (solo mientras mantienes la tecla).
- Recomendado: Ajustes del Sistema → Teclado → "Al pulsar la tecla 🌐" → **No hacer nada**.

## Compilar

Requisitos: [Rust](https://rustup.rs), [Bun](https://bun.sh), Xcode Command Line Tools (Mac).

```bash
bun install
bun tauri dev                    # desarrollo
bun tauri build --bundles app    # Dicta.app
cd src-tauri && cargo test --lib # pruebas
```

## Estado

- ✅ macOS
- 🚧 Windows y Linux: el código compila para ellos, pero falta probarlos y pulir el pegado.

## Créditos

Dicta se apoya en el trabajo de [Handy](https://github.com/cjpais/Handy) (MIT) de CJ Pais, y usa sus librerías
[`transcribe-rs`](https://github.com/cjpais/transcribe-rs) y [`handy-keys`](https://github.com/handy-computer/handy-keys).
Dicta es un proyecto independiente, sin afiliación con Handy.
También usa [Tauri](https://tauri.app), [cpal](https://github.com/RustAudio/cpal), [enigo](https://github.com/enigo-rs/enigo),
y el modelo Parakeet de NVIDIA (exportado a ONNX por [istupakov](https://huggingface.co/istupakov/parakeet-tdt-0.6b-v3-onnx)).

Inspirado por la función de dictado de Meta AI para Mac.

## Licencia

MIT
