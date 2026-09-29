**Novedad principal: Dilo ahora se actualiza sola, con un clic.**

- Actualizaciones desde la app: aviso en Inicio y en el menú de la barra, botón "Actualizar" en Ajustes → Actualizaciones. Descarga la versión nueva, verifica su firma, se instala y se reinicia sola.
- No vuelve a pedir autorización a macOS ni tus permisos de Accesibilidad y Micrófono (la nueva versión se firma con el mismo certificado).
- Si el archivo de actualización no coincide con la firma, se rechaza y no se instala nada.
- Ajustes → Actualizaciones también tiene "Buscar automáticamente" para apagar la búsqueda en segundo plano.
- Mensaje en español cuando falta el permiso de Accesibilidad al cambiar la tecla.

**Si vienes de la 0.2:** esa versión no trae el actualizador, así que instala esta a mano una última vez (abre `Dilo.dmg`, arrastra Dilo a Aplicaciones y reemplaza). De aquí en adelante se actualiza sola.

Requiere Mac con Apple Silicon (M1 o más nuevo) y macOS 13+. Al abrirla por primera vez, macOS pedirá autorizarla: **Ajustes del Sistema → Privacidad y seguridad → "Abrir de todos modos"**, o `xattr -dr com.apple.quarantine /Applications/Dilo.app`.
