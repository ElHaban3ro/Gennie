# Decisiones de descubrimiento — historial preservado

Fuente consolidada del historial; entradas posteriores precisan/reemplazan anteriores. Para implementación actual ver [base](base.md) y [D-013](monochrome-face.md).

# Decisiones de producto y técnicas

Estado: inicialización completada; archivo histórico. Fuente vigente en docs; cierre y límites al final.

## D-001 — Persistencia inicial
- Fecha: 2026-10-11.
- Tema: instrucciones de creación.
- Decisión: guardar la documentación inicial en /Users/fer/Documents/Projects/Fer/Dev/.creation.
- Motivo: el usuario indicó ese destino en respuesta a la pregunta de ubicación.
- Origen: usuario.
- Estado: aceptada para la persistencia inicial; raíz final del proyecto pendiente.
- Condición de revisión: al definir nombre, carpeta final y checkout, trasladar los documentos si corresponde conservando contenido y verificando referencias.

## Pendientes
- Nombre, propósito, alcance, datos, superficies de uso, módulos y Git.

## D-002 — Nombre de la aplicación
- Fecha: 2026-10-11.
- Tema: nombre del producto.
- Decisión: la aplicación se llama gennie.
- Motivo: nombre indicado explícitamente por el usuario.
- Origen: usuario.
- Estado: aceptada.
- Condición de revisión: cambio explícito del usuario.

## D-003 — Plataforma inicial y portabilidad
- Fecha: 2026-10-11.
- Tema: plataformas.
- Decisión: primer hito en macOS; orientación multiplataforma del producto.
- Motivo: macOS es el entorno de desarrollo del usuario, sin intención de exclusividad.
- Origen: usuario.
- Estado: aceptada.
- Condición de revisión: cambio explícito de prioridad; concretar después plataformas adicionales.
- Propuesta pendiente: aislar las integraciones específicas del sistema operativo detrás de interfaces según las capacidades realmente requeridas; no crear crates preventivos.

## D-004 — Base local y persistencia
- Fecha: 2026-10-11.
- Tema: primer alcance de construcción.
- Decisión: aplicación local con SQLite y kernel inicial; sin gestión de usuarios ni sesiones en esta fase.
- Motivo: alcance definido expresamente por el usuario.
- Origen: usuario.
- Estado: aceptada.
- Condición de revisión: nuevos requisitos explícitos de acceso, sincronización o uso compartido.

## D-005 — Prioridad gráfica
- Fecha: 2026-10-11.
- Tema: orden del primer hito.
- Decisión: comenzar con una base gráfica evaluable y feedback visual agradable, acompañada de la base técnica necesaria; posponer grandes funcionalidades.
- Motivo: permitir al usuario evaluar y construir gradualmente sobre la interfaz.
- Origen: usuario.
- Estado: aceptada.
- Condición de revisión: feedback del usuario sobre la primera base y siguientes requisitos.

## D-006 — Interfaz nativa flotante
- Fecha: 2026-10-11.
- Tema: superficie gráfica.
- Decisión: escritorio nativo sin WebView, representado por un círculo pequeño con cara que crece durante actividad y se reduce al terminar.
- Motivo: experiencia visual solicitada por el usuario.
- Origen: usuario.
- Estado: aceptada.
- Condición de revisión: evaluación de la base gráfica.
- Slint: candidato provisional; viabilidad, soporte y licencia por verificar antes de incorporarlo.

## D-007 — Estados de la presencia gráfica
- Fecha: 2026-10-11.
- Tema: comportamiento visual inicial.
- Decisión: siempre encima de otras aplicaciones; cara siempre visible, dormida con zetas en reposo pequeño y contenta en estado activo expandido; demostración por clic.
- Motivo: precisión y aceptación expresas del usuario.
- Origen: usuario.
- Estado: aceptada; precisa D-006, conservando su orientación nativa sin WebView.
- Condición de revisión: feedback de la primera base y limitaciones verificadas del sistema operativo.

## D-008 — Propiedad de la interfaz
- Fecha: 2026-10-11.
- Tema: módulo de interfaz.
- Decisión: una app dedicada posee presentación, estados y animaciones de la presencia gráfica; bootstrap compone y arranca; kernel conserva infraestructura y contratos transversales independientes de esta cara.
- Motivo: responsabilidad significativa y preferencia del usuario por una app dedicada, sin incorporar presentación de dominio al kernel.
- Origen: propuesta del asistente dentro de la decisión delegada por el usuario.
- Estado: aceptada como elección delegada; nombre del crate por concretar al construir.
- Condición de revisión: nuevos recorridos gráficos que aconsejen ampliar o separar responsabilidades.

## D-009 — Checkout definitivo
- Fecha: 2026-10-11.
- Tema: carpeta y Git.
- Decisión: inicializar Git en /Users/fer/Documents/Projects/Fer/Dev/soc y asociar origin al repositorio compartido.
- Motivo: instrucción del usuario de inicializar localmente; remoto vacío según su confirmación.
- Origen: usuario.
- Estado: aceptada; reemplaza la ubicación provisional de D-001. Documentación vigente: gennie/.creation.
- Condición de revisión: cambio explícito de ubicación, rama o remoto.
- Rama inicial: main, seleccionada por Git; no se impuso ni cambió.

## D-010 — Base técnica desde el inicio
- Fecha: 2026-10-11.
- Tema: orden de implementación.
- Decisión: construir kernel y SQLite junto con la primera base, independientemente del toolkit gráfico.
- Motivo: infraestructura reutilizable necesaria para el producto; corrección expresa del usuario a la propuesta de aplazarla.
- Origen: usuario.
- Estado: aceptada; reemplaza la propuesta de demo aislada con conexión posterior.
- Condición de revisión: cambio explícito del alcance.

## D-011 — Capacidad de animación
- Fecha: 2026-10-11.
- Tema: selección del toolkit gráfico.
- Decisión: exigir transiciones suaves y evaluar rebote/resorte para expansión y contracción.
- Motivo: calidad de movimiento solicitada.
- Origen: usuario.
- Estado: aceptada para suavidad; efecto concreto de rebote/resorte provisional.
- Condición de revisión: evaluación visual; no implica un motor físico.

## D-013 — Cara monocromática
- Fecha: 2026-10-11. Tema: identidad visual.
- Decisión: monocromática, moderna y minimalista; grafito/blanco/grises, sin mejillas ni acentos de color. Ojos y boca simples.
- Origen: corrección explícita del usuario; tonos y formas concretas elegidos por el asistente dentro del alcance delegado.
- Estado: aceptada; reemplaza el supuesto azul/rosado en D-012/propuesta de plan.
- Motivo: dirección estética solicitada. Condición de revisión: feedback de la primera base.

## Cierre de la primera base — 2026-10-11
Estado vigente: inicialización completada, con límites de validación runtime documentados. Fuentes vigentes: docs/initialization/requirements.md, docs/initialization/implementation.md y docs/decisions/README.md. Este archivo conserva origen e historial, no es una segunda fuente vigente.
- Base construida: kernel/config, kernel/db, kernel/core, apps/presence y bootstrap. SQLite compartido, migraciones atómicas con SHA-256 por módulo, registro/DAG y cierre inverso.
- Cara monocromática minimalista conforme a D-013. Clic/Espacio, zetas, spring/contracción, M persistido, A y menú Acerca, Escape/clic secundario/menú Salir.
- Verificación final: fmt, 13 tests, Clippy con -D warnings, build y check_invariants verdes. UI observada en macOS; M confirmado por lectura SQLite y reapertura sin migraciones repetidas.
- Límites: capturas secuenciales no equivalen a revisión continua del movimiento. Foco de otras apps, transparencia compositada, always-on-top y paso real de clics requieren comprobación manual OS; automatización app dirigida no demuestra esas garantías. Multi-monitor/Retina/Spaces/otros OS no probados. Click-through específico macOS, no implementado en otros OS.
- Prompt SHA-256 original conservado: 7412501ba941c357418096079a62cff4487dccf9cb138f197c36f4088ea3d9dd. Sin commit/push, rama main y origin conservados, sin procesos temporales vivos.
Último paso completado: construir/verificar/consolidar. Siguiente: evaluación manual de comportamiento de ventana y feedback del usuario; no hay bloqueo de configuración ni de compilación. Conectores y autonomía quedan fuera de este hito.
