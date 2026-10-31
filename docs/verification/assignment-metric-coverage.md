# Encargos: cobertura de métricas observadas

30 de octubre de 2026. Relectura de filas `live.jsonl`, manifiestos, valoraciones y cierres originales, sin nuevas inferencias o modificación de esos archivos. [Resultado y hashes primarios](../../.verification/completion-audit/requirements-31c616f5407c4a119ce02cd1cdff54de/METRIC_COVERAGE.json).

| Medida | Piloto d65965f4 | Extensión 5b9cd7a9 |
|---|---:|---:|
| Casos distintos | 4 | 6 |
| Turnos/celdas de modelo | 16 | 24 |
| Celdas con borrador / sin borrador | 13 / 3 | 19 / 5 |
| Candidatos guardados | 16 | 22 |
| Preguntas guardadas | 3 | 7 |
| Mediana por celda, incluido arranque | 15.056 ms | 15.071 ms |
| Duración de fase | 258.831 ms | 384.213 ms |
| Consultas de guía | 8 | 9 |
| Coste parcial reportado por la guía | 0,0012231 | 0,0012889 |
| Coste monetario completo | Desconocido | Desconocido |
| Valoraciones | Una elección humana B | 24 filas asistidas por IA |

Los diez casos son distintos entre campañas. Sus versiones de fuentes y tiempos se conservan por separado; no se agrupan como un experimento simultáneo o una comparación causal de rendimiento. Las filas comprueban que los planes originales permanecieron iguales. No se aplicaron propuestas.

La elección B corresponde exclusivamente al par `duplicate_scope` del piloto, sin puntuaciones numéricas inventadas. En la extensión, 19 filas permiten rúbrica completa y cinco no son valorables; 15 se marcaron aceptables sin editar y cuatro tras editar, por revisión asistida, no por el usuario.

Estos recuentos miden preparación de borradores. No prueban ejecución de tareas, necesidad de cada pregunta o utilidad de avisos entregados. Ninguna campaña midió entregas contextuales reales ni tasas de falsos positivos/negativos de esos avisos. El coste de la guía es parcial y no sustituye el coste desconocido del bot o el total efectivo; no se atribuye una moneda no comprobada.

La [corrección del contrato contextual](contextual-notice-contract.md) resuelve una incompatibilidad que esas campañas de borradores no atravesaban. El [ensayo posterior de avisos reales](contextual-notice-probe.md) ya comprobó cuatro casos ficticios prefijados con guía/bot reales y Presence: 4/4 acciones acordadas, tres entregas únicas, 0/1 interrupciones falsas y 0/3 omisiones respecto a esas expectativas. Es una medición separada, no una nueva propiedad atribuible a las campañas anteriores ni garantía de utilidad general; conserva revisión asistida y coste completo desconocido.
