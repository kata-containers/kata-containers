#!/usr/bin/env python3
#
# Copyright (c) 2026 Kata Containers contributors
# Copyright (c) 2026 IBM Corporation
#
# SPDX-License-Identifier: Apache-2.0

"""
Maps the files changed by a branch to the set-of-tests (features) to run

The path rules live in required-tests.yaml. A path that matches no rule selects
every feature, so narrowing the selection is always opt-in.

Nothing in here catches errors. A malformed configuration or a failing git
command must surface as a failure, rather than as a quietly smaller selection.
"""

from fnmatch import fnmatchcase
import os
import subprocess

import yaml

CONFIG_PATH = os.path.join(os.path.dirname(__file__), "required-tests.yaml")
RULE_KEYS = {"paths", "features"}


def load_config(path=CONFIG_PATH):
    """
    Read and validate the configuration

    :param path: path to the YAML configuration
    :returns: The parsed configuration
    :raises ValueError: if the configuration is malformed
    """
    with open(path, "r", encoding="utf8") as config_fd:
        config = yaml.safe_load(config_fd)
    validate_config(config)
    return config


def validate_config(config):
    """
    Reject a configuration that would silently select the wrong tests

    A misspelled feature in a rule would otherwise drop that feature's
    coverage from every path the rule matches.

    :param config: the parsed configuration
    :raises ValueError: if the configuration is malformed
    """
    if not isinstance(config, dict):
        raise ValueError("The configuration must be a mapping")
    for key in ("paths", "mapping"):
        if not config.get(key):
            raise ValueError(f"'{key}' is missing or empty")
    known = set(config["mapping"])
    for number, rule in enumerate(config["paths"], 1):
        if not isinstance(rule, dict) or set(rule) != RULE_KEYS:
            raise ValueError(f"paths rule {number} must have exactly the keys "
                             f"{sorted(RULE_KEYS)}: {rule}")
        for key in sorted(RULE_KEYS):
            if not isinstance(rule[key], list) or not all(
                    isinstance(_, str) for _ in rule[key]):
                raise ValueError(f"paths rule {number}: '{key}' must be a "
                                 f"list of strings: {rule}")
        if not rule["paths"]:
            raise ValueError(f"paths rule {number} has no patterns: {rule}")
        unknown = set(rule["features"]) - known
        if unknown:
            raise ValueError(f"paths rule {number} uses unknown features "
                             f"{sorted(unknown)}, known: {sorted(known)}")


def changed_files(target_branch):
    """
    Get the files changed in HEAD since it diverged from `target_branch`

    Diffing against the merge base, rather than the tip of the target branch,
    means that changes merged upstream after the branch was created are not
    counted as part of this change. Uncommitted edits are ignored as well.

    :param target_branch: branch to compare to
    :returns: List of changed file paths
    """
    base = subprocess.check_output(
        ["git", "merge-base", "HEAD", f"origin/{target_branch}"]).decode().strip()
    # Disabling rename detection reports both sides of a move
    cmd = ["git", "diff", "--no-renames", "--name-only", "-z", base, "HEAD"]
    return [os.fsdecode(_) for _ in subprocess.check_output(cmd).split(b'\0')
            if _]


def get_features(changed, config):
    """
    Map changed files to the to-be-tested set-of-tests

    The first rule with a pattern matching a file decides that file's
    features, so rules are sorted by order of importance. A file matching no
    rule selects every feature.

    :param changed: List of changed file paths
    :param config: the validated configuration
    :returns: Set of set-of-tests
    """
    enabled_features = set()
    for changed_file in changed:
        for rule in config["paths"]:
            if any(fnmatchcase(changed_file, pattern)
                   for pattern in rule["paths"]):
                enabled_features.update(rule["features"])
                # this changed_file was treated, ignore other rules
                break
        else:
            # Untreated changed_file, run all tests
            return set(config["mapping"])
    return enabled_features
