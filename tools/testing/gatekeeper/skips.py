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
import os
import sys

import selection


class Checks:
    def __init__(self):
        self.config = selection.load_config()
        self.required_tests = self.config.get('required_tests') or []
        self.required_regexps = self.config.get('required_regexps') or []
        self.mapping = self.config['mapping']
        self.all_set_of_tests = set(self.mapping.keys())

    def run(self, tests, target_branch):
        """
        Find the required features/tests

        :param: tests: report required tests+regexps (bool)
        :param: target_branch: branch/commit to compare to
        """
        enabled_features = self.get_features(target_branch)
        if not tests:
            for feature in sorted(self.all_set_of_tests):
                # Print all features status in "$key=$value" format to allow
                # usage with $GITHUB_OUTPUT
                print(f"skip_{feature}=" +
                      ('no' if feature in enabled_features else 'yes'))
            return 0
        required_tests = set(self.required_tests)
        required_regexps = set(self.required_regexps)
        for feature in enabled_features:
            values = self.mapping.get(feature, {})
            if values.get("names"):
                required_tests.update(values["names"])
            if values.get("regexps"):
                required_regexps.add(values["regexps"])
        print(';'.join(sorted(required_tests)))
        print(';'.join(sorted(required_regexps)))
        return 0

    def get_features(self, target_branch):
        """
        Get changed file to `target_branch` and map them to the
        to-be-tested set-of-tests

        :param target_branch: branch/commit to compare to
        :returns: List of set-of-tests
        """
        changed = selection.changed_files(target_branch)
        print('\n'.join(changed), file=sys.stderr)
        return selection.get_features(changed, self.config)


if __name__ == "__main__":
    if len(sys.argv) == 2:
        _TESTS = sys.argv[1] == '-t'
    else:
        _TESTS = False
    sys.exit(Checks().run(_TESTS, os.getenv("TARGET_BRANCH", "main")))
