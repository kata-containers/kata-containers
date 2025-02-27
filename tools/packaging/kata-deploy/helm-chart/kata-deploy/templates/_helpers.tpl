# Copyright (c) 2024 NVIDIA Corporation
#
# SPDX-License-Identifier: Apache-2.0
#

{{/*
Set the correct containerd conf path depending on the k8s distribution
*/}}
{{- define "containerdConfPath" -}}
{{- if eq .k8sDistribution "rke2" -}}
/var/lib/rancher/rke2/agent/etc/containerd/
{{- else if eq .k8sDistribution "k3s" -}}
 /var/lib/rancher/k3s/agent/etc/containerd/
{{- else if eq .k8sDistribution "k0s" -}}
/etc/k0s/containerd.d/
{{- else -}}
/etc/containerd/
{{- end -}}
{{- end -}}

