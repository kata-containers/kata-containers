#!/bin/sh
# Copyright (c) 2026 Kata Containers community
# SPDX-License-Identifier: Apache-2.0

set -eu

INSTALL_CREDENTIALS=${KATA_DAN_INSTALL_CREDENTIALS:-/kata-dan-cni/install-credentials.sh}
SYNC_INTERVAL_SECONDS=${KATA_DAN_SYNC_INTERVAL_SECONDS:-60}
STATE_DIR=${KATA_DAN_SYNC_STATE_DIR:-/run/kata-dan-cni}

case "${SYNC_INTERVAL_SECONDS}" in
    ""|*[!0-9]*|0)
        echo "KATA_DAN_SYNC_INTERVAL_SECONDS must be a positive integer" >&2
        exit 1
        ;;
esac

mkdir -p "${STATE_DIR}"

while :; do
    if ! (
        flock -x 9 || exit 1
        [ ! -e "${STATE_DIR}/uninstalling" ] || exit 0
        /bin/sh "${INSTALL_CREDENTIALS}"
    ) 9>"${STATE_DIR}/credentials.lock"; then
        echo "failed to synchronize Kata DAN Kubernetes credentials; keeping the last installed files" >&2
    fi
    sleep "${SYNC_INTERVAL_SECONDS}"
done
