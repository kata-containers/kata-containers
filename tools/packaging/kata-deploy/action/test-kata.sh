#!/usr/bin/env bash
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
    cmd="$2"
    sleep_time=5
    echo "waiting for process $cmd"
    while [ "$wait_time" -gt 0 ]; do
        if eval "$cmd"; then
            return 0
        else
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
    sleep_time=5

    echo "waiting for kata-runtime label to be removed"
    while [[ "$wait_time" -gt 0 ]]; do
        # if a node is found which matches node-select, the output will include a column for node name,
        # NAME. Let's look for that 
        if [[ -z $(kubectl get nodes --selector katacontainers.io/kata-runtime 2>&1 | grep NAME) ]]
        then
            return 0
        else
            sleep "$sleep_time"
            wait_time=$((wait_time-sleep_time))
        fi
    done

    echo $(kubectl get pods,nodes --all-namespaces --show-labels)

    echo "failed to cleanup"
    return 1
}

function run_test() {
    YAMLPATH="./tools/packaging/kata-deploy/"
    echo "verify connectivity with a pod using Kata"

    deployment=""
    busybox_pod="test-nginx"
    busybox_image="busybox"
    cmd="kubectl get pods | grep $busybox_pod | grep Completed"
    wait_time=120

    configurations=("nginx-deployment-qemu" "nginx-deployment-clh" "nginx-deployment-dragonball")
    for deployment in "${configurations[@]}"; do
        # start the kata pod:
        kubectl apply -f "$YAMLPATH/examples/${deployment}.yaml"

      # in case the control plane is slow, give it a few seconds to accept the yaml, otherwise
      # our 'wait' for deployment status will fail to find the deployment at all
      sleep 3 

      kubectl wait --timeout=5m --for=condition=Available deployment/${deployment} || kubectl describe pods
      kubectl expose deployment/${deployment}

      # test pod connectivity:
      kubectl run $busybox_pod --restart=Never --image="$busybox_image" -- wget --timeout=5 "$deployment"
      waitForProcess "$wait_time" "$cmd"
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

    YAMLPATH="./tools/packaging/kata-deploy/"

    # This action could be called in two contexts:
    #  1. Packaging workflows: testing in packaging repository, where we assume yaml/packaging
    #   bits under test are already part of teh action workspace.
    #  2. From kata-containers: when creating a release, the appropriate packaging repository is
    #   not yet part of the workspace, and we will need to clone
    if [[ ! -d $YAMLPATH ]]; then
        [[ -d  $YAMLPATH ]] || git clone https://github.com/kata-containers/kata-containers
        cd kata-containers
        git fetch
        git checkout $PKG_SHA
    fi

    kubectl apply -f "$YAMLPATH/kata-rbac/base/kata-rbac.yaml"

    # apply runtime classes:
    kubectl apply -f "$YAMLPATH/runtimeclasses/kata-runtimeClasses.yaml"

    kubectl get runtimeclasses

    # update deployment daemonset to utilize the container under test:
    sed -i "s#quay.io/kata-containers/kata-deploy:latest#quay.io/kata-containers/kata-deploy-ci:${PKG_SHA}#g" $YAMLPATH/kata-deploy/base/kata-deploy.yaml
    sed -i "s#quay.io/kata-containers/kata-deploy:latest#quay.io/kata-containers/kata-deploy-ci:${PKG_SHA}#g" $YAMLPATH/kata-cleanup/base/kata-cleanup.yaml

    cat $YAMLPATH/kata-deploy/base/kata-deploy.yaml

    # deploy kata:
    kubectl apply -f $YAMLPATH/kata-deploy/base/kata-deploy.yaml

    # in case the control plane is slow, give it a few seconds to accept the yaml, otherwise
    # our 'wait' for deployment status will fail to find the deployment at all. If it can't persist
    # the daemonset to etcd in 30 seconds... then we'll fail.
    sleep 30

    # wait for kata-deploy to be up
    kubectl -n kube-system wait --timeout=10m --for=condition=Ready -l name=kata-deploy pod

    # show running pods, and labels of nodes
    kubectl get pods,nodes --all-namespaces --show-labels

    run_test

    kubectl get pods,nodes --show-labels

    # Remove Kata
    kubectl delete -f $YAMLPATH/kata-deploy/base/kata-deploy.yaml
    kubectl -n kube-system wait --timeout=10m --for=delete -l name=kata-deploy pod

    kubectl get pods,nodes --show-labels

    kubectl apply -f $YAMLPATH/kata-cleanup/base/kata-cleanup.yaml

    # The cleanup daemonset will run a single time, since it will clear the node-label. Thus, its difficult to
    # check the daemonset's status for completion. instead, let's wait until the kata-runtime labels are removed
    # from all of the worker nodes. If this doesn't happen after 2 minutes, let's fail
    timeout=120
    waitForLabelRemoval $timeout

    kubectl delete -f $YAMLPATH/kata-cleanup/base/kata-cleanup.yaml

    set +x
}
