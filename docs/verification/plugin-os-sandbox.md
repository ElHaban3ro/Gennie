# Sonda de aislamiento para plugins

10 de noviembre de 2026. `node scripts/probes/plugin_os_sandbox.mjs` inicia dos procesos Node propios bajo Seatbelt de macOS, sin cuenta ni perfil web. La política experimental deniega por defecto y permite el arranque del runtime y sus bibliotecas. El primer proceso intenta leer y modificar un archivo ficticio, abrir una conexión local, crear otro proceso y consultar la señalización del padre: las cinco operaciones se rechazaron. El archivo se conservó intacto y se retiró junto al directorio temporal.

El segundo proceso usa un intercambio ficticio por stdin/stdout: el padre acepta una primera acción, recibe una segunda, cancela una espera de prueba de 5000 ms y comprueba que no aplica la segunda. La muestra también comprueba que el generador del sistema produce valores dentro de 0..5000. La espera fija pertenece solo a la prueba de cancelación; no es configuración del producto.

Esta sonda histórica no usa el ejecutor de plugins ni Patchright y no demuestra que toda la superficie de macOS esté aislada. La política contiene concesiones amplias para bibliotecas y servicios del sistema y necesita auditoría por versión de macOS, firma y distribución. No se integró aislamiento del sistema operativo en el producto. El contrato vigente de las recetas y las restricciones de `program` se describen en D-027.
