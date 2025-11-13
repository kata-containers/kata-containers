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
Get enabled shims for a specific architecture from structured config
Supports backward compatibility with old env.shims_* values
*/}}
{{- define "kata-deploy.getEnabledShimsForArch" -}}
{{- $arch := .arch -}}
{{- $envVar := "" -}}
{{- if eq $arch "amd64" -}}
{{- $envVar = "shims_x86_64" -}}
{{- else if eq $arch "arm64" -}}
{{- $envVar = "shims_aarch64" -}}
{{- else if eq $arch "s390x" -}}
{{- $envVar = "shims_s390x" -}}
{{- else if eq $arch "ppc64le" -}}
{{- $envVar = "shims_ppc64le" -}}
{{- end -}}
{{- /* Check for legacy env value first */ -}}
{{- if and $envVar (index .root.Values.env $envVar) -}}
{{- index .root.Values.env $envVar -}}
{{- else -}}
{{- /* Use new structured config */ -}}
{{- $enabledShims := list -}}
{{- range $shimName, $shimConfig := .root.Values.shims -}}
{{- if $shimConfig.enabled -}}
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
{{- join " " $enabledShims -}}
{{- end -}}
{{- end -}}

{{/*
Get default shim for a specific architecture from structured config
Supports backward compatibility with old env.defaultShim_* values
*/}}
{{- define "kata-deploy.getDefaultShimForArch" -}}
{{- $arch := .arch -}}
{{- $envVar := "" -}}
{{- if eq $arch "amd64" -}}
{{- $envVar = "defaultShim_x86_64" -}}
{{- else if eq $arch "arm64" -}}
{{- $envVar = "defaultShim_aarch64" -}}
{{- else if eq $arch "s390x" -}}
{{- $envVar = "defaultShim_s390x" -}}
{{- else if eq $arch "ppc64le" -}}
{{- $envVar = "defaultShim_ppc64le" -}}
{{- end -}}
{{- /* Check for legacy env value first */ -}}
{{- if and $envVar (index .root.Values.env $envVar) -}}
{{- index .root.Values.env $envVar -}}
{{- else if eq $arch "amd64" -}}
{{- /* Fallback to legacy defaultShim for amd64 */ -}}
{{- if .root.Values.env.defaultShim -}}
{{- .root.Values.env.defaultShim -}}
{{- else -}}
{{- /* Use new structured config */ -}}
{{- index .root.Values.defaultShim $arch -}}
{{- end -}}
{{- else -}}
{{- /* Use new structured config */ -}}
{{- index .root.Values.defaultShim $arch -}}
{{- end -}}
{{- end -}}

{{/*
Get snapshotter handler mapping for a specific architecture from structured config
Format: shim1:snapshotter1,shim2:snapshotter2
Supports backward compatibility with old env.snapshotterHandlerMapping_* values
*/}}
{{- define "kata-deploy.getSnapshotterHandlerMappingForArch" -}}
{{- $arch := .arch -}}
{{- $envVar := "" -}}
{{- if eq $arch "amd64" -}}
{{- $envVar = "snapshotterHandlerMapping_x86_64" -}}
{{- else if eq $arch "arm64" -}}
{{- $envVar = "snapshotterHandlerMapping_aarch64" -}}
{{- else if eq $arch "s390x" -}}
{{- $envVar = "snapshotterHandlerMapping_s390x" -}}
{{- else if eq $arch "ppc64le" -}}
{{- $envVar = "snapshotterHandlerMapping_ppc64le" -}}
{{- end -}}
{{- /* Check for legacy env value first */ -}}
{{- if and $envVar (index .root.Values.env $envVar) -}}
{{- index .root.Values.env $envVar -}}
{{- else -}}
{{- /* Use new structured config */ -}}
{{- $mappings := list -}}
{{- range $shimName, $shimConfig := .root.Values.shims -}}
{{- if $shimConfig.enabled -}}
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
{{- join "," $mappings -}}
{{- end -}}
{{- end -}}

{{/*
Get pull type mapping for a specific architecture from structured config
Format: shim1:pullType1,shim2:pullType2
Supports backward compatibility with old env.pullTypeMapping_* values
*/}}
{{- define "kata-deploy.getPullTypeMappingForArch" -}}
{{- $arch := .arch -}}
{{- $envVar := "" -}}
{{- if eq $arch "amd64" -}}
{{- $envVar = "pullTypeMapping_x86_64" -}}
{{- else if eq $arch "arm64" -}}
{{- $envVar = "pullTypeMapping_aarch64" -}}
{{- else if eq $arch "s390x" -}}
{{- $envVar = "pullTypeMapping_s390x" -}}
{{- else if eq $arch "ppc64le" -}}
{{- $envVar = "pullTypeMapping_ppc64le" -}}
{{- end -}}
{{- /* Check for legacy env value first */ -}}
{{- if and $envVar (index .root.Values.env $envVar) -}}
{{- index .root.Values.env $envVar -}}
{{- else -}}
{{- /* Use new structured config */ -}}
{{- $mappings := list -}}
{{- range $shimName, $shimConfig := .root.Values.shims -}}
{{- if $shimConfig.enabled -}}
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
{{- join "," $mappings -}}
{{- end -}}
{{- end -}}

{{/*
Get allowed hypervisor annotations for a specific architecture from structured config
Supports backward compatibility with old env.allowedHypervisorAnnotations value
*/}}
{{- define "kata-deploy.getAllowedHypervisorAnnotationsForArch" -}}
{{- $arch := .arch -}}
{{- /* Check for legacy env value first (applies to all arches) */ -}}
{{- if .root.Values.env.allowedHypervisorAnnotations -}}
{{- .root.Values.env.allowedHypervisorAnnotations -}}
{{- else -}}
{{- /* Use new structured config */ -}}
{{- $annotations := list -}}
{{- range $shimName, $shimConfig := .root.Values.shims -}}
{{- if $shimConfig.enabled -}}
{{- $archSupported := false -}}
{{- range $shimConfig.supportedArches -}}
{{- if eq . $arch -}}
{{- $archSupported = true -}}
{{- end -}}
{{- end -}}
{{- if $archSupported -}}
{{- range $annotation := $shimConfig.allowedHypervisorAnnotations -}}
{{- $found := false -}}
{{- range $existingAnnotation := $annotations -}}
{{- if eq $existingAnnotation $annotation -}}
{{- $found = true -}}
{{- end -}}
{{- end -}}
{{- if not $found -}}
{{- $annotations = append $annotations $annotation -}}
{{- end -}}
{{- end -}}
{{- end -}}
{{- end -}}
{{- end -}}
{{- join "," $annotations -}}
{{- end -}}
{{- end -}}

{{/*
Get agent HTTPS proxy from structured config
Returns the first non-empty httpsProxy found in enabled shims
Supports backward compatibility with old env.agentHttpsProxy value
*/}}
{{- define "kata-deploy.getAgentHttpsProxy" -}}
{{- /* Check for legacy env value first */ -}}
{{- if .Values.env.agentHttpsProxy -}}
{{- .Values.env.agentHttpsProxy -}}
{{- else -}}
{{- /* Use new structured config */ -}}
{{- $proxy := "" -}}
{{- range $shimName, $shimConfig := .Values.shims -}}
{{- if and $shimConfig.enabled $shimConfig.agent $shimConfig.agent.httpsProxy -}}
{{- $proxy = $shimConfig.agent.httpsProxy -}}
{{- break -}}
{{- end -}}
{{- end -}}
{{- $proxy -}}
{{- end -}}
{{- end -}}

{{/*
Get agent NO_PROXY from structured config
Returns the first non-empty noProxy found in enabled shims
Supports backward compatibility with old env.agentNoProxy value
*/}}
{{- define "kata-deploy.getAgentNoProxy" -}}
{{- /* Check for legacy env value first */ -}}
{{- if .Values.env.agentNoProxy -}}
{{- .Values.env.agentNoProxy -}}
{{- else -}}
{{- /* Use new structured config */ -}}
{{- $proxy := "" -}}
{{- range $shimName, $shimConfig := .Values.shims -}}
{{- if and $shimConfig.enabled $shimConfig.agent $shimConfig.agent.noProxy -}}
{{- $proxy = $shimConfig.agent.noProxy -}}
{{- break -}}
{{- end -}}
{{- end -}}
{{- $proxy -}}
{{- end -}}
{{- end -}}

{{/*
Get snapshotter setup list from structured config
Supports backward compatibility with old env._experimentalSetupSnapshotter value
*/}}
{{- define "kata-deploy.getSnapshotterSetup" -}}
{{- /* Check for legacy env value first */ -}}
{{- if .Values.env._experimentalSetupSnapshotter -}}
{{- .Values.env._experimentalSetupSnapshotter -}}
{{- else -}}
{{- /* Use new structured config */ -}}
{{- join "," .Values.snapshotter.setup -}}
{{- end -}}
{{- end -}}

{{/*
Get debug value from structured config
Supports backward compatibility with old env.debug value
*/}}
{{- define "kata-deploy.getDebug" -}}
{{- /* Check for legacy env value first */ -}}
{{- if .Values.env.debug -}}
{{- .Values.env.debug -}}
{{- else -}}
{{- /* Use new structured config */ -}}
{{- if .Values.debug -}}
{{- "true" -}}
{{- else -}}
{{- "false" -}}
{{- end -}}
{{- end -}}
{{- end -}}

{{/*
Get EXPERIMENTAL_FORCE_GUEST_PULL for a specific architecture from structured config
Supports backward compatibility with old env._experimentalForceGuestPull_* values
*/}}
{{- define "kata-deploy.getForceGuestPullForArch" -}}
{{- $arch := .arch -}}
{{- $envVar := "" -}}
{{- if eq $arch "amd64" -}}
{{- $envVar = "_experimentalForceGuestPull_x86_64" -}}
{{- else if eq $arch "arm64" -}}
{{- $envVar = "_experimentalForceGuestPull_aarch64" -}}
{{- else if eq $arch "s390x" -}}
{{- $envVar = "_experimentalForceGuestPull_s390x" -}}
{{- else if eq $arch "ppc64le" -}}
{{- $envVar = "_experimentalForceGuestPull_ppc64le" -}}
{{- end -}}
{{- /* Check for legacy env value first */ -}}
{{- $legacyValue := "" -}}
{{- if and $envVar (index .root.Values.env $envVar) -}}
{{- $legacyValue = index .root.Values.env $envVar -}}
{{- else if eq $arch "amd64" -}}
{{- /* Fallback to legacy _experimentalForceGuestPull for amd64 */ -}}
{{- if .root.Values.env._experimentalForceGuestPull -}}
{{- $legacyValue = .root.Values.env._experimentalForceGuestPull -}}
{{- end -}}
{{- end -}}
{{- if $legacyValue -}}
{{- /* Legacy value is already a comma-separated list of shim names */ -}}
{{- $legacyValue -}}
{{- else -}}
{{- /* Use new structured config - return comma-separated list of shim names */ -}}
{{- /* Note: EXPERIMENTAL_FORCE_GUEST_PULL only checks containerd.forceGuestPull, not crio.guestPull */ -}}
{{- $shimNames := list -}}
{{- range $shimName, $shimConfig := .root.Values.shims -}}
{{- if $shimConfig.enabled -}}
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
{{- join "," $shimNames -}}
{{- end -}}
{{- end -}}

