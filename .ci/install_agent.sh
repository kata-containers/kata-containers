#!/bin/bash
#
# Copyright (c) 2017-2018 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#

set -e

cidir=$(dirname "$0")

source "${cidir}/lib.sh"

clone_build_and_install "github.com/kata-containers/agent"
