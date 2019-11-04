#!/bin/bash
#
# Copyright (c) 2019 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#

set -o errexit
set -o pipefail
set -o nounset

function die() {
    msg="$*"
    echo "ERROR: $msg" >&2
    exit 1
}

function waitForProcess() {
    wait_time="$1"
    sleep_time="$2"
    cmd="$3"
    while [ "$wait_time" -gt 0 ]; do
        if eval "$cmd"; then
            return 0
        else
            echo "waiting"
            sleep "$sleep_time"
            wait_time=$((wait_time-sleep_time))
        fi
    done
    return 1
}

# waitForLabelRemoval will wait for the kata-runtime labels to removed until a given
# timeout expires
function waitForLabelRemoval() {
    wait_time="$1"
    sleep_time="$2"

    while [[ "$wait_time" -gt 0 ]]; do
        # if a node is found which matches node-select, the output will include a column for node name,
        # NAME. Let's look for that 
        if [[ -z $(kubectl get nodes --selector katacontainers.io/kata-runtime | grep NAME) ]]
        then
            return 0
        else
            echo "waiting for kata-runtime label to be removed"
            sleep "$sleep_time"
            wait_time=$((wait_time-sleep_time))
        fi
    done

    echo "failed to cleanup"
    return 1
}


function run_test() {
    PKG_SHA=$1
    YAMLPATH="https://raw.githubusercontent.com/kata-containers/packaging/$PKG_SHA/kata-deploy"
    echo "verify connectivity with a pod using Kata"

    deployment=""
    busybox_pod="test-nginx"
    busybox_image="busybox"
    cmd="kubectl get pods | grep $busybox_pod | grep Completed"
    wait_time=120
    sleep_time=3

    configurations=("nginx-deployment-qemu" "nginx-deployment-qemu-virtiofs")
    for deployment in "${configurations[@]}"; do
        # start the kata pod:
        kubectl apply -f "$YAMLPATH/examples/${deployment}.yaml"

      # in case the control plane is slow, give it a few seconds to accept the yaml, otherwise
      # our 'wait' for deployment status will fail to find the deployment at all
      sleep 3 

      kubectl wait --timeout=5m --for=condition=Available deployment/${deployment}
      kubectl expose deployment/${deployment}

      # test pod connectivity:
      kubectl run $busybox_pod --restart=Never --image="$busybox_image" -- wget --timeout=5 "$deployment"
      waitForProcess "$wait_time" "$sleep_time" "$cmd"
      kubectl logs "$busybox_pod" | grep "index.html"
      kubectl describe pod "$busybox_pod"

      # cleanup:
      kubectl delete deployment "$deployment"
      kubectl delete service "$deployment"
      kubectl delete pod "$busybox_pod"
  done
}


function test_kata() {
    set -x

    [[ -z "$PKG_SHA" ]] && die "no PKG_SHA provided"
    echo "$PKG_SHA"

    #kubectl all the things
    kubectl get pods,nodes --all-namespaces

    YAMLPATH="https://raw.githubusercontent.com/kata-containers/packaging/$PKG_SHA/kata-deploy"

    kubectl apply -f "$YAMLPATH/kata-rbac.yaml"

    # apply runtime classes:
    kubectl apply -f "$YAMLPATH/k8s-1.14/kata-qemu-runtimeClass.yaml"
    kubectl apply -f "$YAMLPATH/k8s-1.14/kata-qemu-virtiofs-runtimeClass.yaml"

    kubectl get runtimeclasses

    curl -LO "$YAMLPATH/kata-deploy.yaml"
    curl -LO "$YAMLPATH/kata-cleanup.yaml"

    # update deployment daemonset to utilize the container under test:
    sed -i "s#katadocker/kata-deploy#katadocker/kata-deploy-ci:${PKG_SHA}#g" kata-deploy.yaml
    sed -i "s#katadocker/kata-deploy#katadocker/kata-deploy-ci:${PKG_SHA}#g" kata-cleanup.yaml

    cat kata-deploy.yaml

    # deploy kata:
    kubectl apply -f kata-deploy.yaml

    # in case the control plane is slow, give it a few seconds to accept the yaml, otherwise
    # our 'wait' for deployment status will fail to find the deployment at all. If it can't persist
    # the daemonset to etcd in 30 seconds... then we'll fail.
    sleep 30

    # wait for kata-deploy to be up
    kubectl -n kube-system wait --timeout=10m --for=condition=Ready -l name=kata-deploy pod

    # show running pods, and labels of nodes
    kubectl get pods,nodes --all-namespaces --show-labels

    run_test $PKG_SHA

    kubectl get pods,nodes --show-labels

    # Remove Kata
    kubectl delete -f kata-deploy.yaml
    kubectl -n kube-system wait --timeout=10m --for=delete -l name=kata-deploy pod

    kubectl get pods,nodes --show-labels

    kubectl apply -f kata-cleanup.yaml

    # The cleanup daemonset will run a single time, since it will clear the node-label. Thus, its difficult to
    # check the daemonset's status for completion. instead, let's wait until the kata-runtime labels are removed
    # from all of the worker nodes. If this doesn't happen in 45 seconds, let's fail
    timeout=45
    sleeptime=1
    waitForLabelRemoval $timeout $sleeptime

    kubectl delete -f kata-cleanup.yaml

    rm kata-cleanup.yaml
    rm kata-deploy.yaml

    set +x
}
