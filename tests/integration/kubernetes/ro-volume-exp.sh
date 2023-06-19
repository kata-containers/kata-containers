#! /bin/bash
#
# Copyright (c) 2021 Ant Group
#
# SPDX-License-Identifier: Apache-2.0
#

TIMEOUT=${TIMEOUT:-60}

# A simple expect script to help validating readonly volumes
# Run with ro-volume-exp.sh <sandbox-id> <volume-suffix> <tmp-file>
expect -c "
  set timeout $TIMEOUT
  spawn kata-runtime exec $1
  send \"cd /run/kata-containers/shared/containers/*$2/\n\"
  send \"echo 1 > $3\n\"
  send \"exit\n\"
  interact
"
