# D-016 — Control local por voz con Laya

> Contexto histórico: la UI descrita aquí fue retirada al migrar a Flutter. La cara, presencia flotante, globos y sus sondas no forman parte del producto vigente. Para arquitectura y verificación actuales, consultar los documentos de arquitectura y reglas del índice. Las migraciones y los datos conservan su historia.

Historial de la primera integración. El filtro de frases y la configuración descritos aquí se reemplazan por [D-017](semantic-control.md), fuente vigente para la interpretación semántica. La conexión con el dictado normal y el puente `god_call` fueron retirados de Presence; el flujo vigente se describe en [D-021](llm-conversation.md).

Petición del usuario, 2026-10-12: usar Laya multilingüe y el flujo del diagrama adjunto. El README del benchmark es referencia histórica; sus reglas no reemplazan el flujo. El usuario confirmó usar el atajo actual directamente para órdenes.

## Flujo y propiedad

`apps/decisions` posee catálogo, autorización, inferencia local, ejecutores y diario SQLite. Presencia consume su Controller público. Bootstrap registra/inicia/para decisions; kernel conserva su independencia. No listeners, HTTP, IA remota ni ejecución de código generado por el modelo. El helper Python está embebido en el binario Rust, sin depender del cwd del paquete.

Nemotron conserva su captura y sus parciales. Solo `Text(finalized=true)` entrega una frase al router: soltar/terminar el dictado delimita la frase. Correcciones dentro de esa frase se resuelven antes de actuar. Por petición posterior del usuario (2026-10-12), la casilla «Controlar el ordenador por voz (esta sesión)» empieza desmarcada: el atajo solo dicta, sin enviar frases al router. La implementación, configuración, precarga e indicadores de Laya permanecen activos; la casilla permite una reactivación explícita en la sesión. No es una preferencia persistida. Desmarcar o Detener y liberar cancela las decisiones pendientes; una operación ya enviada al OS puede haber ocurrido y no se deshace.

| Ruta | Comportamiento |
|---|---|
| incomplete | Esperar más voz con el mismo atajo; conservar fragmento hasta 30 s |
| ambiguous | Pedir aclaración; conservar contexto hasta 30 s |
| no_order | Terminar sin ejecutar |
| god_call | Comunicar integración pendiente, sin llamar otro modelo |
| complete | Ejecutar una vez, comprobar efecto, guardar comprobación y comunicar después de persistir |

La espera no mantiene abierto el micrófono. Referencias «otra vez» requieren última acción comprobada de esta sesión. No se usan predicciones/fallos como contexto exitoso; no se reconstruye ese contexto con registros antiguos. Las continuaciones se acumulan de forma acotada por el presupuesto de tokens. No ejecuta parciales ni descarta silenciosamente una orden enviada mientras otra está en curso: el globo informa que debe esperar.

## Laya y autorización

Se reutiliza Laya 0.3.5 y el checkpoint multilingual de la comparación, revisión `7b928d828b7b0e022f929d9bd2e44165aa270148`. Carga directa local con MPS, sin Router que descargue otros checkpoints. Python 3.12.13, torch 2.14.0, transformers 5.17.0 y safetensors 0.8.0 pertenecen al entorno externo ya instalado; no se instalaron/actualizaron paquetes allí. Se conserva la ruta del intérprete del venv sin resolver su enlace simbólico: resolverlo pierde pyvenv.cfg y sus paquetes. Rutas en `~/Library/Application Support/soc/decisions/runtime.json`, propiedad de decisions, fuera de Config/AppState. No contiene secretos. `python` y `model`: archivos/directorios existentes con rutas absolutas, obligatorios al solicitar inferencia; se releen al iniciar el hijo. No se sobrescribe `.env`.

La primera clasificación libre de cinco rutas falló: 26/120 correctas, 5 falsas selecciones ejecutables; otra versión 50/120, 6 falsas. Una segunda pregunta de validación del mismo modelo tampoco bastó. Esas inferencias no ejecutaron acciones. Por ese hallazgo se incorporó una autorización independiente y deliberadamente acotada en `python/policy.py`: órdenes completas en español del catálogo, muletillas, negaciones explícitas, corrección clara y referencias comprobadas. Nunca se interpreta una coincidencia parcial dentro de una frase más larga como autorización.

El filtro construye las opciones ejecutables compatibles con la orden y la disponibilidad del sistema. Laya elige entre esas opciones y `god_call`, conservando claves A..L y orden estable. Una respuesta fuera de opciones es error. Las formas desconocidas pueden clasificarse como incompleta/ambigua/no_order/god_call; el modelo por sí solo no les concede ejecución. Un desacuerdo con una orden local reconocida pide aclaración. No se considera la probabilidad una garantía ni se reutilizan los umbrales antiguos. Esta autorización limita la cobertura del lenguaje libre: no es un clasificador multilingüe de propósito general, aunque el checkpoint sea multilingüe.

Texto máximo 1.200 caracteres por entrada; se rechaza también exceder el presupuesto real de tokens, para no truncar una negación/corrección final. El estado/opciones conservan orden reproducible. No registrar voz/texto real en logs/DB; los textos de evaluación son frases del corpus escrito proporcionado.

## Ejecutores y comprobación

Aplicaciones: NSWorkspace obtiene instalación, navegador predeterminado y procesos. `open -b` recibe únicamente un bundle conocido/obtenido del OS; luego se comprueba proceso y aplicación en primer plano. No basta el código de salida del comando. Chrome/Launchpad ausentes no se sustituyen por Safari. El menú se ofrece únicamente si existe Launchpad; no se inventa equivalencia con el lanzador nuevo de macOS.

Música: Apple Music o Spotify ya abiertos, estado legible y destino inequívoco. Un único reproductor sonando determina destino; si hay dos sonando o no puede determinarse, se pide aclaración. Apple Events puede requerir autorización nativa del usuario; el paquete incluye la descripción. Pausa/reanudar se comprueban leyendo player state; siguiente/anterior exige leer identidad de pista y observar cambio. El cambio demuestra transición de pista después del comando, no verifica el orden de una playlist. Sin efecto observado se informa fallo; no se repite la operación.

El modelo nunca suministra comandos shell/AppleScript ni argumentos libres. Los scripts musicales y la lista de bundles son constantes del adaptador. Disponibilidad y destino se comprueban otra vez inmediatamente antes del efecto. Las herramientas OS tienen timeout 8 s. Tras una apertura se comprueba running/frontmost con hasta 12 lecturas, espera 150 ms, máximo 1 s por lectura y plazo total 4 s; errores transitorios de observación no abortan ese plazo. Incluso si open devuelve error o timeout se comprueba el efecto, sin reenviar el comando. Sin confirmación se registra uncertain y se conserva Laya cargado; un fallo al crear open se registra failed. La comprobación musical conserva 12 muestras, espera 150 ms y máximo 8 s por lectura. IPC tiene líneas de 16 KiB, respuestas acotadas y espera máxima 120 s por petición; proceso Python/grupo propio terminado y lector unido al cancelar/error/cierre. La inactividad libera Laya según sus ajustes propios (600 s por defecto o sin plazo); ver [D-019](laya-settings.md). Un solo worker serializa órdenes; no usa el event loop para inferencia/SQL.

## Persistencia y límites

`decision_attempts` es un diario de intentos, con action/target/status/evidence y fechas ISO 8601 UTC. Commit de pending antes del efecto; solo verified significa resultado real. Failed/uncertain no son resultados exitosos. Si el proceso cae, pending se marca uncertain al siguiente inicio, sin reproducirlo. El módulo supone una instancia de control por DB; no usar el CLI ejecutor y la UI simultáneamente sobre esa DB. No hay garantía exactly-once entre OS y SQLite: un fallo de IPC/DB puede dejar efecto ocurrido con resultado incierto. Se informa y jamás se reenvía automáticamente.

El diario no guarda texto/voz, pero sí bundles y, para música, identificadores de pistas como evidencia. No hay limpieza automática ni sincronización. Los ajustes de runtime no se migran a kernel. Historial existente de otras apps conserva sus bytes/checksums.

## Uso y reproducción

Configuración explícita del runtime instalado:

```sh
python3 scripts/configure_laya.py --python /ruta/absoluta/.venv/bin/python --model /ruta/absoluta/multilingual
```

Prueba de decisión sin efectos ni escritura en la DB personal: `cargo run --locked -p gennie -- --decide 'abre Safari'`. Una orden explícita con efecto usa `--command` y GENNIE_DATABASE_PATH; no usarlo simultáneamente con el control activo. `cargo run --locked -p gennie` inicia la presencia y usa el atajo configurado. El binario/paquete en ejecución debe actualizarse por separado; no se reinician instancias ajenas.

Evidencia y límites: [verificación](../verification/decisions.md).
