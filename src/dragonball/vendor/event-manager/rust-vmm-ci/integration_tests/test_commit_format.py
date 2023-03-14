# Copyright 2020 Amazon.com, Inc. or its affiliates. All Rights Reserved.
# SPDX-License-Identifier: Apache-2.0
"""Test the commit message format."""

import os
import subprocess

from utils import get_cmd_output

COMMIT_TITLE_MAX_LEN = 50
COMMIT_BODY_LINE_MAX_LEN = 72
BASE_BRANCH = os.environ['BUILDKITE_PULL_REQUEST_BASE_BRANCH']
BASE_REPO = os.environ['BUILDKITE_REPO']


def test_commit_format():
    """
    Checks commit message format for the current PR's commits.

    Checks if commit messages follow the 50/72 git commit rule
    [https://www.midori-global.com/blog/2018/04/02/git-50-72-rule]
    and if commits are signed.
    """
    # Fetch the upstream repository.
    fetch_base_cmd = "git fetch {} {}".format(BASE_REPO, BASE_BRANCH)
    subprocess.run(fetch_base_cmd, shell=True, check=True)
    # Get hashes of PR's commits in their abbreviated form for
    # a prettier printing.
    shas_cmd = "git log --no-merges --pretty=%h --no-decorate " \
               "FETCH_HEAD..HEAD"
    shas = get_cmd_output(shas_cmd)

    for sha in shas.split():
        # Do not enforce the commit rules when the committer is dependabot.
        author_cmd = "git show -s --format='%ae'"
        author = get_cmd_output(author_cmd)
        if "dependabot" in author:
            continue
        message_cmd = "git show --pretty=format:%B -s " + sha
        message = get_cmd_output(message_cmd)
        message_lines = message.split("\n")
        assert len(message_lines) >= 3,\
            "The commit '{}' should contain at least 3 lines: title, " \
            "blank line and a sign-off one. Please check: " \
            "https://www.midori-global.com/blog/2018/04/02/git-50-72-rule."\
            .format(sha)
        title = message_lines[0]
        assert message_lines[1] == "",\
            "For commit '{}', title is divided into multiple lines. " \
            "Please keep it one line long and make sure you add a blank " \
            "line between title and description.".format(sha)
        assert len(title) <= COMMIT_TITLE_MAX_LEN,\
            "For commit '{}', title exceeds {} chars. " \
            "Please keep it shorter.".format(sha, COMMIT_TITLE_MAX_LEN)

        found_signed_off = False

        for line in message_lines[2:]:
            if line.startswith("Signed-off-by: "):
                found_signed_off = True
                # If we found `Signed-off-by` line, then it means
                # the commit message ended and we don't want to check
                # line lengths anymore for the current commit.
                break
            assert len(line) <= COMMIT_BODY_LINE_MAX_LEN,\
                "For commit '{}', message line '{}' exceeds {} chars. " \
                "Please keep it shorter or split it in " \
                "multiple lines.".format(sha, line,
                                         COMMIT_BODY_LINE_MAX_LEN)
        assert found_signed_off, "Commit '{}' is not signed. " \
                                 "Please run 'git commit -s --amend' " \
                                 "on it.".format(sha)
