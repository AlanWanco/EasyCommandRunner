import unittest

from EasyCommandRunner import analysis

class TestStringMethods(unittest.TestCase):

    def test_analysis(self):
        self.assertEqual(analysis('aaa'), ['aaa'])
        self.assertEqual(
            analysis('aaa --arg0 v0 -b c'),
            ['aaa', '--arg0', 'v0', '-b', 'c'])
        self.assertEqual(analysis('aaa -x -y'), ['aaa', '-x', '', '-y', ''])

    def test_analysis_append(self):
        self.assertEqual(
            analysis('aaa --arg0 v0 -b c', True),
            ['aaa', '', '--arg0', 'v0', '-b', 'c'])
        self.assertEqual(analysis('-x -y', True), ['-x', '', '-y', ''])

    def test_analysis_with_quotes(self):
        self.assertEqual(analysis('aaa "a" "bc"'), ['aaa', '"a"', '"bc"'])
        self.assertEqual(
            analysis('aaa -a "12 34" -b "56 78" "xxx x"'),
            ['aaa', '-a', '"12 34"', '-b', '"56 78"', '"xxx x"', ''])

        self.assertEqual(
            analysis('aaa -x "1 2 3" -y -z uvw'),
            ['aaa', '-x', '"1 2 3"', '-y', '', '-z', 'uvw'])

    def test_analysis_append_with_quotes(self):
        self.assertEqual(analysis('"a" "bc"', True), ['"a"', '"bc"'])
        self.assertEqual(
            analysis(' -a "12 34" -b "56 78" "xxx x"', True),
            ['-a', '"12 34"', '-b', '"56 78"', '"xxx x"', ''])
        self.assertEqual(
            analysis(' -x "1 2 3" -y -z uvw', True),
            ['-x', '"1 2 3"', '-y', '', '-z', 'uvw'])

