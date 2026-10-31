# Editor básico: acciones y foco al mínimo con texto ampliado

30 de octubre de 2026. Fuente habitual previa 4bbc2c92; copia QA oscura, 640 × 420 y texto Flutter al 150 %. Solo main.dart de esa copia cambia tamaño inicial, ThemeMode y TextScaler; no preferencias del sistema, estilos, barra o lógica. Runner/SQLite reales, datos ficticios, módulos de modelos apagados y cero inferencias/guía.

## Fallos observados y regresión

La primera ventana mostró Cancelar y Guardar fuera del espacio disponible en el pie, tanto sin error como al rechazar campos vacíos. El primer campo se desplazó, sin caret/foco de edición. La regresión monta GennieApp completo con barra/sidebar a ancho real, no el editor aislado: **overflow de 219 px** en claro y oscuro al mínimo/150 %, y foco false en una prueba amplia. [Rojo](../../.verification/native-assignment-scaled/650bf8f9-69da-497b-bb48-59e7d52d18f1/red.log).

AssignmentEditor conserva botones y estilo y permite que pasen a líneas con Wrap alineado al final, separación horizontal 12 y vertical 8. Cada campo tiene FocusNode estable; revealField solicita foco tras incorporar el grupo/layout y los nodos se liberan al salir. No cambian barra, fuentes, materiales, API, SQL o comportamiento de guardado. [Parche](../../.verification/native-assignment-scaled/650bf8f9-69da-497b-bb48-59e7d52d18f1/assignments.patch).

Las tres regresiones pasan: rectángulos completos de ambas acciones dentro del pie/ventana antes y después de errores, título y después objetivo reciben foco, sin guardado. **23 pruebas afectadas, 402 Flutter correctas/49 optativas omitidas; análisis, formato, build y firma local correctos**. Solo assignments.dart cambia entre 87 fuentes/recursos habituales cotejados con la puerta anterior. Backend/helper conservan sus hashes; no se repite Rust por un cambio exclusivo de Flutter. [Resultado](../../.verification/native-assignment-scaled/650bf8f9-69da-497b-bb48-59e7d52d18f1/RESULT.json), [release y hashes](../../.verification/native-assignment-scaled/650bf8f9-69da-497b-bb48-59e7d52d18f1/RELEASE.json). Release nuevo: `00dfe9e3b84a432cfac035f103a10ba9a81734f7581ee13e97c281cbacf1a119`, sin instalación/despliegue.

## Recorrido corregido y revisión oscura

Con otro fixture/UUID, Nuevo encargo mostró Cancelar y Guardar completos. Guardar vacío presentó el error y el caret de Título; una tecla simple `q` entró allí sin clicar el campo. Guardar de nuevo desplazó/enfocó el objetivo; `w` entró en ese editor sin clic adicional. typeText no había producido entrada observable y se conservó esa limitación, sin inferir un fallo de foco desde ella. Cancelar descartó ambos textos. Editar el encargo existente permitió consultar hora, zona, método/alcance/vías y preparación; no se guardó ni concedió ninguna vía. Una etiqueta larga de cadencia se elide al 150 %: **sigue pendiente su tratamiento/nombre accesible completo**, sin declarar toda la matriz resuelta.

Otra instancia propia revisó la propuesta en oscuro/150 %: motivo/beneficio y comparación se desplazaron con título y Cerrar/Descartar/Aplicar fijos y visibles. Escape cerró sin resolver; reabrir y Cerrar conservó el estado pendiente. No se pulsó Descartar o Aplicar. El fixture se construyó por SQL con campos opcionales omitidos y el inspector mostró también una diferencia de preparación; esto no acredita que hubiera solo un cambio ni calidad semántica. Falta contrastar esa normalización con el recorrido público validado.

Ambos recorridos posteriores terminaron **producto/driver 0 por Salir**, SQLite íntegra/FK vacías, todos los datos seleccionados idénticos, dos mensajes y cero trabajos/ejecuciones. Sin errores conocidos de motor ni residuales por ruta. Fixtures/preferencias UUID y proyecto/copia QA retirados; no se cambiaron preferencias del sistema o datos personales. Capturas y AX están en la traza de Computer Use. [Editor corregido](../../.verification/native-assignment-scaled/650bf8f9-69da-497b-bb48-59e7d52d18f1/editor-after/run/manifest.json), [propuesta](../../.verification/native-assignment-scaled/650bf8f9-69da-497b-bb48-59e7d52d18f1/proposal/run/manifest.json).

## Error del guard anterior conservado

El primer producto salió 0, pero el driver salió 1: había tomado data_before antes de sembrar los dos mensajes y el proveedor ficticio. Antes de acciones UI se detectó y se obtuvo una lectura independiente mode=ro/query_only del fixture ya preparado; coincide exactamente con data_after. Se conserva el driver/manifest original fallido y esa evidencia con su explicación; no se hace pasar por un guard original correcto ni se repite un recorrido para ocultarlo. Los dos drivers siguientes fijan baseline después de sembrar y pasan sin enmienda. [Prueba de la corrección](../../.verification/native-assignment-scaled/650bf8f9-69da-497b-bb48-59e7d52d18f1/editor/GUARD_CORRECTION.json).

No se infiere VoiceOver, modificadores físicos, contraste cuantitativo nativo, todas las variantes opcionales o guardado de nuevos documentos al mínimo desde esta prueba de cancelación. El guardado normal tiene evidencia anterior separada. El goal mantiene su alcance original y continúa activo.


## Seguimiento de etiqueta y fixture

La [etiqueta de intervalo](assignment-labels.md) ya tiene regresión de recorte/semántica, texto breve con ayuda condicional y observación nativa completa al mínimo oscuro/150 %. Otras instrucciones largas, incluida Pasos de preparación, siguen en revisión. El mismo seguimiento comprobó la serialización pública de propuestas y el fixture corregido: solo aparece el cambio de Objetivo, sin cambiar lógica de comparación del producto. Se conserva el ensayo anterior con su diferencia extra.
