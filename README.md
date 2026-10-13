# soc

Primera base local del asistente personal: presencia nativa flotante con cara siempre visible, kernel y SQLite. Rust + Slint, sin WebView. Primer hito macOS; otros sistemas aún no verificados. Incluye transcripción local Nemotron en un globo temporal y control de aplicaciones/música con Laya local. No conecta ChatGPT/Grok ni pega texto en otras apps. La presencia está configurada para acompañarte entre escritorios macOS; al actualizar debes cerrar y volver a abrir tu instancia para activar cambios de ventana. Pantalla completa tiene elegibilidad auxiliar, sin garantía para todas las apps.

## Ejecutar

Desde la raíz con Rust 1.92 o superior:
```
cargo run
```
.env local ya preparado con `SOC_DATABASE_PATH=data/soc.sqlite3`; al copiar checkout, crear `.env` a partir de `.env.example`. La ruta es obligatoria y relativa al cwd, sin default oculto. No abre puerto ni necesita credenciales.

La captura de voz también funciona con `cargo run`: al lanzarlo desde Terminal.app, macOS atribuye la solicitud a Terminal; autorizar Terminal en el panel de control de dispositivos/Accesibilidad. Verificado en esta sesión mediante registros de TCC, listener activo y confirmación del usuario. La app detecta la concesión sin reiniciar. Autorizar únicamente soc.app no autoriza esa ejecución desde Terminal.

Como alternativa, ejecutar el paquete autorizado. Cerrar primero la instancia existente desde Salir de soc. Después de build y empaquetado, desde esta raíz y usando la DB local indicada arriba:
```
open ./target/debug/soc.app --env "SOC_DATABASE_PATH=$PWD/data/soc.sqlite3"
```
Si usas otra DB, sustituir por su ruta absoluta. Autorizar exactamente este soc.app desde el panel del sistema. El binario target/debug/soc y el de soc.app son ejecutables distintos; conceder permiso al paquete no garantiza acceso al binario iniciado en terminal. Reempaquetar con firma ad hoc tras cambiar el binario puede requerir renovar la autorización.

- Clic en la cara: dormida ↔ activa. Arrastra la cara con el botón izquierdo para reubicar el asistente; arrastrar no cambia su estado. La posición dura hasta cerrar soc; al reiniciar vuelve a la esquina inicial.
- Icono de soc en la barra superior de macOS: abre un menú con Asistente → Despertar / Descansar, Acerca de soc y Salir de soc.
- Menú soc → Salir: cerrar. Menú soc → Acerca de soc: información y atribución Slint.
- Atajos M, A, Espacio y Escape, cierre con clic secundario y ayuda sobre la cara desactivados. Movimiento reducido conserva su preferencia y soporte interno para una configuración posterior.

Para una evaluación aislada: `SOC_DATABASE_PATH=/tmp/soc-review.sqlite3 cargo run`. No borrar datos existentes para repetir la demostración.

## Verificar y continuar

Comandos y alcance en [testing](docs/rules/testing.md). [Arquitectura](docs/architecture/overview.md), [requisitos](docs/initialization/requirements.md), [resultado](docs/initialization/implementation.md), [decisiones](docs/decisions/README.md) y [evidencia](docs/memory/index.md). Antes de modificar, leer [AGENTS.md](AGENTS.md) y reglas afectadas. `.creation` conserva prompt e historia originales, sin sustituir la documentación vigente.

La entrega Git inicial fue autorizada posteriormente; ver [registro de entrega](docs/memory/2026-10-11-git-delivery.md). No se solicita despliegue. Los detalles gráficos son supuestos revisables; consulta la evidencia runtime antes de asumir soporte multi-monitor, Spaces o de otros OS.

## Transcripción local
Con transcripción habilitada, soc carga Nemotron al abrir la aplicación sin activar el micrófono; si falta el archivo, lo descarga/verifica y después lo carga. «Cargando modelo» indica espera; «Modelo listo» confirma carga. La cara se despierta al iniciar dictado y vuelve a dormirse cuando desaparece el último globo. «Habla ahora» aparece solo después de preparar la sesión y abrir la entrada. Tras diez minutos de inactividad se libera; al siguiente dictado vuelve a cargar antes de escuchar. Soltar durante una carga cancela el inicio pendiente.

El menú de barra superior y soc → Voz y órdenes → Voz · Nemotron permite capturar el atajo pulsando sus teclas, elegir micrófono, modo mantener/alternar, borrado del globo y liberación del modelo. Pulsa el botón del atajo y luego la combinación; modificadores solos como Shift se capturan al soltarlos. Escape cancela. Se pausa el atajo global durante la captura. Guardar voz aplica los cambios después de persistirlos. Micrófono predeterminado del sistema o entrada explícita; Actualizar vuelve a listar dispositivos. Si desconectas el elegido, el dictado presenta error y conserva la selección. Default Ctrl+Option+Space, mantener, globo 15 s y modelo 10 min inactivo; 0 min libera al terminar. La migración 0002 actualizó el tiempo anterior a 10 min; la 0003 cambia el borrado de 5 a 15 s y conserva otros tiempos guardados y el resto de ajustes. Cada dictado abre un globo nuevo abajo y desplaza los anteriores hacia arriba, hasta cinco simultáneos. Los parciales completan el globo en curso; cada resultado desaparece según su propio plazo de borrado. Los mensajes son temporales; no se guarda audio/transcripción ni se escribe en otras apps.

Al faltar Nemotron, soc lo descarga automáticamente (~716 MiB), muestra porcentaje y avisa al verificar SHA-256. Desde Voz y órdenes → Voz · Nemotron → Preparar modelo se reintenta; la descarga no carga el modelo en RAM. Almacén: ~/Library/Application Support/soc/models. Error visible ante red/espacio/verificación. Un proceso hijo contiene micrófono y modelo; se cierra tras inactividad, Liberar Nemotron o salida.

macOS requiere permiso de micrófono y Accesibilidad para el atajo. Para generar un paquete local con descripción de micrófono: `cargo build --locked -p soc` y `python3 scripts/package_macos.py`. El script crea target/debug/soc.app sin instalarlo. Conserva SOC_DATABASE_PATH absoluta al lanzarlo; no abrir una segunda instancia sobre la DB real para evaluación. Si falta Accesibilidad, abrir Voz y órdenes o pulsar Activar atajo solicita el permiso nativo y abre el panel correspondiente. El usuario autoriza soc; la app detecta la concesión y activa el atajo sin reiniciar. Arrancar la presencia no muestra solicitudes de Accesibilidad.

Estado y límites de la verificación de dictado: [pruebas y runtime](docs/verification/transcription.md). ASR y descarga probados con audio sintético; atajo físico y micrófono del usuario pendientes de observación.

Cada dictado finaliza automáticamente a los 5 minutos; libera el micrófono y conserva el globo durante su tiempo configurado.

## Control por voz
En Voz y órdenes → Órdenes · Laya puedes guardar su precarga al iniciar, liberación tras 1–120 minutos (10 por defecto) o mantenerlo cargado. También puedes cargarlo o liberarlo manualmente. El plazo cuenta desde el final de la última petición; liberar Nemotron no libera Laya. [Ajustes independientes](docs/decisions/laya-settings.md).
Laya se precarga por defecto al abrir soc; muestra carga/ready por separado de Nemotron. Abrir aplicaciones usa el inventario del equipo y sus nombres/alias, con destino y efecto comprobados. [Diseño vigente y pruebas](docs/decisions/installed-apps.md). La [auditoría amplia](docs/verification/voice-experience.md) documenta correcciones de experiencia y errores semánticos aún pendientes: el modelo todavía puede abrir una app ante una petición distinta. Para activar una compilación nueva hay que cerrar y reabrir el ejecutable actualizado; no hay hot reload.
El atajo de transcripción controla el ordenador con cada frase final. En Voz y órdenes → Órdenes · Laya, desmarcar «Controlar el ordenador por voz (esta sesión)» conserva solo el dictado; comienza marcado al abrir soc. Reutiliza Laya multilingüe local mediante runtime.json configurado con scripts/configure_laya.py. No ejecuta parciales, interpreta la intención con Laya, resuelve la disponibilidad por separado y comprueba el resultado antes de comunicar éxito. Las decisiones inciertas piden aclaración; las operaciones fuera del catálogo terminan en god_call con integración pendiente. No controla lenguaje libre sin límites: [diseño y configuración vigentes](docs/decisions/semantic-control.md), [evidencia](docs/verification/semantic-control.md). `--decide` prueba decisiones sin efectos; `--command` ejecuta una orden explícita. Compilar/reescribir el paquete es distinto de actualizar la instancia en ejecución.

La interpretación semántica, contexto y entrenamiento están en [D-017](docs/decisions/semantic-control.md). Las adaptaciones entrenadas siguen experimentales y no se activaron: [resultados y errores](docs/verification/semantic-control.md). La revisión de voz para después se inicia con `cargo run --locked -p soc -- --voice-review`; usa Ctrl+Option+Shift+R, solo predice y se cierra a los tres minutos. No sustituye la validación del micrófono real por voz sintética.
