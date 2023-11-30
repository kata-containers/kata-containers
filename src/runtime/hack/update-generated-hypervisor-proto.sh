#!/bin/bash
# (C) Copyright IBM Corp. 2022, 2023
# SPDX-License-Identifier: Apache-2.0

set -o errexit -o pipefail -o nounset

HYPERVISOR_PATH="protocols/hypervisor"

protoc \
    -I=$GOPATH/src \
    --proto_path=$HYPERVISOR_PATH \
    --go_out=$HYPERVISOR_PATH \
    --go-ttrpc_out=$HYPERVISOR_PATH \
    $HYPERVISOR_PATH/hypervisor.proto
