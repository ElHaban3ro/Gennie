#!/usr/bin/env python3
"""Tarea web más amplia, con recuperación y variaciones de HTML equivalentes."""
import os

import extended_web_comparison as extended
import real_web_code_agent
import real_web_code_run
import real_web_oracle

OBJECTIVE = (extended.OBJECTIVE +
    " El código también debe seguir funcionando cuando se reordenen atributos HTML, "
    "se añadan clases CSS no relacionadas o se usen comillas simples en atributos class. "
    "La herramienta evalúa esas tres variantes con los mismos datos y reporta qué falla. "
    "Comprueba todos los requisitos y corrige cualquier fallo dentro del presupuesto disponible; "
    "informa con precisión lo que lograste y lo que quede pendiente.")


def main():
    original = real_web_oracle.read
    real_web_code_run.real_web_oracle.read = lambda port: extended.aggregate(original(port))
    real_web_code_agent.OBJECTIVE = OBJECTIVE
    os.environ['GENNIE_EXTENDED_OBJECTIVE'] = OBJECTIVE
    os.environ['GENNIE_EXTENDED_TRANSIENT'] = '1'
    os.environ['GENNIE_LONG_ROBUST'] = '1'
    os.environ['GENNIE_REAL_WEB_SCOPE_SECONDS'] = '180'
    real_web_code_run.main()


if __name__ == '__main__':
    main()
