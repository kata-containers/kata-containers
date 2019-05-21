#!/bin/bash
#
# Copyright (c) 2018 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0

set -e

cidir=$(dirname "$0")
source "${cidir}/lib.sh"

# Build kata-runtime before running static checks
make -C "${cidir}/../"

# Run static checks
run_static_checks
