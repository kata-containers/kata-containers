# Copyright (c) 2019 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0

apiVersion: admissionregistration.k8s.io/v1
kind: MutatingWebhookConfiguration
metadata:
  name: pod-annotate-webhook
  labels:
    app: pod-annotate-webhook
    kind: mutator
webhooks:
  - name: pod-annotate-webhook.kata.xyz
    sideEffects: None
    failurePolicy: Ignore
    admissionReviewVersions: ["v1", "v1beta1"]
    clientConfig:
      service:
        name: pod-annotate-webhook
        namespace: default
        path: "/mutate"
      caBundle: CA_BUNDLE
    rules:
      - operations: [ "CREATE" ]
        apiGroups: [""]
        apiVersions: ["v1"]
        resources: ["pods"]
