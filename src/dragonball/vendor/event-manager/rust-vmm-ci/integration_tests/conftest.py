# Copyright 2019 Amazon.com, Inc. or its affiliates. All Rights Reserved.
# SPDX-License-Identifier: Apache-2.0
import pytest


PROFILE_CI="ci"
PROFILE_DEVEL="devel"


def pytest_addoption(parser):
    parser.addoption(
        "--profile",
        default=PROFILE_CI,
        choices=[PROFILE_CI, PROFILE_DEVEL],
        help="Profile for running the test: {} or {}".format(
            PROFILE_CI,
            PROFILE_DEVEL
        )
    )
    parser.addoption(
        "--no-cleanup",
        action="store_true",
        default=False,
        help="Keep the coverage report in `kcov_output` directory. If this flag is not provided, "
             "both coverage related directories are removed."
    )


@pytest.fixture
def profile(request):
    return request.config.getoption("--profile")


@pytest.fixture
def no_cleanup(request):
    return request.config.getoption("--no-cleanup")


# This is used for defining global variables in pytest.
def pytest_configure():
    pytest.profile_ci = PROFILE_CI
    pytest.profile_devel = PROFILE_DEVEL
