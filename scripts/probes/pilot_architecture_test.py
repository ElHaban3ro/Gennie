#!/usr/bin/env python3
import unittest
import pilot_architecture as architecture


GOOD = {'source':'main','fields':{'emails':{'selector':'a.mail','attribute':'href'}}}
BAD = {'source':'main','fields':{'emails':{'selector':'a.wrong','attribute':'href'}}}


class ArchitectureTests(unittest.TestCase):
    def test_parent_validates_real_output_not_executor_claim(self):
        seen = []
        def agent(role,payload):
            seen.append((role,payload))
            return {'architect':{'objective':'extraer enlaces'},'decision':{'engineer':'web'},'planner':{'proposals':['one','two']},
                    'executor':{'recipe':GOOD,'verdict':'done'},
                    'parent_validator':{'accepted':True}}[role]
        result = architecture.run('simple',agent)
        self.assertEqual(result['status'],'completed')
        self.assertEqual([r for r,_ in seen],['architect','decision','planner','executor','parent_validator'])
        self.assertTrue(seen[-1][1]['observed']['passed'])

    def test_repeated_failure_switches_proposal_and_preserves_diagnosis(self):
        seen = []
        def agent(role,payload):
            seen.append((role,payload))
            if role == 'architect': return {'objective':'extraer enlaces'}
            if role == 'decision': return {'engineer':'web'}
            if role == 'planner': return {'proposals':['first','second']}
            if role == 'executor': return {'recipe':GOOD if payload['proposal']=='second' else BAD,'verdict':'done'}
            if role == 'diagnostician': return {'diagnosis':'selector did not match'}
            if role == 'stagnation_decision': return {'stalled':payload['same_error_count'] >= 2}
            if role == 'parent_validator': return {'accepted':True}
            raise AssertionError(role)
        result = architecture.run('simple',agent)
        self.assertEqual(result['status'],'completed')
        self.assertEqual([a['proposal_index'] for a in result['attempts']],[0,0,1])
        final_executor = [payload for role,payload in seen if role=='executor'][-1]
        self.assertEqual(final_executor['previous_diagnoses'],['selector did not match']*2)

    def test_no_valid_recipe_does_not_report_success(self):
        def agent(role,payload):
            return {'architect':{'objective':'extraer enlaces'},'decision':{'engineer':'web'},'planner':{'proposals':['one','two']},
                    'executor':{'recipe':BAD,'verdict':'done'},
                    'diagnostician':{'diagnosis':'no match'},
                    'stagnation_decision':{'stalled':payload.get('same_error_count',0) >= 2}}[role]
        result = architecture.run('simple',agent)
        self.assertEqual(result['status'],'exhausted')
        self.assertEqual(len(result['attempts']),4)

    def test_invalid_routing_and_plan_are_visible(self):
        self.assertEqual(architecture.run('simple',lambda role,payload: {'objective':'x'} if role=='architect' else {'engineer':'other'})['status'],'routing_failed')
        self.assertEqual(architecture.run('simple',lambda role,payload: {'objective':'x'} if role=='architect' else {'engineer':'web'} if role=='decision' else {'proposals':['one']})['status'],'invalid_plan')


if __name__ == '__main__':
    unittest.main()
