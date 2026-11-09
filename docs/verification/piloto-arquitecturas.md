# Piloto de arquitecturas: preparación y resultados

2026-11-01. Alcance: banco ficticio de tres casos y prototipo experimental aislado. No se han modificado perfiles, permisos o datos personales. La comparación no está completa mientras falte conectar y medir el constructor real de Gennie con los mismos casos.

## Banco

`pilot_fixture.py` contiene HTML ficticio y un verificador independiente de recetas declarativas para lectura sencilla, recuperación por ruta alternativa y objetivo compuesto. `pilot_mcp.py` publica inspección y prueba sin red ni efectos externos. La receta es una abstracción de selectores/atributos y no un plugin JavaScript de producción ni una prueba real de Chromium. Las pruebas unitarias verifican cobertura incompleta y rechazo de recetas inválidas.

`pilot_architecture.py` es un prototipo de orquestación separado: arquitecto, decisión, plan con dos propuestas, ejecutor, diagnóstico, decisión de estancamiento y validación del padre con comprobación independiente del fixture. Sus roles son sesiones sucesivas aisladas, no subagentes nativos de Codex ni el Goal Mode real. Se limita a diez llamadas de rol por celda y cuatro intentos; clasifica explícitamente presupuesto agotado y fallos. No está integrado en Gennie ni sustituye su constructor.

## Preparación y muestras descartadas

- Celda inicial [0edac292](../../.verification/architecture-pilot/0edac292-8315-41e5-9602-a89acc8734e4/result.json): el fixture exigía el campo `emails` sin indicarlo en la petición. La celda agotó diez roles, 91.110 ms y 354.504 tokens reportados, pero no es una comparación válida por el defecto del banco; se conserva.
- Se corrigió la petición para especificar nombres de campo sin revelar los valores. La celda [7fb7af48](../../.verification/architecture-pilot/7fb7af48-bf34-454f-8b17-4a210a66c356/result.json) completó la receta simple en cinco roles, 36.740 ms y 162.560 tokens reportados; no demuestra calidad general ni se mezcla sin señalar que el adaptador cambió después.
- Celda [d1a46504](../../.verification/architecture-pilot/d1a46504-9e5f-41e3-a105-9e87897bfcb5/result.json): el ejecutor encontró y verificó una receta válida para la alternativa, pero la sonda concatenó dos mensajes JSON separados y declaró `invalid_json`. Se conservan el resultado, cuatro roles, 28.199 ms y 113.526 tokens. El adaptador se corrigió para analizar únicamente el último mensaje completo y se comprobó con prueba local; la celda es un fallo técnico del piloto, no un fallo semántico de la arquitectura.

## Instrumentación

La versión local del esquema app-server publica `thread/tokenUsage/updated` con `last` y `total`, incluida entrada, caché, escritura de caché, salida, razonamiento y total. El piloto registra el último `total` de cada hilo de rol aislado y suma esos hilos una sola vez; no suma notificaciones acumuladas ni vuelve a sumar caché o razonamiento al total. Los campos ausentes permanecen desconocidos y no se convierten a cero. Tokens y tiempo no prueban por sí solos calidad ni coste monetario.

Las celdas usan perfiles de prueba temporales, no la base del usuario. Una lectura de metadatos sin escritura del checkout indicó que el perfil de desarrollo guardado usa `gpt-6.1-terra`/medium; el perfil de escritorio predeterminado encontrado tiene un esquema más antiguo. No se afirma cuál instancia está abierta ni que el prototipo use sus instrucciones personales. Para la variante A debe comprobarse explícitamente qué perfil y constructor real se usan; no llamar «Gennie actual» a una simple imitación de prompt.

## Muestras del banco corregido

Una ejecución por celda con `gpt-6.1-terra`/medium, mismo HTML y verificador. La calidad mostrada es la comprobación automatizada de la receta declarativa, no una valoración humana ni un plugin de producción.

| Caso | Prototipo experimental | Codex directo |
|---|---|---|
| Lectura sencilla | Completado; 35.398 ms; 149.507 tokens reportados | Completado; 12.986 ms; 65.146 entrada y 305 salida |
| Recuperación | Receta verificada tras consultar main fallida y después backup; 36.720 ms; 149.466 tokens reportados | Receta verificada; consultó backup antes de main, por lo que no demuestra recuperación posterior al fallo; 15.892 ms; 60.539 entrada y 332 salida |
| Compuesto | Completado; 36.780 ms; 150.287 tokens reportados | Completado; 14.272 ms; 65.383 entrada y 351 salida |

Las tres celdas experimentales [simple](../../.verification/architecture-pilot/6a654f11-a124-4835-8fe1-86f48ee3c77b/result.json), [recuperación](../../.verification/architecture-pilot/190dbfbc-4593-434d-b5fd-a4b4d825e014/result.json) y [compuesto](../../.verification/architecture-pilot/e42909dc-7e01-4e44-9b0c-1666830771da/result.json) usaron cinco roles independientes cada una, con validación del padre y sin deriva de fuentes. Codex directo: [simple](../../.verification/pilot-codex-direct/e235cf23-43db-4626-a6ed-98832d0e3a61/result.json), [recuperación](../../.verification/pilot-codex-direct/ab9d0929-0f74-4c89-8a6d-4c830de43a38/result.json) y [compuesto](../../.verification/pilot-codex-direct/f06c0231-9cf6-4913-a7e6-cbff403007af/result.json). Codex CLI reportó entrada/salida y caché, no un campo total; no se presenta la suma como una factura. Perfiles temporales y grupos de procesos propios retirados según cada informe.

Hay **una sola muestra por celda**, sin aleatorización, sin comparación con Gennie real y sin garantía de equivalencia de las instrucciones o la presentación de herramientas entre clientes. Por tanto, los números sirven para verificar instrumentación y detectar costes de este prototipo, no para elegir una arquitectura ganadora. En la celda de recuperación, Codex directo descubrió primero backup y solo después observó el fallo de main: completó la receta pero la secuencia no permite afirmar que se recuperó del fallo. La recuperación se refiere al estado ficticio que ofrece backup; no prueba recuperación frente a una caída de navegador real. La arquitectura experimental usa sesiones sucesivas, no subagentes nativos ni un Goal real.

## Trabajo pendiente para completar el piloto

1. Adaptar los tres casos a un sitio HTTP local y temporal, con datos sintéticos y un contrato de resultado común que el constructor real pueda implementar; comprobar independientemente sus salidas y la secuencia de recuperación. El test de integración existente del constructor demuestra una vía de arranque aislado, pero su caso, configuración y medición no equivalen a este banco.
2. Ejecutar la ruta real de Gennie con base, perfil de navegador y trabajos temporales; registrar de forma verificable la configuración e instrucciones efectivas sin revelar secretos ni alterar el perfil personal. Capturar el uso de tokens de todos los hilos implicados, o marcarlo desconocido si no puede atribuirse de manera fiable.
3. Ejecutar Codex directo y el prototipo contra el mismo sitio, petición, presupuesto y verificador, registrando diferencias inevitables en instrucciones y herramientas. Conservar trazas de errores, reintentos y tiempos, y comprobar que los procesos temporales terminan.
4. Revisar los nueve resultados y las trazas; separar fallos del banco de fallos de cada sistema y evaluar calidad y autonomía además de tiempos y tokens. Una ejecución por celda es exploratoria; no permite establecer una diferencia reproducible ni atribuir causalidad a arnés, modelo o configuración.

## Solicitud posterior: agentes reales y Decisions directo

El diagrama requiere jerarquía efectiva de agentes: el ingeniero que lanza un subagente es su jefe, recibe el veredicto y valida antes de informar al suyo. El prototipo anterior no acredita esa jerarquía ni Goal Mode real. La API directa [OpenAI Decisions](https://developers.openai.com/api/docs/guides/decisions) documenta `POST /v1/decisions` con `gpt-6-luna`, opciones tipadas y uso en la respuesta; el cliente actual de Gennie utiliza OpenRouter y no equivale a esa integración directa.

Se preparó `scripts/probes/pilot_openai_decisions.py`, aislado del producto, con selección validada y captura del uso reportado; dos pruebas simuladas verifican contrato y fallos. **No se ha llamado al servicio real**: `OPENAI_API_KEY` no estaba disponible en el entorno inspeccionado, ni el repositorio identifica otra fuente de clave directa. Tampoco se han implementado o ejecutado todavía los agentes y subagentes reales de esta nueva solicitud. Las seis muestras previas no deben presentarse como resultados de ella. Queda pendiente conectar y verificar el cliente con una clave válida, implementar y observar la jerarquía real, y completar las tres variantes sobre casos equivalentes.

### Seguimiento: 1 de noviembre

El usuario configuró una clave y autorizó la prueba. Una llamada real independiente a Decisions seleccionó `web` y reportó 119 tokens de entrada. El prototipo `pilot_native_agents.py` utiliza Codex con delegación nativa y llama a Decisions desde el coordinador antes de iniciar el arquitecto; no integra todavía la clasificación de estancamiento de Decisions, ni implementa Goal Mode nativo.

- [a20b3944](../../.verification/pilot-native-agents/a20b3944-75c8-40c8-8b85-89079891c92d/result.json): la política de aprobación rechazó las herramientas MCP y la delegación falló al buscar el rollout de la sesión efímera. Sin receta verificada ni subagente creado. Se conservaron los eventos y se corrigieron la declaración de lectura de las herramientas sintéticas y la persistencia temporal de sesión.
- [ba4fdcd0](../../.verification/pilot-native-agents/ba4fdcd0-642a-48a9-b1c0-724b5b860bf2/result.json): Decisions seleccionó web; el agente informó una cadena arquitecto → ingeniero → ejecutor y cinco pruebas de recetas fallaron porque el esquema del MCP no describía el formato esperado. Es un defecto del banco; se conserva y corrigió el esquema.
- [bd10cef4](../../.verification/pilot-native-agents/bd10cef4-6818-4e75-8a29-14356dc762c9/result.json): receta simple verificada; 37.179 ms, Decisions 137 tokens y raíz Codex 60.415 entrada/527 salida. El modelo informó dos niveles de delegación; los metadatos recogidos acreditan al menos un hijo real. No acreditan todavía de manera independiente la identidad del nieto. La captura contiene archivos con metadatos heredados o repetidos y no permite sumar de manera fiable el consumo de todos los agentes. Solo se presenta el contador de la raíz, no como total de la arquitectura.

La etiqueta `completed` de proceso indica salida normal, no validación de la arquitectura completa. Las tres celdas anteriores son exploratorias, pertenecen a versiones distintas y no se comparan como una campaña. No se han ejecutado las otras dos tareas con agentes nativos ni ninguna celda equivalente de Gennie real. Falta instrumentar jerarquía y tokens sin duplicar rollouts heredados, añadir el clasificador de estancamiento, ejecutar Goal Mode real y preparar los mismos casos para el constructor de Gennie. No se han alterado perfiles ni bases personales.

### Continuación: app-server, Goal y tres casos

Se añadió `scripts/probes/pilot_native_goal.py`: en una instancia temporal de app-server inicia Goal nativo para la raíz, habilita delegación de hasta dos niveles y recoge `thread/tokenUsage/updated` por identificador de hilo. Las notificaciones `subAgentActivity` acreditan la relación de los tres hilos; se toma un solo `total` final por identificador, sin sumar actualizaciones ni sumar de nuevo caché. La comprobación [sin inferencia](../../.verification/pilot-native-goal/cd60f500-81f9-4587-8ea0-955be42ecb4f/result.json) creó y consultó un Goal pausado.

| Caso | Evidencia | Resultado del fixture | Tiempo | Tokens Codex por hilo, suma de los últimos totales | Tokens Decisions de selección |
|---|---|---|---:|---:|---:|
| Sencillo | [ed9a2d1a](../../.verification/pilot-native-goal/ed9a2d1a-b608-485f-a350-728953d0f570/result.json) | Verificado | 34.219 ms | 136.840 + 130.566 + 35.671 = 303.077 | 136 |
| Alternativa | [ebac14c8](../../.verification/pilot-native-goal/ebac14c8-41f9-434d-b2e0-c629408bc5de/result.json) | Verificado | 34.571 ms | 169.527 + 150.224 + 54.186 = 373.937 | 136 |
| Compuesto | [b868dd70](../../.verification/pilot-native-goal/b868dd70-3dce-484b-8020-b10c3ef40365/result.json) | Verificado | 33.710 ms | 154.676 + 149.292 + 54.289 = 358.257 | 143 |

Son una muestra por caso, no una medición estadística; la suma usa los contadores reportados por app-server por hilo, no una factura ni una comprobación externa de cobro. El campo `tokensUsed` del Goal es otro contador y no se sustituye por la suma anterior. El Goal de la raíz alcanzó `complete` en los tres. La petición para crear un Goal en el subagente ejecutor fue rechazada por app-server con `direct app-server input is not allowed for multi-agent v2 sub-agents`; por tanto **no se ha probado Goal Mode en el ejecutor**. El adaptador expone Decisions para estancamiento, pero ninguna de estas celdas lo llamó, así que tampoco se ha verificado ese circuito en vivo. En el caso de alternativa se consultó primero backup y luego main: la salida es correcta, pero no acredita recuperación posterior al fallo.

Las muestras anteriores de Codex directo usaron el mismo HTML ficticio y el mismo verificador, pero no los mismos prompts o mecanismos de herramientas, y la nueva arquitectura tampoco representa todavía el diagrama completo. No se ha ejecutado Gennie real en estos casos: su constructor trabaja con navegador y plugins de producción, no con el MCP de recetas ficticias. Queda crear un entorno común que sirva a las tres rutas, medir el constructor real y resolver Goal en el ejecutor y la observación de recuperación antes de atribuir diferencias a la arquitectura.

### Variante v1 y Goal del ejecutor

Se seleccionó `multi_agent_version=v1` para el mismo modelo mediante una copia temporal del catálogo local, sin modificar el catálogo ni preferencias personales. En [0d8681b6](../../.verification/pilot-native-goal/0d8681b6-aaa0-40cf-8287-98a1eca4de6e/result.json), el servidor aceptó un Goal nativo del ejecutor y Decisions clasificó cuatro errores. La salida final pasó la verificación, pero se realizaron 34 llamadas de prueba de receta: el límite solicitado no estaba impuesto en la herramienta. La muestra no demuestra un bucle acotado correcto.

Se añadió un tope compartido de seis llamadas de prueba por ejecución, incluidas las validaciones; **no equivale a cuatro intentos por ejecutor**, porque el MCP no identifica al llamante. En [18eda626](../../.verification/pilot-native-goal/18eda626-15c3-42f2-938d-189170e1bcca/result.json), el Goal del ejecutor llegó a `complete`, cuatro recetas fallaron y Decisions se consultó cuatro veces, pero el coordinador detuvo la prueba con el Goal raíz aún activo. Ese cierre prematuro se corrigió para esperar continuación o un límite de cuatro turnos de raíz y 180 segundos; no se considera exitoso el resultado fallido.

Las tres muestras siguientes usan la nueva versión v1 con el límite compartido. Se crearon y completaron Goals tanto en raíz como en ejecutor. Los tokens Codex son la suma de los últimos totales reportados por hilo, y Decisions incluye la selección inicial y las clasificaciones registradas por el MCP.

| Caso | Evidencia | Tiempo | Tokens Codex | Tokens Decisions | Pruebas de receta | Calidad |
|---|---|---:|---:|---:|---:|---|
| Sencillo | [ddcd03f9](../../.verification/pilot-native-goal/ddcd03f9-a1e7-4a71-a816-d4c1a03813be/result.json) | 42.630 ms | 251.016 | 136 | 3 | Verificada |
| Alternativa | [90cd4421](../../.verification/pilot-native-goal/90cd4421-f5f3-4089-931a-a11ff64e14a0/result.json) | 45.612 ms | 264.269 | 136 | 3 | Verificada; main falló antes de consultar backup |
| Compuesto | [fcf1e255](../../.verification/pilot-native-goal/fcf1e255-42d6-40a8-96f9-c25c06c9b177/result.json) | 88.426 ms | 587.799 | 1.047 | 5 | Verificada; cuatro consultas adicionales a Decisions |

El experimento sigue aislado: las recetas no son plugins de producción, el límite de seis llamadas no implementa exactamente el presupuesto por agente del diagrama, y no se ha medido Gennie actual en un entorno equivalente. Un resultado correcto y Goals `complete` no bastan por sí solos para demostrar todas las ramas ni completar la comparación de tres sistemas.

### Preparación del sitio compartido

`scripts/probes/pilot_web_fixture.py` sirve en loopback y puerto temporal los mismos HTML y estados de `pilot_fixture`: `/main` y `/backup`, con HTTP 503 para el fallo de la ruta principal. Registra accesos sintéticos, cierra el servidor y no usa perfiles ni cuentas web. `pilot_web_fixture_test.py` comprobó en runtime el fallo, el contenido de respaldo y el cierre. Los adaptadores MCP de Codex directo y del prototipo inspeccionan ese servidor cuando se les suministra `GENNIE_PILOT_ORIGIN`; su verificador sigue siendo el fixture local. Las ejecuciones posteriores y su conexión al constructor real se detallan a continuación.

### Primera ejecución de los tres recorridos sobre HTTP local

**Invalidación posterior de las tres celdas Gennie de esta sección:** el fixture no activó el permiso de bloques programables pese a que el perfil lo permitía. No atribuir sus esperas al comportamiento de Gennie con el perfil correcto. Se conservan como fallos del banco; las repeticiones corregidas están más abajo.

Se añadió una prueba optativa de `gennie-plugins` que inicia el constructor real con base y navegador headless temporales y el perfil `plugin_builder` del checkout leído en modo read-only. El sitio no contiene cuentas ni datos reales. Las tres variantes usaron `gpt-6.1-terra`/`medium` y los mismos documentos sintéticos servidos por HTTP. Dos intentos iniciales de preparación del constructor fallaron porque la base temporal carecía de migraciones del navegador; se corrigió ese orden y se conservaron sus resultados.

| Caso | Gennie: estado, tiempo del runner, tokens reportados | Codex directo: resultado, tiempo, entrada + salida | Arquitectura experimental: resultado, tiempo, tokens Codex + Decisions |
|---|---|---|---|
| Sencillo | `waiting_user`; 56.068 ms; 573.113 | verificada; 10.689 ms; 65.380 + 316 | verificada; 83.587 ms; 604.301 + 752 |
| Alternativa | `waiting_user`; 56.012 ms; 553.315 | verificada; 13.947 ms; 60.757 + 336 | verificada; 39.426 ms; 258.904 + 136 |
| Compuesto | `waiting_user`; 42.537 ms; 442.060 | verificada; 14.202 ms; 65.621 + 349 | verificada; 38.880 ms; 225.833 + 143 |

Gennie: [sencillo](../../.verification/pilot-gennie-real/1ccdf2ad-1eca-4bab-b093-619e0f3d4648/result.json), [alternativa](../../.verification/pilot-gennie-real/15bfd141-51f3-4911-8751-0e81d6550797/result.json), [compuesto](../../.verification/pilot-gennie-real/cd53df92-a654-4ae6-9909-09f22c6b722d/result.json). Se obtuvo un contador de rollout del hilo del constructor por celda; `waiting_user` no prueba por sí mismo la causa ni debe clasificarse como error del modelo. Es la ruta del constructor con su perfil de desarrollo, no el recorrido completo del bot mediante la UI.

Codex directo: [sencillo](../../.verification/pilot-web-comparison/32110c63-7c13-40ac-b206-997e6eb197fc/result.json), [alternativa](../../.verification/pilot-web-comparison/4e1cf53a-facc-41bf-9b76-59789179f5f1/result.json), [compuesto](../../.verification/pilot-web-comparison/f05bf46b-9cf7-4c68-a2a9-5aeefc51dc48/result.json). Arquitectura: [sencillo](../../.verification/pilot-web-comparison/28e8ffc9-6738-4930-8172-a49cd75d763e/result.json), [alternativa](../../.verification/pilot-web-comparison/c0df8494-5c9b-4a22-9de2-a53d87c23943/result.json), [compuesto](../../.verification/pilot-web-comparison/e3604b74-3ce4-41d2-b1c9-cfc25d9eac26/result.json).

La arquitectura tuvo Goals de raíz y ejecutor completados en sencillo y alternativa; en compuesto el último estado observado del ejecutor seguía `active`, aunque la receta pasó y el Goal raíz terminó. El auditor añadido posteriormente falla de forma explícita para ese estado; no se modifica el resultado histórico. En alternativa, Codex directo consultó main y luego backup, mientras que el prototipo consultó primero backup: solo el primero ofrece evidencia de esa secuencia de recuperación.

Limitaciones: una muestra por celda, sin aleatorización; prompts y artefactos distintos. Gennie debía publicar un plugin real ejecutable en Chromium; las otras variantes construyeron recetas declarativas verificadas por un fixture independiente, con inspección HTTP mediante MCP. El tiempo de Gennie incluye el lanzamiento Cargo/test; los otros tiempos son sus runners internos. La caché está incluida en la entrada reportada; no sumarla otra vez ni presentar los tokens como facturación. El tope compartido de seis pruebas no garantiza cuatro intentos por ejecutor. Falta homogeneizar el contrato de resultado, instrumentar ese límite y repetir antes de hacer inferencias causales o elegir arquitectura.

### Corrección del permiso y auditoría de calidad

La repetición [1d7d2816](../../.verification/pilot-gennie-real/1d7d2816-4150-432f-87b1-9e039786b98d/result.json) registró un bloqueo por permiso de ejecutor. El controlador de navegador de la prueba tenía `program_allowed=false` por defecto aunque los cuatro permisos del perfil estaban activos. Se corrigió el montaje temporal para reflejar dichos permisos; no se modificó la configuración del usuario.

| Caso | Resultado ejecutado del plugin Gennie | Tiempo de construcción | Tokens del hilo |
|---|---|---:|---:|
| [Sencillo](../../.verification/pilot-gennie-real/9de24703-ee4a-4fa5-8cc0-f5207b3cf8d8/result.json) | Coincide con los dos enlaces `mailto:` esperados | 22.145 ms | 172.014 |
| [Alternativa](../../.verification/pilot-gennie-real/170f9952-2858-48fb-bd11-402ad281029b/result.json) | Coincide; se observó fallo de main antes de backup | 26.645 ms | 179.700 |
| [Compuesto](../../.verification/pilot-gennie-real/08d7d623-278e-4783-870c-ceb53c43a453/result.json) | **No coincide**: fechas correctas, pero correos sin el prefijo `mailto:` solicitado | 17.617 ms | 172.752 |

El estado del constructor fue `completed` en los tres; el verificador externo exige igualdad exacta y detecta la discrepancia del tercero. Se añadió ese criterio al corredor y se probó con un caso de regresión. Los resultados antiguos no se reescriben.

Se intentó aplicar un límite exacto por ejecutor usando herramientas dinámicas de app-server, que incluyen el identificador del hilo. En [cde010d5](../../.verification/pilot-native-goal/cde010d5-3617-4cb9-9065-4c91c12f7991/result.json), la herramienta no estuvo disponible para los subagentes nativos v1: el ejecutor quedó bloqueado y solo la raíz pudo verificar. Se retiró el adaptador inoperante y se restauró MCP con un máximo global de **cuatro pruebas para toda la jerarquía**, incluidas las validaciones. Es conservador: ningún ejecutor puede superar cuatro llamadas, pero no proporciona cuatro independientes a cada uno.

La muestra [d065618e](../../.verification/pilot-native-goal/d065618e-671e-4133-a2e4-5058d78a6eb9/result.json) completó ambos Goals y produjo una receta válida con tres pruebas globales; sin embargo, la respuesta final atribuyó la corrección y prueba final a la raíz tras fallos del ejecutor. Por ello se reforzó la auditoría posterior: exige evidencia de verificación de la misma receta por el ejecutor y su jefe inmediato, además de Goals y tokens por hilo. El `architecture_audit` anterior a ese cambio no acredita esta condición. Aún falta observar una ejecución que cumpla la auditoría nueva, dar cuatro intentos independientes por ejecutor y equiparar el artefacto real entre las tres variantes.

### Auditoría de jerarquía y comparación corregida

Se incorporó el objetivo literal y sus nombres de campo al Goal del ejecutor, y se pide a su jefe devolverle el diagnóstico si falla. La auditoría consume eventos de app-server atribuidos por `threadId`: exige prueba positiva de **la misma receta** por ejecutor y jefe inmediato, Goal raíz y del ejecutor completados, y contador del hilo del ejecutor. La muestra [0b61e638](../../.verification/pilot-native-goal/0b61e638-5256-43bd-b1ed-8eae44bb2622/result.json) falló esa auditoría aunque la raíz corrigió el resultado: se conserva.

La versión posterior superó la auditoría en los tres casos con tres pruebas cada uno y Goals terminados. Son muestras exploratorias, sin aleatorización. El máximo sigue siendo cuatro pruebas **compartidas por toda la jerarquía**, no cuatro independientes por ejecutor.

| Caso | Gennie: resultado; construcción; tokens | Codex directo: resultado; tiempo; entrada + salida | Arquitectura: resultado y auditoría; tiempo; tokens Codex + Decisions |
|---|---|---|---|
| Sencillo | Coincide; 22.145 ms; 172.014 | Coincide; 10.689 ms; 65.380 + 316 | Coincide, auditoría aprobada; 51.717 ms; 290.930 + 136 |
| Alternativa | Coincide; 26.645 ms; 179.700 | Coincide; 13.947 ms; 60.757 + 336 | Coincide, auditoría aprobada; 40.623 ms; 291.666 + 136 |
| Compuesto | No coincide: `mailto:` ausente; 17.617 ms; 172.752 | Coincide; 14.202 ms; 65.621 + 349 | Coincide, auditoría aprobada; 57.336 ms; 295.417 + 143 |

Arquitectura: [sencillo](../../.verification/pilot-web-comparison/ee848e57-4b95-4401-9bf8-2e095bb83383/result.json), [alternativa](../../.verification/pilot-web-comparison/7054eead-8ab8-47b6-8add-fc9636fba4b8/result.json), [compuesto](../../.verification/pilot-web-comparison/4761cb56-081a-48e9-8b94-a3dbef52065a/result.json). Los enlaces de Gennie y directo son los detallados en las secciones anteriores. En alternativa, los tres recorridos consultaron main antes de backup; es evidencia de la secuencia, no de robustez general.

La comparación es por datos extraídos, no por artefacto idéntico: Gennie creó, publicó y ejecutó un plugin real; los otros dos entregaron una receta declarativa validada con el fixture, sin Chromium. El tiempo de construcción de Gennie excluye el arranque del runner Cargo; los otros tiempos son de sus corredores. Tokens de entrada y caché se solapan; no equivalen a coste facturado. No se puede elegir un ganador ni aislar causalidad con una muestra por celda y estos contratos distintos. Se añadieron hashes de fuentes y binario a nuevas celdas y hash del perfil sin publicar instrucciones; esas huellas no existían en las muestras históricas.

### Plan y prueba de recuperación instrumentados

El servidor MCP exige ahora `record_plan` con al menos dos propuestas distintas y listas de errores estáticos y dinámicos antes de permitir pruebas. La auditoría exige que el jefe inmediato registre el plan, que el ejecutor y ese mismo jefe verifiquen la misma receta, Goals completados y tokens por hilo. Las muestras [sencillo](../../.verification/pilot-web-comparison/18b190c9-60e2-4fc7-a98d-12ffb5ee86f8/result.json), [alternativa](../../.verification/pilot-web-comparison/1b2da8eb-cd9b-4af4-aa20-0de9c36cd684/result.json) y [compuesto](../../.verification/pilot-web-comparison/a5decc86-0a5d-4c49-88dd-d404cde5ca76/result.json) pasaron esa auditoría; tiempos 51.181, 57.471 y 46.440 ms, tokens Codex 260.785, 341.844 y 326.489, más 136, 136 y 143 de Decisions, respectivamente. Sus fuentes y binario no derivaron durante la ejecución. En alternativa se observó main fallida antes de backup.

La prueba separada [87c038b8](../../.verification/pilot-native-goal/87c038b8-45ef-4148-b4ef-8566be30ece3/result.json) inyectó un único fallo transitorio en la primera prueba: el ingeniero consultó Decisions, que eligió continuar; el mismo ejecutor luego verificó, su jefe verificó la misma receta y la raíz hizo una cuarta comprobación. Ambos Goals terminaron y la auditoría pasó. Esta prueba muestra recuperación de un fallo sintético, no prueba clasificación de estancamiento repetido ni todas las clases de error.

En [a71cdad2](../../.verification/pilot-native-goal/a71cdad2-b50a-4d80-8547-da26c349597a/result.json) se inyectaron dos fallos transitorios iguales. Decisions eligió primero `continuar` y después `estancado`; se registraron nuevas propuestas. Las dos pruebas restantes fueron recetas inválidas, se agotó el límite compartido y el Goal raíz terminó `blocked`, sin resultado verificado. La auditoría falló correctamente: se observa clasificación de estancamiento y detención acotada, no recuperación exitosa.

Continúan los límites metodológicos anteriores. En especial el tope es global, no cuatro presupuestos independientes por cada ejecutor, y el artefacto del constructor de Gennie sigue siendo un plugin real frente a las recetas del banco. El plan registra decisiones del ingeniero, pero no demuestra por sí mismo que todas las propuestas sean técnicamente viables.

### Presupuesto por hilo y ejecución común en Chromium

La versión instalada de Codex envía `threadId` en `_meta` de las llamadas MCP; se observó en [d1a7e58b](../../.verification/pilot-native-goal/d1a7e58b-2bc2-4faa-a521-29c2561886a9/result.json) y se cotejó con los eventos de app-server. El servidor aplica ahora cuatro pruebas por hilo y un techo global de doce; rechaza sin identidad válida. Las pruebas locales cubren presupuestos independientes y ausencia de identidad. En [98acc58d](../../.verification/pilot-native-goal/98acc58d-cab4-4e1e-a231-1071d7acb5a1/result.json) se inyectó un fallo, Decisions eligió continuar y ejecutor, jefe y raíz verificaron la misma receta; la auditoría cotejó los identificadores de MCP y app-server, y pasó. El caso de dos fallos conservado arriba se ejecutó con la versión anterior de presupuesto global.

Para acercar los resultados, el banco convierte cada receta declarativa aprobada de Codex directo y del prototipo a un manifiesto programable fijo y ejecuta su función mediante el motor real de plugins de Gennie y Chromium headless, con perfil temporal. La conversión es código del banco, **no trabajo generado por el agente**. El convertidor rechaza recetas que no pasan la verificación limitada y limita el destino a loopback. Todas las verificaciones externas siguientes devolvieron los datos exactos; Gennie utilizó su propio plugin creado y publicado por el constructor.

| Caso | Gennie: resultado; tiempo total; tokens | Codex directo: resultado; tiempo total; entrada + salida | Arquitectura: resultado y auditoría; tiempo total; Codex + Decisions |
|---|---|---|---|
| Sencillo | Coincide; 25.525 ms; 172.014 | Coincide; 13.086 ms; 65.447 + 323 | Coincide, auditoría aprobada; 48.077 ms; 278.974 + 136 |
| Alternativa | Coincide; 27.003 ms; 179.700 | Coincide; 12.363 ms; 65.362 + 306 | Coincide, auditoría aprobada; 51.246 ms; 351.369 + 136 |
| Compuesto | No coincide: correos sin `mailto:`; 18.559 ms; 172.752 | Coincide; 15.378 ms; 60.939 + 359 | Coincide, auditoría aprobada; 50.955 ms; 324.094 + 143 |

Codex directo: [sencillo](../../.verification/pilot-web-comparison/cbe00532-21c3-4b75-aadd-1a31b9f8f622/result.json), [alternativa](../../.verification/pilot-web-comparison/f4782e0a-5660-4c1d-b80f-ddc99a1f7700/result.json), [compuesto](../../.verification/pilot-web-comparison/5de8a08f-3596-47c6-8047-4956003c4765/result.json). Arquitectura: [sencillo](../../.verification/pilot-web-comparison/91b51cc0-d414-47a1-84ac-d12c8be81878/result.json), [alternativa](../../.verification/pilot-web-comparison/d82b0039-7cb2-427a-b3b3-4fd92d53e16e/result.json), [compuesto](../../.verification/pilot-web-comparison/261c1b2c-7b13-4fdf-90f4-0813833ec53c/result.json). Gennie: las tres muestras corregidas enlazadas arriba. Los tiempos totales incluyen arranques y comprobaciones; los runners de Codex y el prototipo registran el tiempo del agente y el del navegador por separado. Una muestra por celda, sin aleatorización, no determina superioridad general ni coste facturado.

En la muestra de alternativa de la arquitectura y en la de Codex directo, backup se consultó antes de main, por lo que no prueban recuperación después del fallo principal. La muestra separada de fallo inyectado sí observa diagnóstico y continuación. La arquitectura permanece experimental y aislada: no se ha integrado al producto ni se ha demostrado con sitios reales o cargas dinámicas.

### Tarea real: los tres agentes construyen el plugin

Ante la petición de medir construcción real, se preparó un caso separado: crear y verificar un plugin `fixture`, función `extract`, que lea mediante `transport=page` los nombres del directorio local y devuelva el array exacto `["Ana", "Luis"]`. Los tres recorridos usaron el mismo HTML servido por loopback, `gpt-6.1-terra`/medium y Chromium headless. El plugin se guardó y verificó por el propio agente o su ejecutor, y se ejecutó de nuevo desde el host para comprobar el resultado. **En esta prueba el banco no convierte ninguna receta ni crea el plugin por el agente**. El host limita origen y funciones a lectura `page` sobre `/main`; no se usa perfil web personal.

| Variante | Resultado independiente | Tiempo total del runner | Tokens reportados |
|---|---|---:|---:|
| [Gennie actual](../../.verification/pilot-gennie-real/874239a5-c637-4807-b4ff-a7ed870faf7b/result.json) | Plugin publicado, verificado y ejecutado; datos exactos | 18.198 ms | 170.674 Codex |
| [Codex directo](../../.verification/pilot-real-plugin/28aa4a72-2e6d-40bc-954c-7819b65a8fce/result.json) | Plugin guardado, verificado y ejecutado; datos exactos | 31.539 ms | 116.260 Codex |
| [Arquitectura experimental](../../.verification/pilot-real-plugin/f115fc60-813f-4c61-9a8d-40d3a986cb50/result.json) | Plugin guardado, verificado y ejecutado; datos exactos | 76.569 ms | 446.213 Codex + 330 Decisions |

Los tiempos totales incluyen arranque del test y sus procesos, construcción, verificación, ejecución independiente y cierre; no incluyen crear el servidor HTTP, que ya estaba activo. En la arquitectura experimental el ingeniero registró dos propuestas, el ejecutor verificó el plugin, su jefe lo ejecutó y comprobó, y ambos Goals terminaron. Decisions respondió dos consultas. La identidad y el conteo de cada uno de los tres hilos Codex se conservan en la evidencia.

La ejecución preliminar [9d52182e](../../.verification/pilot-real-plugin/9d52182e-ca4a-4277-b49b-0c975844e64c/result.json) produjo un plugin funcional, pero el runner terminó al final del primer turno mientras el Goal raíz seguía activo. Es un fallo del runner, no prueba de abandono; se corrigió para esperar la continuación y se ejecutó la celda enlazada en la tabla. Las cifras previas de 13 segundos medían recetas convertidas por el banco y no deben presentarse como construcción de plugin real.

Esta es una sola muestra por variante, sin aleatorización, de un sitio pequeño, estático, público y sin login; no demuestra tiempos típicos ni calidad general. Los permisos y el inventario de herramientas difieren entre el constructor real y el host restringido, y el resultado `page` no evalúa generación de bloques programables. Codex directo usó app-server sin instrucciones ni orquestación de Gennie; la arquitectura usó subagentes nativos v1, Goal y Decisions. **El host de plugin real no aplicó el límite por hilo de la herramienta de recetas sintéticas**, por lo que esta celda no verifica ese control en las herramientas reales. Los tokens de caché forman parte de la entrada y no se suman de nuevo; no equivalen a coste facturado.
