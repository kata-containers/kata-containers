#!/bin/bash
#
# Copyright (c) 2017-2018 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#

# Currently these are the CRI-O tests that are not working

declare -a skipCRIOTests=(
'test "ctr oom"'
'test "ctr stats output"'
'test "ctr with run_as_username set to redis should get 101 as the gid for redis:alpine"'
'test "ctr with run_as_user set to 100 should get 101 as the gid for redis:alpine"'
'test "additional devices permissions"'
);
