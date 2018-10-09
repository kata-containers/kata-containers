#!/usr/bin/env bash
#
# Copyright (c) 2018 SUSE LLC
#
# SPDX-License-Identifier: Apache-2.0

set -euo pipefail

source config.sh

removeRepos=(repo-non-oss repo-update-non-oss repo-oss repo-update)

for r in ${removeRepos[@]}; do
	zypper --non-interactive removerepo $r
done

zypper --non-interactive addrepo ${SUSE_FULLURL_OSS} osbuilder-oss
zypper --non-interactive addrepo ${SUSE_FULLURL_UPDATE} osbuilder-update


# Workaround for zypper slowdowns observed when running inside
# a container: see https://github.com/openSUSE/zypper/pull/209
# The fix is upstream but it will take a while before landing
# in Leap
ulimit -n 1024
zypper --non-interactive refresh
zypper --non-interactive install --no-recommends --force-resolution curl git gcc make python3-kiwi tar
zypper --non-interactive clean --all

