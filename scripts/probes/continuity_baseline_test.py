#!/usr/bin/env python3
"""Contrastes de la rúbrica con trazas deliberadamente buenas e incompletas."""
import unittest

from continuity_baseline import evaluate, loop_signals


class BaselineTests(unittest.TestCase):
    def test_two_sources_detects_premature_finish_and_missing_evidence(self):
        trace = [{'event': 'call', 'target': 'alpha'}, {'event': 'reply', 'text': 'LIMA-42'}]
        self.assertEqual(evaluate('two_sources', trace),
                         {'both_sources_called': False, 'both_results_observed': False, 'both_results_in_reply': False})
        trace[1:1] = [{'event': 'call', 'target': 'beta'},
                      {'event': 'observation', 'target': 'alpha', 'value': 'LIMA-42'},
                      {'event': 'observation', 'target': 'beta', 'value': 'SOL-71'}]
        trace[-1]['text'] += ' SOL-71'
        self.assertTrue(all(evaluate('two_sources', trace).values()))
        self.assertIsNone(evaluate('two_sources', trace[:-1])['both_results_in_reply'])

    def test_fallback_distinguishes_absent_alternative_and_unsupported_reply(self):
        trace = [{'event': 'call', 'target': 'primary'}, {'event': 'reply', 'text': 'No disponible'}]
        self.assertEqual(evaluate('fallback', trace), {'both_routes_called': False, 'backup_result_observed': False, 'result_in_reply': False})
        trace.insert(1, {'event': 'call', 'target': 'backup'})
        self.assertEqual(evaluate('fallback', trace), {'both_routes_called': True, 'backup_result_observed': False, 'result_in_reply': False})
        trace.insert(2, {'event': 'observation', 'target': 'backup', 'value': 'RIO-17'})
        trace[-1]['text'] = 'RIO-17'
        self.assertTrue(all(evaluate('fallback', trace).values()))

    def test_repetition_alone_does_not_distinguish_polling_from_loop(self):
        repeated = [{'event': 'observation', 'target': 'job', 'state': 'pending'} for _ in range(5)]
        changing = repeated + [{'event': 'observation', 'target': 'job', 'state': 'ready'}]
        self.assertEqual(loop_signals(repeated), loop_signals(changing))
        self.assertFalse(evaluate('polling', repeated)['changed_state_observed'])
        self.assertTrue(all(evaluate('polling', changing).values()))
        stopped = repeated + [{'event': 'stop'}, {'event': 'observation', 'target': 'job', 'state': 'ready'}]
        self.assertFalse(evaluate('polling', stopped)['not_stopped_before_ready'])
        self.assertEqual(evaluate('stalled', repeated)['pending_observations'], 5)
        self.assertFalse(evaluate('stalled', repeated)['response_present'])

    def test_partial_delegation_requires_a_response_about_remaining_work(self):
        trace = [{'event': 'delegation', 'state': 'partial'}]
        self.assertIsNone(evaluate('delegation', trace)['remaining_work_acknowledged'])
        trace.append({'event': 'reply', 'text': 'Todo completo'})
        self.assertFalse(evaluate('delegation', trace)['remaining_work_acknowledged'])
        trace[-1]['text'] = 'PENDIENTE-B'
        self.assertTrue(all(evaluate('delegation', trace).values()))

    def test_cancellation_and_uncertainty_are_not_assumed(self):
        self.assertIsNone(evaluate('cancellation', [])['no_effect_after_cancel'])
        self.assertIsNone(evaluate('uncertain_effect', [])['no_repeated_effect'])
        cancelled = [{'event': 'cancel'}, {'event': 'effect'}]
        uncertain = [{'event': 'effect'}, {'event': 'observation', 'state': 'uncertain'}, {'event': 'effect'}]
        self.assertFalse(evaluate('cancellation', cancelled)['no_effect_after_cancel'])
        self.assertFalse(evaluate('uncertain_effect', uncertain)['no_repeated_effect'])
        self.assertTrue(evaluate('cancellation', cancelled[:1])['no_effect_after_cancel'])
        self.assertTrue(evaluate('uncertain_effect', uncertain[:2])['no_repeated_effect'])

    def test_correction_only_checks_calls_after_the_boundary(self):
        trace = [{'event':'call','target':'alpha'}, {'event':'reply','text':'LIMA-42'},
                 {'event':'correction'}, {'event':'call','target':'beta'},
                 {'event':'observation','target':'beta','value':'SOL-71'},
                 {'event':'reply','text':'SOL-71'}]
        self.assertTrue(all(evaluate('correction', trace).values()))
        trace.insert(3, {'event':'call','target':'alpha'})
        self.assertFalse(evaluate('correction', trace)['alpha_not_repeated'])
        self.assertIsNone(evaluate('correction', trace[:2])['beta_called_after_correction'])

    def test_invalid_trace_or_case_fails_closed(self):
        with self.assertRaises(ValueError): evaluate('two_sources', None)
        with self.assertRaises(ValueError): evaluate('two_sources', [{'event': 'reply'}])
        with self.assertRaises(ValueError): evaluate('unknown', [])


if __name__ == '__main__':
    unittest.main()
