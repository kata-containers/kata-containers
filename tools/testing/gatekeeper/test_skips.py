#!/usr/bin/env python3
#
# Copyright (c) 2026 IBM Corporation
#
# SPDX-License-Identifier: Apache-2.0

"""Tests for how skips.py finds the files changed by a branch"""

import contextlib
import io
import os
import subprocess
import tempfile
import unittest

import skips


class ChangedFilesTests(unittest.TestCase):
    """Run against a temporary repository whose origin/main is a local ref"""

    def setUp(self):
        tempdir = tempfile.TemporaryDirectory()
        self.addCleanup(tempdir.cleanup)
        self.repo = tempdir.name
        self.git("init", "-b", "main")
        self.git("config", "user.name", "Skips test")
        self.git("config", "user.email", "skips@example.invalid")
        self.commit({"README": "base\n"})
        self.set_origin_main()
        self.git("checkout", "-b", "topic")
        previous = os.getcwd()
        os.chdir(self.repo)
        self.addCleanup(os.chdir, previous)

    def git(self, *args):
        return subprocess.check_output(["git", "-C", self.repo, *args],
                                       stderr=subprocess.DEVNULL)

    def write(self, name, content="fixture\n"):
        path = os.path.join(self.repo, name)
        os.makedirs(os.path.dirname(path), exist_ok=True)
        with open(path, "w", encoding="utf8") as fd:
            fd.write(content)

    def commit(self, files):
        for name, content in files.items():
            self.write(name, content)
        self.git("add", ".")
        self.git("commit", "-m", "test: fixture")

    def set_origin_main(self):
        self.git("update-ref", "refs/remotes/origin/main", "HEAD")

    def features(self):
        # get_features() prints the changed files to stderr
        with contextlib.redirect_stderr(io.StringIO()):
            return skips.Checks().get_features("main")

    def test_branch_changes_are_reported(self):
        self.commit({"docs/a.md": "a\n", "src/agent/b.rs": "b\n"})
        self.assertEqual(sorted(skips.changed_files("main")),
                         ["docs/a.md", "src/agent/b.rs"])

    def test_no_changes(self):
        self.assertEqual(skips.changed_files("main"), [])

    def test_upstream_changes_are_not_counted(self):
        self.commit({"docs/a.md": "a\n"})
        self.git("checkout", "main")
        self.commit({"src/agent/upstream.rs": "upstream\n"})
        self.set_origin_main()
        self.git("checkout", "topic")
        self.assertEqual(skips.changed_files("main"), ["docs/a.md"])
        self.assertEqual(self.features(), {"docs"})

    def test_working_tree_edits_are_ignored(self):
        self.commit({"docs/a.md": "a\n"})
        self.write("src/agent/untracked.rs")
        self.write("README", "modified but not staged\n")
        self.write("docs/a.md", "modified but not staged\n")
        self.assertEqual(skips.changed_files("main"), ["docs/a.md"])
        self.assertEqual(self.features(), {"docs"})

    def test_rename_reports_both_sides(self):
        self.commit({"docs/a.md": "same content\n"})
        self.set_origin_main()
        os.makedirs(os.path.join(self.repo, "src/agent"))
        self.git("mv", "docs/a.md", "src/agent/a.rs")
        self.git("commit", "-m", "test: move")
        self.assertEqual(sorted(skips.changed_files("main")),
                         ["docs/a.md", "src/agent/a.rs"])
        # The unmapped destination makes the move run everything
        self.assertEqual(self.features(), skips.Checks().all_set_of_tests)

    def test_unusual_file_names_are_not_mangled(self):
        # Without -z git quotes names like these
        name = "docs/caf\u00e9 \"quoted\".md"
        self.commit({name: "a\n"})
        self.assertEqual(skips.changed_files("main"), [name])


if __name__ == "__main__":
    unittest.main()
