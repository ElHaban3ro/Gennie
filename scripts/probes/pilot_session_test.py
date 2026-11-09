#!/usr/bin/env python3
import unittest
import pilot_session


class SessionTests(unittest.TestCase):
    def test_only_last_complete_message_is_used(self):
        self.assertEqual(pilot_session.parse_final(['{"status":"working"}','{"recipe":{}}']),{'recipe':{}})
        self.assertIsNone(pilot_session.parse_final(['{"status":"working"}','not json']))
        self.assertIsNone(pilot_session.parse_final(['[]']))
        self.assertIsNone(pilot_session.parse_final([]))

    def test_usage_is_bounded_and_retains_unknowns(self):
        result = pilot_session.usage_snapshot({'last':{'inputTokens':3,'cacheWriteInputTokens':0,'outputTokens':True,'secret':'x'},'total':{'totalTokens':5}})
        self.assertEqual(result,{'last':{'inputTokens':3,'cacheWriteInputTokens':0},'total':{'totalTokens':5}})
        self.assertIsNone(pilot_session.usage_snapshot(None))


if __name__ == '__main__':
    unittest.main()
