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
Get the shims that run DCGM in the guest from structured config
Builds a semicolon-separated list of shim names: "shim1;shim2"

The value is a set rather than the "shim=value" mapping the proxies use,
because the setting is a boolean: naming a shim here means it is on.

A shim with no nvrc block at all reads as off, so a values file written before
this setting existed - or one defining a shim the chart does not - keeps
upgrading cleanly instead of failing to render.
*/}}
{{- define "kata-deploy.getNvrcEnableDcgm" -}}
{{- $disableAll := .Values.shims.disableAll | default false -}}
{{- $shims := list -}}
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
{{- $nvrc := $shimConfig.nvrc | default dict -}}
{{- if and $shimEnabled ($nvrc.enableDCGM | default false) -}}
{{- $shims = append $shims $shimName -}}
{{- end -}}
{{- end -}}
{{- end -}}
{{- join ";" $shims -}}
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
Image reference for k8s-job-dispatcher.
Supports tag (reference:tag) and digest (reference@sha256:...) formats.
*/}}
{{- define "kata-deploy.dispatcherImage" -}}
{{- $ref := .Values.job.dispatcherImage.reference -}}
{{- $tag := .Values.job.dispatcherImage.tag | toString -}}
{{- if contains "@" $ref -}}
{{- $ref -}}
{{- else if eq $tag "" -}}
{{- fail "job.dispatcherImage.tag is required when job.dispatcherImage.reference is not a digest" -}}
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
The nodeBinaries entries, validated.

Values making an entry a no-op fail the render, as on the node they would only
surface much later, as a missing command or a layer conversion failure.
*/}}
{{- define "kata-deploy.nodeBinaries" -}}
{{- $entries := .Values.nodeBinaries | default dict -}}
{{- if $entries -}}
{{- if ne (.Values.deploymentMode | default "daemonset") "job" -}}
{{- fail (printf "\n\nERROR: nodeBinaries is set (%s), which requires deploymentMode: job.\n\nInstalling binaries onto the node relies on the staged install pipeline, where they are in place before the host check looks for them. The DaemonSet runs the whole install in one container and has no such ordering.\n" (keys $entries | sortAlpha | join ", ")) -}}
{{- end -}}
{{- /* Names of containers the install and cleanup pods carry of their own, which
       an entry cannot take without the API server rejecting the pod for two
       containers sharing a name. */}}
{{- $taken := list "artifacts" "cri" "dispatcher" "host-check" "kube-kata" "load-kernel-modules" "node-binaries-install" "node-binaries-remove" "rb-cleanup" "remove-artifacts" "revert-cri" "selinux-policy" -}}
{{- range $name, $spec := $entries -}}
{{- /* A DNS-1123 label, which is all a container name may be. */}}
{{- if not (regexMatch "^[a-z0-9]([a-z0-9-]*[a-z0-9])?$" $name) -}}
{{- fail (printf "\n\nERROR: nodeBinaries key %q is not usable as a container name.\n\nIt names the container that stages those binaries, so it may hold only lowercase letters, digits and dashes, and has to begin and end with a letter or a digit.\n" $name) -}}
{{- end -}}
{{- if gt (len $name) 63 -}}
{{- fail (printf "\n\nERROR: nodeBinaries key %q is %d characters long.\n\nIt names the container that stages those binaries, and Kubernetes stops at 63.\n" $name (len $name)) -}}
{{- end -}}
{{- if has $name $taken -}}
{{- fail (printf "\n\nERROR: nodeBinaries key %q is the name of a container kata-deploy runs itself.\n\nTwo containers in a pod cannot share a name, so pick another: %s are taken.\n" $name (join ", " $taken)) -}}
{{- end -}}
{{- if not ($spec.image | default "" | trim) -}}
{{- fail (printf "\n\nERROR: nodeBinaries.%s.image is empty.\n\nSet it to the image carrying %s, or drop the entry.\n" $name $name) -}}
{{- end -}}
{{- if not ($spec.binaries | default list) -}}
{{- fail (printf "\n\nERROR: nodeBinaries.%s.binaries is empty.\n\nList the binaries to take out of %s. Nothing is installed by default, so that an image built on a distribution does not put the rest of its userland on the node.\n" $name ($spec.image | default "the image")) -}}
{{- end -}}
{{- /* Each name reaches the node as a word of shell, so anything a shell would
       take apart -- whitespace, a glob, a separator -- has to be turned away
       here rather than split, expanded or run there. A path would also escape
       the directory the binaries are staged in. */}}
{{- range $binary := ($spec.binaries | default list) -}}
{{- if not (regexMatch "^[A-Za-z0-9][A-Za-z0-9._+-]*$" (toString $binary)) -}}
{{- fail (printf "\n\nERROR: nodeBinaries.%s.binaries lists %q, which is not a plain file name.\n\nEach entry names one binary to take out of the image, so it may hold only letters, digits, dots, underscores, plus signs and dashes, and has to begin with a letter or a digit.\n" $name (toString $binary)) -}}
{{- end -}}
{{- end -}}
{{- end -}}
{{- end -}}
{{- toYaml $entries -}}
{{- end -}}

{{/*
Whether the installer's stages are SELinux-confined. Empty when off, so call
sites read as `if include "kata-deploy.selinuxEnabled" . | trim`.
*/}}
{{- define "kata-deploy.selinuxEnabled" -}}
{{- if (.Values.selinux | default dict).enabled -}}
true
{{- end -}}
{{- end -}}

{{/*
The seLinuxOptions block for one stage, or nothing when confinement is off.

level: s0 is pinned rather than left to the runtime, which would hand the stage
per-pod MCS categories. Every host path the installer writes is s0.

Arguments (dict):
  root   - the top-level context (.)
  domain - the SELinux type for this stage, e.g. kata_deploy_cri_t

Emitted at column 0; indent with `nindent` at the call site.
*/}}
{{- define "kata-deploy.seLinuxOptions" -}}
{{- if and (include "kata-deploy.selinuxEnabled" .root | trim) .domain -}}
seLinuxOptions:
  type: {{ .domain }}
  level: s0
{{- end -}}
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
{{- $nvrcEnableDcgm := include "kata-deploy.getNvrcEnableDcgm" . | trim -}}
{{- if $nvrcEnableDcgm }}
- name: NVRC_ENABLE_DCGM
  value: {{ $nvrcEnableDcgm | quote }}
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
{{- if not (.Values.containerd.configDir | trim) }}
{{- /* This value picks the host directory mounted at /etc/containerd, while the
       install detects the runtime itself and picks the file written within it, so
       the install needs it to refuse a directory its runtime does not read.
       Omitted when configDir overrides the derivation being checked. */}}
- name: K8S_DISTRIBUTION
  value: {{ .Values.k8sDistribution | quote }}
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
requirements. This is handed to `k8s-job-dispatcher --node-selector`, which
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
{{- fail (printf "affinity.nodeAffinity matchExpressions: unsupported operator %q for key %q. Node selection compiles to a label selector, which supports In, NotIn, Exists and DoesNotExist only - Gt and Lt cannot be expressed." $op $expr.key) -}}
{{- end -}}
{{- end -}}
{{- join "," $parts -}}
{{- end -}}

{{/*
Flags handing the node-level API work to the dispatcher, for the stage named in
`stage` ("install" or "cleanup").

The per-node Jobs can run without a token because this work happens here instead.
Install labels the node only after its runtime serves what was installed. Cleanup
removes the label first, so no new Kata workload lands on a node that is about to
lose its runtime.

Arguments (dict): root, stage. Emitted at column 0; `nindent` at the call site.
*/}}
{{/*
The label key this install marks its nodes with: the per-install half of the
scheduling gate.

katacontainers.io/kata-runtime is shared by every install on the node, so it
cannot say *which* install is serving Kata there. With multiInstallSuffix set, the
RuntimeClasses of an install therefore select this mark as well: taking it away is
then what stops that install's workloads reaching the node, while the other
installs keep theirs.
*/}}
{{- define "kata-deploy.instanceMarkerLabel" -}}
{{- if .Values.env.multiInstallSuffix -}}
kata-deploy.katacontainers.io/{{ .Values.env.multiInstallSuffix }}
{{- else -}}
kata-deploy.katacontainers.io/default
{{- end -}}
{{- end -}}

{{- define "kata-deploy.dispatcherNodeWorkFlags" -}}
{{- $root := .root -}}
{{- /* Preserve the tracking and node-management contract of the dispatcher that
       originally lived in this repository. */ -}}
- "--tracking-label-prefix=kata-deploy-job-dispatcher"
- "--node-label-key=katacontainers.io/kata-runtime"
- "--instance-label-prefix=kata-deploy.katacontainers.io"
- "--require-node-runtime-version"
- "--require-node-machine-id"
{{- /* Installs share katacontainers.io/kata-runtime, so each one also marks its
       own nodes and gives the shared label up only once no other mark is left.
       Both stages need to know which mark is ours. */}}
{{- with $root.Values.env.multiInstallSuffix }}
- "--multi-install-suffix={{ . }}"
{{- end }}
{{- if eq .stage "cleanup" }}
- "--remove-node-label"
{{- else }}
- "--node-label=true"
- "--claim-node-pending"
- "--wait-node-ready-secs={{ $root.Values.job.waitNodeReadySeconds | default 300 }}"
{{- with include "kata-deploy.criHandlers" $root | trim }}
- "--require-node-handlers={{ . }}"
{{- end }}
{{- with $root.Values.startupTaints }}
- "--remove-node-taints={{ join "," . }}"
{{- end }}
{{- with include "kata-deploy.kubeletTimeoutWarnSecs" $root | trim }}
- "--kubelet-timeout-warn-secs={{ . }}"
{{- end }}
{{- end }}
{{- end -}}

{{/*
The CRI runtime handlers this release installs, across every architecture.

A node only serves the handlers built for its architecture, so look for any one of
these rather than all of them. A node that serves none of them has a runtime that
never read what the install wrote. Custom runtimes are included: on a release that
configures nothing else, they are the only handlers there are.
*/}}
{{- define "kata-deploy.criHandlers" -}}
{{- $root := . -}}
{{- $suffix := $root.Values.env.multiInstallSuffix | default "" -}}
{{- $handlers := list -}}
{{- range $arch := list "amd64" "arm64" "s390x" "ppc64le" -}}
{{- range $shim := include "kata-deploy.getEnabledShimsForArch" (dict "root" $root "arch" $arch) | trim | splitList " " -}}
{{- if $shim -}}
{{- $handler := printf "kata-%s" $shim -}}
{{- if $suffix -}}
{{- $handler = printf "kata-%s-%s" $shim $suffix -}}
{{- end -}}
{{- if not (has $handler $handlers) -}}
{{- $handlers = append $handlers $handler -}}
{{- end -}}
{{- end -}}
{{- end -}}
{{- end -}}
{{- if and $root.Values.customRuntimes.enabled $root.Values.customRuntimes.runtimes -}}
{{- range $name := keys $root.Values.customRuntimes.runtimes | sortAlpha -}}
{{- $runtime := index $root.Values.customRuntimes.runtimes $name -}}
{{- with $runtime.runtimeClass -}}
{{- $handler := (fromYaml . | default dict).handler | default "" -}}
{{- if and $handler (not (has $handler $handlers)) -}}
{{- $handlers = append $handlers $handler -}}
{{- end -}}
{{- end -}}
{{- end -}}
{{- end -}}
{{- join "," $handlers -}}
{{- end -}}

{{/*
Seconds below which a node's kubelet `runtimeRequestTimeout` is worth warning
about, or EMPTY when this configuration has no reason to care.

Only pulling or converting an image inside `CreateContainer` takes long enough to
hit that timeout. The kubelet default is 2 minutes, so warning in any other case
would warn about every cluster.
*/}}
{{- define "kata-deploy.kubeletTimeoutWarnSecs" -}}
{{- $needed := false -}}
{{- range $arch := list "amd64" "arm64" "s390x" "ppc64le" -}}
{{- if include "kata-deploy.getForceGuestPullForArch" (dict "root" $ "arch" $arch) | trim -}}
{{- $needed = true -}}
{{- end -}}
{{- if contains "guest-pull" (include "kata-deploy.getPullTypeMappingForArch" (dict "root" $ "arch" $arch) | trim) -}}
{{- $needed = true -}}
{{- end -}}
{{- end -}}
{{- if contains "erofs" (include "kata-deploy.getSnapshotterSetup" . | trim) -}}
{{- $needed = true -}}
{{- end -}}
{{- if .Values.customRuntimes.enabled -}}
{{- range $runtime := (.Values.customRuntimes.runtimes | default list) -}}
{{- if contains "guest-pull" (dig "crio" "pullType" "" $runtime | toString) -}}
{{- $needed = true -}}
{{- end -}}
{{- end -}}
{{- end -}}
{{- if $needed -}}600{{- end -}}
{{- end -}}

{{/*
Whether to render the NFD-derived resources: the `NodeFeatureRule` that advertises
TEE key counts, and the matching `overhead.podFixed` entries that make Kata's
confidential RuntimeClasses consume one of those keys per pod.

Returns "true" or the empty string.

`nodeFeatureRules.create: auto` (the default) mirrors what the kata-deploy binary
used to check at run time - is NFD actually around? - from three angles: this
release installs it, an existing installation was found, or the CRD is registered
in the cluster (which also covers an NFD deployed under names the lookup does not
recognise). The CRD check only sees a live cluster, so `helm template` renders
nothing under `auto`; pass `--set nodeFeatureRules.create=true` to inspect it.

Both resources hang off one signal on purpose: an extended-resource request that
nothing advertises makes every pod on that RuntimeClass unschedulable, so the
keys must never be requested without the rule that produces them.
*/}}
{{- define "kata-deploy.nfdRulesActive" -}}
{{- $nfr := .Values.nodeFeatureRules | default dict -}}
{{- /* Read through hasKey rather than `default`, which counts a boolean false
       as empty and would turn `create: false` back into `auto`. */ -}}
{{- $create := "auto" -}}
{{- if and (hasKey $nfr "create") (not (kindIs "invalid" $nfr.create)) -}}
{{- $create = toString $nfr.create -}}
{{- end -}}
{{- if eq $create "true" -}}
true
{{- else if eq $create "false" -}}
{{- else if ne $create "auto" -}}
{{- fail (printf "nodeFeatureRules.create must be one of auto, true, false (got %q)" $create) -}}
{{- else -}}
{{- $nfdEnabled := index .Values "node-feature-discovery" "enabled" | default false -}}
{{- $existingNFD := ne (include "kata-deploy.detectExistingNFD" . | trim) "" -}}
{{- $crdPresent := .Capabilities.APIVersions.Has "nfd.k8s-sigs.io/v1alpha1" -}}
{{- if or $nfdEnabled $existingNFD $crdPresent -}}
true
{{- end -}}
{{- end -}}
{{- end -}}

{{/*
Where a dispatcher pod may run: `nodeSelector` and `tolerations` blocks for the
install and cleanup dispatchers.

This is about the dispatcher pod, not about which nodes get Kata. The distinction
matters because the dispatcher is the only part of job mode that holds
credentials, and root on the node it lands on can read them; confining it to
trusted nodes is a hardening step a DaemonSet cannot offer. The per-node Jobs,
which do run on every target node, hold no token at all.

Tolerations fall back to the top-level `tolerations` so the dispatcher stays
schedulable wherever the per-node Jobs are allowed to run - without that, a
cluster whose every node is tainted could select nodes it cannot dispatch from.

Emitted at column 0; embed with `nindent` at the call site.
*/}}
{{- define "kata-deploy.dispatcherPlacement" -}}
{{- $job := .Values.job | default dict -}}
{{- with $job.dispatcherNodeSelector }}
nodeSelector:
{{ toYaml . | indent 2 }}
{{- end }}
{{- with ($job.dispatcherTolerations | default .Values.tolerations) }}
tolerations:
{{ toYaml . | indent 2 }}
{{- end }}
{{- end -}}

{{/*
Reject the job-mode-specific node-selection keys that used to exist, rather than
silently ignoring them: an ignored node-selection rule means installing Kata on
nodes the operator never intended.

Note that `nodeSelector` and `affinity.nodeAffinity` may be combined freely, just
as they can on any pod spec - Kubernetes ANDs them, and so do we.
*/}}
{{- define "kata-deploy.failOnRemovedJobSelectionKeys" -}}
{{- $job := .Values.job | default dict -}}
{{- range $key := (list "nodeSelector" "nodeAffinity" "nodeSelectorExpressions") -}}
{{- if hasKey $job $key -}}
{{- fail (printf "job.%s has been removed. Node selection is now expressed once, at the top level, and applies to both deployment modes: use `nodeSelector` for exact label matches and/or `affinity.nodeAffinity` for match expressions. Use `job.nodes` to name nodes explicitly." $key) -}}
{{- end -}}
{{- end -}}
{{- if hasKey ($job.cleanup | default dict) "nodeSelectorExpressions" -}}
{{- fail "job.cleanup.nodeSelectorExpressions has been renamed to job.cleanup.nodeAffinity, which takes the standard Kubernetes nodeAffinity shape. Move each entry into job.cleanup.nodeAffinity.requiredDuringSchedulingIgnoredDuringExecution.nodeSelectorTerms[].matchExpressions." -}}
{{- end -}}
{{- end -}}

{{/*
Compile node selection into the list of label-selector STRINGS handed to the
dispatcher as repeated `--node-selector` flags (job mode). The dispatcher unions
the matches, which is what preserves nodeAffinity's OR semantics:
nodeSelectorTerms are OR-ed, while the matchExpressions inside one term are
AND-ed - and a single label selector is exactly one AND-group, hence one selector
string per term.

Selection comes from the same `nodeSelector` / `affinity.nodeAffinity` the
DaemonSet uses, plus the NFD virtualization requirements when NFD is
chart-managed (AND-ed as a cross-product, exactly as
`kata-deploy.daemonsetAffinity` does), so both modes target the same nodes.

`nodeSelector` and `affinity.nodeAffinity` may be set together, exactly as on a
pod spec: the DaemonSet lets Kubernetes AND the two, and we reproduce that by
folding the `nodeSelector` equalities into EVERY term, which is the same thing by
distribution - `eq AND (t1 OR t2)` is `(eq AND t1) OR (eq AND t2)`.

`preferredDuringSchedulingIgnoredDuringExecution` is intentionally ignored rather
than rejected: it never restricts which nodes a DaemonSet may land on, so
honouring only the required terms keeps the two modes in agreement.

Returns the selector strings joined by newlines. EMPTY output means "every node",
and the call site then passes no `--node-selector` at all. Note that this is not
the same as "every node gets Kata": the dispatcher still drops nodes whose taints
the install does not tolerate, mirroring what the scheduler does for a DaemonSet.
*/}}
{{- define "kata-deploy.installNodeSelectors" -}}
{{- include "kata-deploy.failOnRemovedJobSelectionKeys" . -}}
{{- $eq := .Values.nodeSelector | default dict -}}
{{- $terms := list -}}
{{- $nodeAffinity := (.Values.affinity | default dict).nodeAffinity | default dict -}}
{{- $hasRequired := hasKey $nodeAffinity "requiredDuringSchedulingIgnoredDuringExecution" -}}
{{- with $nodeAffinity -}}
{{- $required := .requiredDuringSchedulingIgnoredDuringExecution | default dict -}}
{{- $terms = $required.nodeSelectorTerms | default list -}}
{{- end -}}
{{- if and $hasRequired (not $terms) -}}
{{- fail "affinity.nodeAffinity.requiredDuringSchedulingIgnoredDuringExecution.nodeSelectorTerms must not be empty with deploymentMode: job. Kubernetes defines an empty required term list as matching no nodes; remove requiredDuringSchedulingIgnoredDuringExecution to select every otherwise eligible node." -}}
{{- end -}}
{{- range $term := $terms -}}
{{- if $term.matchFields -}}
{{- fail "affinity.nodeAffinity: matchFields cannot be used with deploymentMode: job. Node selection is a label-selector LIST against the Kubernetes API server, which cannot match on fields. To target nodes by name, use job.nodes instead." -}}
{{- end -}}
{{- if not ($term.matchExpressions | default list) -}}
{{- fail "affinity.nodeAffinity: an empty nodeSelectorTerm matches no nodes in Kubernetes and cannot be represented as an API label selector in job mode. Remove the empty term or add a matchExpressions requirement." -}}
{{- end -}}
{{- end -}}
{{- if index .Values "node-feature-discovery" "enabled" -}}
{{- $nfd := include "kata-deploy.nfdVirtualizationNodeAffinity" . | fromYaml -}}
{{- $nfdTerms := $nfd.nodeAffinity.requiredDuringSchedulingIgnoredDuringExecution.nodeSelectorTerms | default list -}}
{{- $merged := list -}}
{{- range $nfdTerm := $nfdTerms -}}
{{- if $terms -}}
{{- range $userTerm := $terms -}}
{{- $merged = append $merged (dict "matchExpressions" (concat ($nfdTerm.matchExpressions | default list) ($userTerm.matchExpressions | default list))) -}}
{{- end -}}
{{- else -}}
{{- $merged = append $merged $nfdTerm -}}
{{- end -}}
{{- end -}}
{{- $terms = $merged -}}
{{- end -}}
{{- $selectors := list -}}
{{- if $terms -}}
{{- range $term := $terms -}}
{{- $selectors = append $selectors (include "kata-deploy.nodeLabelSelector" (dict "eq" $eq "exprs" ($term.matchExpressions | default list))) -}}
{{- end -}}
{{- else -}}
{{- $selectors = append $selectors (include "kata-deploy.nodeLabelSelector" (dict "eq" $eq "exprs" list)) -}}
{{- end -}}
{{- $selectors = $selectors | uniq -}}
{{- /* An empty selector matches every node, so it subsumes all the others. */ -}}
{{- if not (has "" $selectors) -}}
{{- join "\n" $selectors -}}
{{- end -}}
{{- end -}}

{{/*
Compile the UNINSTALL dispatcher's node selection (job.cleanup.*).

Kept separate from install on purpose: cleanup must reach every node the install
ever labeled - not the nodes currently selected - so it defaults to "nodes
carrying katacontainers.io/kata-runtime" and is never narrowed by the top-level
selection or by the NFD requirements.

Same return contract as `kata-deploy.installNodeSelectors`.
*/}}
{{- define "kata-deploy.cleanupNodeSelectors" -}}
{{- $cleanup := (.Values.job | default dict).cleanup | default dict -}}
{{- $eq := $cleanup.nodeSelector | default dict -}}
{{- $terms := list -}}
{{- with $cleanup.nodeAffinity -}}
{{- $required := .requiredDuringSchedulingIgnoredDuringExecution | default dict -}}
{{- $terms = $required.nodeSelectorTerms | default list -}}
{{- end -}}
{{- $selectors := list -}}
{{- if $terms -}}
{{- range $term := $terms -}}
{{- if $term.matchFields -}}
{{- fail "job.cleanup.nodeAffinity: matchFields is not supported; use job.cleanup.nodes to name nodes explicitly." -}}
{{- end -}}
{{- $selectors = append $selectors (include "kata-deploy.nodeLabelSelector" (dict "eq" $eq "exprs" ($term.matchExpressions | default list))) -}}
{{- end -}}
{{- else -}}
{{- $selectors = append $selectors (include "kata-deploy.nodeLabelSelector" (dict "eq" $eq "exprs" list)) -}}
{{- end -}}
{{- $selectors = $selectors | uniq -}}
{{- if not (has "" $selectors) -}}
{{- join "\n" $selectors -}}
{{- end -}}
{{- end -}}

{{/*
Per-node staged Job manifest (deploymentMode: job), embedded verbatim into the
job-templates ConfigMap. k8s-job-dispatcher clones this once per
target node, injecting metadata.name + spec.template.spec.nodeName, so the
template itself carries NO node identity and NO Helm hook annotations.

Arguments (dict):
  root  - top-level context (.)
  stage - "install" | "cleanup"

install pipeline:  load-kernel-modules -> host-check -> artifacts (initContainers) ; cri (main)
cleanup pipeline:  revert-cri              (initContainer)  ; remove-artifacts (main)

The node label is not a stage here: the dispatcher sets it once the Job as a whole
has succeeded (and removes it before a cleanup Job runs), which is what lets these
pods run without a ServiceAccount token.

Emitted at column 0 (a standalone Job document); embed with `indent` at the call
site under a ConfigMap data key.
*/}}
{{- define "kata-deploy.perNodeJob" -}}
{{- $root := .root -}}
{{- $stage := .stage -}}
{{- /* The dispatcher polls each Job for its result. A Job deleted before the next
       poll leaves its node with no result, and that counts as a failure. */}}
{{- if lt (int $root.Values.job.ttlSecondsAfterFinished) 60 -}}
{{- fail (printf "job.ttlSecondsAfterFinished is %v, which is too short for the dispatcher to observe a per-node Job finishing: the Job is deleted before it is next polled and its node is reported as failed even though its install succeeded. Use 60 or more." $root.Values.job.ttlSecondsAfterFinished) -}}
{{- end -}}
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
  {{- /* The dispatcher waits for every node it dispatched to. Without a deadline,
         one stuck pod would hold up the whole rollout. */}}
  activeDeadlineSeconds: {{ $root.Values.job.activeDeadlineSeconds | default 3600 }}
  template:
    metadata:
      labels:
{{- with $root.Values.podLabels }}
{{- toYaml . | nindent 8 }}
{{- end }}
        app.kubernetes.io/name: {{ include "kata-deploy.name" $root }}
        app.kubernetes.io/instance: {{ $root.Release.Name }}
        kata-deploy/stage: {{ $stage }}
{{- $podAnnotations := include "kata-deploy.podTemplateAnnotations" $root | trim }}
{{- if $podAnnotations }}
      annotations:
{{- $podAnnotations | nindent 8 }}
{{- end }}
    spec:
{{- with $root.Values.imagePullSecrets }}
      imagePullSecrets:
{{- toYaml . | nindent 8 }}
{{- end }}
      {{- /* Without this, Kubernetes mounts the namespace's default token. These
             pods need no API access: the dispatcher does all of it. */}}
      automountServiceAccountToken: false
      restartPolicy: Never
{{- if eq $stage "cleanup" }}
      {{- /* nodeName gets the pod past the scheduler, but not past the taint
             manager, which evicts even a bound pod. A node tainted after the
             install would keep its Kata configuration for good. */}}
      tolerations:
        - operator: Exists
{{- else }}
      {{- /* The DaemonSet controller adds these to its own pods, and job mode
             installs on the same nodes. not-ready matters most: this Job
             restarts the CRI runtime, which takes the node NotReady long enough
             for the taint manager to evict it mid-install. */}}
      tolerations:
{{- if $root.Values.job.nodes }}
        {{- /* Explicit names are a deliberate admission override. nodeName gets
               past the scheduler, but an untolerated NoExecute taint can still
               evict the bound pod before it finishes. */}}
        - operator: Exists
{{- else }}
        - key: node.kubernetes.io/not-ready
          operator: Exists
          effect: NoExecute
        - key: node.kubernetes.io/unreachable
          operator: Exists
          effect: NoExecute
        - key: node.kubernetes.io/disk-pressure
          operator: Exists
          effect: NoSchedule
        - key: node.kubernetes.io/memory-pressure
          operator: Exists
          effect: NoSchedule
        - key: node.kubernetes.io/pid-pressure
          operator: Exists
          effect: NoSchedule
        - key: node.kubernetes.io/unschedulable
          operator: Exists
          effect: NoSchedule
{{- end }}
{{- with $root.Values.tolerations }}
{{- toYaml . | nindent 8 }}
{{- end }}
{{- end }}
{{- with $root.Values.priorityClassName }}
      priorityClassName: {{ . | quote }}
{{- end }}
{{- if eq $stage "install" }}
      initContainers:
{{- /* First of all: a stage asking for a domain the node lacks cannot start. */}}
{{- if include "kata-deploy.selinuxEnabled" $root | trim }}
{{- include "kata-deploy.stageContainer" (dict "root" $root "name" "selinux-policy" "action" "install-stage-selinux-policy" "privileged" true "mountHost" true "mountHostRoot" true "hostRootWritable" true) | nindent 8 }}
{{- end }}
{{- /* All before the host check, so it validates the binaries being installed
       rather than the versions they are there to replace. One staging container
       per nodeBinaries entry, so a new entry is a values change only. */}}
{{- $nodeBinaries := include "kata-deploy.nodeBinaries" $root | fromYaml }}
{{- range $name, $spec := $nodeBinaries }}
{{- include "kata-deploy.nodeBinariesStageContainer" (dict "root" $root "name" $name "spec" $spec) | nindent 8 }}
{{- end }}
{{- if $nodeBinaries }}
{{- include "kata-deploy.nodeBinariesInstallContainer" (dict "root" $root "name" "node-binaries-install" "staged" true) | nindent 8 }}
{{- end }}
{{- /* Privileged, and holding the host root, because it runs the host's own modprobe. */}}
{{- include "kata-deploy.stageContainer" (dict "root" $root "name" "load-kernel-modules" "action" "install-stage-load-kernel-modules" "privileged" true "mountHost" true "mountHostRoot" true "mountModulesLoad" true) | nindent 8 }}
{{- include "kata-deploy.stageContainer" (dict "root" $root "name" "host-check" "action" "install-stage-host-check" "privileged" false "mountHost" true "selinuxDomain" "kata_deploy_check_t") | nindent 8 }}
{{- include "kata-deploy.stageContainer" (dict "root" $root "name" "artifacts" "action" "install-stage-artifacts" "privileged" false "mountHost" true "selinuxDomain" "kata_deploy_artifacts_t") | nindent 8 }}
      containers:
{{- include "kata-deploy.stageContainer" (dict "root" $root "name" "cri" "action" "install-stage-cri" "privileged" false "mountHost" true "selinuxDomain" "kata_deploy_cri_t") | nindent 8 }}
{{- else }}
      initContainers:
{{- /* Here too, since these stages are confined as well: a node whose module went
       missing could otherwise never be uninstalled. Re-loading it is idempotent. */}}
{{- if include "kata-deploy.selinuxEnabled" $root | trim }}
{{- include "kata-deploy.stageContainer" (dict "root" $root "name" "selinux-policy" "action" "install-stage-selinux-policy" "privileged" true "mountHost" true "mountHostRoot" true "hostRootWritable" true) | nindent 8 }}
{{- end }}
{{- include "kata-deploy.stageContainer" (dict "root" $root "name" "revert-cri" "action" "cleanup-stage-revert-cri" "privileged" false "mountHost" true "selinuxDomain" "kata_deploy_cri_t") | nindent 8 }}
{{- /* After the revert, so containerd is no longer converting layers with them.
       Unconditional, and driven by the marker alone, so that an uninstall tidies
       up even when the entries were dropped from the values first. */}}
{{- include "kata-deploy.nodeBinariesInstallContainer" (dict "root" $root "name" "node-binaries-remove") | nindent 8 }}
      containers:
{{- include "kata-deploy.stageContainer" (dict "root" $root "name" "remove-artifacts" "action" "cleanup-stage-remove-artifacts" "privileged" false "mountHost" true "mountModulesLoad" true "selinuxDomain" "kata_deploy_artifacts_t") | nindent 8 }}
{{- end }}
      volumes:
{{- include "kata-deploy.commonVolumes" $root | nindent 8 }}
        - name: modules-load-d
          hostPath:
            path: /etc/modules-load.d
            type: DirectoryOrCreate
{{- /* The cleanup pipeline holds the host root for the policy stage alone. */}}
{{- if or (eq $stage "install") (include "kata-deploy.selinuxEnabled" $root | trim) }}
        - name: host-root
          hostPath:
            path: /
            type: Directory
{{- end }}
{{- if and (eq $stage "install") (include "kata-deploy.nodeBinaries" $root | fromYaml) }}
        {{- /* Pod-local, so those images reach nothing of the node's. */}}
        - name: node-binaries
          emptyDir: {}
{{- end }}
{{- end -}}

{{/*
Service account name (honoring multiInstallSuffix) for the DaemonSet, the only
workload that carries the privileged host-mutation rights. Job mode's per-node
Jobs deliberately have no ServiceAccount; its dispatcher uses
kata-deploy.dispatcherServiceAccountName.
*/}}
{{- define "kata-deploy.serviceAccountName" -}}
{{- if .Values.env.multiInstallSuffix -}}
{{ .Chart.Name }}-sa-{{ .Values.env.multiInstallSuffix }}
{{- else -}}
{{ .Chart.Name }}-sa
{{- end -}}
{{- end -}}

{{/*
ServiceAccount name for k8s-job-dispatcher. Separate from
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
Compute the host install directory (must match Config::from_env in the binary):
  {installationPrefix}/opt/kata[-{multiInstallSuffix}]
*/}}
{{- define "kata-deploy.installDir" -}}
{{- $prefix := .Values.env.installationPrefix | default "" -}}
{{- $dir := printf "%s/opt/kata" $prefix -}}
{{- if .Values.env.multiInstallSuffix -}}
{{- $dir = printf "%s-%s" $dir .Values.env.multiInstallSuffix -}}
{{- end -}}
{{- $dir -}}
{{- end -}}

{{/*
Copy one nodeBinaries entry's binaries out of the image carrying them.

The only containers running images kata-deploy did not build, hence the only ones
reaching nothing but the volume they write to.

Arguments (dict):
  root - the top-level context (.)
  name - the entry's name, which is also the container's
  spec - the entry: image, binaries, and optionally pullPolicy

Emitted at column 0; indent with `nindent` at the call site.
*/}}
{{- define "kata-deploy.nodeBinariesStageContainer" -}}
- name: {{ .name }}
  image: {{ .spec.image | trim | quote }}
  imagePullPolicy: {{ .spec.pullPolicy | default .root.Values.imagePullPolicy | quote }}
  command: ["/bin/sh", "-c"]
  args:
    {{- /* Every plausible layout is searched so the image is the only value to
           set. Its own directory, so two entries offering the same name are the
           install container's to reject rather than a silent overwrite. */}}
    - |
      set -eu

      staged="/node-binaries/{{ .name }}"
      mkdir -p "${staged}"

      for binary in {{ .spec.binaries | join " " }}; do
        for dir in /usr/local/bin /usr/local/sbin /usr/bin /usr/sbin /bin /sbin /; do
          if [ -f "${dir}/${binary}" ]; then
            cp "${dir}/${binary}" "${staged}/${binary}"
            echo "staged ${dir}/${binary}"
            break
          fi
        done

        if [ ! -f "${staged}/${binary}" ]; then
          echo "ERROR: no ${binary} in this image, in any directory searched above." >&2
          exit 1
        fi
      done
  securityContext:
    privileged: false
    readOnlyRootFilesystem: true
    {{- /* The image's own user may not be able to write the volume. */}}
    runAsUser: 0
  volumeMounts:
    - name: node-binaries
      mountPath: /node-binaries
{{- with .root.Values.resources }}
  resources:
{{- toYaml . | nindent 4 }}
{{- end }}
{{- end -}}

{{/*
Put the staged binaries in /usr/local/bin, ahead of /usr/bin in containerd's
PATH, and take out what a previous run put there. With nothing staged, taking
them out is the whole job.

Installs whatever the staging containers left rather than a list of its own, so
a new nodeBinaries entry needs no change here.

Runs the kubectl image because kata-deploy's own is distroless: without a shell
it cannot do this, and giving the images carrying the binaries the node's
/usr/local/bin would put images we do not build on the node's PATH.

Arguments (dict):
  root   - the top-level context (.)
  name   - container name
  staged - bool, whether the staged binaries are mounted (install, not just remove)

Emitted at column 0; indent with `nindent` at the call site.
*/}}
{{- define "kata-deploy.nodeBinariesInstallContainer" -}}
- name: {{ .name }}
  image: {{ include "kata-deploy.kubectlImage" .root }}
  imagePullPolicy: {{ .root.Values.imagePullPolicy }}
  command: ["/bin/sh", "-c"]
  args:
    - |
      set -eu

      node_bin=/host-usr-local/bin-writable
      staged=/node-binaries

      {{- /* The lock every other kata-deploy mutation of this node takes, so
             that two releases installing at once take turns rather than
             interleaving their writes. Held until this container exits. */}}
      exec 9>/host-run-lock/kata-deploy.lock
      flock -x 9

      {{- /* /usr/local/bin holds files from many sources and none of them say
             where they came from. Kept beside them rather than under the install
             directory, which cleanup empties before this can read it, and named
             after this installation so that side-by-side ones own separate sets
             rather than removing each other's. */}}
      marker="${node_bin}/.kata-deploy-node-binaries{{ with .root.Values.env.multiInstallSuffix }}-{{ . }}{{ end }}"

      {{- /* What a previous run of this installation put there. Read, not acted
             on yet: nothing is removed before this run knows it can carry out
             what it is replacing them with. */}}
      owned=""
      if [ -f "${marker}" ]; then
        while read -r binary; do
          [ -n "${binary}" ] || continue
          owned="${owned}${binary} "
        done < "${marker}"
      fi

      {{- /* No staged directory means nothing is configured any more, or this is
             the cleanup: taking them out is the whole job. */}}
      if [ ! -d "${staged}" ]; then
        for binary in ${owned}; do
          rm -f "${node_bin}/${binary}"
          echo "removed /usr/local/bin/${binary}"
        done
        rm -f "${marker}"
        exit 0
      fi

      {{- /* Whatever the staging containers left, each in its own directory, so
             two entries offering the same name are caught here rather than one
             silently overwriting the other. */}}
      claim=""
      for path in "${staged}"/*/*; do
        [ -f "${path}" ] || continue
        binary="${path##*/}"
        case " ${claim}" in
        *" ${binary} "*)
          echo "ERROR: ${binary} was staged by more than one nodeBinaries entry." >&2
          exit 1
          ;;
        esac
        claim="${claim}${binary} "
      done

      if [ -z "${claim}" ]; then
        echo "ERROR: nothing was staged; see the staging containers' logs." >&2
        exit 1
      fi

      {{- /* All of them checked before anything is written or removed, so an
             install this node refuses leaves it the set it already had. */}}
      for binary in ${claim}; do
        case " ${owned}" in
        *" ${binary} "*)
          continue
          ;;
        esac
        if [ -e "${node_bin}/${binary}" ] || [ -L "${node_bin}/${binary}" ]; then
          echo "ERROR: /usr/local/bin/${binary} was not installed by kata-deploy." >&2
          echo "Remove it, or drop it from nodeBinaries to keep using it." >&2
          exit 1
        fi
      done

      {{- /* Everything this run may touch, claimed before any of it is written:
             a run that dies midway leaves the next one something to remove,
             rather than a binary nothing owns. */}}
      union="${claim}"
      for binary in ${owned}; do
        case " ${claim}" in
        *" ${binary} "*)
          continue
          ;;
        esac
        union="${union}${binary} "
      done
      printf '%s\n' ${union} > "${marker}"

      for binary in ${claim}; do
        set -- "${staged}"/*/"${binary}"
        {{- /* Renamed into place so nothing ever finds a partial binary. */}}
        cp "${1}" "${node_bin}/.${binary}.new"
        chmod 0755 "${node_bin}/.${binary}.new"
        mv "${node_bin}/.${binary}.new" "${node_bin}/${binary}"
        echo "installed /usr/local/bin/${binary}"
      done

      {{- /* What the previous run installed and this one no longer offers. */}}
      for binary in ${owned}; do
        case " ${claim}" in
        *" ${binary} "*)
          continue
          ;;
        esac
        rm -f "${node_bin}/${binary}"
        echo "removed /usr/local/bin/${binary}"
      done

      printf '%s\n' ${claim} > "${marker}"
  securityContext:
    privileged: false
    allowPrivilegeEscalation: false
    readOnlyRootFilesystem: true
    {{- /* Writes into the node's /usr/local/bin. */}}
    runAsUser: 0
    {{- /* Its own domain: the node's PATH is out of reach of every other stage,
           and /opt/kata and the CRI config out of reach of this one. */}}
{{- $seLinux := include "kata-deploy.seLinuxOptions" (dict "root" .root "domain" "kata_deploy_node_binaries_t") | trim }}
{{- if $seLinux }}
{{- $seLinux | nindent 4 }}
{{- end }}
  volumeMounts:
    - name: host-usr-local-bin
      mountPath: /host-usr-local/bin-writable
    - name: host-run-lock
      mountPath: /host-run-lock
{{- if .staged }}
    - name: node-binaries
      mountPath: /node-binaries
      readOnly: true
{{- end }}
{{- with .root.Values.resources }}
  resources:
{{- toYaml . | nindent 4 }}
{{- end }}
{{- end -}}

{{/*
Render a single staged-pipeline container that runs one kata-deploy stage action.
Used by the per-node staged install/cleanup Jobs (deploymentMode: job).

Arguments (dict):
  root        - the top-level context (.)
  name        - container name
  action      - kata-deploy subcommand (e.g. install-stage-cri)
  privileged  - bool, whether the container runs privileged
  mountHost   - bool, whether to mount the host paths (crio/containerd/install/...)
  mountHostRoot - bool, whether to mount the host root read-only at /host
  hostRootWritable - bool, whether that host root mount is writable
  mountModulesLoad - bool, whether to mount the host modules-load.d directory writable
  selinuxDomain - SELinux type to confine this stage to, when selinux.enabled

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
    readOnlyRootFilesystem: true
{{- $seLinux := include "kata-deploy.seLinuxOptions" (dict "root" .root "domain" .selinuxDomain) | trim }}
{{- if $seLinux }}
{{- $seLinux | nindent 4 }}
{{- end }}
  volumeMounts:
{{- if .mountHost }}
{{- include "kata-deploy.commonVolumeMounts" .root | nindent 4 }}
{{- else }}
{{- include "kata-deploy.tmpVolumeMount" . | nindent 4 }}
{{- end }}
{{- if .mountHostRoot }}
    - name: host-root
      mountPath: /host
      {{- /* The policy stage writes: semodule rebuilds the node's policy store. */}}
      readOnly: {{ not .hostRootWritable }}
{{- end }}
{{- if .mountModulesLoad }}
    - name: modules-load-d
      mountPath: /host-modules-load.d
{{- end }}
{{- end -}}

{{/*
Writable /tmp for readOnlyRootFilesystem containers (host tools / libraries).
*/}}
{{- define "kata-deploy.tmpVolumeMount" -}}
- name: tmp
  mountPath: /tmp
{{- end -}}

{{- define "kata-deploy.tmpVolume" -}}
- name: tmp
  emptyDir: {}
{{- end -}}

{{/*
Common volumeMounts for any pod that runs the kata-deploy binary against the
host. Emitted at column 0; indent with `nindent` at the call site.
*/}}
{{- define "kata-deploy.commonVolumeMounts" -}}
{{ include "kata-deploy.tmpVolumeMount" . }}
- name: crio-conf
  mountPath: /etc/crio/
- name: containerd-conf
  mountPath: /etc/containerd/
- name: kata-install
  mountPath: {{ include "kata-deploy.installDir" . | quote }}
- name: systemd-system
  mountPath: /etc/systemd/system
- name: systemd-private
  mountPath: /run/systemd/private
- name: boot
  mountPath: /boot
  readOnly: true
- name: host-machine-id
  mountPath: /host-machine-id
  readOnly: true
- name: host-run-lock
  mountPath: /host-run-lock
- name: host-usr-bin
  mountPath: /host-usr/bin
  readOnly: true
- name: host-usr-sbin
  mountPath: /host-usr/sbin
  readOnly: true
- name: host-usr-local-bin
  mountPath: /host-usr-local/bin
  readOnly: true
- name: host-usr-local-sbin
  mountPath: /host-usr-local/sbin
  readOnly: true
- name: host-bin
  mountPath: /host-bin
  readOnly: true
- name: host-sbin
  mountPath: /host-sbin
  readOnly: true
{{- if .Values.containerd.userDropIn | trim }}
- name: custom-containerd-config
  mountPath: /custom-containerd-config/
  readOnly: true
{{- end }}
{{- if eq (include "kata-deploy.hasCustomConfigsConfigMap" . | trim) "true" }}
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
{{ include "kata-deploy.tmpVolume" . }}
- name: crio-conf
  hostPath:
    path: /etc/crio/
- name: containerd-conf
  hostPath:
    path: '{{- template "containerdConfPath" .Values }}'
- name: kata-install
  hostPath:
    path: {{ include "kata-deploy.installDir" . | quote }}
    type: DirectoryOrCreate
- name: systemd-system
  hostPath:
    path: /etc/systemd/system
    type: DirectoryOrCreate
- name: systemd-private
  hostPath:
    path: /run/systemd/private
    type: Socket
- name: boot
  hostPath:
    path: /boot
- name: host-machine-id
  hostPath:
    path: /etc/machine-id
    type: File
{{- /* Writable, unlike the other host mounts: this is where the lock the
       installs on this node take against each other is created. */}}
- name: host-run-lock
  hostPath:
    path: /run/lock
    type: DirectoryOrCreate
- name: host-usr-bin
  hostPath:
    path: /usr/bin
- name: host-usr-sbin
  hostPath:
    path: /usr/sbin
- name: host-usr-local-bin
  hostPath:
    path: /usr/local/bin
- name: host-usr-local-sbin
  hostPath:
    path: /usr/local/sbin
- name: host-bin
  hostPath:
    path: /bin
- name: host-sbin
  hostPath:
    path: /sbin
{{- if .Values.containerd.userDropIn | trim }}
- name: custom-containerd-config
  configMap:
{{- if .Values.env.multiInstallSuffix }}
    name: {{ .Chart.Name }}-containerd-user-dropin-{{ .Values.env.multiInstallSuffix }}
{{- else }}
    name: {{ .Chart.Name }}-containerd-user-dropin
{{- end }}
{{- end }}
{{- if eq (include "kata-deploy.hasCustomConfigsConfigMap" . | trim) "true" }}
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
Returns "true" when the custom-configs ConfigMap is rendered and mounted.
*/}}
{{- define "kata-deploy.hasCustomConfigsConfigMap" -}}
{{- $hasCustomRuntimes := and .Values.customRuntimes.enabled .Values.customRuntimes.runtimes -}}
{{- $hasDefaultRuntimeDropIns := eq (include "kata-deploy.hasDefaultRuntimeDropIns" . | trim) "true" -}}
{{- if or $hasCustomRuntimes $hasDefaultRuntimeDropIns -}}true{{- end -}}
{{- end -}}

{{/*
ConfigMap containing custom runtime configuration and default shim drop-ins.
Mounted into kata-deploy pods at /custom-configs/.
*/}}
{{- define "kata-deploy.customConfigsConfigMap" -}}
{{- $hasCustomRuntimes := and .Values.customRuntimes.enabled .Values.customRuntimes.runtimes -}}
{{- $hasDefaultRuntimeDropIns := eq (include "kata-deploy.hasDefaultRuntimeDropIns" . | trim) "true" -}}
{{- if or $hasCustomRuntimes $hasDefaultRuntimeDropIns }}
apiVersion: v1
kind: ConfigMap
metadata:
{{- if .Values.env.multiInstallSuffix }}
  name: {{ .Chart.Name }}-custom-configs-{{ .Values.env.multiInstallSuffix }}
{{- else }}
  name: {{ .Chart.Name }}-custom-configs
{{- end }}
  namespace: {{ .Release.Namespace }}
  labels:
    {{- include "kata-deploy.labels" . | nindent 4 }}
data:
{{- if $hasCustomRuntimes }}
  custom-runtimes.list: |
{{- range $name := keys .Values.customRuntimes.runtimes | sortAlpha }}
{{- $runtime := index $.Values.customRuntimes.runtimes $name }}
{{- $handler := "" }}
{{- if $runtime.runtimeClass }}
{{- range (splitList "\n" $runtime.runtimeClass) }}
{{- $line := trim . }}
{{- if hasPrefix "handler:" $line }}
{{- $handler = trim (trimPrefix "handler:" $line) }}
{{- end }}
{{- end }}
{{- end }}
{{- if $handler }}
    {{ $handler }}:{{ $runtime.baseConfig }}:{{ dig "containerd" "snapshotter" "" $runtime }}:{{ dig "crio" "pullType" "" $runtime }}
{{- end }}
{{- end }}
{{- range $name := keys .Values.customRuntimes.runtimes | sortAlpha }}
{{- $runtime := index $.Values.customRuntimes.runtimes $name }}
{{- $handler := "" }}
{{- if $runtime.runtimeClass }}
{{- range (splitList "\n" $runtime.runtimeClass) }}
{{- $line := trim . }}
{{- if hasPrefix "handler:" $line }}
{{- $handler = trim (trimPrefix "handler:" $line) }}
{{- end }}
{{- end }}
{{- end }}
{{- if and $handler $runtime.dropIn }}
  dropin-{{ $handler }}.toml: |
{{ $runtime.dropIn | indent 4 }}
{{- end }}
{{- end }}
{{- end }}
{{- $disableAll := .Values.shims.disableAll | default false -}}
{{- range $shimName := keys .Values.shims | sortAlpha }}
{{- if ne $shimName "disableAll" }}
{{- $shimConfig := index $.Values.shims $shimName -}}
{{- $shimEnabled := eq (include "kata-deploy.isShimEnabled" (dict "shimConfig" $shimConfig "disableAll" $disableAll) | trim) "true" -}}
{{- if and $shimEnabled $shimConfig.dropIn (ne (trim $shimConfig.dropIn) "") }}
  dropin-{{ $shimName }}.toml: |
{{ $shimConfig.dropIn | indent 4 }}
{{- end }}
{{- end }}
{{- end }}
{{- end }}
{{- end -}}

{{/*
Checksum annotations for ConfigMaps mounted into kata-deploy pods. Changing a
mounted ConfigMap does not update the pod spec by itself; hashing the rendered
ConfigMap into the pod template forces a rollout when the data changes.
*/}}
{{- define "kata-deploy.configMapChecksumAnnotations" -}}
{{- $annotations := dict -}}
{{- if .Values.containerd.userDropIn | trim }}
{{- $_ := set $annotations "checksum/containerd-user-dropin" (include (print $.Template.BasePath "/containerd-user-dropin-config.yaml") . | sha256sum) -}}
{{- end }}
{{- if eq (include "kata-deploy.hasCustomConfigsConfigMap" . | trim) "true" }}
{{- $_ := set $annotations "checksum/custom-configs" (include "kata-deploy.customConfigsConfigMap" . | sha256sum) -}}
{{- end }}
{{- toYaml $annotations -}}
{{- end -}}

{{/*
Pod template annotations: user-provided podAnnotations plus ConfigMap checksums.
Checksums are applied last so a user-supplied "checksum/*" key can never
override a computed value and silently disable the rollout trigger.
*/}}
{{- define "kata-deploy.podTemplateAnnotations" -}}
{{- $annotations := dict -}}
{{- with .Values.podAnnotations }}
{{- range $key, $value := . }}
{{- $_ := set $annotations $key $value -}}
{{- end }}
{{- end }}
{{- $checksums := fromYaml (include "kata-deploy.configMapChecksumAnnotations" .) | default dict -}}
{{- range $key, $value := $checksums }}
{{- $_ := set $annotations $key $value -}}
{{- end }}
{{- if $annotations }}
{{- toYaml $annotations -}}
{{- end }}
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
{{- include "kata-deploy.failOnRemovedJobSelectionKeys" . -}}
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
