#!/usr/bin/env python3
"""Comprueba que fallar robustez impide aprobar el programa."""
import os
import tempfile
import unittest
from unittest.mock import patch

import long_web_robustness
import real_web_code_mcp as mcp


class RobustnessGateTest(unittest.TestCase):
    def test_failed_variant_cannot_pass_even_if_original_matches(self):
        expected = {'quotes': []}
        execution = {'exit_code': 0, 'output': expected, 'elapsed_ms': 1}
        variants = {'attribute_order': True, 'extra_class': False, 'single_quotes': True}
        with tempfile.TemporaryDirectory() as root, \
             patch.dict(os.environ, {'GENNIE_LONG_ROBUST': '1', 'GENNIE_EXTENDED_TRANSIENT': '0'}), \
             patch.object(mcp.pilot_native_mcp, 'reserve_recipe', return_value=None), \
             patch.object(mcp.real_web_code_sandbox, 'execute', return_value=execution), \
             patch.object(long_web_robustness, 'check', return_value=variants):
            result = mcp.run_tool('run_javascript', {'source': 'x'}, 1, root + '/trace', {}, expected)
        self.assertFalse(result['passed'])
        self.assertEqual(result['status'], 'mismatch')
        self.assertEqual(result['robustness'], variants)


if __name__ == '__main__':
    unittest.main()
