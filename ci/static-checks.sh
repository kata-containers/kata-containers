#!/bin/bash
#
# Copyright (c) 2017-2018 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0

set -e

cidir=$(dirname "$0")
source "${cidir}/lib.sh"

/usr/bin/git version
/usr/bin/git branch
pwd
/usr/bin/git show
/usr/bin/git show HEAD~
/usr/bin/git show HEAD~2
/usr/bin/git show HEAD~3
/usr/bin/git show HEAD~4

run_static_checks
