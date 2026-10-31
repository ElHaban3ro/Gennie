# Creación, edición y aplicación nativas de Encargos

29 de octubre de 2026. Preparación `.verification/native-assignment-edit/69224db6-e279-4ff8-ac00-b6c4b89f9d87`. Dos copias del release habitual a 1100 × 720, bundles UUID y SQLite ficticias independientes, solo Presence/Assignments activos. Sin activar conversación, guía, Plugins, Runner, cuentas o preferencias del sistema. No hay cambios de código runtime, estilos o barra lateral.

## Crear y editar

PID 6121. Nuevo encargo abrió el formulario, con título/objetivo y horario de entrega. Se guardó «Encargo QA nuevo», objetivo «Leer solo datos ficticios.», recurrencia diaria 08:00 UTC y opciones adicionales sin activar. La ficha confirmó guardado y mostró esos valores; `CREATED_READONLY.json` exige revisión 1, documento completo esperado, cuatro encargos anteriores iguales y cero políticas/intentos/hilos/mensajes.

Editar cambió únicamente el objetivo a «Leer solo datos ficticios y agrupar novedades.». Guardar volvió a la ficha; reabrir mostró objetivo y horario persistidos, y Cancelar no añadió una revisión. `EDITED_READONLY.json` pasó el guard completo: quinto encargo en revisión 2, documento exacto, cuatro originales y sus propuestas intactos, perfil/historial/vínculos/revisiones iguales y cero políticas/versiones/intentos/hilos/mensajes.

El control de campos expone un límite de la automatización: Tab no completó el objetivo previsto y super+a no seleccionó el valor de zona. setValue cambió el valor mostrado por AX sin actualizar inmediatamente el texto rasterizado. Se corrigió el borrador con coordenadas del screenshot, End/BackSpace y texto literal; la ficha y las lecturas ro acreditan los valores guardados. No se presenta esa secuencia como prueba de modificadores físicos o nombres anunciados por VoiceOver.

Salir por Quit terminó el producto con 0. **El script salió con 1 después**, al serializar assignment_revisions como sqlite3.Row: timeline_record había cambiado row_factory sin restaurarlo. El JSON parcial, log y drivers originales se conservan. Ese archivo ya contenía completos timeline_before/timeline_after, salida 0, integridad y FK. `manifest-recovered.json` recupera exactamente ese prefijo anterior a assignment_revisions, guarda hash del original y pasa de nuevo require_final; coincide con la comprobación ro independiente anterior a Salir. No se rellena el resto perdido: el recuento final de trabajos del primer script no quedó serializado. Bundle/base/preferencias propios ausentes se comprobaron por sus rutas exactas. No se repite el recorrido por un fallo de escritura del informe con estado completo recuperable.

## Aplicar un borrador ficticio

El fixture restaura ahora row_factory antes de devolver su registro; el preflight serializa también los metadatos posteriores. PID 6663, otra base con cuatro encargos/revisiones/borradores precargados. La propuesta 1 mostró un único cambio: Actual «Revisar novedades» → Propuesto «Agrupar solo los cambios nuevos», motivo y beneficio; no modificó horario, método o permisos. Escape cerró y `CANCELLED_READONLY.json` confirma estado completo idéntico, las cuatro propuestas pendientes.

Reabrir y Aplicar cambios guardó el objetivo nuevo y la ficha mostró «Cambios aplicados». Otra inspección conservó las diferencias originales como consulta, con solo Cerrar y sin segunda acción Aplicar/Descartar. No se invocó una aplicación duplicada. `APPLIED_READONLY.json` y el cierre exigen solo descripción/revisión 2/updated_at del encargo 1 y su recibo applied/applied_revision=2/resolved_at; identidad, clave y creación originales se conservan, al igual que los otros tres encargos/propuestas y todas las revisiones/historial/perfil. Cero políticas/versiones/intentos/hilos/mensajes. No equivale a aceptación humana de calidad: se aplica un borrador de prueba para verificar la ruta técnica.

Producto y script terminaron con 0 por Quit, integridad/FK/canal limpios, cero trabajos/intentos, recursos/preferencias propios retirados y Debug 49455 conservado. `run-apply-proposal/manifest.json` conserva los datos completos. Fuentes/driver antes y después archivados; el release mantiene SHA-256 074eec394111fac8b38d4abd330acc21d7db4efb3cecc0479e92a257c73b0660. Sello de fuentes/binarios/rustc de la campaña preparada coincidente. No se repiten suites/build al no cambiar el producto.

## Alcance pendiente

Capturas/AX en Computer Use. Este recorrido cubre creación/edición básica recurrente y una propuesta ficticia simple a ventana normal; no toda la combinación de calendarios, campos opcionales, mínimo/texto ampliado, errores de guardado, VoiceOver, teclado físico o contraste nativo cuantitativo. La generación/comparación de borradores con modelos y su revisión humana siguen pendientes de los planes aplicables. Goal activo; sin nueva inferencia, commit/push, instalación o despliegue ni cambios a Encargos personales.


## Seguimiento al mínimo con texto ampliado

El [editor básico oscuro al mínimo/150 %](assignment-editor-accessibility.md) detectó y corrigió recorte del pie y ausencia de foco. Tiene regresión roja/verde, 402 Flutter/49 optativas, build/firma y cancelación/cierre nativos con guard exacto; la revisión de propuesta oscura también tiene consulta/Escape/reapertura/Cerrar, sin aplicar. Se conservan el fallo del guard inicial, la etiqueta larga aún elidida y el límite del fixture de propuesta. No reemplaza los guardados normales históricos ni acredita todos los calendarios/opcionales.
