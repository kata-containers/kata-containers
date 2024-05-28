#!/bin/bash
#
# Copyright (c) 2024 Red Hat, Inc.
#
# SPDX-License-Identifier: Apache-2.0
#
# This script tries to removes most of the resources added by `test.sh` script
# from the cluster.

scripts_dir=$(dirname $0)
deployments_dir=${scripts_dir}/cluster/deployments
configs_dir=${scripts_dir}/configs

source ${scripts_dir}/lib.sh

# Set to 'yes' if you want to configure SELinux to permissive on the cluster
# workers.
#
SELINUX_PERMISSIVE=${SELINUX_PERMISSIVE:-no}

# Enable workaround for OCP 4.13 https://github.com/kata-containers/kata-containers/pull/9206
#
WORKAROUND_9206_CRIO=${WORKAROUND_9206_CRIO:-no}

# Ignore errors as we want best-effort-approach here
trap - ERR

# Delete webhook resources
oc delete -f "${scripts_dir}/../../tools/testing/kata-webhook/deploy"
oc delete -f "${scripts_dir}/cluster/deployments/configmap_kata-webhook.yaml.in"

# Delete potential smoke-test resources
oc delete -f "${scripts_dir}/smoke/service.yaml"
oc delete -f "${scripts_dir}/smoke/service_kubernetes.yaml"
oc delete -f "${scripts_dir}/smoke/http-server.yaml"

# Delete test.sh resources
oc delete -f "${deployments_dir}/relabel_selinux.yaml"
if [[ "$WORKAROUND_9206_CRIO" == "yes" ]]; then
	oc delete -f "${deployments_dir}/workaround-9206-crio-ds.yaml"
	oc delete -f "${deployments_dir}/workaround-9206-crio.yaml"
fi
[ ${SELINUX_PERMISSIVE} == "yes" ] && oc delete -f "${deployments_dir}/machineconfig_selinux.yaml.in"

# Delete kata-containers
pushd "$katacontainers_repo_dir/tools/packaging/kata-deploy"
oc delete -f kata-deploy/base/kata-deploy.yaml
oc -n kube-system wait --timeout=10m --for=delete -l name=kata-deploy pod
oc apply -f kata-cleanup/base/kata-cleanup.yaml
echo "Wait for all related pods to be gone"
( repeats=1; for i in $(seq 1 600); do
  oc get pods -l name="kubelet-kata-cleanup" --no-headers=true -n kube-system 2>&1 | grep "No resources found" -q && ((repeats++)) || repeats=1
  [ "$repeats" -gt 5 ] && echo kata-cleanup finished && break
  sleep 1
done) || { echo "There are still some kata-cleanup related pods after 600 iterations"; oc get all -n kube-system; exit -1; }
oc delete -f kata-cleanup/base/kata-cleanup.yaml
oc delete -f kata-rbac/base/kata-rbac.yaml
oc delete -f runtimeclasses/kata-runtimeClasses.yaml

