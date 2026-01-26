# Copyright (c) 2024 NVIDIA Corporation
#
# SPDX-License-Identifier: Apache-2.0
#

{{/*
Expand the name of the chart.
*/}}
{{- define "kata-deploy.name" -}}
{{- default .Chart.Name .Values.nameOverride | trunc 63 | trimSuffix "-" }}
{{- end }}

{{/*
Create a default fully qualified app name.
*/}}
{{- define "kata-deploy.fullname" -}}
{{- if .Values.fullnameOverride }}
{{- .Values.fullnameOverride | trunc 63 | trimSuffix "-" }}
{{- else }}
{{- $name := default .Chart.Name .Values.nameOverride }}
{{- if contains $name .Release.Name }}
{{- .Release.Name | trunc 63 | trimSuffix "-" }}
{{- else }}
{{- printf "%s-%s" .Release.Name $name | trunc 63 | trimSuffix "-" }}
{{- end }}
{{- end }}
{{- end }}

{{/*
Common labels
*/}}
{{- define "kata-deploy.labels" -}}
helm.sh/chart: {{ .Chart.Name }}-{{ .Chart.Version | replace "+" "_" }}
{{ include "kata-deploy.selectorLabels" . }}
app.kubernetes.io/managed-by: {{ .Release.Service }}
{{- end }}

{{/*
Selector labels
*/}}
{{- define "kata-deploy.selectorLabels" -}}
app.kubernetes.io/name: {{ include "kata-deploy.name" . }}
app.kubernetes.io/instance: {{ .Release.Name }}
{{- end }}

{{/*
Set the correct containerd conf path depending on the k8s distribution
*/}}
{{- define "containerdConfPath" -}}
{{- if eq .k8sDistribution "rke2" -}}
/var/lib/rancher/rke2/agent/etc/containerd/
{{- else if eq .k8sDistribution "k3s" -}}
 /var/lib/rancher/k3s/agent/etc/containerd/
{{- else if eq .k8sDistribution "k0s" -}}
/etc/k0s/
{{- else if eq .k8sDistribution "microk8s" -}}
/var/snap/microk8s/current/args/
{{- else -}}
/etc/containerd/
{{- end -}}
{{- end -}}

{{/*
Check if node-feature-discovery is already installed by someone else
Returns the namespace where node-feature-discovery is found, or empty string if not found
*/}}
{{- define "kata-deploy.detectExistingNFD" -}}
{{- $nfdWorkers := lookup "apps/v1" "DaemonSet" "" "" -}}
{{- $nfdMasters := lookup "apps/v1" "Deployment" "" "" -}}
{{- $foundNamespace := "" -}}
{{- $currentRelease := .Release.Name -}}
{{- range $nfdWorkers.items -}}
{{- if eq .metadata.name "node-feature-discovery-worker" -}}
{{- $helmRelease := "" -}}
{{- if .metadata.labels -}}
{{- $helmRelease = index .metadata.labels "app.kubernetes.io/instance" | default (index .metadata.labels "helm.sh/release") | default "" -}}
{{- end -}}
{{- if or (ne .metadata.namespace $.Release.Namespace) (and (eq .metadata.namespace $.Release.Namespace) (ne $helmRelease $currentRelease)) -}}
{{- $foundNamespace = .metadata.namespace -}}
{{- end -}}
{{- end -}}
{{- end -}}
{{- if not $foundNamespace -}}
{{- range $nfdMasters.items -}}
{{- if eq .metadata.name "node-feature-discovery-master" -}}
{{- $helmRelease := "" -}}
{{- if .metadata.labels -}}
{{- $helmRelease = index .metadata.labels "app.kubernetes.io/instance" | default (index .metadata.labels "helm.sh/release") | default "" -}}
{{- end -}}
{{- if or (ne .metadata.namespace $.Release.Namespace) (and (eq .metadata.namespace $.Release.Namespace) (ne $helmRelease $currentRelease)) -}}
{{- $foundNamespace = .metadata.namespace -}}
{{- end -}}
{{- end -}}
{{- end -}}
{{- end -}}
{{- $foundNamespace -}}
{{- end -}}

{{/*
Get enabled shims for a specific architecture from structured config.
Uses null-based defaults for disableAll support:
- enabled: ~ (null) + disableAll: false → enabled
- enabled: ~ (null) + disableAll: true  → disabled
- enabled: true  → always enabled (explicit override)
- enabled: false → always disabled (explicit override)
*/}}
{{- define "kata-deploy.getEnabledShimsForArch" -}}
{{- $arch := .arch -}}
{{- $disableAll := .root.Values.shims.disableAll | default false -}}
{{- $enabledShims := list -}}
{{- range $shimName, $shimConfig := .root.Values.shims -}}
{{- if ne $shimName "disableAll" -}}
{{- /* Determine if shim is enabled based on enabled field and disableAll */ -}}
{{- $shimEnabled := false -}}
{{- if eq $shimConfig.enabled true -}}
{{- /* Explicit true: always enabled */ -}}
{{- $shimEnabled = true -}}
{{- else if eq $shimConfig.enabled false -}}
{{- /* Explicit false: always disabled */ -}}
{{- $shimEnabled = false -}}
{{- else -}}
{{- /* Null/unset: use inverse of disableAll (enabled by default, disabled when disableAll=true) */ -}}
{{- if not $disableAll -}}
{{- $shimEnabled = true -}}
{{- end -}}
{{- end -}}
{{- if $shimEnabled -}}
{{- $archSupported := false -}}
{{- range $shimConfig.supportedArches -}}
{{- if eq . $arch -}}
{{- $archSupported = true -}}
{{- end -}}
{{- end -}}
{{- if $archSupported -}}
{{- $enabledShims = append $enabledShims $shimName -}}
{{- end -}}
{{- end -}}
{{- end -}}
{{- end -}}
{{- join " " $enabledShims -}}
{{- end -}}

{{/*
Get default shim for a specific architecture from structured config
*/}}
{{- define "kata-deploy.getDefaultShimForArch" -}}
{{- $arch := .arch -}}
{{- index .root.Values.defaultShim $arch -}}
{{- end -}}

{{/*
Get snapshotter handler mapping for a specific architecture from structured config
Format: shim1:snapshotter1,shim2:snapshotter2
*/}}
{{- define "kata-deploy.getSnapshotterHandlerMappingForArch" -}}
{{- $arch := .arch -}}
{{- $disableAll := .root.Values.shims.disableAll | default false -}}
{{- $mappings := list -}}
{{- range $shimName, $shimConfig := .root.Values.shims -}}
{{- if ne $shimName "disableAll" -}}
{{- $shimEnabled := false -}}
{{- if eq $shimConfig.enabled true -}}
{{- $shimEnabled = true -}}
{{- else if eq $shimConfig.enabled false -}}
{{- $shimEnabled = false -}}
{{- else if not $disableAll -}}
{{- $shimEnabled = true -}}
{{- end -}}
{{- if $shimEnabled -}}
{{- $archSupported := false -}}
{{- range $shimConfig.supportedArches -}}
{{- if eq . $arch -}}
{{- $archSupported = true -}}
{{- end -}}
{{- end -}}
{{- if $archSupported -}}
{{- if $shimConfig.containerd -}}
{{- $snapshotter := $shimConfig.containerd.snapshotter -}}
{{- if $snapshotter -}}
{{- $mappings = append $mappings (printf "%s:%s" $shimName $snapshotter) -}}
{{- end -}}
{{- end -}}
{{- end -}}
{{- end -}}
{{- end -}}
{{- end -}}
{{- join "," $mappings -}}
{{- end -}}

{{/*
Get pull type mapping for a specific architecture from structured config
Format: shim1:pullType1,shim2:pullType2
*/}}
{{- define "kata-deploy.getPullTypeMappingForArch" -}}
{{- $arch := .arch -}}
{{- $disableAll := .root.Values.shims.disableAll | default false -}}
{{- $mappings := list -}}
{{- range $shimName, $shimConfig := .root.Values.shims -}}
{{- if ne $shimName "disableAll" -}}
{{- $shimEnabled := false -}}
{{- if eq $shimConfig.enabled true -}}
{{- $shimEnabled = true -}}
{{- else if eq $shimConfig.enabled false -}}
{{- $shimEnabled = false -}}
{{- else if not $disableAll -}}
{{- $shimEnabled = true -}}
{{- end -}}
{{- if $shimEnabled -}}
{{- $archSupported := false -}}
{{- range $shimConfig.supportedArches -}}
{{- if eq . $arch -}}
{{- $archSupported = true -}}
{{- end -}}
{{- end -}}
{{- if $archSupported -}}
{{- $forceGuestPull := false -}}
{{- if and $shimConfig.containerd $shimConfig.containerd.forceGuestPull -}}
{{- $forceGuestPull = $shimConfig.containerd.forceGuestPull -}}
{{- end -}}
{{- if and $shimConfig.crio $shimConfig.crio.guestPull -}}
{{- $forceGuestPull = $shimConfig.crio.guestPull -}}
{{- end -}}
{{- if $forceGuestPull -}}
{{- $mappings = append $mappings (printf "%s:guest-pull" $shimName) -}}
{{- end -}}
{{- end -}}
{{- end -}}
{{- end -}}
{{- end -}}
{{- join "," $mappings -}}
{{- end -}}

{{/*
Get allowed hypervisor annotations for a specific architecture from structured config
Output format: "shim:annotation1,annotation2" (space-separated entries, each with shim:annotations where annotations are comma-separated)
*/}}
{{- define "kata-deploy.getAllowedHypervisorAnnotationsForArch" -}}
{{- $arch := .arch -}}
{{- $disableAll := .root.Values.shims.disableAll | default false -}}
{{- $perShimAnnotations := list -}}
{{- range $shimName, $shimConfig := .root.Values.shims -}}
{{- if ne $shimName "disableAll" -}}
{{- $shimEnabled := false -}}
{{- if eq $shimConfig.enabled true -}}
{{- $shimEnabled = true -}}
{{- else if eq $shimConfig.enabled false -}}
{{- $shimEnabled = false -}}
{{- else if not $disableAll -}}
{{- $shimEnabled = true -}}
{{- end -}}
{{- if $shimEnabled -}}
{{- $archSupported := false -}}
{{- range $shimConfig.supportedArches -}}
{{- if eq . $arch -}}
{{- $archSupported = true -}}
{{- end -}}
{{- end -}}
{{- if $archSupported -}}
{{- $shimAnnotations := list -}}
{{- range $annotation := $shimConfig.allowedHypervisorAnnotations -}}
{{- $shimAnnotations = append $shimAnnotations $annotation -}}
{{- end -}}
{{- if gt (len $shimAnnotations) 0 -}}
{{- $annotationsComma := join "," $shimAnnotations -}}
{{- $perShimEntry := printf "%s:%s" $shimName $annotationsComma -}}
{{- $perShimAnnotations = append $perShimAnnotations $perShimEntry -}}
{{- end -}}
{{- end -}}
{{- end -}}
{{- end -}}
{{- end -}}
{{- join " " $perShimAnnotations -}}
{{- end -}}

{{/*
Get agent HTTPS proxy from structured config
Builds per-shim semicolon-separated list: "shim1=value1;shim2=value2"
*/}}
{{- define "kata-deploy.getAgentHttpsProxy" -}}
{{- $disableAll := .Values.shims.disableAll | default false -}}
{{- $proxies := list -}}
{{- range $shimName, $shimConfig := .Values.shims -}}
{{- if ne $shimName "disableAll" -}}
{{- $shimEnabled := false -}}
{{- if eq $shimConfig.enabled true -}}
{{- $shimEnabled = true -}}
{{- else if eq $shimConfig.enabled false -}}
{{- $shimEnabled = false -}}
{{- else if not $disableAll -}}
{{- $shimEnabled = true -}}
{{- end -}}
{{- if and $shimEnabled $shimConfig.agent $shimConfig.agent.httpsProxy -}}
{{- $entry := printf "%s=%s" $shimName $shimConfig.agent.httpsProxy -}}
{{- $proxies = append $proxies $entry -}}
{{- end -}}
{{- end -}}
{{- end -}}
{{- join ";" $proxies -}}
{{- end -}}

{{/*
Get agent NO_PROXY from structured config
Builds per-shim semicolon-separated list: "shim1=value1;shim2=value2"
*/}}
{{- define "kata-deploy.getAgentNoProxy" -}}
{{- $disableAll := .Values.shims.disableAll | default false -}}
{{- $proxies := list -}}
{{- range $shimName, $shimConfig := .Values.shims -}}
{{- if ne $shimName "disableAll" -}}
{{- $shimEnabled := false -}}
{{- if eq $shimConfig.enabled true -}}
{{- $shimEnabled = true -}}
{{- else if eq $shimConfig.enabled false -}}
{{- $shimEnabled = false -}}
{{- else if not $disableAll -}}
{{- $shimEnabled = true -}}
{{- end -}}
{{- if and $shimEnabled $shimConfig.agent $shimConfig.agent.noProxy -}}
{{- $entry := printf "%s=%s" $shimName $shimConfig.agent.noProxy -}}
{{- $proxies = append $proxies $entry -}}
{{- end -}}
{{- end -}}
{{- end -}}
{{- join ";" $proxies -}}
{{- end -}}

{{/*
Get snapshotter setup list from structured config
*/}}
{{- define "kata-deploy.getSnapshotterSetup" -}}
{{- join "," .Values.snapshotter.setup -}}
{{- end -}}

{{/*
Get debug value from structured config
*/}}
{{- define "kata-deploy.getDebug" -}}
{{- if .Values.debug -}}
{{- "true" -}}
{{- else -}}
{{- "false" -}}
{{- end -}}
{{- end -}}

{{/*
Get EXPERIMENTAL_FORCE_GUEST_PULL for a specific architecture from structured config
Returns comma-separated list of shim names with forceGuestPull enabled
Note: EXPERIMENTAL_FORCE_GUEST_PULL only checks containerd.forceGuestPull, not crio.guestPull
*/}}
{{- define "kata-deploy.getForceGuestPullForArch" -}}
{{- $arch := .arch -}}
{{- $disableAll := .root.Values.shims.disableAll | default false -}}
{{- $shimNames := list -}}
{{- range $shimName, $shimConfig := .root.Values.shims -}}
{{- if ne $shimName "disableAll" -}}
{{- $shimEnabled := false -}}
{{- if eq $shimConfig.enabled true -}}
{{- $shimEnabled = true -}}
{{- else if eq $shimConfig.enabled false -}}
{{- $shimEnabled = false -}}
{{- else if not $disableAll -}}
{{- $shimEnabled = true -}}
{{- end -}}
{{- if $shimEnabled -}}
{{- $archSupported := false -}}
{{- range $shimConfig.supportedArches -}}
{{- if eq . $arch -}}
{{- $archSupported = true -}}
{{- end -}}
{{- end -}}
{{- if $archSupported -}}
{{- if and $shimConfig.containerd $shimConfig.containerd.forceGuestPull -}}
{{- $shimNames = append $shimNames $shimName -}}
{{- end -}}
{{- end -}}
{{- end -}}
{{- end -}}
{{- end -}}
{{- join "," $shimNames -}}
{{- end -}}
