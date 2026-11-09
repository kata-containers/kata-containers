//
// Copyright 2017 The Kubernetes Authors.
// Copyright (c) 2023 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

package deploy

import (
	"os"
	"path/filepath"
	"strings"
	"testing"

	"gopkg.in/yaml.v3"
)

// The tests in this file pin the relationship between the deployment
// manifests and the driver's capacity behavior (#14151, the "manifest
// tests" of the issue's test plan): scheduler capacity filtering only
// works when CSIDriver.storageCapacity, the provisioner's capacity
// controller, and the DaemonSet's mounts agree with each other, and
// none of that is visible to `kubectl apply` — a manifest that is
// individually valid but internally inconsistent deploys "fine" and
// then silently publishes nothing.
//
// Paths are resolved relative to this package's directory so the
// tests run from any working directory, the same way `go test ./...`
// from the repo root does.

const (
	driverName = "directvolume.csi.katacontainers.io"

	pluginYAML     = "kata-directvolume/csi-directvol-plugin.yaml"
	driverinfoYAML = "kata-directvolume/csi-directvol-driverinfo.yaml"
	exampleSC      = "../examples/pod-with-capacity/csi-storageclass.yaml"
	exampleCM      = "../examples/pod-with-capacity/capacity-configmap.yaml"
)

// manifestPath resolves a path relative to this package's directory
// (src/tools/csi-kata-directvolume/deploy), so the example paths
// escape with ../ while the manifest paths stay inside deploy/.
func manifestPath(t *testing.T, rel string) string {
	t.Helper()
	path, err := filepath.Abs(rel)
	if err != nil {
		t.Fatal(err)
	}
	return path
}

// loadManifest reads and unmarshals one YAML document.
func loadManifest(t *testing.T, path string, into interface{}) {
	t.Helper()
	content, err := os.ReadFile(path)
	if err != nil {
		t.Fatalf("read %s: %v", path, err)
	}
	if err := yaml.Unmarshal(content, into); err != nil {
		t.Fatalf("parse %s: %v", path, err)
	}
}

func loadPlugin(t *testing.T) map[string]interface{} {
	t.Helper()
	var doc map[string]interface{}
	loadManifest(t, manifestPath(t, pluginYAML), &doc)
	return doc
}

func loadDriverInfo(t *testing.T) map[string]interface{} {
	t.Helper()
	var doc map[string]interface{}
	loadManifest(t, manifestPath(t, driverinfoYAML), &doc)
	return doc
}

// containersOf digs the DaemonSet pod template's containers out of the
// untyped manifest.
func containersOf(t *testing.T, manifest map[string]interface{}) []map[string]interface{} {
	t.Helper()
	spec, ok := manifest["spec"].(map[string]interface{})
	if !ok {
		t.Fatal("DaemonSet has no spec")
	}
	template, ok := spec["template"].(map[string]interface{})
	if !ok {
		t.Fatal("DaemonSet spec has no template")
	}
	podSpec, ok := template["spec"].(map[string]interface{})
	if !ok {
		t.Fatal("pod template has no spec")
	}
	containers, ok := podSpec["containers"].([]interface{})
	if !ok {
		t.Fatal("pod template has no containers")
	}
	out := make([]map[string]interface{}, 0, len(containers))
	for _, c := range containers {
		container, ok := c.(map[string]interface{})
		if !ok {
			t.Fatal("container entry is not a mapping")
		}
		out = append(out, container)
	}
	return out
}

func containerByName(t *testing.T, manifest map[string]interface{}, name string) map[string]interface{} {
	t.Helper()
	for _, c := range containersOf(t, manifest) {
		if c["name"] == name {
			return c
		}
	}
	t.Fatalf("container %q not found in DaemonSet", name)
	return nil
}

func containerArgs(t *testing.T, manifest map[string]interface{}, name string) []string {
	t.Helper()
	raw, ok := containerByName(t, manifest, name)["args"].([]interface{})
	if !ok {
		t.Fatalf("container %q has no args", name)
	}
	args := make([]string, 0, len(raw))
	for _, a := range raw {
		s, ok := a.(string)
		if !ok {
			t.Fatalf("container %q has a non-string arg %v", name, a)
		}
		args = append(args, s)
	}
	return args
}

func containerMounts(t *testing.T, manifest map[string]interface{}, name string) []map[string]interface{} {
	t.Helper()
	raw, ok := containerByName(t, manifest, name)["volumeMounts"].([]interface{})
	if !ok {
		t.Fatalf("container %q has no volumeMounts", name)
	}
	out := make([]map[string]interface{}, 0, len(raw))
	for _, m := range raw {
		mount, ok := m.(map[string]interface{})
		if !ok {
			t.Fatalf("container %q has a non-mapping volumeMount %v", name, m)
		}
		out = append(out, mount)
	}
	return out
}

func daemonsetVolumes(t *testing.T, manifest map[string]interface{}) []map[string]interface{} {
	t.Helper()
	spec, ok := manifest["spec"].(map[string]interface{})
	if !ok {
		t.Fatal("DaemonSet has no spec")
	}
	template, ok := spec["template"].(map[string]interface{})
	if !ok {
		t.Fatal("DaemonSet spec has no template")
	}
	podSpec, ok := template["spec"].(map[string]interface{})
	if !ok {
		t.Fatal("pod template has no spec")
	}
	raw, ok := podSpec["volumes"].([]interface{})
	if !ok {
		t.Fatal("pod template has no volumes")
	}
	out := make([]map[string]interface{}, 0, len(raw))
	for _, v := range raw {
		volume, ok := v.(map[string]interface{})
		if !ok {
			t.Fatalf("volume entry is not a mapping: %v", v)
		}
		out = append(out, volume)
	}
	return out
}

func hasArg(args []string, want string) bool {
	for _, a := range args {
		if a == want || strings.HasPrefix(a, want+"=") {
			return true
		}
	}
	return false
}

func argValue(args []string, flag string) (string, bool) {
	prefix := flag + "="
	for _, a := range args {
		if strings.HasPrefix(a, prefix) {
			return strings.TrimPrefix(a, prefix), true
		}
		if a == flag {
			return "", true
		}
	}
	return "", false
}

// TestDriverInfoEnablesStorageCapacity pins scheduler capacity
// filtering: with storageCapacity false the scheduler skips the
// CSIStorageCapacity check entirely, which is the default this issue
// started from.
func TestDriverInfoEnablesStorageCapacity(t *testing.T) {
	doc := loadDriverInfo(t)
	if doc["kind"] != "CSIDriver" {
		t.Fatalf("driverinfo manifest is a %v, want CSIDriver", doc["kind"])
	}
	meta, _ := doc["metadata"].(map[string]interface{})
	if meta["name"] != driverName {
		t.Fatalf("CSIDriver name: want %q, got %v", driverName, meta["name"])
	}
	spec, ok := doc["spec"].(map[string]interface{})
	if !ok {
		t.Fatal("CSIDriver has no spec")
	}
	got, ok := spec["storageCapacity"].(bool)
	if !ok || !got {
		t.Fatalf("CSIDriver.spec.storageCapacity: want true, got %v (of %T)", spec["storageCapacity"], spec["storageCapacity"])
	}
}

// TestProvisionerCapacityControllerPinned verifies the sidecar's
// capacity controller is on, with the DaemonSet as the owner level:
// ownerref level 1 (not 0) keeps the CSIStorageCapacity objects alive
// across individual pod restarts, since a DaemonSet-owned pod is
// recreated routinely.
func TestProvisionerCapacityControllerPinned(t *testing.T) {
	args := containerArgs(t, loadPlugin(t), "csi-provisioner")
	if !hasArg(args, "--enable-capacity") {
		t.Errorf("csi-provisioner args: --enable-capacity missing from %v", args)
	}
	value, ok := argValue(args, "--capacity-ownerref-level")
	if !ok || value != "1" {
		t.Errorf("csi-provisioner args: --capacity-ownerref-level: want 1, got %q (present=%v)", value, ok)
	}
	if _, ok := argValue(args, "--capacity-poll-interval"); !ok {
		t.Errorf("csi-provisioner args: --capacity-poll-interval missing from %v", args)
	}
}

// TestCapacityConfigWiring checks the per-node capacity chain end to
// end: the driver's --capacity-config-dir points at a mount of the
// optional csi-kata-directvol-capacity ConfigMap named
// capacity-config. optional must stay true — without the ConfigMap
// the DaemonSet still comes up in no-pool mode (#14151 step 3).
func TestCapacityConfigWiring(t *testing.T) {
	manifest := loadPlugin(t)

	dir, ok := argValue(containerArgs(t, manifest, "kata-directvolume"), "--capacity-config-dir")
	if !ok || dir == "" {
		t.Fatalf("driver args: --capacity-config-dir missing")
	}

	var mount map[string]interface{}
	for _, m := range containerMounts(t, manifest, "kata-directvolume") {
		if m["mountPath"] == dir {
			mount = m
			break
		}
	}
	if mount == nil {
		t.Fatalf("driver has no volumeMount at the --capacity-config-dir path %q", dir)
	}
	if name := mount["name"]; name != "capacity-config" {
		t.Fatalf("mount at %q has name %v, want capacity-config", dir, name)
	}
	if readOnly, _ := mount["readOnly"].(bool); !readOnly {
		t.Errorf("capacity-config mount should be readOnly")
	}

	var volume map[string]interface{}
	for _, v := range daemonsetVolumes(t, manifest) {
		if v["name"] == "capacity-config" {
			volume = v
			break
		}
	}
	if volume == nil {
		t.Fatalf("DaemonSet has no volume named capacity-config for mount at %q", dir)
	}
	cm, ok := volume["configMap"].(map[string]interface{})
	if !ok {
		t.Fatalf("capacity-config volume is not a configMap: %v", volume)
	}
	if cm["name"] != "csi-kata-directvol-capacity" {
		t.Errorf("capacity-config volume references ConfigMap %v, want csi-kata-directvol-capacity", cm["name"])
	}
	if optional, _ := cm["optional"].(bool); !optional {
		t.Errorf("capacity-config ConfigMap must be optional: without it the DaemonSet must still start")
	}
}

// TestVolumeMountsAndVolumesAgree guards against the classic silent
// manifest break: a mount referencing a volume that no longer exists
// (pod stuck in ContainerCreating) or a volume that is mounted
// nowhere (component quietly off — the capacity-config volume would
// fail exactly this way if a refactor dropped its mount). Volumes may
// legitimately be mounted by several containers (e.g. socket-dir); what
// must not happen is a mount without a volume, a volume without any
// mount, or a duplicate volume name.
func TestVolumeMountsAndVolumesAgree(t *testing.T) {
	manifest := loadPlugin(t)
	volumes := make(map[string]bool)
	for _, v := range daemonsetVolumes(t, manifest) {
		name, ok := v["name"].(string)
		if !ok || name == "" {
			t.Fatalf("volume without a name: %v", v)
		}
		if volumes[name] {
			t.Errorf("duplicate volume name %q", name)
		}
		volumes[name] = true
	}

	mounted := make(map[string]bool)
	for _, container := range containersOf(t, manifest) {
		name, _ := container["name"].(string)
		mounts, ok := container["volumeMounts"].([]interface{})
		if !ok {
			continue // no mounts is valid (e.g. a sidecar mounting nothing)
		}
		for _, m := range mounts {
			mount, ok := m.(map[string]interface{})
			if !ok {
				t.Fatalf("%s: non-mapping volumeMount %v", name, m)
			}
			ref, _ := mount["name"].(string)
			if !volumes[ref] {
				t.Errorf("%s: volumeMount %q has no matching volume", name, ref)
			}
			mounted[ref] = true
		}
	}

	for name := range volumes {
		if !mounted[name] {
			t.Errorf("volume %q is never mounted", name)
		}
	}
}

// TestExampleProvisionerAndBindingMode checks the shipped example for
// scheduler capacity filtering actually triggers it: the StorageClass
// must name this driver's provisioner and bind with
// WaitForFirstConsumer — with Immediate, volumes are created before
// the pod is scheduled and CSIStorageCapacity never filters anything.
func TestExampleProvisionerAndBindingMode(t *testing.T) {
	var doc map[string]interface{}
	loadManifest(t, manifestPath(t, exampleSC), &doc)

	if doc["provisioner"] != driverName {
		t.Errorf("example StorageClass provisioner: want %q, got %v", driverName, doc["provisioner"])
	}
	if doc["volumeBindingMode"] != "WaitForFirstConsumer" {
		t.Errorf("example StorageClass volumeBindingMode: want WaitForFirstConsumer, got %v", doc["volumeBindingMode"])
	}
	params, _ := doc["parameters"].(map[string]interface{})
	if params["kind"] == "" || params["kind"] == nil {
		t.Errorf("example StorageClass parameters: kind (capacity pool selector) missing: %v", params)
	}
}

// TestExampleConfigMapMatchesMountedName keeps the example ConfigMap
// and the DaemonSet mount talking about the same object: the example
// must create the ConfigMap the driver is configured to read.
func TestExampleConfigMapMatchesMountedName(t *testing.T) {
	var doc map[string]interface{}
	loadManifest(t, manifestPath(t, exampleCM), &doc)

	if doc["kind"] != "ConfigMap" {
		t.Fatalf("example is a %v, want ConfigMap", doc["kind"])
	}
	meta, _ := doc["metadata"].(map[string]interface{})
	if meta["name"].(string) != "csi-kata-directvol-capacity" {
		t.Errorf("example ConfigMap name: got %v, want csi-kata-directvol-capacity", meta["name"])
	}

	// The example names the nodes its quotas apply to; those keys are
	// node names by contract with LoadNodeCapacity, so none of them may
	// be something that cannot be a file name (see
	// pkg/utils.TestLoadNodeCapacity).
	data, _ := doc["data"].(map[string]interface{})
	if len(data) == 0 {
		t.Fatalf("example ConfigMap has no node entries")
	}
	for node := range data {
		_, content := data[node].(string)
		if !content {
			t.Errorf("entry for node %q is not a string", node)
		}
		if node != filepath.Base(node) || strings.HasPrefix(node, ".") || node == "." || node == ".." {
			t.Errorf("node key %q cannot be a capacity file name", node)
		}
	}
}
