# Perfil del asistente: comprobación nativa

29 de octubre de 2026. Tres recorridos completados mediante Computer Use en Flutter macOS release, FRB, Rust y SQLite reales. Datos ficticios, modelos apagados y sin base personal. La instancia Debug previa del usuario (PID 49455) se conservó. No cambió código visual, estilo general o barra lateral.

| Recorrido | Resultado guardado | Run en .verification/native-ui |
|---|---|---|
| Editar nombre, elegir PNG en OpenPanel y Guardar | Prueba local, revisión de avatar 1, PNG preparado de 27083 bytes | efa29870-c114-4257-b4e8-4bfdb4155a35 |
| Editar nombre, cancelar OpenPanel y cancelar el perfil | Gennie, revisión 0, sin imagen; perfil idéntico al inicial | 52856a5d-e397-4fd5-9333-ae1be4a9cb28 |
| Usar pingüino y Guardar sobre perfil personalizado | Prueba local conservado, revisión 2, imagen eliminada | fee7a828-ca43-4db4-814f-ecb61ef1c503 |

Se observó la previsualización y el resultado en fila/cabecera. Los tres terminaron con Salir por UI y salida 0, SQLite íntegro, sin FK inválidas ni avisos conocidos de motor/canal. Se conservaron dos mensajes, revisiones 1/1 de encargos y cero trabajos/ejecuciones. Directorios, paquetes y preferencias exclusivas del ensayo se retiraron. Los manifiestos originales y logs se conservan; native-profile-finish.json agrega comprobación posterior sin modificar esos registros.

native_ui.py clona el paquete release en un directorio temporal y asigna un CFBundleIdentifier UUID exclusivo antes de firmarlo ad hoc. Solo aísla metadatos/firma: evita confundir activación por bundle con la instancia del usuario. El SHA-256 del ejecutable de origen era a89943a3d6fc3ca12528e09bec02dea1498ccb9bc6f7b240c97076f02a3d1208; cambiar la firma cambia el hash de la copia. La selección se hace por el bundle exclusivo anunciado por el harness y tras reconocer contenido ficticio. Ventana → Gennie activa correctamente la ventana propia. El ensayo usa PNG del icono incluido en el repositorio, sin imagen personal.

Comando optativo: python3 scripts/probes/native_ui.py --require-clean-quit --profile-scenario save, cancel o restore. Anunciar antes el propósito; límite de 180 s para estos recorridos y 60 s para el lector habitual, sin puertos. Las acciones son por Computer Use. Confirmar Salir antes de liberar stdin; el fallback por señal no pasa el gate de cierre. El harness registra el veredicto del perfil antes de escribir el manifiesto, incluso ante discrepancia.

## Límites y observaciones

Los recorridos del perfil se observaron a 1100 × 720. El intento de arrastrar al mínimo no cambió el tamaño: no acredita el perfil nativo a 640 × 420. Las capturas/árbol AX están en la traza de Computer Use; no se generaron PNG exportados. No prueba VoiceOver, todo el teclado físico, todas las escalas/contrastes o una matriz completa de macOS. Las capturas software y pruebas del perfil conservan su evidencia separada.

La herramienta no aplicó setValue/selectText/selección por atajo al campo Flutter; la edición se completó con borrado por teclas y texto, verificando el valor visible. Es una limitación observada de la vía de automatización, sin diagnóstico de fallo del campo para uso físico. Los intentos iniciales 708d5291-fac4-4122-b0a4-2a63ec5a55de y 6c7daeb4-dc2e-44e3-8510-035377d13d9e alcanzaron el plazo sin Salir y se conservan como fallidos; no se cuentan entre los tres correctos.

D-051 amplió posteriormente el teclado: Tab→Cancelar→Espacio en 3bd5e1a9-b00e-41e0-b008-cf61eba38c58; Escape antes y después del selector nativo en 406874b1-67c6-433c-a617-f7753fe3d958, con nombre sin guardar conservado y cierre limpio. El mínimo nativo no se alcanzó con los arrastres. Ver [decisión y pruebas](../decisions/profile-keyboard.md).

D-056 amplió la observación a una captura nativa de 961 × 498 mediante Window → Move & Resize → Top Left. En 092a295a-87d9-4099-a0da-a94c5052abc8 se abrió el perfil con sus acciones visibles y se canceló con Escape; no se repitió el selector/guardado en ese tamaño. Ese recorrido también comprobó citas y teclado. No acredita el mínimo exacto 640 × 420 o la matriz completa del perfil. La revisión de Encargos en el tamaño compacto detectó y corrigió Escape en AppReviewDialog; evidencia anterior/posterior, hash de datos conservados y límites en [D-056](../decisions/review-keyboard.md).

## Recorrido sobre el paquete vigente tras la campaña de proveedor

El run d4774212-c131-4b61-8bdf-89a6b0f2a597 abrió una copia del release cuyo ejecutable de origen tiene SHA-256 69896d7046707a0d52be4e33668cf4b896cc50cfcd5a2211a9a99995fa763f3d. Se observó el lector a 1100 × 720 y, mediante Window → Move & Resize → Top Left, una captura de 961 × 498. La cita llevó al original con contorno de foco visible. La tecla Menu posterior no mostró menú en la observación; no se cuenta como comprobación adicional del atajo. Pulsar el contenedor de cabecera tampoco abrió el perfil, por lo que no añade evidencia del diálogo.

Un arrastre horizontal desde el borde derecho no cambió la ventana. El menú nativo ofrece mitades/cuartos, sin control de dimensiones exactas; este recorrido sigue sin acreditar 640 × 420. No repetir el mismo arrastre como si aportase evidencia nueva. Las capturas permanecen en la traza de Computer Use y no se exportaron archivos de imagen.

Salir mediante el menú propio terminó con código 0, SQLite íntegro, sin errores de canal/motor. Las dos sesiones, cita y conversación activa coincidieron antes/después; dos mensajes, revisiones de encargos 1/1 y cero trabajos/ejecuciones. Fixture, bundle y preferencias propios retirados. Modelos apagados, sin puertos ni datos personales; instancia Debug PID 49455 preservada. En ese recorrido no se alcanzó el mínimo exacto.

Posteriormente una copia QA compilada con tamaño inicial 640 × 420 verificó el perfil y Escape en claro y oscuro, con acciones visibles y cierre limpio. La variante oscura selecciona tanto darkAqua de AppKit como el darkTheme Flutter existente solo en la copia de prueba. No acredita cambios del tema del sistema, arrastre físico o VoiceOver. Fuentes y producto habituales intactos; [método, capturas y límites](native-minimum.md).
