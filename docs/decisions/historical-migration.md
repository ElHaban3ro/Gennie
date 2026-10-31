# D-040 — Migración ensayada sobre una copia histórica real

29 de octubre de 2026. Verificación local completada para la base configurada del checkout, sin migrar el origen. Continúa Chats 1 y la etapa 2 del [plan](../plans/gennie-chats-agentes.md). No cambia código de producto, migraciones, UI, FRB ni configuración del usuario.

## Origen y aislamiento

`python3 scripts/probes/historical_migration.py` resuelve únicamente GENNIE_DATABASE_PATH del entorno o `.env` explícito, sin ejecutar expansiones ni mostrar otras variables. Abre el origen con SQLite `mode=ro` y `query_only=ON`; SQLite Backup obtiene una instantánea coherente incluso con otro proceso usando WAL. No sustituye Backup por copiar el archivo principal abierto ni migra la conexión original. Las copias usan directorio privado temporal, archivos 0600 e instantánea previa 0400; no se suben ni se conservan al terminar.

Conjunto observado: 57 sesiones, 1.046 mensajes, un encargo y 26 trabajos. Versiones previas: presence 0011, llm 0004, plugins 0005 y events/assignments 0004. El ensayo exige presence 0011; no fuerza downgrade para otra versión. No publica títulos, mensajes, hilos del proveedor, configuración o cuerpos de resultados.

Solo la copia desactiva las selecciones de módulos. El test Rust optativo abre el coordinador real con configuración explícita y Secrets vacío: migra todos los compilados sin iniciar módulos, modelos, constructor, scheduler, navegador, audio o servicios externos. Permanece su host MCP Unix vacío, comprobado sin herramientas/clientes y cerrado junto con SQLite. No hay UI, HTTP entrante ni cuenta de ChatGPT.

## Interrupción, preservación y recuperación

Un hijo Python aplica el SQL inmutable real de presence 0012 y su recibo/checksum dentro de BEGIN IMMEDIATE, con WAL/synchronous FULL. Se interrumpe solo ese PID después de preparar datos/recibo y antes de COMMIT. Reabrir conserva exactamente el inventario anterior, versión 0011 e integridad. Es un corte de proceso en esa transacción, no un corte eléctrico físico ni cada punto de cada migración.

Después Rust migra por la composición del producto, abre, pagina, cierra y reabre. Compara todas las columnas anteriores de 28 tablas mediante representaciones tipadas/digest lógico, sin guardar sus valores en el informe. Se permiten columnas nuevas; los historiales conservan sus filas/checksum/fecha previos y admiten nuevas versiones. La selección de módulos se compara tras la desactivación deliberada del fixture. Los contadores de secuencia existentes no retroceden.

Se conservaron IDs, contenido, fechas, orden lógico, títulos, selección, referencia del proveedor, documentos/revisiones de Encargos, trabajos/eventos y preferencias. Las 57 sesiones quedaron vinculadas a Gennie. Los 26 trabajos mantienen legacy_origin=1 y delegation_origin ausente, sin procedencia moderna inventada. Bot/helper conservan modelo/esfuerzo/permisos/fecha históricos; guía deshabilitada y cero políticas de ejecución creadas. Integridad/FK correctas. Versiones finales: presence 0018, llm 0009, plugins 0008 y events 0010; las restantes historias permanecen compatibles.

La instantánea previa también se restauró en otra ruta privada y se comprobó igualdad del inventario/integridad. Se cerraron conexiones y retiraron todas las copias. Es un ensayo de recuperación: no deja respaldo permanente ni autoriza actualizar el origen.

Para actualizarlo en el futuro con autorización: obtener un respaldo SQLite coherente nuevo y conservar el binario compatible. Ante recuperación, detener de forma controlada la instancia nueva y conservar ambas bases; restaurar en otra ruta sin reutilizar WAL ajeno ni borrar datos posteriores. Reabrir el respaldo con binario compatible y reconciliar lo escrito después antes de elegir la base de uso. No retirar checksums ni forzar downgrade sobre el archivo migrado.

## Paginación y tiempos

Ejecución final `b302de00-d22c-4b3d-bc5a-8a6367404fd9`:

| Medida local | Resultado |
|---|---:|
| Migración/apertura | 56,812 ms |
| Reapertura | 3,437 ms |
| Primera página medida | 60 mensajes, 8.224 caracteres de texto, 3 sesiones |
| Duración de esa página | 4,203 ms |
| Recorrido completo | 1.046 mensajes en 18 páginas, sin duplicados/omisiones |
| Suma de lecturas | 48,501 ms |
| Máxima lectura observada | 4,203 ms |
| Mediana de cinco primeras páginas repetidas | 3,842 ms |

Límites públicos: 60 mensajes/24.000 caracteres, orden/cursor exclusivos y mensaje individual íntegro aunque exceda el presupuesto. El recorrido coincide exactamente con los IDs visibles de SQLite y conserva la longitud de primera página al reabrir. El conjunto no tenía citas/evidencias modernas que añadir al presupuesto; sus casos tienen pruebas separadas.

Son tiempos del backend debug en este equipo. Apertura/migración y snapshot calientan cachés antes de medir: no son disco frío, renderizado Flutter, proveedor o distribución de carga. Los datos reales respaldan este presupuesto para el conjunto observado; no justifican asumir tres sesiones para cualquier historial, cambiar límites o declarar completa la matriz de rendimiento.

## Evidencia y pendientes

Informe agregado/log privado: `.verification/migration-audit/b302de00-d22c-4b3d-bc5a-8a6367404fd9/`. Dos pruebas Python comprueban lectura sin escritura/creación y cierre del origen, y detección de cambios/pérdida de columnas antiguas aunque se añadan otras. Test Rust optativo fuera de suites automáticas. Cierre: 365 pruebas Rust correctas, 24 optativas omitidas; dos Python correctas; Clippy con -D warnings, formato, invariantes y diff correctos. No se recompiló macOS ni repitieron capturas sin cambios de producto/UI.

Logs adicionales: `.verification/assistant-chat-real/historical-migration-{final,python,workspace,clippy}.log`. La lectura de control posterior del origen sigue mostrando presence 0011 y ausencia de presence_assistants; no se aplicó allí la migración. El informe conserva solo datos agregados, sin textos o digests individuales de mensajes.

El primer ensayo conservado falló por una aserción del harness que suponía ausencia de host MCP. Se corrigió para comprobar host vacío sin clientes, sin modificar el producto, y el ensayo completo pasó. El fallo y su log permanecen como diagnóstico.

Quedan comprobadas migración/interrupción/reapertura y paginación de esta copia. Siguen pendientes actualización real autorizada, escala mayor, matriz física/VoiceOver, comprensión real de citas, auditoría integral/procedencia y pendientes MCP/web. La campaña comparativa sigue pendiente de autorización. Goal activo, sin commit/push, instalación o despliegue.


## Revalidación hasta Presence 0019 — D-054

El reporte inicial de D-040 llegaba a 0018. La revalidación a05a272d-1bf8-40e2-9ea7-91d94d9eee3f volvió a tomar copia coherente del origen 0011 en lectura y comprobó migración actual (Presence 0019/Browser 0001), interrupción/rollback, restauración, reapertura y datos existentes intactos. Conservó 57 sesiones y 1.046 mensajes, paginados íntegros en 22 páginas con máximo tres sesiones distintas por página. Cero módulos de efectos y copias crudas retiradas; origen e instancia del usuario preservados. [D-054](chat-session-budget.md) conserva métricas y alcance; no constituye actualización de la base original.
