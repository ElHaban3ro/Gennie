#!/usr/bin/env python3
import unittest
import pilot_codex_direct as direct


class DirectTests(unittest.TestCase):
    def test_last_usage_and_unknown_are_not_zero(self):
        self.assertIsNone(direct.extract_usage([]))
        events = [{'type':'turn.completed','usage':{'input_tokens':8,'output_tokens':2}},
                  {'type':'turn.completed','usage':{'input_tokens':10,'output_tokens':3,'cached_input_tokens':True,'secret':'x'}}]
        self.assertEqual(direct.extract_usage(events),{'input_tokens':10,'output_tokens':3})


if __name__ == '__main__':
    unittest.main()
