# Instrucciones coherentes con el ejecutor de Encargos

30 de octubre de 2026. Seguimiento acotado del [goal de pendientes funcionales](../plans/gennie-pendientes-funcionales.md). No cambia UI, barra lateral, permisos, calendario, datos o API FRB. No inicia modelos, cuentas o ventanas.

## Corrección

Las instrucciones web incorporadas por `codex::instructions` negaban que existieran scheduler y ejecutor de Encargos. Los metadatos añadían `scheduled_execution_available=false` sin distinguir el constructor del ejecutor. La regresión sobre las instrucciones finales falló antes de corregirlos, con el diagnóstico «Contrato obsoleto en instrucciones finales».

El contrato distingue ahora el constructor bajo demanda del ejecutor local de Encargos. Este último requiere configuración/activación humanas, función disponible/autorizada y Gennie abierta. La disponibilidad técnica (`ready`) no demuestra una política activada o un efecto. `execution_enabled=false` de una llamada de registro solo indica que esa operación no activó ejecuciones; no describe las políticas existentes. El bot conserva las consultas de actividad bajo el origen y las herramientas disponibles, sin poder activar políticas.

Los metadatos se limitan al propietario correcto mediante `builder_schedules_assignments=false`, sin inventar un estado de disponibilidad del ejecutor. El constructor también recibe esa separación y el texto principal aclara el alcance de las herramientas de registro. No se añade una capacidad, una herramienta ni un permiso.

## Verificación

`composed_bot_instructions_distinguish_assignment_execution_from_building` comprueba seis composiciones: sin herramientas, Encargos y Encargos+web/constructor, cada una con permisos apagados/encendidos. Excluye las negaciones antiguas, conserva los límites de registro/activación y verifica los metadatos nuevos. Es una comprobación del contrato final, no una garantía de interpretación de todos los modelos.

Tras la regresión roja, las suites afectadas `gennie-llm` y `gennie-plugins` pasaron **158 pruebas, 17 optativas omitidas**. Clippy de ambos paquetes/all-targets, formato e invariantes correctos. No se repiten la suite Flutter, las campañas de modelos o pruebas de accesibilidad; no hay cambios Dart o de API que lo requieran.

Fuentes, logs y compilación en [abed7af5](../../.verification/assignment-instructions/abed7af5-70d7-4775-8d77-ab688d521abd/PLAN.md). El paquete local actualizado y sus verificaciones se registran en ese mismo directorio. Los artefactos anteriores se conservan con su versión; no se instala o despliega esta actualización.

## Entrega actualizada

[Descargar Gennie para macOS arm64](../../.verification/assignment-instructions/abed7af5-70d7-4775-8d77-ab688d521abd/Gennie-macos-arm64.zip), [SHA-256](../../.verification/assignment-instructions/abed7af5-70d7-4775-8d77-ab688d521abd/SHA256SUMS) y [resultado primario](../../.verification/assignment-instructions/abed7af5-70d7-4775-8d77-ab688d521abd/RESULT.json). Release `17c988b2788438680e833e686d10011c49761ec625b4cc46a22305f8f7cc32c0`, construido con `./scripts/build.sh`, firma local comprobada. La biblioteca contiene las tres referencias del contrato nuevo y ninguna de las tres negaciones/metadatos obsoletos comprobados. Las98 fuentes Dart cotejadas permanecen iguales.

El ZIP se extrajo en un temporal propio: firma correcta y cuatro hashes iguales al paquete de origen; el temporal se retiró. Sin archivos de credenciales/.env/SQLite, instalación o arranque. El paquete inicial a3502e73 se conserva como antecedente; esta entrega lo actualiza. Goal funcional completado con este alcance; no se afirma una nueva prueba semántica del proveedor o cambio de la aplicación ya abierta.
