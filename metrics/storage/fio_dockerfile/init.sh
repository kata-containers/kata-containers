#!/bin/bash
# Copyright (c) 2018 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0

set -e

TESTDIR=/testdir

init() {
	# If the testdir does not exist (that likely means has not been mounted as
	# a volume), then create it. If it is not a volume then it should be created
	# on the container root 'writeable overlay' (be that overlayfs, devmapper etc.)
	if [ ! -d ${TESTDIR} ]; then
		mkdir -p ${TESTDIR} || true
	fi
}

init
echo "Now pausing forever..."
tail -f /dev/null
