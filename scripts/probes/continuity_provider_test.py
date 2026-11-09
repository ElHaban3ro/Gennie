#!/usr/bin/env python3
"""Contrato de informes de la sonda optativa de continuidad."""
import unittest
import chat_provider_probe as probe


class ContinuityReportTests(unittest.TestCase):
    def report(self, mode):
        cases = {}
        for key in ('two_sources', 'fallback'):
            trace = [] if mode == 'protocol' else [{'event': 'reply', 'text': 'Respuesta incompleta'}]
            cases[key] = {'status': 'completed', 'schemas_exact': True, 'socket_removed': True,
                          'thread_id': key, 'trace': trace,
                          'response': None if mode == 'protocol' else 'Respuesta incompleta'}
        return {'mode': mode, 'scenario': 'continuity', 'status': 'completed',
                'profile_removed': True, 'guide_calls': 0, 'model_turns': 0 if mode == 'protocol' else 2,
                'protocol': {'account_linked': False, 'generation_turns': 0, 'schemas_exact': True}, **cases}

    def test_protocol_never_contains_calls_or_generation(self):
        report = self.report('protocol')
        self.assertTrue(probe.continuity_report_matches(report, 'protocol'))
        report['two_sources']['trace'] = [{'event': 'call', 'target': 'alpha'}]
        self.assertFalse(probe.continuity_report_matches(report, 'protocol'))
        report = self.report('protocol')
        report['protocol']['generation_turns'] = 1
        self.assertFalse(probe.continuity_report_matches(report, 'protocol'))

    def test_incomplete_behaviour_is_valid_measurement_not_success(self):
        report = self.report('live')
        self.assertTrue(probe.continuity_report_matches(report, 'live'))
        self.assertFalse(probe.continuity_baseline.evaluate('two_sources', report['two_sources']['trace'])['both_sources_called'])
        report['fallback']['trace'][-1]['text'] = 'otra respuesta'
        self.assertFalse(probe.continuity_report_matches(report, 'live'))

    def test_fails_closed_on_wrong_budget_schema_or_shared_thread(self):
        for key in ('model_turns', 'schemas_exact', 'thread_id', 'socket_removed'):
            report = self.report('live')
            if key == 'model_turns': report[key] = 1
            elif key == 'thread_id': report['fallback'][key] = report['two_sources'][key]
            else: report['fallback'][key] = False
            self.assertFalse(probe.continuity_report_matches(report, 'live'))

    def test_extended_scenario_checks_its_own_cases(self):
        report = self.report('protocol')
        report['polling'] = report.pop('two_sources')
        report['delegation'] = report.pop('fallback')
        self.assertTrue(probe.continuity_report_matches(report, 'protocol', 'continuity_extended'))
        self.assertFalse(probe.continuity_report_matches(report, 'protocol', 'continuity'))
        report['delegation']['trace'] = [{'event': 'call', 'target': 'partial'}]
        self.assertFalse(probe.continuity_report_matches(report, 'protocol', 'continuity_extended'))

    def test_explicit_scenario_has_isolated_case_names(self):
        report = self.report('protocol')
        report['explicit_two_sources'] = report.pop('two_sources')
        report['explicit_polling'] = report.pop('fallback')
        self.assertTrue(probe.continuity_report_matches(report, 'protocol', 'continuity_explicit'))
        self.assertFalse(probe.continuity_report_matches(report, 'protocol', 'continuity'))

    def test_stall_scenario_cannot_pass_without_both_cases(self):
        report = self.report('protocol')
        report['stalled'] = report.pop('two_sources')
        report['explicit_delegation'] = report.pop('fallback')
        self.assertTrue(probe.continuity_report_matches(report, 'protocol', 'continuity_stall'))
        del report['stalled']
        self.assertFalse(probe.continuity_report_matches(report, 'protocol', 'continuity_stall'))

    def test_correction_requires_one_thread_and_two_distinct_turns(self):
        report = self.report('protocol')
        report['correction'] = report.pop('two_sources')
        report.pop('fallback')
        self.assertTrue(probe.continuity_report_matches(report, 'protocol', 'continuity_correction'))
        report['model_turns'] = 2
        report['correction']['response'] = 'SOL-71'
        report['correction']['first_response'] = 'LIMA-42'
        report['correction']['trace'] = [{'event':'reply','text':'LIMA-42'},
                                         {'event':'correction'}, {'event':'reply','text':'SOL-71'}]
        self.assertTrue(probe.continuity_report_matches(report, 'live', 'continuity_correction'))
        report['correction']['trace'].pop(1)
        self.assertFalse(probe.continuity_report_matches(report, 'live', 'continuity_correction'))

    def test_comparison_requires_four_isolated_cases_and_four_turns(self):
        report = self.report('protocol')
        report.pop('two_sources')
        report.pop('fallback')
        for key in probe.CONTINUITY_CASES['continuity_compare']:
            report[key] = {'status':'completed','schemas_exact':True,'socket_removed':True,
                           'thread_id':key,'trace':[],'response':None}
        self.assertTrue(probe.continuity_report_matches(report, 'protocol', 'continuity_compare'))
        report['model_turns'] = 4
        for key in probe.CONTINUITY_CASES['continuity_compare']:
            report[key]['response'] = 'Sin datos'
            report[key]['trace'] = [{'event':'reply','text':'Sin datos'}]
        self.assertTrue(probe.continuity_report_matches(report, 'live', 'continuity_compare'))
        report['compare_b2']['thread_id'] = report['compare_b1']['thread_id']
        self.assertFalse(probe.continuity_report_matches(report, 'live', 'continuity_compare'))


if __name__ == '__main__':
    unittest.main()
