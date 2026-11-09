#!/usr/bin/env python3
"""Auditoría real de varias rondas con subagentes y revisión cruzada."""
import os

import long_project_delegation as earlier

probe = earlier.probe
probe.OBJECTIVE = (
    'Audita en profundidad los archivos reales de Gennie disponibles en este directorio. '
    'El objetivo es elaborar un diagnóstico accionable de interrupciones, bucles y efectos inciertos, '
    'en modo estrictamente lectura y sin acceder a red ni datos fuera del directorio. '
    'Trabaja en tres fases reales. Fase 1: lanza tres subagentes con encargos distintos: '
    'A: ciclo de turno, plazos y cancelación; B: transporte MCP, efectos inciertos y persistencia; '
    'C: constructor de plugins, detección de bucles, reintentos y delegaciones. Espera los tres informes. '
    'Fase 2: solicita revisión cruzada: A debe buscar contraejemplos y límites en B, '
    'B en C, y C en A. Cada uno debe citar código, identificar al menos dos afirmaciones que requieren '
    'matiz o que están respaldadas, y proponer un caso de prueba falsable. Espera esa segunda ronda. '
    'Fase 3: contrasta personalmente las afirmaciones contra el código e integra el resultado. '
    'Entrega: (1) mapa de responsabilidades y secuencia de una solicitud, con al menos diez referencias '
    'a archivos y líneas; (2) tabla de plazos y presupuestos y si son de inactividad o totales; '
    '(3) ocho riesgos priorizados con condición, efecto, evidencia y clasificación confirmado o hipótesis; '
    '(4) cinco escenarios de fallo completos indicando estados antes/después, persistencia, recuperación y '
    'si puede repetirse un efecto; (5) análisis de los detectores de bucles y de verificación del objetivo; '
    '(6) matriz de al menos ocho pruebas propuestas con fixture, pasos y aserción concreta; '
    '(7) tres cambios mínimos priorizados con riesgos y compatibilidad; '
    '(8) contradicciones o correcciones de la revisión cruzada y límites pendientes. '
    'No inventes pruebas ejecutadas, no modifiques archivos ni repitas efectos. '
    'Entrega todos los apartados o indica claramente cuáles quedan incompletos y por qué.'
)


if __name__ == '__main__':
    os.environ['GENNIE_PROJECT_LONG'] = '1'
    probe.main()
