#!/usr/bin/env python3
#
# Copyright (c) 2024 Red Hat Inc.
#
# SPDX-License-Identifier: Apache-2.0

"""
Gets changes of the current git to env variable TARGET_BRANCH
and reports feature skips in form of "skip_$feature=yes|no"
or list of required tests (based on argv[1])
"""

import json
import os
import sys

from selection import CONFIG, from_git


class Checks:
    def __init__(self):
        config = CONFIG
        if config.get('required_tests'):
            self.required_tests = config['required_tests']
        else:
            self.required_tests = []
        if config.get('required_regexps'):
            self.required_regexps = config['required_regexps']
        else:
            self.required_regexps = []
        if config.get('mapping'):
            self.mapping = config['mapping']
        else:
            self.mapping = {}
        self.all_set_of_tests = set(self.mapping.keys())

    def run(self, tests, target_branch):
        """
        Find the required features/tests

        :param: tests: report required tests+regexps (bool)
        :param: target_branch: branch/commit to compare to
        """
        selection = from_git(target_branch)
        enabled_features = self.get_features(target_branch, selection)
        if not tests:
            print("test_jobs=" + json.dumps(selection.jobs))
            print("skip_test=" + ("no" if selection.suites else "yes"))
            for feature in sorted(self.all_set_of_tests):
                # Print all features status in "$key=$value" format to allow
                # usage with $GITHUB_OUTPUT
                print(f"skip_{feature}=" +
                      ('no' if feature in enabled_features else 'yes'))
            return 0
        required_tests = set(self.required_tests)
        required_regexps = set(self.required_regexps)
        required_labels = set()
        for feature in enabled_features:
            values = self.mapping.get(feature, {})
            if values.get("names"):
                required_tests.update(values["names"])
            if values.get("regexps"):
                required_regexps.add(values["regexps"])
            if values.get("required-labels"):
                required_labels.update(values["required-labels"])
        print(';'.join(sorted(required_tests)))
        print(';'.join(sorted(required_regexps)))
        print(';'.join(sorted(required_labels)))
        return 0

    def get_features(self, target_branch, selection=None):
        """Use the same suite selection as the workflows that schedule CI."""
        selection = selection or from_git(target_branch)
        features = {"static"} | selection.suites
        if selection.build:
            features.add("build")
        return features


if __name__ == "__main__":
    if len(sys.argv) == 2:
        _TESTS = sys.argv[1] == '-t'
    else:
        _TESTS = False
    sys.exit(Checks().run(_TESTS, os.getenv("TARGET_BRANCH", "main")))
