import unittest

from EasyCommandRunner import analysis

class TestStringMethods(unittest.TestCase):

    def test_analysis(self):
        self.assertEqual(analysis('aaa'), ['aaa'])
        self.assertEqual(analysis('aaa --arg0 v0 -b c'), ['aaa', '--arg0', 'v0', '-b', 'c'])
        self.assertEqual(analysis('aaa -x -y'), ['aaa', '-x', '', '-y', ''])
