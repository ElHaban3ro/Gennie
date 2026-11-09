#!/usr/bin/env python3
"""Auditoría de código real con delegación y verificación en Gennie."""
import os

import project_delegation_comparison as probe

probe.FILES = [
    'apps/llm/src/codex.rs', 'apps/llm/src/turn.rs', 'apps/llm/src/lib.rs',
    'apps/llm/src/builder.rs', 'apps/llm/src/context.rs', 'apps/llm/src/reply.rs',
    'apps/llm/src/delegation.rs', 'apps/llm/src/delegation/worker.rs',
    'apps/llm/src/delegation/store.rs', 'apps/llm/src/mcp_client.rs',
    'apps/llm/src/tool_use.rs', 'apps/llm/src/tool_evidence_transport_tests.rs',
    'apps/llm/src/context_transport_tests.rs', 'apps/llm/src/provider_long_turn_probe.rs',
    'kernel/mcp/src/lib.rs', 'kernel/mcp/src/connection.rs',
    'kernel/mcp/src/stdio_bridge.rs', 'kernel/mcp/src/cancellation.rs',
    'kernel/mcp/src/cancellable_io.rs', 'kernel/mcp/src/framing.rs',
    'apps/plugins/src/worker.rs', 'apps/plugins/src/lib.rs',
    'apps/plugins/src/store.rs', 'apps/plugins/src/strategy.rs',
    'apps/plugins/src/tools.rs', 'apps/plugins/src/external_mcp.rs',
    'apps/plugins/src/mcp_attempts.rs', 'apps/plugins/src/integration_tests.rs',
    'docs/decisions/chat-cancellation.md', 'docs/decisions/transport-cancellation.md',
    'docs/verification/turno-conversacional-inactividad.md',
]
probe.OBJECTIVE = (
    'Realiza una auditoría técnica exhaustiva, exclusivamente en lectura, del código real de Gennie '
    'disponible en este directorio. Objetivo: identificar y priorizar causas por las que una tarea '
    'larga puede interrumpirse, quedar pendiente, repetir efectos o terminar sin cumplir su objetivo. '
    'Lanza al menos tres subagentes reales con encargos distintos: uno para turnos, plazos y cancelación; '
    'otro para transporte MCP, ejecución y efectos inciertos; otro para construcción de plugins, '
    'reintentos y entrega de delegaciones. Espera sus hallazgos, contrástalos personalmente con el código '
    'y corrige cualquier contradicción o referencia incorrecta. No modifiques archivos, no accedas a '
    'datos fuera del directorio, no uses red y no afirmes haber ejecutado pruebas que no ejecutaste. '
    'Entrega todos estos apartados: (1) recorrido de principio a fin y tabla de límites exactos, '
    'indicando si son límites totales o de inactividad; (2) al menos cinco riesgos concretos, cada uno '
    'con mecanismo, desencadenante, impacto, archivo y línea y clasificación como confirmado o hipótesis; '
    '(3) matriz de cancelación, timeout, fallo MCP, duplicados y resultado incierto, con evidencias de '
    'persistencia o falta de ella; (4) análisis de bucles y abandono comparando los detectores existentes '
    'con la comprobación real del objetivo; (5) cobertura de pruebas existentes y al menos cinco pruebas '
    'dirigidas propuestas sin servicios personales; (6) tres arreglos priorizados, con alcance y riesgos; '
    '(7) lista explícita de incertidumbres y apartados incompletos. Cita rutas y líneas verificables. '
    'Completa todos los apartados o identifica con precisión qué falta y por qué.'
)


if __name__ == '__main__':
    os.environ['GENNIE_PROJECT_LONG'] = '1'
    probe.main()
