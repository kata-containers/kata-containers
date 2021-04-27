#!/bin/bash
#
# Copyright (c) 2019 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#
set -o errexit
set -o pipefail
set -o nounset

export AZURE_HTTP_USER_AGENT="GITHUBACTIONS_${GITHUB_ACTION_NAME}_${GITHUB_REPOSITORY}"

LOCATION=${LOCATION:-westus2}
DNS_PREFIX=${DNS_PREFIX:-kata-deploy-${GITHUB_SHA:0:10}}
CLUSTER_CONFIG=${CLUSTER_CONFIG:-/kubernetes-containerd.json}

function die() {
    msg="$*"
    echo "ERROR: $msg" >&2
    exit 1
}

function destroy_aks() {
    set +x

    export KUBECONFIG="${PWD}/_output/${DNS_PREFIX}/kubeconfig/kubeconfig.${LOCATION}.json"

    az login --service-principal -u "$AZ_APPID" -p "$AZ_PASSWORD" --tenant "$AZ_TENANT_ID"
    az group delete --name "$DNS_PREFIX" --yes --no-wait
    az logout
}

function setup_aks() {
    [[ -z "$AZ_APPID" ]] && die "no Azure service principal ID provided"
    [[ -z "$AZ_PASSWORD" ]] && die "no Azure service principal secret provided"
    [[ -z "$AZ_SUBSCRIPTION_ID" ]] && die "no Azure subscription ID provided"
    [[ -z "$AZ_TENANT_ID" ]] && die "no Azure tenant ID provided"

    aks-engine deploy --subscription-id "$AZ_SUBSCRIPTION_ID" \
        --client-id "$AZ_APPID" --client-secret "$AZ_PASSWORD" \
        --location "$LOCATION" --dns-prefix "$DNS_PREFIX" \
        --api-model "$CLUSTER_CONFIG" --force-overwrite

    export KUBECONFIG="${PWD}/_output/${DNS_PREFIX}/kubeconfig/kubeconfig.${LOCATION}.json"

    # wait for the cluster to be settled:
    kubectl wait --timeout=10m --for=condition=Ready --all nodes

    # make sure coredns is up before moving forward:
    kubectl wait --timeout=10m -n kube-system --for=condition=Available deployment/coredns
}
