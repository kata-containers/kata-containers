#!/bin/bash

# Copyright (c) 2017-2018 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#
# Description: Central script to run all static checks.
#   This script should be called by all other repositories to ensure
#   there is only a single source of all static checks.

set -e

# Check the commits in the branch
make checkcommits
checkcommits \
	--need-fixes \
	--need-sign-offs \
	--verbose
