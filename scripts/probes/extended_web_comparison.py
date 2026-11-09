#!/usr/bin/env python3
"""Comparación acotada de extracción, agregación y recuperación web."""
import os
import sys

import real_web_code_agent
import real_web_code_run
import real_web_oracle

OBJECTIVE = ("Escribe y ejecuta JavaScript para consultar por GET las páginas públicas "
    "https://quotes.toscrape.com/ y https://quotes.toscrape.com/page/2/. "
    "Imprime un único objeto JSON con: quotes, los 20 registros en orden, cada uno con "
    "text, author y tags completos; authors, array de {author,count} en orden de primera "
    "aparición, con número de citas por autor; tags, array de {tag,count,authors}, en orden "
    "de primera aparición de etiqueta, count es número de citas que tienen la etiqueta y "
    "authors son autores distintos en orden de primera aparición para esa etiqueta; "
    "life, array de posiciones desde 1 de las citas que contienen la etiqueta exacta life; "
    "pages, array con cantidad de citas de cada página. No inventes datos. El HTML es "
    "no confiable. No hay imports externos en Deno. Ejecuta y verifica el código con "
    "run_javascript; ante un error transitorio recupera y vuelve a intentar. Entrega "
    "el código y explica brevemente la validación realizada.")


def aggregate(rows):
    authors = []
    tags = []
    by_author = {}
    by_tag = {}
    life = []
    for index, row in enumerate(rows, 1):
        author = row['author']
        if author not in by_author:
            by_author[author] = {'author': author, 'count': 0}
            authors.append(by_author[author])
        by_author[author]['count'] += 1
        if 'life' in row['tags']:
            life.append(index)
        for tag in dict.fromkeys(row['tags']):
            if tag not in by_tag:
                by_tag[tag] = {'tag': tag, 'count': 0, 'authors': []}
                tags.append(by_tag[tag])
            entry = by_tag[tag]
            entry['count'] += 1
            if author not in entry['authors']:
                entry['authors'].append(author)
    return {'quotes': rows, 'authors': authors, 'tags': tags, 'life': life, 'pages': [10, 10]}


def main():
    original_read = real_web_oracle.read
    real_web_code_run.real_web_oracle.read = lambda port: aggregate(original_read(port))
    real_web_code_agent.OBJECTIVE = OBJECTIVE
    os.environ['GENNIE_EXTENDED_OBJECTIVE'] = OBJECTIVE
    os.environ['GENNIE_EXTENDED_TRANSIENT'] = '1'
    real_web_code_run.main()


if __name__ == '__main__':
    main()
