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
