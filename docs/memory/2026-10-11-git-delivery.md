# Entrega Git inicial — 2026-10-11

Autorización explícita del usuario: «Commitea y pushea TODO en distintos commits. Sin pruebas avanzadas». Alcance: todos los archivos apropiados del proyecto, sin secretos/datos de uso/artefactos. Checkout y rama main conservados; origin git@github.com:ElHaban3ro/soc.git. No PR, despliegue ni reinicio de procesos.

Convención elegida dentro de la delegación: mensajes descriptivos en español `tipo: resumen`, sin atribuciones automáticas. Agrupaciones: requisitos originales .creation; base modular completa kernel/config/SQLite, bootstrap y Slint monocromático; corrección de pertenencia a Spaces; documentación/reglas/scripts. La separación del fix de Spaces se realiza en el índice, conservando el archivo final del checkout.

Revisión: inventario y exclusiones con git check-ignore, búsqueda acotada de patrones de secretos sin imprimir valores, revisión staged y diff --check. No nuevas compilaciones, tests, UI ni campañas avanzadas; resultados recientes conservados en las memorias de base/Spaces. `.env`, data, target y .verification permanecen fuera de Git. Prompt original conserva SHA-256 7412501ba941c357418096079a62cff4487dccf9cb138f197c36f4088ea3d9dd.

Acceso SSH: host-key inicial no disponible; huella Ed25519 contrastada con [GitHub oficial](https://docs.github.com/es/authentication/keeping-your-account-and-data-secure/githubs-ssh-key-fingerprints). Se usa archivo temporal de confianza en .verification y StrictHostKeyChecking=yes solo para los comandos de entrega, sin desactivar verificaciones ni modificar configuración global.

Push ordinario de main solicitado; hashes y resultado definitivo se reportan al usuario después de ejecutarlo y verificar remoto. Este registro conserva intención y revisión, no sustituye la evidencia del comando ni concede autorizaciones futuras.

## Incorporación concurrente durante la entrega
Después de subir los cuatro commits iniciales, se detectaron ocho archivos modificados por el chat «Desactiva opciones actuales de la UI». Se verificó la petición directa del usuario y que ese trabajo estaba terminado: desactivar M/A/Espacio/Escape/clic derecho y quitar ayuda, conservar clic y menú nativo, reservar preferencias para configuración futura. Se incluyen en un quinto commit coherente, sin sobrescribir ni excluir ese trabajo. El chat de esa tarea reportó 13 tests, build, formato, Clippy e invariantes verdes; no se repiten en esta entrega ni se toma esa comprobación como validación runtime.
