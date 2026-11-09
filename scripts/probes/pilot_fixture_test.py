#!/usr/bin/env python3
import unittest
import pilot_fixture as fixture


class FixtureTests(unittest.TestCase):
    def recipe(self, source='main'):
        return {'source':source,'fields':{'emails':{'selector':'a.mail','attribute':'href'}}}

    def test_simple_requires_all_values(self):
        self.assertTrue(fixture.verify('simple',self.recipe())['passed'])
        wrong = self.recipe()
        wrong['fields']['emails']['selector'] = 'a.other'
        self.assertFalse(fixture.verify('simple',wrong)['passed'])

    def test_fallback_requires_available_source(self):
        self.assertEqual(fixture.inspect('fallback','main')['status'],'unavailable')
        recipe = self.recipe('backup')
        recipe['fields']['emails']['selector'] = 'a.contact'
        self.assertTrue(fixture.verify('fallback',recipe)['passed'])
        self.assertFalse(fixture.verify('fallback',self.recipe())['passed'])

    def test_compound_requires_both_fields_and_exact_values(self):
        recipe = self.recipe()
        self.assertFalse(fixture.verify('compound',recipe)['passed'])
        recipe['fields']['dates'] = {'selector':'time.date','attribute':'datetime'}
        self.assertTrue(fixture.verify('compound',recipe)['passed'])

    def test_invalid_recipes_do_not_execute_code(self):
        for bad in ({}, {'source':'main','fields':{'x':{'selector':'*','attribute':'href'}}},
                    {'source':'main','fields':{'x':{'selector':'a.mail','attribute':'__class__'}}}):
            self.assertEqual(fixture.verify('simple',bad)['status'],'invalid_recipe')


if __name__ == '__main__':
    unittest.main()
