#!/bin/sh
# Copyright (c) 2026 Kata Containers community
# SPDX-License-Identifier: Apache-2.0

set -eu

REGISTRY="docker.io/katadancni"
VERSION="v0.1.1"

cat <<EOF > kata-dan.yaml
# Copyright (c) 2026 Kata Containers community
# SPDX-License-Identifier: Apache-2.0

apiVersion: v1
kind: Namespace
metadata:
  name: kata-dan-system
  labels:
    pod-security.kubernetes.io/enforce: privileged
    pod-security.kubernetes.io/enforce-version: latest
---
apiVersion: v1
kind: ServiceAccount
metadata:
  name: kata-dan-cni
  namespace: kata-dan-system
automountServiceAccountToken: false
---
apiVersion: rbac.authorization.k8s.io/v1
kind: ClusterRole
metadata:
  name: kata-dan-cni
rules:
- apiGroups: [""]
  resources: ["pods"]
  verbs: ["get"]
- apiGroups: [""]
  resources: ["nodes"]
  verbs: ["get"]
- apiGroups: ["kubeovn.io"]
  resources: ["subnets"]
  verbs: ["get"]
---
apiVersion: rbac.authorization.k8s.io/v1
kind: ClusterRoleBinding
metadata:
  name: kata-dan-cni
roleRef:
  apiGroup: rbac.authorization.k8s.io
  kind: ClusterRole
  name: kata-dan-cni
subjects:
- kind: ServiceAccount
  name: kata-dan-cni
  namespace: kata-dan-system
---
apiVersion: apps/v1
kind: DaemonSet
metadata:
  name: kata-dan-cni
  namespace: kata-dan-system
  labels:
    app.kubernetes.io/name: kata-dan-cni
    app.kubernetes.io/component: installer
spec:
  selector:
    matchLabels:
      app.kubernetes.io/name: kata-dan-cni
      app.kubernetes.io/component: installer
  template:
    metadata:
      labels:
        app.kubernetes.io/name: kata-dan-cni
        app.kubernetes.io/component: installer
    spec:
      serviceAccountName: kata-dan-cni
      automountServiceAccountToken: false
      hostNetwork: true
      dnsPolicy: ClusterFirstWithHostNet
      terminationGracePeriodSeconds: 5
      nodeSelector:
        kubernetes.io/os: linux
      initContainers:
      - name: prepare-credentials
        image: "${REGISTRY}/kata-dan-cni:${VERSION}"
        imagePullPolicy: IfNotPresent
        command: ["/kata-dan-cni/install-credentials.sh"]
        securityContext:
          runAsUser: 0
          runAsGroup: 0
          privileged: false
          allowPrivilegeEscalation: false
          readOnlyRootFilesystem: true
          capabilities:
            drop: ["ALL"]
          seccompProfile:
            type: RuntimeDefault
        volumeMounts:
        - name: api-access
          mountPath: /var/run/secrets/kata-dan-cni
          readOnly: true
        - name: host-credentials
          mountPath: /etc/kata-dan-cni
      - name: install-cni
        image: "${REGISTRY}/kata-dan-cni:${VERSION}"
        imagePullPolicy: IfNotPresent
        command: ["/kata-dan-cni/install-cni.sh"]
        env:
        - name: KATA_DAN_NODE_NAME
          valueFrom:
            fieldRef:
              fieldPath: spec.nodeName
        - name: KATA_DAN_RUNTIME_CLASS
          value: kata-qemu-runtime-rs
        - name: KATA_DAN_CONF_DIR
          value: /run/kata-containers/dans
        securityContext:
          runAsUser: 0
          runAsGroup: 0
          privileged: false
          allowPrivilegeEscalation: false
          readOnlyRootFilesystem: true
          capabilities:
            drop: ["ALL"]
          seccompProfile:
            type: RuntimeDefault
        volumeMounts:
        - name: cni-bin
          mountPath: /opt/cni/bin
        - name: cni-conf
          mountPath: /etc/cni/net.d
      containers:
      - name: credential-syncer
        image: "${REGISTRY}/kata-dan-cni:${VERSION}"
        imagePullPolicy: IfNotPresent
        command: ["/kata-dan-cni/sync-credentials.sh"]
        resources:
          requests:
            cpu: 1m
            memory: 4Mi
          limits:
            cpu: 10m
            memory: 16Mi
        securityContext:
          runAsUser: 0
          runAsGroup: 0
          privileged: false
          allowPrivilegeEscalation: false
          readOnlyRootFilesystem: true
          capabilities:
            drop: ["ALL"]
          seccompProfile:
            type: RuntimeDefault
        volumeMounts:
        - name: api-access
          mountPath: /var/run/secrets/kata-dan-cni
          readOnly: true
        - name: host-credentials
          mountPath: /etc/kata-dan-cni
        - name: cni-bin
          mountPath: /opt/cni/bin
        - name: cni-conf
          mountPath: /etc/cni/net.d
        - name: installer-state
          mountPath: /run/kata-dan-cni
      volumes:
      - name: installer-state
        emptyDir: {}
      - name: api-access
        projected:
          defaultMode: 0400
          sources:
          - serviceAccountToken:
              path: token
              expirationSeconds: 3600
          - configMap:
              name: kube-root-ca.crt
              items:
              - key: ca.crt
                path: ca.crt
      - name: host-credentials
        hostPath:
          path: /etc/kata-dan-cni
          type: DirectoryOrCreate
      - name: cni-bin
        hostPath:
          path: /opt/cni/bin
          type: DirectoryOrCreate
      - name: cni-conf
        hostPath:
          path: /etc/cni/net.d
          type: Directory
EOF

kubectl apply -f kata-dan.yaml
kubectl -n kata-dan-system rollout status daemonset/kata-dan-cni --timeout=180s

desired=$(kubectl -n kata-dan-system get daemonset/kata-dan-cni \
    -o 'jsonpath={.status.desiredNumberScheduled}')
if ! [ "${desired}" -gt 0 ] 2>/dev/null; then
    echo "Kata DAN installer has no target nodes" >&2
    exit 1
fi

echo "Kata DAN CNI installed"
