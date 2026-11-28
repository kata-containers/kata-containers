#!/usr/bin/env python3
#
# Copyright (c) 2026 IBM Corporation
#
# SPDX-License-Identifier: Apache-2.0

"""Tests for how the files changed by a branch select the tests to run"""

import contextlib
import copy
import io
import os
import subprocess
import tempfile
import unittest
from unittest import mock

import selection
import skips


def make_config():
    """A small configuration, so the tests do not depend on the real rules"""
    return {
        "paths": [
            {"paths": ["ci/*"], "features": []},
            {"paths": ["docs/*.md", "*.md"], "features": ["docs"]},
            {"paths": ["docs/*"], "features": ["static"]},
            {"paths": ["src/a/*"], "features": ["build", "test"]},
        ],
        "mapping": {"build": {}, "docs": {}, "static": {}, "test": {}},
    }


class ValidateConfigTests(unittest.TestCase):
    def assert_rejected(self, config, message):
        with self.assertRaisesRegex(ValueError, message):
            selection.validate_config(config)

    def test_valid_configuration(self):
        selection.validate_config(make_config())

    def test_not_a_mapping(self):
        self.assert_rejected(["paths"], "must be a mapping")
        self.assert_rejected(None, "must be a mapping")

    def test_missing_or_empty_sections(self):
        for key in ("paths", "mapping"):
            config = make_config()
            del config[key]
            self.assert_rejected(config, f"'{key}' is missing or empty")
            config[key] = []
            self.assert_rejected(config, f"'{key}' is missing or empty")

    def test_rule_must_be_a_mapping_with_exactly_the_known_keys(self):
        for rule in ("docs/*", ["docs/*"], {"paths": ["docs/*"]},
                     {"features": ["docs"]},
                     {"paths": ["docs/*"], "features": [], "extra": 1},
                     # The old schema, a regexp mapped to features
                     {"^docs/": ["docs"]}):
            config = make_config()
            config["paths"].append(rule)
            self.assert_rejected(config, "paths rule 5 must have exactly")

    def test_rule_values_must_be_lists_of_strings(self):
        for key in ("paths", "features"):
            for value in ("docs", None, [1], [["docs"]]):
                config = make_config()
                config["paths"].append({
                    "paths": ["docs/*"], "features": ["docs"], **{key: value}})
                self.assert_rejected(config, f"'{key}' must be a list of strings")

    def test_rule_needs_a_pattern(self):
        config = make_config()
        config["paths"].append({"paths": [], "features": ["docs"]})
        self.assert_rejected(config, "paths rule 5 has no patterns")

    def test_unknown_feature_is_rejected(self):
        config = make_config()
        # A misspelt feature would silently drop that feature from the selection
        config["paths"].append({"paths": ["x/*"], "features": ["missing"]})
        self.assert_rejected(config, r"unknown features \['missing'\]")

    def test_load_config_validates(self):
        with tempfile.TemporaryDirectory() as tempdir:
            path = os.path.join(tempdir, "required-tests.yaml")
            with open(path, "w", encoding="utf8") as fd:
                fd.write("paths:\n  - paths: ['x/*']\n    features: ['nope']\n"
                         "mapping:\n  build: {}\n")
            with self.assertRaisesRegex(ValueError, "unknown features"):
                selection.load_config(path)

    def test_load_config_returns_the_parsed_file(self):
        with tempfile.TemporaryDirectory() as tempdir:
            path = os.path.join(tempdir, "required-tests.yaml")
            with open(path, "w", encoding="utf8") as fd:
                fd.write("paths:\n  - paths: ['x/*']\n    features: ['build']\n"
                         "mapping:\n  build: {}\n")
            self.assertEqual(selection.load_config(path), {
                "paths": [{"paths": ["x/*"], "features": ["build"]}],
                "mapping": {"build": {}}})

    def test_shipped_configuration_is_valid(self):
        config = selection.load_config()
        self.assertEqual(set(config["mapping"]),
                         {"build", "docs", "static", "test"})


class GetFeaturesTests(unittest.TestCase):
    def setUp(self):
        self.config = make_config()

    def features(self, *changed):
        return selection.get_features(list(changed), self.config)

    def test_no_changes_select_nothing(self):
        self.assertEqual(self.features(), set())

    def test_rule_selects_its_features(self):
        self.assertEqual(self.features("src/a/x.rs"), {"build", "test"})

    def test_rule_without_features_selects_nothing(self):
        self.assertEqual(self.features("ci/x"), set())

    def test_first_matching_rule_wins(self):
        # Matches the docs and the static rule, and only docs may apply
        self.assertEqual(self.features("docs/a.md"), {"docs"})
        self.assertEqual(self.features("docs/a.sh"), {"static"})

    def test_any_pattern_of_a_rule_matches(self):
        self.assertEqual(self.features("README.md"), {"docs"})

    def test_features_of_several_files_are_combined(self):
        self.assertEqual(self.features("docs/a.md", "docs/a.sh", "src/a/x"),
                         {"docs", "static", "build", "test"})

    def test_star_matches_path_separators(self):
        self.assertEqual(self.features("src/a/b/c/d.rs"), {"build", "test"})

    def test_matching_is_anchored_and_case_sensitive(self):
        everything = set(self.config["mapping"])
        self.assertEqual(self.features("x/ci/y"), everything)
        self.assertEqual(self.features("CI/x"), everything)
        self.assertEqual(self.features("src/A/x"), everything)

    def test_unmapped_file_selects_everything(self):
        everything = set(self.config["mapping"])
        self.assertEqual(self.features("src/b/x"), everything)

    def test_unmapped_file_wins_over_narrow_ones(self):
        everything = set(self.config["mapping"])
        self.assertEqual(self.features("docs/a.md", "src/b/x", "ci/x"),
                         everything)
        self.assertEqual(self.features("src/b/x", "docs/a.md"), everything)

    def test_configuration_is_not_modified(self):
        before = copy.deepcopy(self.config)
        self.features("docs/a.md", "src/b/x")
        self.assertEqual(self.config, before)

    def test_result_is_not_the_configuration_keys(self):
        # Callers update the result, which must not alter the mapping
        result = self.features("src/b/x")
        result.add("another")
        self.assertNotIn("another", self.config["mapping"])


class ShippedRulesTests(unittest.TestCase):
    """The rules in required-tests.yaml, which the other tests do not use"""

    EXPECTED = {
        "ci/openshift-ci/x": set(),
        ".claude/skills/x/SKILL.sh": set(),
        ".github/workflows/static-checks.yaml": {"static"},
        ".github/workflows/static-checks-self-hosted.yaml": {"static"},
        ".github/workflows/other.yaml": {"build"},
        "docs/how-to/a.md": {"docs"},
        "docs/how-to/pic.png": {"docs"},
        "docs/.nav.yml": {"docs"},
        "mkdocs.yaml": {"docs"},
        "README.md": {"docs"},
        "tools/packaging/kernel/README.md": {"docs"},
        "tests/spellcheck/kata-dictionary.txt": {"docs"},
        "docs/Dockerfile": {"static"},
        "docs/requirements.txt": {"static"},
        "docs/how-to/offline_cpu.sh": {"static"},
        "docs/how-to/data/dashboard.json": {"static"},
        # first match wins, so the .md rule beats the .claude one
        ".claude/skills/x/SKILL.md": {"docs"},
    }

    def setUp(self):
        self.config = selection.load_config()

    def test_paths(self):
        for path, expected in self.EXPECTED.items():
            with self.subTest(path=path):
                self.assertEqual(selection.get_features([path], self.config),
                                 expected)

    def test_unmapped_paths_select_everything(self):
        everything = set(self.config["mapping"])
        for path in ("src/agent/x.rs", "versions.yaml", "tools/testing/x",
                     "tests/integration/kubernetes/x.bats", "Makefile",
                     "docs", "mkdocs.yml"):
            with self.subTest(path=path):
                self.assertEqual(selection.get_features([path], self.config),
                                 everything)

    def test_docs_with_a_source_change_selects_everything(self):
        self.assertEqual(
            selection.get_features(["docs/a.md", "src/agent/x.rs"],
                                   self.config),
            set(self.config["mapping"]))

    def test_tests_are_never_selected_without_the_build(self):
        # kata-containers-ci-on-push is skipped with the build, so a test
        # name could never be satisfied without it
        paths = list(self.EXPECTED) + ["src/x", "versions.yaml"]
        for path in paths:
            features = selection.get_features([path], self.config)
            with self.subTest(path=path):
                self.assertTrue("test" not in features or "build" in features)


class SkipsOutputTests(unittest.TestCase):
    """What skips.py prints, which the workflows consume"""

    def run_skips(self, changed, tests):
        stdout = io.StringIO()
        with mock.patch.object(selection, "changed_files",
                               return_value=changed), \
                contextlib.redirect_stdout(stdout), \
                contextlib.redirect_stderr(io.StringIO()):
            self.assertEqual(skips.Checks().run(tests, "main"), 0)
        return stdout.getvalue().splitlines()

    def test_skip_outputs_for_docs_only(self):
        self.assertEqual(self.run_skips(["docs/a.md"], False), [
            "skip_build=yes", "skip_docs=no", "skip_static=yes",
            "skip_test=yes"])

    def test_skip_outputs_for_a_static_only_workflow(self):
        self.assertEqual(
            self.run_skips([".github/workflows/static-checks.yaml"], False), [
                "skip_build=yes", "skip_docs=yes", "skip_static=no",
                "skip_test=yes"])

    def test_skip_outputs_for_other_workflows_keep_the_build(self):
        self.assertEqual(
            self.run_skips([".github/workflows/other.yaml"], False), [
                "skip_build=no", "skip_docs=yes", "skip_static=yes",
                "skip_test=yes"])

    def test_skip_outputs_for_nothing_to_run(self):
        self.assertEqual(self.run_skips(["ci/openshift-ci/x"], False), [
            "skip_build=yes", "skip_docs=yes", "skip_static=yes",
            "skip_test=yes"])

    def test_skip_outputs_for_a_source_change(self):
        self.assertEqual(self.run_skips(["src/agent/x.rs"], False), [
            "skip_build=no", "skip_docs=no", "skip_static=no",
            "skip_test=no"])

    def test_skip_outputs_for_no_changes(self):
        self.assertEqual(self.run_skips([], False), [
            "skip_build=yes", "skip_docs=yes", "skip_static=yes",
            "skip_test=yes"])

    def required(self, changed):
        names, regexps = self.run_skips(changed, True)
        return names.split(";") if names else [], regexps

    def test_required_names_are_sorted_and_unique(self):
        for changed in (["docs/a.md"], ["src/agent/x.rs"], []):
            names, _ = self.required(changed)
            with self.subTest(changed=changed):
                self.assertEqual(names, sorted(set(names)))

    def test_required_names_always_include_the_base_set(self):
        config = selection.load_config()
        for changed in (["docs/a.md"], ["src/agent/x.rs"], []):
            names, _ = self.required(changed)
            with self.subTest(changed=changed):
                self.assertTrue(set(config["required_tests"]) <= set(names))

    def test_required_names_for_docs_only(self):
        names, regexps = self.required(["docs/a.md"])
        self.assertIn("Spelling check / check-spelling", names)
        self.assertIn("Static checks / check-broken-symlinks", names)
        self.assertNotIn("Darwin tests / test", names)
        self.assertFalse([_ for _ in names if "build-checks" in _])
        self.assertEqual(regexps, "")

    def test_required_names_for_an_other_workflow_are_the_build_ones(self):
        names, _ = self.required([".github/workflows/other.yaml"])
        self.assertIn("Darwin tests / test", names)
        self.assertNotIn("Spelling check / check-spelling", names)

    def test_required_names_for_a_source_change(self):
        config = selection.load_config()
        expected = set(config["required_tests"])
        for feature in config["mapping"].values():
            expected.update(feature.get("names", []))
        names, _ = self.required(["src/agent/x.rs"])
        self.assertEqual(set(names), expected)

    def test_regexps_of_the_selected_features_are_required(self):
        config = {"required_tests": ["always"], "required_regexps": ["base.*"],
                  "paths": [{"paths": ["a/*"], "features": ["one"]}],
                  "mapping": {"one": {"names": ["n1"], "regexps": "r1|r2"},
                              "two": {"names": ["n2"], "regexps": "r3"}}}
        with mock.patch.object(selection, "load_config",
                               return_value=config):
            self.assertEqual(self.run_skips(["a/x"], True),
                             ["always;n1", "base.*;r1|r2"])
            self.assertEqual(self.run_skips(["b/x"], True),
                             ["always;n1;n2", "base.*;r1|r2;r3"])

    def test_changed_files_are_logged_to_stderr(self):
        stderr = io.StringIO()
        with mock.patch.object(selection, "changed_files",
                               return_value=["docs/a.md", "src/x"]), \
                contextlib.redirect_stdout(io.StringIO()), \
                contextlib.redirect_stderr(stderr):
            skips.Checks().run(False, "main")
        self.assertEqual(stderr.getvalue(), "docs/a.md\nsrc/x\n")

    def test_selector_errors_are_not_swallowed(self):
        for tests in (False, True):
            with mock.patch.object(
                    selection, "changed_files",
                    side_effect=subprocess.CalledProcessError(128, "git")):
                with self.assertRaises(subprocess.CalledProcessError):
                    skips.Checks().run(tests, "main")


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
        self.assertEqual(sorted(selection.changed_files("main")),
                         ["docs/a.md", "src/agent/b.rs"])

    def test_no_changes(self):
        self.assertEqual(selection.changed_files("main"), [])

    def test_upstream_changes_are_not_counted(self):
        self.commit({"docs/a.md": "a\n"})
        self.git("checkout", "main")
        self.commit({"src/agent/upstream.rs": "upstream\n"})
        self.set_origin_main()
        self.git("checkout", "topic")
        self.assertEqual(selection.changed_files("main"), ["docs/a.md"])
        self.assertEqual(self.features(), {"docs"})

    def test_working_tree_edits_are_ignored(self):
        self.commit({"docs/a.md": "a\n"})
        self.write("src/agent/untracked.rs")
        self.write("README", "modified but not staged\n")
        self.write("docs/a.md", "modified but not staged\n")
        self.assertEqual(selection.changed_files("main"), ["docs/a.md"])
        self.assertEqual(self.features(), {"docs"})

    def test_rename_reports_both_sides(self):
        self.commit({"docs/a.md": "same content\n"})
        self.set_origin_main()
        os.makedirs(os.path.join(self.repo, "src/agent"))
        self.git("mv", "docs/a.md", "src/agent/a.rs")
        self.git("commit", "-m", "test: move")
        self.assertEqual(sorted(selection.changed_files("main")),
                         ["docs/a.md", "src/agent/a.rs"])
        # The unmapped destination makes the move run everything
        self.assertEqual(self.features(), skips.Checks().all_set_of_tests)

    def test_unusual_file_names_are_not_mangled(self):
        # Without -z git quotes names like these
        name = "docs/caf\u00e9 \"quoted\".md"
        self.commit({name: "a\n"})
        self.assertEqual(selection.changed_files("main"), [name])


if __name__ == "__main__":
    unittest.main()
