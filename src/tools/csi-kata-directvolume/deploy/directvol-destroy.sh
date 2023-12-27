#!/usr/bin/env bash
#
# Copyright 2017 The Kubernetes Authors.
# Copyright (c) 2023 Ant Group
#
# SPDX-License-Identifier: Apache-2.0
#

set -e
set -o pipefail

# Deleting all the resources installed by the directvol-deploy script.
# Every resource in the driver installation has the label representing the installation instance.
# Using app.kubernetes.io/instance: directvolume.csi.katacontainers.io and app.kubernetes.io/part-of: 
# csi-driver-kata-directvolume labels to identify the installation set
kubectl delete all --all-namespaces -l app.kubernetes.io/instance=directvolume.csi.katacontainers.io,app.kubernetes.io/part-of=csi-driver-kata-directvolume --wait=true
kubectl delete role,clusterrole,rolebinding,clusterrolebinding,serviceaccount,storageclass,csidriver --all-namespaces -l app.kubernetes.io/instance=directvolume.csi.katacontainers.io,app.kubernetes.io/part-of=csi-driver-kata-directvolume --wait=true
