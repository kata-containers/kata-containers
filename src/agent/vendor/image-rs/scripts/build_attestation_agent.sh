#!/bin/bash
#
# Copyright (c) 2022 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#

set -o errexit
set -o nounset
set -o pipefail

[ -n "${BASH_VERSION:-}" ] && set -o errtrace
[ -n "${DEBUG:-}" ] && set -o xtrace

source $HOME/.cargo/env

SCRIPT_DIR=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
AA_DIR=$SCRIPT_DIR/attestation_agent

pushd $SCRIPT_DIR
git clone --depth 1 "https://github.com/confidential-containers/attestation-agent.git" $AA_DIR
pushd $AA_DIR
make KBC=offline_fs_kbc
make DESTDIR="$SCRIPT_DIR" install
popd

cleanup() {
  rm -rf "$AA_DIR"
}
trap cleanup EXIT
