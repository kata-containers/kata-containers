#!/usr/bin/env python3
# Copyright (c) 2026 Kata Containers contributors
# SPDX-License-Identifier: Apache-2.0

"""Select integration suites; unknown inputs deliberately select everything."""

import argparse
from fnmatch import fnmatchcase
import json
import os
from pathlib import Path
import subprocess

import yaml


CONFIG_PATH = Path(__file__).with_name("required-tests.yaml")
with CONFIG_PATH.open(encoding="utf8") as config_file:
    CONFIG = yaml.safe_load(config_file)
SUITES = {name for name, group in CONFIG["mapping"].items() if "jobs" in group}


def validate_config(config):
    """Reject misspelled suites instead of quietly dropping test coverage."""
    suites = {name for name, group in config["mapping"].items() if "jobs" in group}
    referenced = set()
    for rule in config["paths"]:
        referenced.update(rule["suites"])
    for dependencies in config["dependencies"].values():
        referenced.update(dependencies)
    if referenced - suites:
        raise ValueError(f"Unknown suites: {sorted(referenced - suites)}")


validate_config(CONFIG)


def git(*args):
    """Read committed PR contents, independently of local working tree edits."""
    return subprocess.check_output(["git", *args])


def changed_files(target_branch):
    base = git("merge-base", "HEAD", f"origin/{target_branch or 'main'}").decode().strip()
    # Disabling rename detection includes both sides of a move across suites.
    paths = git("diff", "--no-renames", "--name-only", "-z", base, "HEAD")
    return base, [os.fsdecode(path) for path in paths.split(b"\0") if path]


class Selection:
    def __init__(self, suites=(), build=False):
        self.suites = set(suites)
        self.build = build

    @classmethod
    def all(cls):
        return cls(SUITES, build=True)

    @property
    def jobs(self):
        return sorted({job for suite in self.suites for job in CONFIG["mapping"][suite]["jobs"]})


def select(paths, dependency_changes=None):
    """Union independent changes. Only explicitly mapped paths narrow CI."""
    result = Selection()
    for path in paths:
        if path == "versions.yaml":
            dependencies = CONFIG["dependencies"]
            if not dependency_changes or not dependency_changes <= dependencies.keys():
                return Selection.all()
            for name in dependency_changes:
                result.suites.update(dependencies[name])
            result.build = True
            continue
        for rule in CONFIG["paths"]:
            if not any(fnmatchcase(path, pattern) for pattern in rule["paths"]):
                continue
            result.suites.update(rule["suites"])
            result.build |= rule.get("build", True)
            break
        else:
            return Selection.all()
    return result


def from_git(target_branch):
    base, paths = changed_files(target_branch)
    dependency_changes = None
    if "versions.yaml" in paths:
        before = yaml.safe_load(git("show", f"{base}:versions.yaml"))
        after = yaml.safe_load(git("show", "HEAD:versions.yaml"))
        old_externals = before.pop("externals", {})
        new_externals = after.pop("externals", {})
        if before == after:
            dependency_changes = {key for key in old_externals.keys() | new_externals.keys()
                                  if old_externals.get(key) != new_externals.get(key)}
    return select(paths, dependency_changes)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--target-branch", default=os.getenv("TARGET_BRANCH", "main"))
    args = parser.parse_args()
    print(json.dumps(sorted(from_git(args.target_branch).suites)))


if __name__ == "__main__":
    main()
