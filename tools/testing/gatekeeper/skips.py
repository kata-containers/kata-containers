#!/usr/bin/env python3
#
# Copyright (c) 2024 Red Hat Inc.
#
# SPDX-License-Identifier: Apache-2.0

# Gets changes of the current git to env variable TARGET_BRANCH
# and reports feature skips in form of "skip_$feature=yes|no"
# or list of required tests (based on argv[1])

import os
import re
import subprocess
import sys


# TODO: Add always-required tests


# TODO: Rework as yaml file (so devels don't need to read python)
# Changed file path to feature mapping
FEATURE_MAPPING = {
    r"^ci/": [],
    r"\.github/workflows/": [],
    r"\.rst$": ["build"],
    r"\.md$": ["build"],
    r"^src/": ["test"],
}

# Feature to required tests mapping
# TODO: Allow individual tests as well as regexps with min amount of results
REQUIRED_TESTS = {
    "build": ".*build.*",
    "test": ".*test.*"
}

# All features defined
ALL_FEATURES = set(key
                   for features in FEATURE_MAPPING.values()
                   for key in features)

class Checks:
    def run(self, tests, target_branch):
        enabled_features = self.get_features(target_branch)
        if not tests:
            for feature in ALL_FEATURES:
                print(f"skip_{feature}=" +
                      ('no' if feature in enabled_features else 'yes'))
            return 0
        tests = set(REQUIRED_TESTS[feature] for feature in enabled_features)
        print(','.join(tests))
        return 0

    def get_features(self, target_branch):
        """Check each changed file and find out to-be-tested features"""
        cmd = ["git", "diff", "--name-only", f"origin/{target_branch}"]
        mapping = {re.compile(key): value
                   for key, value in FEATURE_MAPPING.items()}
        changes = [_.decode("utf-8")
                   for _ in subprocess.check_output(cmd).split(b'\n')
                   if _.strip()]
        print('\n'.join(changes), file=sys.stderr)
        enabled_features = set()
        # Go through lines and find what features should be covered
        for line in changes:
            for regexp, features in mapping.items():
                if regexp.search(line):
                    for feature in features:
                        enabled_features.add(feature)
                    break
            else:
                # Untreated line, run all tests
                return ALL_FEATURES
        return enabled_features


if __name__ == "__main__":
    if len(sys.argv) == 2:
        tests = sys.argv[1] == '-t'
    else:
        tests = False
    exit(Checks().run(tests, os.getenv("TARGET_BRANCH", "main")))
