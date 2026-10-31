# Gennie: pendiente funcional detectado tras la entrega

Estado: completado y paquete local actualizado, 30 de octubre de 2026. Alcance limitado a los requisitos originales de Encargos y su uso desde el bot; no reabrir funciones que ya tienen pruebas suficientes.

## Hallazgo comprobado

Antes del arreglo, `apps/llm/src/web-plugin-instructions.md` afirmaba tanto «No hay scheduler ni ejecución periódica de Encargos» como «No existe ejecutor de Encargos». `codex.rs` incorpora ese archivo a las instrucciones conversacionales. Sin embargo, `apps/assignments/src/execution.rs` implementa el Runner y bootstrap lo compone con Plugins y las políticas explícitas. Las instrucciones principales ya explican la configuración y activación humanas separadas. El texto final resultaba contradictorio.

El cierre anterior no detectó esta contradicción. El hallazgo demuestra un desajuste de instrucciones; no se afirma haber observado una negativa del modelo en una conversación real.

La revisión del mismo flujo encontró también `web_plugins.scheduled_execution_available=false` en los metadatos del bot y una frase ambigua en las instrucciones del constructor. El booleano se sustituye por `builder_schedules_assignments=false`, limitado a su propietario, y el texto del constructor distingue su preparación del ejecutor de Encargos. Se aclara además que `execution_enabled=false` de una llamada de registro no informa del estado de políticas anteriores. No cambian el calendario, los permisos, SQL o API FRB.

## Trabajo del goal

1. Corregir las afirmaciones obsoletas en las instrucciones del bot. Distinguir el constructor bajo demanda del ejecutor periódico de Encargos y explicar que guardar un encargo no configura, activa ni ejecuta una política. Conservar activación humana, permisos y lectura de resultados; no añadir activación desde el modelo.
2. Verificar la composición final de las instrucciones y los consumidores afectados con comprobaciones dirigidas, sin repetir campañas de modelos. Revisar únicamente contradicciones directamente relacionadas con ese flujo, no iniciar otra auditoría general.
3. Compilar y actualizar el paquete local porque las instrucciones están embebidas en Rust. Comprobar firma y contenido y actualizar el estado de entrega; no instalar, migrar datos personales, hacer commit/push o desplegar.

## Criterio de cierre

El bot recibe un contrato coherente con las funciones reales: Encargos puede ejecutar políticas explícitas mientras Gennie está abierta; el constructor no es el scheduler; el registro o una función disponible no prueban activación o efectos. Pruebas focalizadas y paquete actualizado comprobados, sin ampliar capacidades o permisos.

## Exclusiones expresas

Sin trabajo de accesibilidad, VoiceOver, contraste, rediseño, cambios de barra lateral, bots adicionales, daemon, comparaciones nuevas de formatos o campañas extensas. Preservar cambios existentes y datos. Si no aparecen otros fallos concretos de este flujo, no inventar tareas adicionales.

## Resultado verificado

Regresión roja antes del arreglo y seis composiciones finales correctas. Suites LLM/Plugins:158 pruebas correctas/17 optativas omitidas, Clippy, formato e invariantes0. Build/firma y ZIP extraído correctos; se verificó que la biblioteca contiene el contrato nuevo y no las negaciones obsoletas. Sin modelos, UI, instalación o cambios personales. [Informe y paquete actualizado](../verification/assignment-instructions.md). No quedan pasos pendientes de este goal.
