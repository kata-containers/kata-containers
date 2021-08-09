#!/bin/bash
set -e

export CLUSTER_NAME="${CLUSTER_NAME:-kata-ci}"
export GLOBAL_CONFIG=${GLOBAL_CONFIG:-/kubeconfig}

ksmith teardown "${CLUSTER_NAME}" --kubeconfig "${GLOBAL_CONFIG}"
