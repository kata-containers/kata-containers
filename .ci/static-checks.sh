#!/bin/bash

# Copyright (c) 2017-2018 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#
# Description: Central script to run all static checks.
#   This script should be called by all other repositories to ensure
#   there is only a single source of all static checks.

set -e

# Since this script is called from another repositories directory,
# ensure the utility is built before running it.
self="$GOPATH/src/github.com/kata-containers/tests"
(cd "$self" && make checkcommits)

# Check the commits in the branch
checkcommits \
	--need-fixes \
	--need-sign-offs \
	--verbose
