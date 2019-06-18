#!/bin/bash
#
# Copyright (c) 2018 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#


set -e

export GOPATH="${GOPATH:-/tmp/go}"

script_dir="$(dirname $(readlink -f $0))"

sudo -E PATH="$PATH" bash "${script_dir}/../tests/test_images.sh"
