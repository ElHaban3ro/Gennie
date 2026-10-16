# D-019 — Ajustes propios de Laya

Estado vigente desde 2026-10-16: Laya permanece en `soc-decisions` para herramientas independientes; Presence ya no muestra la pestaña ni crea su Controller. Las referencias a UI y precarga de sesión a continuación describen la integración anterior.

2026-10-12. El usuario observa liberación temprana y pide configuración independiente para Laya. No se ha medido el intervalo de su incidente concreto. El código anterior fijaba 600 s y cerraba el helper también ante error/cancelación; la UI atribuía Unloaded a inactividad sin conocer la causa.

`decisions` posee `decision_preferences`, añadida mediante 0002 sin modificar 0001 ni el diario. Settings público contiene preload=true y unload_seconds=Some(600). Admite minutos enteros entre 1 y 120; None significa no liberar por inactividad. La precarga se decide al conectar el Controller; desactivarla no impide cargar al recibir una orden. El temporizador cambia después del commit y cuenta desde la finalización de la última petición o carga, sin contar inferencia como inactividad.

La cola acotada de preferencias se procesa en el worker serial existente, fuera de main. Solo un guardado pendiente; resultado confirmado después del commit, sin aplicar valores rechazados ni borrar el borrador de UI ante fallo. Cierre drena el guardado pendiente. No añade procesos, dependencias ni Config global. El runtime externo y la configuración de Nemotron conservan su propiedad.

Menú soc/barra superior → Voz y órdenes → Órdenes · Laya. Precarga, mantener cargado y plazo tienen guardado propio. Cargar Laya y Liberar Laya son acciones explícitas de sesión; liberar cancela la petición pendiente y no la repite. El siguiente pedido explícito puede cargar de nuevo. Liberar Nemotron afecta solo al reconocimiento de voz. El estado descargado ya no afirma que se debió al temporizador; errores conservan su estado diferenciado. Los logs de liberación manual/inactividad usan motivos fijos, sin contenido del usuario.

La opción mantener cargado desactiva únicamente el vencimiento por inactividad. No evita cierres explícitos, cancelaciones de IPC ni fallos del helper. La cara dormida y el vencimiento del globo no cambian el temporizador de Laya.

Verificación breve: tests de DB temporal/reapertura, conservación del diario, límites, confirmación tras commit, cierre con guardado pendiente, precarga desactivada y reloj simulado para liberar/conservar el proceso. Sin cargar modelos, abrir otras apps ni esperar minutos reales. Compilación de la UI no demuestra interacción nativa ni reproduce el cierre observado por el usuario. La instancia existente requiere reabrir el binario actualizado.

Resultado: las 23 pruebas de soc-decisions pasan (0,91 s de ejecución, 18 s de compilación). Formato e invariantes documentales correctos. Tras corregir una propiedad de Tab no admitida, `cargo build --locked -p soc` compiló la interfaz y el binario en 1 min 18 s, dentro del límite de 2 minutos autorizado por el usuario. Emitió un aviso de código sin uso para Chat::cancel, sin errores. No se arrancó ni reinició la aplicación y no se ejecutaron modelos reales.
