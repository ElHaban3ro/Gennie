# Gennie — diseño de escritorio

Estado vigente tras la migración a Flutter del 24 de octubre de 2026. La interfaz está en `frontend/lib`; [reglas frontend](docs/rules/frontend.md). La presencia flotante, cara y globos se retiraron sin reemplazo. Las antiguas preferencias SQLite se conservan por compatibilidad, sin recrear su UI.

## Identidad y tipografía

Nombre visible **Gennie**; identificadores técnicos en minúscula. Conservar el icono original, fondo oscuro `#24262c` y símbolo blanco, desde los assets de frontend. Inter 4.1 local para textos, pesos 400/500/600/700; mantener licencia OFL. No depender de fuentes instaladas ni descargar durante uso.

## Ventana y navegación

Ventana normal redimensionable de 1100 × 720 puntos, mínimo 640 × 420, controles macOS y nombre Gennie. Sidebar sin bordes exteriores ni acento azul. Barra de navegación de 60 px, contenido inicialmente 260 px con rango 216–420 y al menos 320 px para el lector. El ancho y pestaña son transitorios. Agarre invisible de 8 px con cursor horizontal, sin borde.

Historial y Plugins usan botones de 36 × 36 px, iconos de 18 px y separación de 6 px; Configuración queda al pie. Etiquetas accesibles y navegación por teclado. Hover y selección grises translúcidos (`#80808014` / `#80808024`), transición de 180 ms; iconos sin atenuación. Títulos «Historial de chats» y «Plugins» en Inter de 12 px regular y gris `#737373`. Mensaje vacío exacto: «No hay contenido disponible».

Historial ordena chats por actividad con filas de 36 px, una línea sin fecha/hora y menú de tres puntos. Consultar otro chat no cambia el hilo activo. El lector muestra texto, emisor y fecha, conserva referencias y permite desplazamiento; errores separados de respuestas. El compositor envía al chat activo mediante Rust. Plugins conserva su estado vacío.

## Acciones y ajustes

Renombrar y eliminar operan solo sobre los datos locales. Nombre de 1–200 caracteres sin saltos; prioridad nombre local, nombre del proveedor, título provisional. Abrir menú no selecciona otro chat. Escape/clic fuera cierran; Tab, flechas e Intro permiten operar. Diálogos compartidos con foco visible y borrador conservado ante fallo; bloquear acciones mientras se guarda, cerrar tras commit. Eliminar no elimina el hilo de Codex ni resucita mensajes por respuestas tardías.

Ajustes de asistente, dictado y voz consumen sus preferencias Rust existentes. No añadir controles de apariencia flotante. Mostrar estados de conexión, trabajo y errores dentro del escritorio. La respuesta escrita permanece si falla narración.

## Verificación visual

Preservar estas decisiones al ajustar una parte. La integración nativa de ventana corresponde al runner Flutter; no se considera comprobada por herencia de una UI anterior. El runner macOS release se compiló con Xcode 27.1. La interacción física y composición visual nativa siguen pendientes de verificación. Análisis Dart, tests de widgets y compilación Rust no prueban vidrio, foco, controles físicos, permisos, Dock ni Stage Manager. No cerrar ni reiniciar una instancia ajena.
