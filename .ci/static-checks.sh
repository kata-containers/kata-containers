#!/bin/bash
#
# Copyright (c) 2018,2020 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0

set -e

cidir=$(dirname "$0")
source "${cidir}/lib.sh"

# Run static checks
run_static_checks
