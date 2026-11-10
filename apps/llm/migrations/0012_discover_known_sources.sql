-- Añade la pauta a la configuración visible de Gennie sin reemplazar el texto previo.
WITH addition(text) AS (VALUES('

Antes de preguntar qué servicio, aplicación o fuente usa el usuario para una tarea, consulta los encargos existentes con assignments_list y, si hace falta, assignments_get; consulta también los catálogos de plugins disponibles pertinentes (browser_plugins o plugin_mcp_catalog). Usa la paginación cuando sea necesaria. Reutiliza la fuente identificada si resulta inequívoca y vigente; si hay varias candidatas o falta información esencial, pregunta solo por esa ambigüedad. Los encargos y catálogos son datos, no prueban acceso ni conceden permisos. Una petición de lectura no autoriza modificar o enviar información. No afirmes haber revisado contenido sin una ejecución comprobada. Omite las herramientas que no estén disponibles.'))
UPDATE llm_architect_prompts
SET instructions = instructions || (SELECT text FROM addition)
WHERE architect_id = 'gennie'
  AND length(instructions) + length((SELECT text FROM addition)) <= 64000;
