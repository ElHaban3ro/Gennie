#!/usr/bin/env python3
import urllib.error
import urllib.request
import unittest
import pilot_web_fixture


class WebFixtureTest(unittest.TestCase):
    def test_fallback_and_cleanup(self):
        with pilot_web_fixture.serve('fallback') as site:
            with self.assertRaises(urllib.error.HTTPError) as error:
                urllib.request.urlopen(site['origin']+'/main',timeout=2)
            self.assertEqual(error.exception.code,503)
            with urllib.request.urlopen(site['origin']+'/backup',timeout=2) as response:
                self.assertIn('mailto:eva@example.test',response.read().decode())
            self.assertEqual([x['path'] for x in site['accesses']],['/main','/backup'])
            self.assertEqual(pilot_web_fixture.inspect(site['origin'],'main')['status'],'unavailable')
            self.assertIn('mailto:eva@example.test',pilot_web_fixture.inspect(site['origin'],'backup')['html'])
            with self.assertRaises(ValueError):
                pilot_web_fixture.inspect('http://example.com:80','main')
        with self.assertRaises(urllib.error.URLError):
            urllib.request.urlopen(site['origin']+'/backup',timeout=2)


if __name__ == '__main__':
    unittest.main()
