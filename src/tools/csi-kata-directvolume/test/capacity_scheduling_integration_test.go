// Copyright (c) 2026 Kata Contributors
//
// SPDX-License-Identifier: Apache-2.0
//
//go:build integration

// Package test contains opt-in Kubernetes integration scenarios. It uses the
// kubectl binary instead of a Kubernetes client so the normal unit-test build
// does not acquire cluster-client dependencies.
package test

import (
	"context"
	"fmt"
	"os"
	"os/exec"
	"strings"
	"testing"
	"time"
)

// TestDirectVolumeCapacityScheduling exercises the real two-node scheduling
// path. It is intentionally excluded from go test ./...; unit tests must not
// need a cluster.
//
// Prerequisites for a local Linux run:
//
//  1. Create a kind cluster with two workers (Kata's worker-image and runtime
//     installation instructions still apply; a stock kind node is not enough):
//
//     kind create cluster --name kata-capacity --config=- <<'EOF'
//     kind: Cluster
//     apiVersion: kind.x-k8s.io/v1alpha4
//     nodes:
//     - role: control-plane
//     - role: worker
//     - role: worker
//     EOF
//
//  2. Install the Kata runtime and this CSI driver on both workers. Build and
//     load the driver image into kind, then configure node-a with
//     --capacity=fast=2Gi and node-b with --capacity=fast=8Gi. The provisioner
//     must use --enable-capacity, --capacity-ownerref-level=1 and a polling
//     interval of at most 10s. CSIDriver.spec.storageCapacity must be true.
//     These are prerequisites of issue #14151, not changes made by this test.
//
//  3. Create a StorageClass with volumeBindingMode: WaitForFirstConsumer and
//     parameters.kind: fast. Verify that CSIStorageCapacity objects for both
//     workers have appeared before starting the test.
//
//  4. Run the scenario from src/tools/csi-kata-directvolume:
//
//     export KATA_DIRECTVOLUME_E2E=1
//     export KATA_DIRECTVOLUME_E2E_SMALL_NODE=kata-capacity-worker
//     export KATA_DIRECTVOLUME_E2E_LARGE_NODE=kata-capacity-worker2
//     export KATA_DIRECTVOLUME_E2E_STORAGE_CLASS=csi-kata-directvolume-capacity
//     export KATA_DIRECTVOLUME_E2E_RUNTIME_CLASS=kata
//     go test -tags=integration ./test -run TestDirectVolumeCapacityScheduling -v
//
// The scenario provisions 4 GiB and expects the large node, keeps a second
// 5 GiB claim pending while both nodes lack room, then deletes the first claim
// and expects the second claim to recover on the large node. It asserts
// structured object state rather than version-specific scheduler event text.
func TestDirectVolumeCapacityScheduling(t *testing.T) {
	if os.Getenv("KATA_DIRECTVOLUME_E2E") != "1" {
		t.Skip("set KATA_DIRECTVOLUME_E2E=1 after preparing the documented two-node kind cluster")
	}

	smallNode := requiredCapacityE2EEnv(t, "KATA_DIRECTVOLUME_E2E_SMALL_NODE")
	largeNode := requiredCapacityE2EEnv(t, "KATA_DIRECTVOLUME_E2E_LARGE_NODE")
	storageClass := requiredCapacityE2EEnv(t, "KATA_DIRECTVOLUME_E2E_STORAGE_CLASS")
	runtimeClass := requiredCapacityE2EEnv(t, "KATA_DIRECTVOLUME_E2E_RUNTIME_CLASS")
	if smallNode == largeNode {
		t.Fatal("small and large capacity nodes must be different")
	}

	ctx, cancel := context.WithTimeout(context.Background(), 10*time.Minute)
	defer cancel()

	storageCapacity := kubectlForCapacityE2E(t, ctx, "", "get", "csidriver",
		"directvolume.csi.katacontainers.io", "-o", "jsonpath={.spec.storageCapacity}")
	if storageCapacity != "true" {
		t.Fatalf("CSIDriver storageCapacity is %q, want true; complete the capacity publication setup first", storageCapacity)
	}
	for _, node := range []string{smallNode, largeNode} {
		kubectlForCapacityE2E(t, ctx, "", "wait", "--for=condition=Ready", "node/"+node, "--timeout=30s")
	}

	namespace := fmt.Sprintf("directvolume-capacity-%d", time.Now().Unix())
	kubectlForCapacityE2E(t, ctx, "", "create", "namespace", namespace)
	t.Cleanup(func() {
		_, err := runKubectlForCapacityE2E(context.Background(), "", "delete", "namespace", namespace,
			"--ignore-not-found=true", "--wait=false")
		if err != nil {
			t.Logf("cleanup namespace %s: %v", namespace, err)
		}
	})

	applyCapacityE2EWorkload(t, ctx, namespace, storageClass, runtimeClass, "four", 4)
	kubectlForCapacityE2E(t, ctx, "", "wait", "-n", namespace,
		"--for=jsonpath={.status.phase}=Bound", "pvc/claim-four", "--timeout=120s")
	kubectlForCapacityE2E(t, ctx, "", "wait", "-n", namespace,
		"--for=jsonpath={.status.phase}=Running", "pod/consumer-four", "--timeout=180s")
	assertCapacityE2EPodNode(t, ctx, namespace, "consumer-four", largeNode)

	volumeName := kubectlForCapacityE2E(t, ctx, "", "get", "-n", namespace, "pvc/claim-four",
		"-o", "jsonpath={.spec.volumeName}")
	nodeAffinity := kubectlForCapacityE2E(t, ctx, "", "get", "pv/"+volumeName,
		"-o", "jsonpath={.spec.nodeAffinity.required.nodeSelectorTerms}")
	if !strings.Contains(nodeAffinity, largeNode) {
		t.Fatalf("PV %s node affinity %s does not select %s", volumeName, nodeAffinity, largeNode)
	}

	applyCapacityE2EWorkload(t, ctx, namespace, storageClass, runtimeClass, "five", 5)
	// Allow several 10-second capacity polls and scheduling cycles. Neither
	// node can fit 5 GiB while the 4 GiB volume remains reserved.
	time.Sleep(30 * time.Second)
	assertCapacityE2EPodPhase(t, ctx, namespace, "consumer-five", "Pending")
	claimPhase := kubectlForCapacityE2E(t, ctx, "", "get", "-n", namespace, "pvc/claim-five",
		"-o", "jsonpath={.status.phase}")
	if claimPhase != "Pending" {
		t.Fatalf("exhausted claim phase is %q, want Pending", claimPhase)
	}
	selectedNode := kubectlForCapacityE2E(t, ctx, "", "get", "-n", namespace, "pod/consumer-five",
		"-o", "jsonpath={.spec.nodeName}")
	if selectedNode != "" {
		t.Fatalf("exhausted pod was assigned to %q, want no selected node", selectedNode)
	}

	kubectlForCapacityE2E(t, ctx, "", "delete", "-n", namespace, "pod/consumer-four", "--wait=true")
	kubectlForCapacityE2E(t, ctx, "", "delete", "-n", namespace, "pvc/claim-four", "--wait=true")
	kubectlForCapacityE2E(t, ctx, "", "wait", "-n", namespace,
		"--for=jsonpath={.status.phase}=Bound", "pvc/claim-five", "--timeout=120s")
	kubectlForCapacityE2E(t, ctx, "", "wait", "-n", namespace,
		"--for=jsonpath={.status.phase}=Running", "pod/consumer-five", "--timeout=180s")
	assertCapacityE2EPodNode(t, ctx, namespace, "consumer-five", largeNode)
}

func requiredCapacityE2EEnv(t *testing.T, name string) string {
	t.Helper()
	value := strings.TrimSpace(os.Getenv(name))
	if value == "" {
		t.Fatalf("%s must be set for the capacity scheduling integration test", name)
	}
	return value
}

func applyCapacityE2EWorkload(t *testing.T, ctx context.Context, namespace, storageClass, runtimeClass, suffix string, sizeGiB int) {
	t.Helper()
	manifest := fmt.Sprintf(`apiVersion: v1
kind: PersistentVolumeClaim
metadata:
  name: claim-%[1]s
spec:
  accessModes: [ReadWriteOnce]
  storageClassName: %[2]s
  resources:
    requests:
      storage: %[4]dGi
---
apiVersion: v1
kind: Pod
metadata:
  name: consumer-%[1]s
spec:
  runtimeClassName: %[3]s
  automountServiceAccountToken: false
  restartPolicy: Never
  terminationGracePeriodSeconds: 1
  containers:
    - name: pause
      image: registry.k8s.io/pause:3.9
  volumes:
    - name: data
      persistentVolumeClaim:
        claimName: claim-%[1]s
`, suffix, storageClass, runtimeClass, sizeGiB)
	kubectlForCapacityE2E(t, ctx, manifest, "apply", "-n", namespace, "-f", "-")
}

func assertCapacityE2EPodNode(t *testing.T, ctx context.Context, namespace, pod, expectedNode string) {
	t.Helper()
	node := kubectlForCapacityE2E(t, ctx, "", "get", "-n", namespace, "pod/"+pod, "-o", "jsonpath={.spec.nodeName}")
	if node != expectedNode {
		t.Fatalf("pod %s landed on %q, want %q", pod, node, expectedNode)
	}
}

func assertCapacityE2EPodPhase(t *testing.T, ctx context.Context, namespace, pod, expectedPhase string) {
	t.Helper()
	phase := kubectlForCapacityE2E(t, ctx, "", "get", "-n", namespace, "pod/"+pod, "-o", "jsonpath={.status.phase}")
	if phase != expectedPhase {
		t.Fatalf("pod %s phase is %q, want %q", pod, phase, expectedPhase)
	}
}

func kubectlForCapacityE2E(t *testing.T, ctx context.Context, input string, args ...string) string {
	t.Helper()
	output, err := runKubectlForCapacityE2E(ctx, input, args...)
	if err != nil {
		t.Fatalf("kubectl %s failed: %v\n%s", strings.Join(args, " "), err, output)
	}
	return strings.TrimSpace(output)
}

func runKubectlForCapacityE2E(ctx context.Context, input string, args ...string) (string, error) {
	command := exec.CommandContext(ctx, "kubectl", args...)
	command.Stdin = strings.NewReader(input)
	output, err := command.CombinedOutput()
	return string(output), err
}
