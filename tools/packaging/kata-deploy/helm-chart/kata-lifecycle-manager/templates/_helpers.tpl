{{/*
Copyright (c) 2026 The Kata Containers Authors
SPDX-License-Identifier: Apache-2.0
*/}}

{{/*
Expand the name of the chart.
*/}}
{{- define "kata-lifecycle-manager.name" -}}
{{- default .Chart.Name .Values.nameOverride | trunc 63 | trimSuffix "-" }}
{{- end }}

{{/*
Create a default fully qualified app name.
*/}}
{{- define "kata-lifecycle-manager.fullname" -}}
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
{{- define "kata-lifecycle-manager.labels" -}}
helm.sh/chart: {{ include "kata-lifecycle-manager.name" . }}-{{ .Chart.Version }}
app.kubernetes.io/name: {{ include "kata-lifecycle-manager.name" . }}
app.kubernetes.io/instance: {{ .Release.Name }}
app.kubernetes.io/version: {{ .Chart.AppVersion | quote }}
app.kubernetes.io/managed-by: {{ .Release.Service }}
app.kubernetes.io/part-of: kata-containers
{{- end }}

{{/*
ServiceAccount name
*/}}
{{- define "kata-lifecycle-manager.serviceAccountName" -}}
{{- include "kata-lifecycle-manager.fullname" . }}
{{- end }}
