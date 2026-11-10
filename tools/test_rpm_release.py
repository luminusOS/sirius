import pathlib
import shutil
import subprocess
import sys
import unittest

sys.path.insert(0, str(pathlib.Path(__file__).parent))
from rpm_release import derive  # noqa: E402


def vercmp(first, second):
    script = f"%{{lua: print(rpm.vercmp('{first}', '{second}'))}}"
    return int(subprocess.run(["rpm", "--eval", script], capture_output=True, text=True, check=True).stdout)


class DeriveTests(unittest.TestCase):
    def test_final_and_candidate_tags(self):
        self.assertEqual(derive("v1.2.3", "1.2.3"), ("1.2.3", "1"))
        self.assertEqual(derive("v1.2.3-rc.1", "1.2.3"), ("1.2.3~rc.1", "1"))
        self.assertEqual(derive("v1.2.3-rc2", "1.2.3"), ("1.2.3~rc.2", "1"))

    def test_tag_and_workspace_version_must_agree(self):
        with self.assertRaises(ValueError):
            derive("v1.2.4", "1.2.3")

    def test_malformed_tags_are_rejected(self):
        for tag in ("1.2.3", "v1.2", "v1.2.3-beta.1", "v01.2.3", "v1.2.3-rc.0", "v1.2.3-rc.1-x", "v1.2.3 "):
            with self.assertRaises(ValueError, msg=tag):
                derive(tag, "1.2.3")


@unittest.skipUnless(shutil.which("rpm"), "rpm is required")
class OrderingTests(unittest.TestCase):
    def evr(self, tag, base):
        version, release = derive(tag, base)
        return f"{version}-{release}"

    def test_candidates_sort_before_the_final_release(self):
        self.assertEqual(vercmp(self.evr("v1.2.3-rc.1", "1.2.3"), self.evr("v1.2.3", "1.2.3")), -1)
        self.assertEqual(vercmp(self.evr("v1.2.3-rc.2", "1.2.3"), self.evr("v1.2.3-rc.10", "1.2.3")), -1)

    def test_a_newer_candidate_sorts_after_the_older_final_release(self):
        self.assertEqual(vercmp(self.evr("v1.2.4-rc.1", "1.2.4"), self.evr("v1.2.3", "1.2.3")), 1)


if __name__ == "__main__":
    unittest.main()
