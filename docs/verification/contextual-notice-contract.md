# Contrato del transporte de avisos contextuales

30 de octubre de 2026. Corrección interna de LLM; sin cambios de UI, barra lateral, API FRB, permisos, dependencias o migraciones. No se inició ningún modelo, cuenta, red o instancia gráfica.

## Fallo reproducido

La ruta contextual pedía al bot `action`, `reason` y `message`, pero `Session::render_scoped_message` imponía un objeto cerrado que solo admitía `message`. Un proveedor que respetara ese esquema no podía satisfacer `Decision::parse`. El host conservaba el resultado con `render_failed` y sin aviso. Las pruebas anteriores inyectaban directamente una decisión válida, por lo que no detectaban esta incompatibilidad del transporte.

La regresión extrae el envío del turno sin alterar su comportamiento y conecta el mismo constructor/envío/lector de producción a un proveedor Python stdio ficticio que respeta el esquema recibido. Recorre guía simulada → redacción → parser → SQLite/diario de prueba para las cuatro acciones. Antes del arreglo, las cuatro acabaron en `record` con motivo de fallo y cero mensajes; se reprodujo dos veces, 0,14 s la primera. El control del parser ya aceptaba decisiones válidas. No hizo falta instrumentación de depuración o consultar al proveedor real.

## Corrección y comprobación

El consumidor selecciona un `ScopedOutput` tipado, que el worker transmite sin deducirlo de palabras en las instrucciones. Delegaciones y hechos operativos seleccionan `Message`; la valoración contextual selecciona `ContextualDecision`. Este último publica el esquema cerrado de su propietario: acción entre `record/notify/ask/offer_help`, motivo de texto y mensaje de texto o null, con los tres campos requeridos. El parser mantiene sus límites y las relaciones entre acción y mensaje.

La regresión verde confirma las cuatro decisiones y su motivo, cero mensajes para `record`, uno para las otras acciones y una sola consulta/redacción aunque se vuelva a atender el inbox. Otro caso recorre un aviso ordinario mediante el mismo transporte y conserva su entrega única. Los procesos stdio propios se recogen y se comprueba que su handle queda vacío. Las reservas, revocación, recuperación y ausencia de reintentos mantienen sus pruebas existentes; no se relajan capacidades del renderer.

Fuentes, logs rojos/verdes, controles y resultados en [el ensayo d96e16b2](../../.verification/contextual-notice-contract/d96e16b2-ee6c-4612-85bb-516a0372d9b9/PLAN.md). Suite Rust completa: **439 correctas, 28 optativas omitidas**; formato, invariantes y Clippy workspace/all-targets correctos. `./scripts/build.sh` y firma local correctos; paquete sin abrir, instalar o desplegar. No se repitió Flutter por un cambio Rust sin API o UI nuevas; sus 438 pruebas/49 optativas conservan su evidencia anterior.

Esta prueba acredita el contrato local y su recorrido stdio simulado. Por sí sola no acredita autenticación, aceptación del esquema por una versión instalada de Codex ni calidad de decisiones con guía/bot reales. El [ensayo posterior real](contextual-notice-probe.md) añade aceptación del contrato y cuatro acciones entregadas/registradas con proveedores reales, sin sustituirlo por otro ensayo de borradores ni extrapolar utilidad universal. La [auditoría de métricas](assignment-metric-coverage.md) conserva separados esos alcances.
