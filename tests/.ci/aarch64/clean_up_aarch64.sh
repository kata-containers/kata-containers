#!/bin/bash
#
# Copyright (c) 2018 ARM Limited
#
# SPDX-License-Identifier: Apache-2.0

source "/etc/os-release" || source "/usr/lib/os-release"

tests_repo="${tests_repo:-github.com/kata-containers/tests}"
lib_script="${GOPATH}/src/${tests_repo}/.ci/lib.sh"
source "${lib_script}"

gen_clean_arch || info "Arch cleanup scripts failed"

## Cleaning up stale files under $TMPDIR of last CI run
## $TMPDIR has been set as "/tmp/kata-containers" on all ARM CI node.
if [ "${TMPDIR}" != "/tmp" ]; then
	if [ -n "$(ls -A "${TMPDIR}")" ]; then
                echo "WARNING: ${TMPDIR} is Not Empty"
                sudo -E sh -c "rm -rf "${TMPDIR}""
        fi
fi
