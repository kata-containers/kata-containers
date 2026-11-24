#!/bin/sh
# Copyright (c) 2026 Kata Containers community
# SPDX-License-Identifier: Apache-2.0

set -eu

NAMESPACE=kata-dan-system
SELECTOR='app.kubernetes.io/name=kata-dan-cni,app.kubernetes.io/component=installer'

kubectl -n "${NAMESPACE}" rollout status daemonset/kata-dan-cni --timeout=180s
desired=$(kubectl -n "${NAMESPACE}" get daemonset/kata-dan-cni \
    -o 'jsonpath={.status.desiredNumberScheduled}')
if ! [ "${desired}" -gt 0 ] 2>/dev/null; then
    echo "Kata DAN installer has no target nodes" >&2
    exit 1
fi

pods=$(kubectl -n "${NAMESPACE}" get pods \
    -l "${SELECTOR}" \
    -o 'jsonpath={range .items[*]}{.metadata.name}{"\n"}{end}')
# Kubernetes Pod names contain no whitespace or shell glob characters.
# shellcheck disable=SC2086
set -- ${pods}
if [ "$#" -ne "${desired}" ]; then
    echo "Expected ${desired} installer Pods, found $#; leaving Kubernetes resources in place" >&2
    exit 1
fi

echo "removing Kata DAN host files from Linux nodes"
for pod in "$@"; do
    kubectl -n "${NAMESPACE}" exec "${pod}" -c credential-syncer \
        -- /kata-dan-cni/uninstall-cni.sh
done

current_pods=$(kubectl -n "${NAMESPACE}" get pods -l "${SELECTOR}" \
    -o 'jsonpath={range .items[*]}{.metadata.name}{"\n"}{end}')
original_pods=$(printf '%s\n' "${pods}" | LC_ALL=C sort)
current_pods=$(printf '%s\n' "${current_pods}" | LC_ALL=C sort)
if [ "${current_pods}" != "${original_pods}" ]; then
    echo "Installer Pods changed during cleanup; leaving Kubernetes resources in place" >&2
    exit 1
fi

echo "stopping the Kata DAN installer"
kubectl -n "${NAMESPACE}" delete daemonset kata-dan-cni \
    --ignore-not-found --cascade=foreground --wait=true

echo "removing Kata DAN Kubernetes resources"
kubectl delete clusterrolebinding kata-dan-cni --ignore-not-found
kubectl delete clusterrole kata-dan-cni --ignore-not-found
kubectl delete namespace "${NAMESPACE}" --ignore-not-found --wait=true

echo "Kata DAN CNI uninstalled"
