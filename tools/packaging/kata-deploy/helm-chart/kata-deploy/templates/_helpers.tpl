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
Version annotations for RuntimeClass objects.
Uses AppVersion (Kata Containers release), matching the default kata-deploy image tag.
*/}}
{{- define "kata-deploy.runtimeclassAnnotations" -}}
katacontainers.io/kata-version: {{ .Chart.AppVersion | quote }}
{{- end }}

{{/*
Set the correct containerd conf path depending on the k8s distribution.
If containerd.configDir is set explicitly, use that instead.
*/}}
{{- define "containerdConfPath" -}}
{{- if and .containerd .containerd.configDir -}}
{{- .containerd.configDir -}}
{{- else if eq .k8sDistribution "rke2" -}}
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
Set the CRI containerd socket URI depending on the k8s distribution.
If containerd.runtimeSocket is set explicitly, use that instead.
*/}}
{{- define "containerdRuntimeSocket" -}}
{{- if and .containerd .containerd.runtimeSocket -}}
{{- .containerd.runtimeSocket -}}
{{- else if or (eq .k8sDistribution "k3s") (eq .k8sDistribution "rke2") -}}
unix:///run/k3s/containerd/containerd.sock
{{- else if eq .k8sDistribution "k0s" -}}
unix:///run/k0s/containerd.sock
{{- else if eq .k8sDistribution "microk8s" -}}
unix:///var/snap/microk8s/common/run/containerd.sock
{{- else -}}
unix:///run/containerd/containerd.sock
{{- end -}}
{{- end -}}

{{/*
Resolve the kata-monitor CRI runtime endpoint.
When monitor.runtimeEndpoint is empty, inherit containerd.runtimeSocket or
derive it from k8sDistribution.
*/}}
{{- define "monitorRuntimeEndpoint" -}}
{{- if .Values.monitor.runtimeEndpoint -}}
{{- .Values.monitor.runtimeEndpoint -}}
{{- else -}}
{{- include "containerdRuntimeSocket" .Values -}}
{{- end -}}
{{- end -}}

{{/*
Filesystem path of the CRI runtime socket, derived from monitorRuntimeEndpoint.
*/}}
{{- define "monitorRuntimeSocketPath" -}}
{{- $endpoint := include "monitorRuntimeEndpoint" . -}}
{{- if hasPrefix "unix://" $endpoint -}}
{{- trimPrefix "unix://" $endpoint -}}
{{- else if hasPrefix "unix:" $endpoint -}}
{{- trimPrefix "unix:" $endpoint -}}
{{- else -}}
{{- $endpoint -}}
{{- end -}}
{{- end -}}

{{/*
Host directory containing the CRI runtime socket, derived from monitorRuntimeEndpoint.
Used for kata-monitor volume hostPath and mountPath so the socket is reachable in-container.
*/}}
{{- define "monitorRuntimeSocketDir" -}}
{{- include "monitorRuntimeSocketPath" . | dir -}}
{{- end -}}

{{/*
Resolve kata-monitor log level.
Honors monitor.logLevel, then the chart-wide logLevel, then debug:true -> debug.
*/}}
{{- define "monitorLogLevel" -}}
{{- $logLevel := .Values.monitor.logLevel | default "" | trim -}}
{{- if not $logLevel -}}
{{- $logLevel = .Values.logLevel | default "" | trim -}}
{{- end -}}
{{- if and (not $logLevel) .Values.debug -}}
{{- $logLevel = "debug" -}}
{{- end -}}
{{- if not $logLevel -}}
{{- $logLevel = "info" -}}
{{- end -}}
{{- $logLevel -}}
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
Get default shim for a specific architecture from structured config.
Returns the configured default shim only if it is actually enabled and
supports the requested architecture. Returns empty string otherwise so
that callers can skip setting the env var rather than propagating a
bogus value that would cause kata-deploy to fail at runtime.
*/}}
{{- define "kata-deploy.getDefaultShimForArch" -}}
{{- $arch := .arch -}}
{{- $defaultShim := index .root.Values.defaultShim $arch -}}
{{- if $defaultShim -}}
{{- $enabledShims := include "kata-deploy.getEnabledShimsForArch" (dict "root" .root "arch" $arch) | trim | splitList " " -}}
{{- if has $defaultShim $enabledShims -}}
{{- $defaultShim -}}
{{- end -}}
{{- end -}}
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
Main kata-deploy image reference for the DaemonSet.
Supports tag (reference:tag) and digest (reference@sha256:...) formats.
When reference contains "@" (digest), use reference as-is; otherwise use reference:tag (tag defaults to Chart.AppVersion).
*/}}
{{- define "kata-deploy.image" -}}
{{- $ref := .Values.image.reference -}}
{{- $tag := default .Chart.AppVersion .Values.image.tag | toString -}}
{{- if contains "@" $ref -}}
{{- $ref -}}
{{- else -}}
{{- printf "%s:%s" $ref $tag -}}
{{- end -}}
{{- end -}}

{{/*
kubectl image reference for verification and cleanup jobs.
Supports tag (reference:tag) and digest (reference@sha256:...) formats.
When reference already contains "@" (digest) or tag is empty, use reference as-is.
*/}}
{{- define "kata-deploy.kubectlImage" -}}
{{- $ref := .Values.kubectlImage.reference -}}
{{- $tag := .Values.kubectlImage.tag | toString -}}
{{- if or (contains "@" $ref) (eq $tag "") -}}
{{- $ref -}}
{{- else -}}
{{- printf "%s:%s" $ref $tag -}}
{{- end -}}
{{- end -}}

{{/*
kata-monitor image reference for optional monitor DaemonSet.
Supports tag (reference:tag) and digest (reference@sha256:...) formats.
When reference contains "@" (digest), use reference as-is; otherwise use
reference:tag (tag defaults to Chart.AppVersion).
*/}}
{{- define "kata-deploy.monitorImage" -}}
{{- $ref := .Values.monitor.image.reference -}}
{{- $tag := default .Chart.AppVersion .Values.monitor.image.tag | toString -}}
{{- if contains "@" $ref -}}
{{- $ref -}}
{{- else -}}
{{- printf "%s:%s" $ref $tag -}}
{{- end -}}
{{- end -}}

{{/*
Dispatcher image reference for the job-mode dispatcher (kata-deploy-job-dispatcher).
Supports tag (reference:tag) and digest (reference@sha256:...) formats; tag
defaults to Chart.AppVersion.
*/}}
{{- define "kata-deploy.dispatcherImage" -}}
{{- $ref := .Values.job.dispatcherImage.reference -}}
{{- $tag := default .Chart.AppVersion .Values.job.dispatcherImage.tag | toString -}}
{{- if contains "@" $ref -}}
{{- $ref -}}
{{- else -}}
{{- printf "%s:%s" $ref $tag -}}
{{- end -}}
{{- end -}}

{{/*
Get snapshotter setup list from structured config
*/}}
{{- define "kata-deploy.getSnapshotterSetup" -}}
{{- join "," .Values.snapshotter.setup -}}
{{- end -}}

{{/*
Get EROFS merge mode from structured config ("merged" or "unmerged")
*/}}
{{- define "kata-deploy.getErofsMergeMode" -}}
{{- .Values.snapshotter.erofsMergeMode | default "" -}}
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
Common environment variables for any pod that runs the kata-deploy binary
(DaemonSet, staged JobSet install/cleanup Jobs, reconcile-created Jobs).

These are all derived from chart values and are independent of the deployment
model, so they are shared verbatim. HEALTH_PORT and the health probes are NOT
included here: they only matter for the long-running install pod (DaemonSet),
not the short-lived staged Jobs.

Emitted at column 0; callers must indent with `nindent` to the right depth,
e.g. `{{- include "kata-deploy.commonEnv" . | nindent 8 }}`.
*/}}
{{- define "kata-deploy.commonEnv" -}}
- name: NODE_NAME
  valueFrom:
    fieldRef:
      fieldPath: spec.nodeName
{{- if .Values.env.multiInstallSuffix }}
- name: DAEMONSET_NAME
  value: {{ printf "%s-%s" .Chart.Name .Values.env.multiInstallSuffix | quote }}
{{- else }}
- name: DAEMONSET_NAME
  value: {{ .Chart.Name | quote }}
{{- end }}
- name: DEBUG
  value: {{ include "kata-deploy.getDebug" . | quote }}
{{- $shimsAmd64 := include "kata-deploy.getEnabledShimsForArch" (dict "root" . "arch" "amd64") | trim -}}
{{- if $shimsAmd64 }}
- name: SHIMS_X86_64
  value: {{ $shimsAmd64 | quote }}
{{- end }}
{{- $shimsArm64 := include "kata-deploy.getEnabledShimsForArch" (dict "root" . "arch" "arm64") | trim -}}
{{- if $shimsArm64 }}
- name: SHIMS_AARCH64
  value: {{ $shimsArm64 | quote }}
{{- end }}
{{- $shimsS390x := include "kata-deploy.getEnabledShimsForArch" (dict "root" . "arch" "s390x") | trim -}}
{{- if $shimsS390x }}
- name: SHIMS_S390X
  value: {{ $shimsS390x | quote }}
{{- end }}
{{- $shimsPpc64le := include "kata-deploy.getEnabledShimsForArch" (dict "root" . "arch" "ppc64le") | trim -}}
{{- if $shimsPpc64le }}
- name: SHIMS_PPC64LE
  value: {{ $shimsPpc64le | quote }}
{{- end }}
{{- $defaultShimAmd64 := include "kata-deploy.getDefaultShimForArch" (dict "root" . "arch" "amd64") | trim -}}
{{- if $defaultShimAmd64 }}
- name: DEFAULT_SHIM_X86_64
  value: {{ $defaultShimAmd64 | quote }}
{{- end }}
{{- $defaultShimArm64 := include "kata-deploy.getDefaultShimForArch" (dict "root" . "arch" "arm64") | trim -}}
{{- if $defaultShimArm64 }}
- name: DEFAULT_SHIM_AARCH64
  value: {{ $defaultShimArm64 | quote }}
{{- end }}
{{- $defaultShimS390x := include "kata-deploy.getDefaultShimForArch" (dict "root" . "arch" "s390x") | trim -}}
{{- if $defaultShimS390x }}
- name: DEFAULT_SHIM_S390X
  value: {{ $defaultShimS390x | quote }}
{{- end }}
{{- $defaultShimPpc64le := include "kata-deploy.getDefaultShimForArch" (dict "root" . "arch" "ppc64le") | trim -}}
{{- if $defaultShimPpc64le }}
- name: DEFAULT_SHIM_PPC64LE
  value: {{ $defaultShimPpc64le | quote }}
{{- end }}
{{- $allowedHypervisorAnnotationsAmd64 := include "kata-deploy.getAllowedHypervisorAnnotationsForArch" (dict "root" . "arch" "amd64") | trim -}}
{{- if $allowedHypervisorAnnotationsAmd64 }}
- name: ALLOWED_HYPERVISOR_ANNOTATIONS_X86_64
  value: {{ $allowedHypervisorAnnotationsAmd64 | quote }}
{{- end }}
{{- $allowedHypervisorAnnotationsArm64 := include "kata-deploy.getAllowedHypervisorAnnotationsForArch" (dict "root" . "arch" "arm64") | trim -}}
{{- if $allowedHypervisorAnnotationsArm64 }}
- name: ALLOWED_HYPERVISOR_ANNOTATIONS_AARCH64
  value: {{ $allowedHypervisorAnnotationsArm64 | quote }}
{{- end }}
{{- $allowedHypervisorAnnotationsS390x := include "kata-deploy.getAllowedHypervisorAnnotationsForArch" (dict "root" . "arch" "s390x") | trim -}}
{{- if $allowedHypervisorAnnotationsS390x }}
- name: ALLOWED_HYPERVISOR_ANNOTATIONS_S390X
  value: {{ $allowedHypervisorAnnotationsS390x | quote }}
{{- end }}
{{- $allowedHypervisorAnnotationsPpc64le := include "kata-deploy.getAllowedHypervisorAnnotationsForArch" (dict "root" . "arch" "ppc64le") | trim -}}
{{- if $allowedHypervisorAnnotationsPpc64le }}
- name: ALLOWED_HYPERVISOR_ANNOTATIONS_PPC64LE
  value: {{ $allowedHypervisorAnnotationsPpc64le | quote }}
{{- end }}
{{- $snapshotterHandlerMappingAmd64 := include "kata-deploy.getSnapshotterHandlerMappingForArch" (dict "root" . "arch" "amd64") | trim -}}
{{- if $snapshotterHandlerMappingAmd64 }}
- name: SNAPSHOTTER_HANDLER_MAPPING_X86_64
  value: {{ $snapshotterHandlerMappingAmd64 | quote }}
{{- end }}
{{- $snapshotterHandlerMappingArm64 := include "kata-deploy.getSnapshotterHandlerMappingForArch" (dict "root" . "arch" "arm64") | trim -}}
{{- if $snapshotterHandlerMappingArm64 }}
- name: SNAPSHOTTER_HANDLER_MAPPING_AARCH64
  value: {{ $snapshotterHandlerMappingArm64 | quote }}
{{- end }}
{{- $snapshotterHandlerMappingS390x := include "kata-deploy.getSnapshotterHandlerMappingForArch" (dict "root" . "arch" "s390x") | trim -}}
{{- if $snapshotterHandlerMappingS390x }}
- name: SNAPSHOTTER_HANDLER_MAPPING_S390X
  value: {{ $snapshotterHandlerMappingS390x | quote }}
{{- end }}
{{- $snapshotterHandlerMappingPpc64le := include "kata-deploy.getSnapshotterHandlerMappingForArch" (dict "root" . "arch" "ppc64le") | trim -}}
{{- if $snapshotterHandlerMappingPpc64le }}
- name: SNAPSHOTTER_HANDLER_MAPPING_PPC64LE
  value: {{ $snapshotterHandlerMappingPpc64le | quote }}
{{- end }}
{{- $agentHttpsProxy := include "kata-deploy.getAgentHttpsProxy" . | trim -}}
{{- if $agentHttpsProxy }}
- name: AGENT_HTTPS_PROXY
  value: {{ $agentHttpsProxy | quote }}
{{- end }}
{{- $agentNoProxy := include "kata-deploy.getAgentNoProxy" . | trim -}}
{{- if $agentNoProxy }}
- name: AGENT_NO_PROXY
  value: {{ $agentNoProxy | quote }}
{{- end }}
{{- $pullTypeMappingAmd64 := include "kata-deploy.getPullTypeMappingForArch" (dict "root" . "arch" "amd64") | trim -}}
{{- if $pullTypeMappingAmd64 }}
- name: PULL_TYPE_MAPPING_X86_64
  value: {{ $pullTypeMappingAmd64 | quote }}
{{- end }}
{{- $pullTypeMappingArm64 := include "kata-deploy.getPullTypeMappingForArch" (dict "root" . "arch" "arm64") | trim -}}
{{- if $pullTypeMappingArm64 }}
- name: PULL_TYPE_MAPPING_AARCH64
  value: {{ $pullTypeMappingArm64 | quote }}
{{- end }}
{{- $pullTypeMappingS390x := include "kata-deploy.getPullTypeMappingForArch" (dict "root" . "arch" "s390x") | trim -}}
{{- if $pullTypeMappingS390x }}
- name: PULL_TYPE_MAPPING_S390X
  value: {{ $pullTypeMappingS390x | quote }}
{{- end }}
{{- $pullTypeMappingPpc64le := include "kata-deploy.getPullTypeMappingForArch" (dict "root" . "arch" "ppc64le") | trim -}}
{{- if $pullTypeMappingPpc64le }}
- name: PULL_TYPE_MAPPING_PPC64LE
  value: {{ $pullTypeMappingPpc64le | quote }}
{{- end }}
- name: INSTALLATION_PREFIX
  value: {{ .Values.env.installationPrefix | quote }}
- name: MULTI_INSTALL_SUFFIX
  value: {{ .Values.env.multiInstallSuffix | quote }}
{{- $snapshotterSetup := include "kata-deploy.getSnapshotterSetup" . | trim -}}
{{- if $snapshotterSetup }}
- name: EXPERIMENTAL_SETUP_SNAPSHOTTER
  value: {{ $snapshotterSetup | quote }}
{{- end }}
{{- $erofsMergeMode := include "kata-deploy.getErofsMergeMode" . | trim -}}
{{- if $erofsMergeMode }}
- name: EROFS_MERGE_MODE
  value: {{ $erofsMergeMode | quote }}
{{- end }}
{{- if .Values.snapshotter.erofsSnapshotterMode | trim }}
- name: EROFS_SNAPSHOTTER_MODE
  value: {{ .Values.snapshotter.erofsSnapshotterMode | trim | quote }}
{{- end }}
{{- if .Values.snapshotter.erofsDmverity }}
- name: EROFS_DMVERITY
  value: "dmverity"
{{- end }}
{{- $forceGuestPullAmd64 := include "kata-deploy.getForceGuestPullForArch" (dict "root" . "arch" "amd64") | trim -}}
{{- if $forceGuestPullAmd64 }}
- name: EXPERIMENTAL_FORCE_GUEST_PULL_X86_64
  value: {{ $forceGuestPullAmd64 | quote }}
{{- end }}
{{- $forceGuestPullArm64 := include "kata-deploy.getForceGuestPullForArch" (dict "root" . "arch" "arm64") | trim -}}
{{- if $forceGuestPullArm64 }}
- name: EXPERIMENTAL_FORCE_GUEST_PULL_AARCH64
  value: {{ $forceGuestPullArm64 | quote }}
{{- end }}
{{- $forceGuestPullS390x := include "kata-deploy.getForceGuestPullForArch" (dict "root" . "arch" "s390x") | trim -}}
{{- if $forceGuestPullS390x }}
- name: EXPERIMENTAL_FORCE_GUEST_PULL_S390X
  value: {{ $forceGuestPullS390x | quote }}
{{- end }}
{{- $forceGuestPullPpc64le := include "kata-deploy.getForceGuestPullForArch" (dict "root" . "arch" "ppc64le") | trim -}}
{{- if $forceGuestPullPpc64le }}
- name: EXPERIMENTAL_FORCE_GUEST_PULL_PPC64LE
  value: {{ $forceGuestPullPpc64le | quote }}
{{- end }}
{{- if .Values.containerd.configFileName | trim }}
- name: CONTAINERD_CONFIG_FILE_NAME
  value: {{ .Values.containerd.configFileName | trim | quote }}
{{- end }}
{{- if .Values.containerd.userDropIn | trim }}
- name: CONTAINERD_USER_DROP_IN_SOURCE_FILE
  value: "/custom-containerd-config/containerd-user-dropin.toml"
{{- end }}
{{- with .Values.env.hostOS }}
- name: HOST_OS
  value: {{ . | quote }}
{{- end }}
{{- if and .Values.customRuntimes.enabled .Values.customRuntimes.runtimes }}
- name: CUSTOM_RUNTIMES_ENABLED
  value: "true"
{{- end }}
{{- /* Devkit debug extension: only effective together with debug (the debug
       console must be enabled for it to be reachable). */ -}}
{{- if and .Values.debug .Values.devkit }}
- name: DEVKIT
  value: "true"
{{- end }}
{{- with .Values.startupTaints }}
- name: STARTUP_TAINTS
  value: {{ join "," . | quote }}
{{- end }}
{{- end -}}

{{/*
Build a Kubernetes label-selector STRING (the form accepted by the apiserver
and `kubectl --selector`) from an equality map plus a list of match-expression
requirements. This is handed to `kata-deploy-job-dispatcher --node-selector`, which
resolves the actual target nodes LIVE at run time (so node membership is never
frozen into the Helm release).

Arguments (dict):
  eq    - equality label map           -> "k=v"
  exprs - list of {key, operator, values}:
            Exists       -> "key"
            DoesNotExist -> "!key"
            In           -> "key in (v1,v2)"
            NotIn        -> "key notin (v1,v2)"

Returns the comma-joined selector string (possibly empty, meaning "all nodes").
*/}}
{{- define "kata-deploy.nodeLabelSelector" -}}
{{- $parts := list -}}
{{- range $k, $v := (.eq | default dict) -}}
{{- $parts = append $parts (printf "%s=%s" $k $v) -}}
{{- end -}}
{{- range $expr := (.exprs | default list) -}}
{{- $op := $expr.operator -}}
{{- if eq $op "Exists" -}}
{{- $parts = append $parts $expr.key -}}
{{- else if eq $op "DoesNotExist" -}}
{{- $parts = append $parts (printf "!%s" $expr.key) -}}
{{- else if eq $op "In" -}}
{{- $parts = append $parts (printf "%s in (%s)" $expr.key (join "," ($expr.values | default list))) -}}
{{- else if eq $op "NotIn" -}}
{{- $parts = append $parts (printf "%s notin (%s)" $expr.key (join "," ($expr.values | default list))) -}}
{{- else -}}
{{- fail (printf "nodeSelectorExpressions: unsupported operator %q for key %q (use In, NotIn, Exists, DoesNotExist)" $op $expr.key) -}}
{{- end -}}
{{- end -}}
{{- join "," $parts -}}
{{- end -}}

{{/*
Per-node staged Job manifest (deploymentMode: job), embedded verbatim into the
job-templates ConfigMap. The dispatcher (kata-deploy-job-dispatcher) clones this once per
target node, injecting metadata.name + spec.template.spec.nodeName, so the
template itself carries NO node identity and NO Helm hook annotations.

Arguments (dict):
  root  - top-level context (.)
  stage - "install" | "cleanup"

install pipeline:  host-check -> artifacts -> cri (initContainers) ; label (main)
cleanup pipeline:  unlabel -> revert-cri    (initContainers) ; remove-artifacts (main)

Emitted at column 0 (a standalone Job document); embed with `indent` at the call
site under a ConfigMap data key.
*/}}
{{- define "kata-deploy.perNodeJob" -}}
{{- $root := .root -}}
{{- $stage := .stage -}}
apiVersion: batch/v1
kind: Job
metadata:
  labels:
    app.kubernetes.io/name: {{ include "kata-deploy.name" $root }}
    app.kubernetes.io/instance: {{ $root.Release.Name }}
    kata-deploy/stage: {{ $stage }}
spec:
  backoffLimit: {{ $root.Values.job.backoffLimit }}
  ttlSecondsAfterFinished: {{ $root.Values.job.ttlSecondsAfterFinished }}
  template:
    metadata:
      labels:
        app.kubernetes.io/name: {{ include "kata-deploy.name" $root }}
        app.kubernetes.io/instance: {{ $root.Release.Name }}
        kata-deploy/stage: {{ $stage }}
    spec:
{{- with $root.Values.imagePullSecrets }}
      imagePullSecrets:
{{- toYaml . | nindent 8 }}
{{- end }}
      serviceAccountName: {{ include "kata-deploy.serviceAccountName" $root }}
      restartPolicy: Never
      hostPID: true
{{- with $root.Values.tolerations }}
      tolerations:
{{- toYaml . | nindent 8 }}
{{- end }}
{{- with $root.Values.priorityClassName }}
      priorityClassName: {{ . | quote }}
{{- end }}
{{- if eq $stage "install" }}
      initContainers:
{{- include "kata-deploy.stageContainer" (dict "root" $root "name" "host-check" "action" "install-stage-host-check" "privileged" true "mountHost" true) | nindent 8 }}
{{- include "kata-deploy.stageContainer" (dict "root" $root "name" "artifacts" "action" "install-stage-artifacts" "privileged" true "mountHost" true) | nindent 8 }}
{{- include "kata-deploy.stageContainer" (dict "root" $root "name" "cri" "action" "install-stage-cri" "privileged" true "mountHost" true) | nindent 8 }}
      containers:
{{- include "kata-deploy.stageContainer" (dict "root" $root "name" "label" "action" "install-stage-label" "privileged" false "mountHost" false) | nindent 8 }}
{{- else }}
      initContainers:
{{- include "kata-deploy.stageContainer" (dict "root" $root "name" "unlabel" "action" "cleanup-stage-unlabel" "privileged" false "mountHost" false) | nindent 8 }}
{{- include "kata-deploy.stageContainer" (dict "root" $root "name" "revert-cri" "action" "cleanup-stage-revert-cri" "privileged" true "mountHost" true) | nindent 8 }}
      containers:
{{- include "kata-deploy.stageContainer" (dict "root" $root "name" "remove-artifacts" "action" "cleanup-stage-remove-artifacts" "privileged" true "mountHost" true) | nindent 8 }}
{{- end }}
      volumes:
{{- include "kata-deploy.commonVolumes" $root | nindent 8 }}
{{- end -}}

{{/*
Service account name (honoring multiInstallSuffix), shared by all kata-deploy
workloads (DaemonSet and staged Jobs).
*/}}
{{- define "kata-deploy.serviceAccountName" -}}
{{- if .Values.env.multiInstallSuffix -}}
{{ .Chart.Name }}-sa-{{ .Values.env.multiInstallSuffix }}
{{- else -}}
{{ .Chart.Name }}-sa
{{- end -}}
{{- end -}}

{{/*
ServiceAccount name for the job-mode dispatcher (kata-deploy-job-dispatcher). Separate from
kata-deploy.serviceAccountName: the dispatcher is a pure API client (list nodes,
manage Jobs) and must NOT carry the privileged kata-deploy host-mutation rights.
*/}}
{{- define "kata-deploy.dispatcherServiceAccountName" -}}
{{- if .Values.env.multiInstallSuffix -}}
{{ .Chart.Name }}-dispatcher-sa-{{ .Values.env.multiInstallSuffix }}
{{- else -}}
{{ .Chart.Name }}-dispatcher-sa
{{- end -}}
{{- end -}}

{{/*
Render a single staged-pipeline container that runs one kata-deploy stage action.
Used by the per-node staged install/cleanup Jobs (deploymentMode: job).

Arguments (dict):
  root        - the top-level context (.)
  name        - container name
  action      - kata-deploy subcommand (e.g. install-stage-cri)
  privileged  - bool, whether the container runs privileged (host nsenter/restart)
  mountHost   - bool, whether to mount the host paths (crio/containerd/host)

Emitted at column 0; indent with `nindent` at the call site.
*/}}
{{- define "kata-deploy.stageContainer" -}}
- name: {{ .name }}
  image: {{ include "kata-deploy.image" .root }}
  imagePullPolicy: {{ .root.Values.imagePullPolicy }}
  command: ["/usr/bin/kata-deploy", "{{ .action }}"]
  env:
{{- include "kata-deploy.commonEnv" .root | nindent 4 }}
  securityContext:
    privileged: {{ .privileged }}
{{- if .mountHost }}
  volumeMounts:
{{- include "kata-deploy.commonVolumeMounts" .root | nindent 4 }}
{{- end }}
{{- end -}}

{{/*
Common volumeMounts for any pod that runs the kata-deploy binary against the
host. Emitted at column 0; indent with `nindent` at the call site.
*/}}
{{- define "kata-deploy.commonVolumeMounts" -}}
- name: crio-conf
  mountPath: /etc/crio/
- name: containerd-conf
  mountPath: /etc/containerd/
- name: host
  mountPath: /host/
{{- if .Values.containerd.userDropIn | trim }}
- name: custom-containerd-config
  mountPath: /custom-containerd-config/
  readOnly: true
{{- end }}
{{- if or (and .Values.customRuntimes.enabled .Values.customRuntimes.runtimes) (eq (include "kata-deploy.hasDefaultRuntimeDropIns" . | trim) "true") }}
- name: custom-configs
  mountPath: /custom-configs/
  readOnly: true
{{- end }}
{{- end -}}

{{/*
Common host/configMap volumes backing the mounts above. Emitted at column 0;
indent with `nindent` at the call site.
*/}}
{{- define "kata-deploy.commonVolumes" -}}
- name: crio-conf
  hostPath:
    path: /etc/crio/
- name: containerd-conf
  hostPath:
    path: '{{- template "containerdConfPath" .Values }}'
- name: host
  hostPath:
    path: /
{{- if .Values.containerd.userDropIn | trim }}
- name: custom-containerd-config
  configMap:
{{- if .Values.env.multiInstallSuffix }}
    name: {{ .Chart.Name }}-containerd-user-dropin-{{ .Values.env.multiInstallSuffix }}
{{- else }}
    name: {{ .Chart.Name }}-containerd-user-dropin
{{- end }}
{{- end }}
{{- if or (and .Values.customRuntimes.enabled .Values.customRuntimes.runtimes) (eq (include "kata-deploy.hasDefaultRuntimeDropIns" . | trim) "true") }}
- name: custom-configs
  configMap:
{{- if .Values.env.multiInstallSuffix }}
    name: {{ .Chart.Name }}-custom-configs-{{ .Values.env.multiInstallSuffix }}
{{- else }}
    name: {{ .Chart.Name }}-custom-configs
{{- end }}
{{- end }}
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

{{/*
Returns "true" when a shim is enabled according to `enabled` + `disableAll`.
Input:
  dict:
    shimConfig: the `.Values.shims.<name>` object
    disableAll: global `.Values.shims.disableAll`
*/}}
{{- define "kata-deploy.isShimEnabled" -}}
{{- $shimEnabled := false -}}
{{- if eq .shimConfig.enabled true -}}
{{- $shimEnabled = true -}}
{{- else if eq .shimConfig.enabled false -}}
{{- $shimEnabled = false -}}
{{- else if not .disableAll -}}
{{- $shimEnabled = true -}}
{{- end -}}
{{- if $shimEnabled -}}true{{- end -}}
{{- end -}}

{{/*
Returns "true" when at least one default shim has a non-empty dropIn value.
*/}}
{{- define "kata-deploy.hasDefaultRuntimeDropIns" -}}
{{- $has := false -}}
{{- $disableAll := .Values.shims.disableAll | default false -}}
{{- range $shimName := keys .Values.shims | sortAlpha -}}
{{- if ne $shimName "disableAll" -}}
{{- $shimConfig := index $.Values.shims $shimName -}}
{{- $shimEnabled := eq (include "kata-deploy.isShimEnabled" (dict "shimConfig" $shimConfig "disableAll" $disableAll) | trim) "true" -}}
{{- if and $shimEnabled $shimConfig.dropIn (ne (trim $shimConfig.dropIn) "") -}}
{{- $has = true -}}
{{- end -}}
{{- end -}}
{{- end -}}
{{- if $has -}}true{{- end -}}
{{- end -}}

{{/*
NFD virtualization nodeAffinity for the kata-deploy DaemonSet.
Applied when node-feature-discovery is managed by this chart (enabled: true).
Kata Containers requires hardware virtualization support to function.

Note: Virtualization checks are ONLY enforced when node-feature-discovery is
      managed by kata-deploy. If node-feature-discovery is installed
      independently (enabled: false), no checks are applied because we cannot
      guarantee the external node-feature-discovery configuration and labels.

NOTE: For kata-remote/peer-pods support in the future, add a condition here:
      if and (index .Values "node-feature-discovery" "enabled") (not .Values.cloud-api-adaptor.enabled)
*/}}
{{- define "kata-deploy.nfdVirtualizationNodeAffinity" -}}
nodeAffinity:
  requiredDuringSchedulingIgnoredDuringExecution:
    nodeSelectorTerms:
    # x86_64: Intel VT-x (VMX) support
    - matchExpressions:
      - key: feature.node.kubernetes.io/cpu-cpuid.VMX
        operator: In
        values:
        - "true"
      - key: kubernetes.io/arch
        operator: In
        values:
        - "amd64"
    # x86_64: AMD-V (SVM) support
    - matchExpressions:
      - key: feature.node.kubernetes.io/cpu-cpuid.SVM
        operator: In
        values:
        - "true"
      - key: kubernetes.io/arch
        operator: In
        values:
        - "amd64"
    # aarch64: Allow all ARM64 nodes (virtualization check not yet implemented)
    # TODO: Implement proper virtualization detection for aarch64
    - matchExpressions:
      - key: kubernetes.io/arch
        operator: In
        values:
        - "arm64"
        - "aarch64"
    # s390x: Allow all s390x nodes (virtualization check not yet implemented)
    # TODO: Implement proper virtualization detection for s390x
    - matchExpressions:
      - key: kubernetes.io/arch
        operator: In
        values:
        - "s390x"
    # ppc64le: Allow all ppc64le nodes (virtualization check not yet implemented)
    # TODO: Implement proper virtualization detection for ppc64le
    - matchExpressions:
      - key: kubernetes.io/arch
        operator: In
        values:
        - "ppc64le"
    # riscv64: Allow all RISC-V nodes (virtualization support not yet available)
    # TODO: Implement virtualization detection when RISC-V virt support is available
    - matchExpressions:
      - key: kubernetes.io/arch
        operator: In
        values:
        - "riscv64"
{{- end -}}

{{/*
Merged affinity for the kata-deploy DaemonSet.
When NFD is enabled, the built-in virtualization nodeAffinity is always applied.
Kubernetes semantics:
  - nodeSelectorTerms are OR within a group (match any one term)
  - matchExpressions and matchFields are AND within a term (all must match)
If the user sets affinity.nodeAffinity, their required nodeSelectorTerms are
combined with the NFD terms as (NFD OR-group) AND (user OR-group) via cross-
product: each NFD term is AND-ed with each user term. NFD virtualization
requirements cannot be bypassed by user affinity.
*/}}
{{- define "kata-deploy.daemonsetAffinity" -}}
{{- $affinity := .Values.affinity | default dict | deepCopy -}}
{{- if index .Values "node-feature-discovery" "enabled" -}}
{{- $nfd := include "kata-deploy.nfdVirtualizationNodeAffinity" . | fromYaml -}}
{{- $nfdNodeAffinity := $nfd.nodeAffinity -}}
{{- if not (hasKey $affinity "nodeAffinity") -}}
{{- $affinity = merge $affinity $nfd -}}
{{- else -}}
{{- $userNodeAffinity := $affinity.nodeAffinity | deepCopy -}}
{{- $nfdRequired := $nfdNodeAffinity.requiredDuringSchedulingIgnoredDuringExecution | default dict -}}
{{- $nfdTerms := $nfdRequired.nodeSelectorTerms | default list -}}
{{- $userRequired := $userNodeAffinity.requiredDuringSchedulingIgnoredDuringExecution | default dict -}}
{{- $userTerms := $userRequired.nodeSelectorTerms | default list -}}
{{- $mergedTerms := list -}}
{{- if $userTerms -}}
{{- range $nfdTerm := $nfdTerms -}}
{{- range $userTerm := $userTerms -}}
{{- $mergedTerm := dict -}}
{{- $exprs := concat ($nfdTerm.matchExpressions | default list) ($userTerm.matchExpressions | default list) -}}
{{- $fields := concat ($nfdTerm.matchFields | default list) ($userTerm.matchFields | default list) -}}
{{- if $exprs -}}
{{- $_ := set $mergedTerm "matchExpressions" $exprs -}}
{{- end -}}
{{- if $fields -}}
{{- $_ := set $mergedTerm "matchFields" $fields -}}
{{- end -}}
{{- $mergedTerms = append $mergedTerms $mergedTerm -}}
{{- end -}}
{{- end -}}
{{- else -}}
{{- $mergedTerms = $nfdTerms -}}
{{- end -}}
{{- $_ := set $userNodeAffinity "requiredDuringSchedulingIgnoredDuringExecution" (dict "nodeSelectorTerms" $mergedTerms) -}}
{{- $_ := set $affinity "nodeAffinity" $userNodeAffinity -}}
{{- end -}}
{{- end -}}
{{- if $affinity -}}
{{- $affinity | toYaml -}}
{{- end -}}
{{- end -}}
