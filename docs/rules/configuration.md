# Configuración

Propietario: kernel/config. `Config` contiene ruta DB y busy timeout; `Secrets` separado y vacío en esta base, sin Debug ni AppState. Bootstrap consume Secrets al construir clientes futuros. No inventar credenciales. Prefijo SOC_. Precedencia: entorno de proceso > `.env` explícito del cwd > defaults. No modificar entorno global mediante unsafe set_var; fusionar valores en memoria. Exportado distinto al archivo genera aviso solo con nombre.

## Variables de proceso
- SOC_DATABASE_PATH obligatoria sin default: archivo SQLite, no vacío ni `:memory:`. .env local autorizado usa data/soc.sqlite3. Rutas relativas al cwd; para lanzar desde otro lugar proporcionar ruta absoluta y entorno.
- SOC_DB_BUSY_TIMEOUT_MS default 500, entero 1..5000. Una conexión, sin parámetros de pool o HTTP.

.env fuera de Git. .env.example lista todas las variables y distingue obligatorias/defaults; test compara declaraciones y defaults. No sobrescribir .env existente. Falta de ruta falla temprano con nombre, no valor. Error de parseo no debe filtrar valores sensibles; cuando se añadan secretos sanitizar mensajes del parser también.

## Preferencias de app
presence posee `reduced_motion` booleano, default false, aplicable sin reinicio, sin unidad. Se carga al abrir y se confirma después del commit. Estado activo/dormido no persiste. No hay app configs ni ConfigSource porque solo hay un consumidor y un ajuste propio; extraer contrato kernel cuando otro módulo necesite lectura transversal, sin importar repositorio de otra app. Si se añaden catálogos, declarar tipo/default/restricciones/reinicio, rechazar duplicados/defaults inválidos y conservar ajustes de módulos apagados. Aviso futuro configs.changed lleva nombres, sin valores, después de commit. Nunca usar default para ruta externa obligatoria.

## Verificación
Validar obligatorias, rangos, precedencia y sincronización ejemplo/declaraciones; reapertura del archivo demuestra persistencia. Ver testing y database para transacciones.
