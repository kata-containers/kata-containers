#!/bin/sh
# Copyright (c) 2026 Kata Containers community
# SPDX-License-Identifier: Apache-2.0

set -eu

CNI_BIN_DIR=${KATA_DAN_CNI_BIN_DIR:-/opt/cni/bin}
CNI_CONF_DIR=${KATA_DAN_CNI_CONF_DIR:-/etc/cni/net.d}
CREDENTIAL_DIR=${KATA_DAN_CREDENTIAL_DIR:-/etc/kata-dan-cni}
STATE_DIR=${KATA_DAN_SYNC_STATE_DIR:-/run/kata-dan-cni}

# Wait for an active credential update, then prevent further updates in this Pod.
mkdir -p "${STATE_DIR}"
exec 9>"${STATE_DIR}/credentials.lock"
flock -x 9
touch "${STATE_DIR}/uninstalling"

# Remove the entry point first so new requests cannot select files being removed.
rm -f "${CNI_CONF_DIR}/01-kata-dan.conflist"
rm -f "${CNI_BIN_DIR}/kata-dan-cni"
rm -f \
    "${CREDENTIAL_DIR}/kubeconfig" \
    "${CREDENTIAL_DIR}/token" \
    "${CREDENTIAL_DIR}/ca.crt"

echo "removed Kata DAN CNI host files"
