# Copyright 2019 Amazon.com, Inc. or its affiliates. All Rights Reserved.
# SPDX-License-Identifier: Apache-2.0
"""Test the coverage and update the threshold when coverage is increased."""

import json, os, re, shutil, subprocess, platform
import pytest

from utils import get_repo_root_path

REPO_ROOT_PATH = get_repo_root_path()
if platform.machine() == "x86_64":
    COVERAGE_CONFIG_PATH = os.path.join(REPO_ROOT_PATH, "coverage_config_x86_64.json")
elif platform.machine() == "aarch64":
    COVERAGE_CONFIG_PATH = os.path.join(REPO_ROOT_PATH, "coverage_config_aarch64.json")


def _read_test_config():
    """
    Reads the config of the coverage for the repository being tested.

    Returns a JSON object with the configuration.
    """
    coverage_config = {}
    with open(COVERAGE_CONFIG_PATH) as config_file:
        coverage_config = json.load(config_file)

    assert "coverage_score" in coverage_config
    assert "exclude_path" in coverage_config
    assert "crate_features" in coverage_config

    return coverage_config


def _write_coverage_config(coverage_config):
    """Updates the coverage config file as per `coverage_config`"""
    with open(COVERAGE_CONFIG_PATH, 'w') as outfile:
        json.dump(coverage_config, outfile)


def _get_current_coverage(coverage_config, no_cleanup):
    """Helper function that returns the coverage computed with kcov."""
    kcov_output_dir = os.path.join(REPO_ROOT_PATH, "kcov_output")

    # By default the build output for kcov and unit tests are both in the debug
    # directory. This causes some linker errors that I haven't investigated.
    # Error: error: linking with `cc` failed: exit code: 1
    # An easy fix is to have separate build directories for kcov & unit tests.
    kcov_build_dir = os.path.join(REPO_ROOT_PATH, "kcov_build")

    # Remove kcov output and build directory to be sure we are always working
    # on a clean environment.
    shutil.rmtree(kcov_output_dir, ignore_errors=True)
    shutil.rmtree(kcov_build_dir, ignore_errors=True)

    exclude_pattern = (
        '${CARGO_HOME:-$HOME/.cargo/},'
        'usr/lib/,'
        'lib/'
    )
    exclude_region = "'mod tests {'"
    additional_exclude_path = coverage_config["exclude_path"]
    if additional_exclude_path:
        exclude_pattern += ',' + additional_exclude_path

    additional_kcov_param = ''
    crate_features = coverage_config["crate_features"]
    if crate_features:
        additional_kcov_param += '--features=' + crate_features

    kcov_cmd = "CARGO_TARGET_DIR={} cargo kcov {} --all " \
               "--output {} -- " \
               "--exclude-region={} " \
               "--exclude-pattern={} " \
               "--verify".format(
        kcov_build_dir,
        additional_kcov_param,
        kcov_output_dir,
        exclude_region,
        exclude_pattern
    )

    # Pytest closes stdin by default, but some tests might need it to be open.
    # In the future, should the need arise, we can feed custom data to stdin.
    subprocess.run(kcov_cmd, shell=True, check=True, input=b'')

    # Read the coverage reported by kcov.
    coverage_file = os.path.join(kcov_output_dir, 'index.js')
    with open(coverage_file) as cov_output:
        coverage = float(re.findall(
            r'"covered":"(\d+\.\d)"',
            cov_output.read()
        )[0])

    # Remove coverage related directories.
    # If user provided `--no-cleanup` flag, `kcov_output_dir` should not be removed.
    if not no_cleanup:
        shutil.rmtree(kcov_output_dir, ignore_errors=True)
    shutil.rmtree(kcov_build_dir, ignore_errors=True)

    return coverage


def test_coverage(profile, no_cleanup):
    coverage_config = _read_test_config()
    current_coverage = _get_current_coverage(coverage_config, no_cleanup)
    previous_coverage = coverage_config["coverage_score"]
    if previous_coverage < current_coverage:
        if profile == pytest.profile_ci:
            # In the CI Profile we expect the coverage to be manually updated.
            assert False, "Coverage is increased from {} to {}. " \
                          "Please update the coverage in " \
                          "tests/coverage.".format(
                previous_coverage,
                current_coverage
            )
        elif profile == pytest.profile_devel:
            coverage_config["coverage_score"] = current_coverage
            _write_coverage_config(coverage_config)
        else:
            # This should never happen because pytest should only accept
            # the valid test profiles specified with `choices` in
            # `pytest_addoption`.
            assert False, "Invalid test profile."
    elif previous_coverage > current_coverage:
        diff = float(previous_coverage - current_coverage)
        assert False, "Coverage drops by {:.2f}%. Please add unit tests for" \
                      "the uncovered lines.".format(diff)
