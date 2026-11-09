#!/usr/bin/env python3
"""Pruebas offline de agregación y fallo transitorio de la comparación."""
import os
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

import extended_web_comparison as comparison
import real_web_code_mcp as mcp


class ExtendedComparisonTest(unittest.TestCase):
    def test_aggregation_counts_distinct_authors_and_quote_positions(self):
        rows = [
            {'text': 'a', 'author': 'A', 'tags': ['life', 'x']},
            {'text': 'b', 'author': 'B', 'tags': ['x']},
            {'text': 'c', 'author': 'A', 'tags': ['life', 'x']},
        ]
        result = comparison.aggregate(rows)
        self.assertEqual(result['authors'], [{'author': 'A', 'count': 2}, {'author': 'B', 'count': 1}])
        self.assertEqual(result['tags'], [
            {'tag': 'life', 'count': 2, 'authors': ['A']},
            {'tag': 'x', 'count': 3, 'authors': ['A', 'B']},
        ])
        self.assertEqual(result['life'], [1, 3])

    def test_failure_is_injected_once_across_calls(self):
        with tempfile.TemporaryDirectory() as directory, \
             patch.dict(os.environ, {'GENNIE_EXTENDED_TRANSIENT': '1'}), \
             patch.object(mcp.pilot_native_mcp, 'reserve_recipe', return_value=None), \
             patch.object(mcp.real_web_code_sandbox, 'execute', return_value={
                 'exit_code': 0, 'output': {'ok': True}, 'elapsed_ms': 1}):
            trace = str(Path(directory) / 'trace')
            args = {'source': 'console.log(1)'}
            first = mcp.run_tool('run_javascript', args, 1, trace, {}, {'ok': True})
            second = mcp.run_tool('run_javascript', args, 1, trace, {}, {'ok': True})
            self.assertEqual(first['status'], 'transient_unavailable')
            self.assertEqual(second['status'], 'verified')


if __name__ == '__main__':
    unittest.main()
