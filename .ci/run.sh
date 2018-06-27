#!/bin/bash
#
# Copyright (c) 2017-2018 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#

# This script will execute the Kata Containers Test Suite. 

set -e

export RUNTIME="kata-runtime"

echo "INFO: Running checks"
sudo -E PATH="$PATH" bash -c "make check"

echo "INFO: Running functional and integration tests ($PWD)"
sudo -E PATH="$PATH" bash -c "make test"
