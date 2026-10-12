# Verificación proporcional

Propietario del alcance de checks. Un proceso Cargo por target; no duplicar builds/test en paralelo. Tests internos comparten builders mínimos locales; no hay kernel/testkit sin consumidores reales. Fixtures no mutan entorno global ni datos del usuario. DB en memoria o temporal propio. No pruebas de router sin HTTP.

## Comandos
Desde raíz con Rust >=1.92 y toolchain del host:
```
cargo fmt --all -- --check
cargo test --workspace --locked
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo build --locked -p soc
python3 scripts/check_invariants.py
```
Para cambios solo SQL/persistencia: `cargo test --locked -p soc-db -p soc-presence`; para config: `cargo test --locked -p soc-config`. Si cambian AppState/registro/features/lifecycle, ampliar al workspace. No repetir campañas tras checks verdes sin nueva modificación o duda concreta. Documentación pura requiere enlaces/enrutamiento, no tests artificiales.

## Casos necesarios
Registro: duplicados, ciclos, requerida ausente/apagada, opcional presente/ausente, orphans y conservación de selección. Migraciones: rollback real, checksum, namespace, reapertura y downgrade rechazado. Lifecycle: idempotencia, orden inverso, fallo de startup y error secundario sin ocultar original. Preferencias: escritura y reapertura real; UI actualiza éxito después de commit. Futuras compensaciones/eventos deben probar sus efectos, no simular una garantía ausente.

## Runtime macOS
Anunciar propósito antes de arrancar sin puertos; usar SOC_DATABASE_PATH temporal explícito para evaluación. Verificar cara en reposo/activo, movimiento en secuencia (captura estática no basta), foco y first click, transparencia, click-through fuera del círculo, posición y siempre encima con ventanas propias. Salir por interacción documentada y comprobar PID cerrado; no detener procesos ajenos. Herramientas computer-use según entorno. Registrar qué se observó y qué no; otros OS, Spaces y multi-monitor requieren pruebas independientes.

Dependencias: consultar registros oficiales y RustSec antes de incorporar, registrar límites. Escaneo manual de avisos no equivale a cargo audit ni prueba ausencia de compromiso. Pruebas caras/externas necesitan autorización si no está incluida; no añadirlas a hooks/CI obligatorios. Esta base se valida localmente, sin despliegue ni producción.

Sonda manual local de interacción: `cargo run --locked -p soc-presence --example hit_test_probe`, ventana sin datos/negocio con contador. Abrir luego soc en otra terminal con SQLite temporal; probar físicamente fuera/dentro y levantar sonda para comprobar z-order. No tomar una acción dirigida por app como prueba OS. Cerrar ambas al terminar. El ejemplo es solo herramienta de evaluación, no se incorpora a soc.

## Spaces de macOS
Reproducción a través de instancia aislada del binario real, SQLite propia y NSWindow.isOnActiveSpace durante cambios efectivos de escritorio. Una captura de ventana o una assertion de collectionBehavior no prueba seguimiento. Comparar ausencia/presencia con versión previa, volver al escritorio inicial y cerrar solo instancias propias. Registrar por separado escritorio normal, fullscreen y Stage Manager; no invadir procesos existentes ni ejecutar Ctrl+Q sobre una app ajena. Si las acciones dirigidas por app no cambian el escritorio real, pedir observación opcional al usuario y explicitar el límite. Instrumentación de diagnóstico etiquetada debe retirarse del binario final.
