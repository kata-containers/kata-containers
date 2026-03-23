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

import argparse
from collections import OrderedDict
import os
import re
import subprocess
import sys

import requests
import yaml


class Checks:
    def __init__(self, from_target_branch=False, target_branch=None):
        config = self._load_config(from_target_branch, target_branch) or {}
        self._parse_config(config)

    def _load_config(self, from_target_branch, target_branch):
        """
        Load the required-tests.yaml config.

        :param from_target_branch: If True, fetch config from the target branch
            via GitHub raw URL instead of using local file
        :param target_branch: The target branch to fetch from (required if
            from_target_branch is True)
        :returns: Parsed YAML config dict
        """
        if from_target_branch:
            repo = os.environ.get('GITHUB_REPOSITORY')
            if not repo:
                raise RuntimeError(
                    "GITHUB_REPOSITORY env var required when using "
                    "--from-target-branch")
            if not target_branch:
                raise RuntimeError(
                    "target_branch required when using --from-target-branch")
            url = (f"https://raw.githubusercontent.com/{repo}/"
                   f"refs/heads/{target_branch}/"
                   "tools/testing/gatekeeper/required-tests.yaml")
            print(f"Fetching config from: {url}", file=sys.stderr)
            response = requests.get(url, timeout=30)
            response.raise_for_status()
            return yaml.load(response.text, Loader=yaml.SafeLoader)
        config_path = os.path.join(os.path.dirname(__file__),
                                   "required-tests.yaml")
        with open(config_path, "r", encoding="utf8") as config_fd:
            return yaml.load(config_fd, Loader=yaml.SafeLoader)

    def _parse_config(self, config):
        """Parse the config dict into instance attributes."""
        self.required_tests = config.get('required_tests') or []
        self.required_regexps = config.get('required_regexps') or []
        self.paths = OrderedDict((re.compile(key), value)
                                   for items in config.get('paths', [])
                                   for key, value in items.items())
        self.mapping = config.get('mapping') or {}
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
    parser = argparse.ArgumentParser(
        description="Get required tests based on changed files")
    parser.add_argument(
        "-t", "--tests", action="store_true",
        help="Report required tests and regexps instead of skip flags")
    parser.add_argument(
        "--from-target-branch", action="store_true",
        help="Fetch required-tests.yaml from the target branch via GitHub "
             "raw URL instead of using local file. This prevents PRs from "
             "modifying the required tests config. Requires GITHUB_REPOSITORY "
             "and TARGET_BRANCH env vars.")
    args = parser.parse_args()

    TARGET_BRANCH = os.getenv("TARGET_BRANCH", "main")
    checks = Checks(from_target_branch=args.from_target_branch,
                    target_branch=TARGET_BRANCH)
    sys.exit(checks.run(args.tests, TARGET_BRANCH))
