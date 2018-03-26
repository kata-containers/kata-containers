#!/bin/bash
#
# Copyright (c) 2017-2018 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#

# Currently these are the CRI-O tests that are not working

declare -a skipCRIOTests=(
'test "ctr hostname env"'
'test "ctr execsync failure"'
'test "ctr execsync exit code"'
'test "ctr execsync std{out,err}"'
'test "ctr stop idempotent"'
'test "ctr caps drop"'
'test "run ctr with image with Config.Volumes"'
'test "ctr oom"'
'test "ctr create with non-existent command"'
'test "ctr create with non-existent command \[tty\]"'
'test "ctr update resources"'
'test "ctr correctly setup working directory"'
'test "ctr execsync conflicting with conmon env"'
'test "ctr resources"'
'test "ctr \/etc\/resolv.conf rw\/ro mode"'
);
