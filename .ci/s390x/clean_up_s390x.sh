#!/bin/bash
#
# Copyright (c) 2019 IBM
#
# SPDX-License-Identifier: Apache-2.0
#

source "/etc/os-release" || source "/usr/lib/os-release"

lib_script="${GOPATH}/src/${tests_repo}/.ci/lib.sh"
source "${lib_script}"

gen_clean_arch || info "Arch cleanup scripts failed"
