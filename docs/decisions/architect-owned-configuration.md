# D-058 — Configuración propia y visible de cada arquitecto

2026-10-31. El usuario aclara que Personalidad e Instrucciones son datos del arquitecto, no complementos de un rol oculto. La ficha debe mostrar la configuración real que consume su proveedor.

## Fuente única

La migración 0011 materializa en `llm_architect_prompts` la personalidad y las instrucciones completas de cada instancia existente. Conserva las instrucciones previas de Gennie, navegación y constructor; separa el párrafo de tono del bot y la identidad del constructor como personalidad. Incorpora el criterio anterior de la guía y conserva los textos personalizados existentes. Las bases y sus referencias, modelos, esfuerzos, permisos, habilitación y revisiones se mantienen. Los campos antiguos quedan como historia de migración; no participan en las lecturas o guardados actuales.

Cada perfil devuelve los textos propios desde esa tabla. Guardar sustituye esos valores completos en la misma transacción que revisión/metadatos y el espejo histórico de Gennie. Crear y duplicar guardan sus propios textos; eliminar una instancia adicional retira también su configuración en la misma transacción. La migración no vuelve a inicializar textos ya editados o vaciados. No se modifican migraciones 0001..0010.

El bot envía `Settings::agent_instructions` y las capacidades/permisos vigentes; el constructor envía sus propios ajustes por el mismo formateador. Ninguno añade los archivos base de instrucciones en cada conexión. La guía recibe sus propios textos y el criterio de la valoración solicitada. Las sesiones auxiliares del bot mantienen el perfil y añaden el contrato específico de su operación, por ejemplo el esquema de una decisión o el límite de un aviso. Esos contratos y los permisos del host siguen siendo límites técnicos, no una personalidad alternativa.

Los archivos Markdown anteriores conservan el texto de referencia con el que se preparó el snapshot de 0011; cambiar esos archivos ya no sustituye silenciosamente la configuración del usuario. Cualquier evolución futura de valores iniciales debe preservar las ediciones existentes.

El 10 de noviembre, la migración 0012 añade al perfil visible de Gennie una indicación para consultar encargos y catálogos pertinentes antes de preguntar por una fuente desconocida. Conserva el texto anterior y los demás perfiles; si el texto resultante excedería 64.000 caracteres, lo deja intacto para no impedir el arranque. Después de la migración el usuario puede editar o retirar la indicación desde la ficha. La consulta no acredita acceso ni autorización. En una prueba aislada con el modelo configurado y herramientas ficticias, la respuesta anterior preguntó el servicio sin consultar herramientas; con la indicación consultó encargos y plugins e identificó Gmail. Es una muestra, no una garantía para todas las respuestas.

## Interfaz y validación

La ficha muestra Personalidad e Instrucciones, sin «adicionales» ni una capa de personalización. Conserva el formulario y estilo existentes. Admite 4.000 caracteres Unicode de personalidad y 64.000 de instrucciones, con validación independiente en UI, Rust y SQLite. El proveedor sigue validando modelo/esfuerzo y aplicando los permisos reales. No se inicia inferencia al consultar o guardar.

Pruebas dirigidas: dos regresiones rojas sobre perfil vacío e instrucciones ocultas; migración con edición anterior, conservación de modelos/permisos/revisiones, texto largo íntegro, guardado transaccional, borrado, vaciado explícito y no reinicialización. El compositor del bot y el de la guía comprueban que los cambios sustituyen el texto anterior; widgets verifican carga de datos, edición y sustitución de instrucciones largas. Sin campañas de modelos, cambios de accesibilidad, paquetes, instalación o reinicio de una instancia personal.

Resultado: 214 pruebas Rust correctas/22 optativas omitidas en LLM, Plugins, Proactivity y bootstrap; las doce regresiones de arquitectos se comprobaron después del último ajuste de vaciado/borrado. Doce widgets correctos, análisis Flutter, Clippy de los cuatro paquetes/todos los targets, formato e invariantes correctos. `scripts/dev.sh --build-only` compiló biblioteca/helper de desarrollo, sin abrir UI. Capturas software del formulario con datos ficticios, no evidencia nativa.

La lectura final de la base activa encontró ya aplicada 0011 y comprobó su checksum contra la fuente. Los perfiles base conservaban modelos/esfuerzos/revisiones y tenían sus textos completos en la tabla nueva. Esta tarea no abrió ni reinició la instancia personal; la observación no atribuye a sus pruebas la apertura externa que aplicó la migración. Logs y metadatos sin cuerpos personales en `.verification/architect-owned-config/20261031/`.
