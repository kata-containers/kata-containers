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

from collections import OrderedDict
import os
import re
import subprocess
import sys

import yaml


class Checks:
    def __init__(self):
        config_path = os.path.join(os.path.dirname(__file__), "required-tests.yaml")
        with open(config_path, "r", encoding="utf8") as config_fd:
            config = yaml.load(config_fd, Loader=yaml.SafeLoader)
        if config.get('required_tests'):
            self.required_tests = config['required_tests']
        else:
            self.required_tests = []
        if config.get('required_regexps'):
            self.required_regexps = config['required_regexps']
        else:
            self.required_regexps = []
        if config.get('paths'):
            self.paths = OrderedDict((re.compile(key), value)
                                       for items in config['paths']
                                       for key, value in items.items())
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
        enabled_features = self.get_features(target_branch)
        if not tests:
            for feature in self.all_set_of_tests:
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
        print(';'.join(required_tests))
        print(';'.join(required_regexps))
        print(';'.join(required_labels))
        return 0

    def get_features(self, target_branch):
        """
        Get changed file to `target_branch` and map them to the
        to-be-tested set-of-tests

        :param target_branch: branch/commit to compare to
        :returns: List of set-of-tests
        """
        cmd = ["git", "diff", "--name-only", f"origin/{target_branch}"]
        changed_files = [_.decode("utf-8")
                         for _ in subprocess.check_output(cmd).split(b'\n')
                         if _.strip()]
        print('\n'.join(changed_files), file=sys.stderr)
        enabled_features = set()
        # Go through lines and find what features should be covered
        for changed_file in changed_files:
            for regexp, features in self.paths.items():
                if regexp.search(changed_file):
                    for feature in features:
                        enabled_features.add(feature)
                    # this changed_file was treated, ignore other regexps
                    break
            else:
                # Untreated changed_file, run all tests
                return self.all_set_of_tests
        return enabled_features


if __name__ == "__main__":
    if len(sys.argv) == 2:
        _TESTS = sys.argv[1] == '-t'
    else:
        _TESTS = False
    sys.exit(Checks().run(_TESTS, os.getenv("TARGET_BRANCH", "main")))
