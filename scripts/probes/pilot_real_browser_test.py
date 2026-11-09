#!/usr/bin/env python3
import unittest
import pilot_real_browser


class RealBrowserAdapterTest(unittest.TestCase):
    def test_quality_requires_successful_verification_and_execution(self):
        expected={'emails':['mailto:ana@example.test','mailto:luis@example.test']}
        result={'verify':{'status':'ok','executed':True},'run':{'status':'ok','executed':True,'data':expected}}
        self.assertTrue(pilot_real_browser.quality('simple',result))
        result['verify']['status']='failed'
        self.assertFalse(pilot_real_browser.quality('simple',result))
        result['verify']['status']='ok'
        result['run']['executed']=False
        self.assertFalse(pilot_real_browser.quality('simple',result))

    def test_only_verified_recipe_becomes_manifest(self):
        recipe={'source':'main','fields':{'emails':{'selector':'a.mail','attribute':'href'}}}
        value=pilot_real_browser.manifest('simple',recipe,'http://127.0.0.1:8000')
        self.assertEqual(value['functions'][0]['transport'],'program')
        self.assertIn('"a.mail"',value['functions'][0]['code'])
        recipe['fields']['emails']['selector']='a;evil'
        with self.assertRaises(ValueError):
            pilot_real_browser.manifest('simple',recipe,'http://127.0.0.1:8000')


if __name__=='__main__':
    unittest.main()
