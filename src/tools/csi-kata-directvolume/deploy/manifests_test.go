// Copyright (c) 2026 Kata Contributors
//
// SPDX-License-Identifier: Apache-2.0
//

// Package deploy holds tests which check that the deployment manifests and
// scripts publish storage capacity consistently.
package deploy

import (
	"bytes"
	"errors"
	"io"
	"os"
	"os/exec"
	"path/filepath"
	"strings"
	"testing"

	"github.com/stretchr/testify/require"
	"gopkg.in/yaml.v3"

	"kata-containers/csi-kata-directvolume/pkg/utils"
)

const (
	driverName        = "directvolume.csi.katacontainers.io"
	namespace         = "kata-directvolume"
	capacityConfigMap = "csi-kata-directvol-capacity"
	// serviceAccount is the account which rbac-deploy.sh binds the
	// external-provisioner RBAC rules to.
	serviceAccount = "csi-provisioner"

	driverInfoManifest = "kata-directvolume/csi-directvol-driverinfo.yaml"
	pluginManifest     = "kata-directvolume/csi-directvol-plugin.yaml"
	capacityExampleDir = "../examples/pod-with-capacity"
)

type objectMeta struct {
	Name      string `yaml:"name"`
	Namespace string `yaml:"namespace"`
}

type csiDriver struct {
	Kind     string     `yaml:"kind"`
	Metadata objectMeta `yaml:"metadata"`
	Spec     struct {
		AttachRequired  *bool `yaml:"attachRequired"`
		StorageCapacity *bool `yaml:"storageCapacity"`
	} `yaml:"spec"`
}

type envVar struct {
	Name      string `yaml:"name"`
	ValueFrom *struct {
		FieldRef *struct {
			FieldPath string `yaml:"fieldPath"`
		} `yaml:"fieldRef"`
	} `yaml:"valueFrom"`
}

type volumeMount struct {
	Name      string `yaml:"name"`
	MountPath string `yaml:"mountPath"`
	ReadOnly  bool   `yaml:"readOnly"`
}

type container struct {
	Name         string        `yaml:"name"`
	Image        string        `yaml:"image"`
	Args         []string      `yaml:"args"`
	Env          []envVar      `yaml:"env"`
	VolumeMounts []volumeMount `yaml:"volumeMounts"`
}

type volume struct {
	Name      string `yaml:"name"`
	ConfigMap *struct {
		Name     string `yaml:"name"`
		Optional *bool  `yaml:"optional"`
	} `yaml:"configMap"`
}

type daemonSet struct {
	Kind     string     `yaml:"kind"`
	Metadata objectMeta `yaml:"metadata"`
	Spec     struct {
		Template struct {
			Spec struct {
				ServiceAccountName string      `yaml:"serviceAccountName"`
				Containers         []container `yaml:"containers"`
				Volumes            []volume    `yaml:"volumes"`
			} `yaml:"spec"`
		} `yaml:"template"`
	} `yaml:"spec"`
}

type storageClass struct {
	Kind              string            `yaml:"kind"`
	Metadata          objectMeta        `yaml:"metadata"`
	Provisioner       string            `yaml:"provisioner"`
	Parameters        map[string]string `yaml:"parameters"`
	VolumeBindingMode string            `yaml:"volumeBindingMode"`
}

type configMap struct {
	Kind     string            `yaml:"kind"`
	Metadata objectMeta        `yaml:"metadata"`
	Data     map[string]string `yaml:"data"`
}

func decodeFile(t *testing.T, path string, out any) {
	t.Helper()
	content, err := os.ReadFile(path)
	require.NoError(t, err)
	require.NoError(t, yaml.Unmarshal(content, out), "decode %s", path)
}

// decodeDocuments decodes every YAML document of a stream as a map.
func decodeDocuments(t *testing.T, stream []byte) []map[string]any {
	t.Helper()
	var docs []map[string]any
	decoder := yaml.NewDecoder(bytes.NewReader(stream))
	for {
		var doc map[string]any
		err := decoder.Decode(&doc)
		if errors.Is(err, io.EOF) {
			return docs
		}
		require.NoError(t, err)
		if doc != nil {
			docs = append(docs, doc)
		}
	}
}

func (ds *daemonSet) container(t *testing.T, name string) container {
	t.Helper()
	for _, c := range ds.Spec.Template.Spec.Containers {
		if c.Name == name {
			return c
		}
	}
	require.Failf(t, "missing container", "the DaemonSet has no %q container", name)
	return container{}
}

// flagValue returns the value of a --name=value argument.
func (c container) flagValue(name string) (string, bool) {
	for _, arg := range c.Args {
		arg = strings.TrimLeft(arg, "-")
		if arg == name {
			return "", true
		}
		if value, found := strings.CutPrefix(arg, name+"="); found {
			return value, true
		}
	}
	return "", false
}

func (c container) requireFieldRef(t *testing.T, env, fieldPath string) {
	t.Helper()
	for _, e := range c.Env {
		if e.Name == env {
			require.NotNil(t, e.ValueFrom, "%s of container %s", env, c.Name)
			require.NotNil(t, e.ValueFrom.FieldRef, "%s of container %s", env, c.Name)
			require.Equal(t, fieldPath, e.ValueFrom.FieldRef.FieldPath, "%s of container %s", env, c.Name)
			return
		}
	}
	require.Failf(t, "missing environment variable", "container %s has no %s", c.Name, env)
}

func TestCSIDriverPublishesCapacity(t *testing.T) {
	var driver csiDriver
	decodeFile(t, driverInfoManifest, &driver)

	require.Equal(t, "CSIDriver", driver.Kind)
	require.Equal(t, driverName, driver.Metadata.Name)
	require.NotNil(t, driver.Spec.StorageCapacity)
	require.True(t, *driver.Spec.StorageCapacity, "the scheduler must use the published capacity")
	require.NotNil(t, driver.Spec.AttachRequired)
	require.False(t, *driver.Spec.AttachRequired)
}

func TestPluginPublishesCapacity(t *testing.T) {
	var ds daemonSet
	decodeFile(t, pluginManifest, &ds)

	// --capacity-ownerref-level=1 makes the owner of the pod, which must be
	// this DaemonSet, the owner of the CSIStorageCapacity objects.
	require.Equal(t, "DaemonSet", ds.Kind)
	require.Equal(t, namespace, ds.Metadata.Namespace)
	require.Equal(t, serviceAccount, ds.Spec.Template.Spec.ServiceAccountName)

	provisioner := ds.container(t, "csi-provisioner")
	for flag, expected := range map[string]string{
		"enable-capacity":         "",
		"capacity-ownerref-level": "1",
		"node-deployment":         "true",
		"strict-topology":         "true",
	} {
		value, ok := provisioner.flagValue(flag)
		require.True(t, ok, "csi-provisioner needs --%s", flag)
		require.Equal(t, expected, value, "csi-provisioner --%s", flag)
	}
	_, ok := provisioner.flagValue("capacity-poll-interval")
	require.True(t, ok, "csi-provisioner needs --capacity-poll-interval, directvol-deploy.sh overrides it")
	// Needed by the node deployment and to look up the owner of the pod.
	provisioner.requireFieldRef(t, "NODE_NAME", "spec.nodeName")
	provisioner.requireFieldRef(t, "NAMESPACE", "metadata.namespace")
	provisioner.requireFieldRef(t, "POD_NAME", "metadata.name")

	driver := ds.container(t, "kata-directvolume")
	name, _ := driver.flagValue("drivername")
	require.Equal(t, driverName, name)
	// The capacity file and the topology segment are found by node name.
	nodeID, _ := driver.flagValue("nodeid")
	require.Equal(t, "$(KUBE_NODE_NAME)", nodeID)
	driver.requireFieldRef(t, "KUBE_NODE_NAME", "spec.nodeName")
	if topology, set := driver.flagValue("enable-topology"); set {
		require.Equal(t, "true", topology, "per-node capacity needs the node topology")
	}

	configDir, ok := driver.flagValue("capacity-config-dir")
	require.True(t, ok, "the driver needs --capacity-config-dir")
	var mount *volumeMount
	for i := range driver.VolumeMounts {
		if driver.VolumeMounts[i].MountPath == configDir {
			mount = &driver.VolumeMounts[i]
		}
	}
	require.NotNil(t, mount, "--capacity-config-dir %s must be a mounted volume", configDir)
	require.True(t, mount.ReadOnly)

	var configVolume *volume
	for i := range ds.Spec.Template.Spec.Volumes {
		if ds.Spec.Template.Spec.Volumes[i].Name == mount.Name {
			configVolume = &ds.Spec.Template.Spec.Volumes[i]
		}
	}
	require.NotNil(t, configVolume)
	require.NotNil(t, configVolume.ConfigMap, "the capacity configuration comes from a ConfigMap")
	require.Equal(t, capacityConfigMap, configVolume.ConfigMap.Name)
	require.NotNil(t, configVolume.ConfigMap.Optional)
	require.True(t, *configVolume.ConfigMap.Optional, "the driver must start without capacity configuration")
}

func TestRBACMatchesProvisioner(t *testing.T) {
	var ds daemonSet
	decodeFile(t, pluginManifest, &ds)
	image := ds.container(t, "csi-provisioner").Image
	require.Contains(t, image, "/csi-provisioner:")

	// rbac-deploy.sh downloads the RBAC rules of the external-provisioner
	// release used by the DaemonSet, which grant the csi-provisioner
	// account access to CSIStorageCapacity objects and to its own pod.
	script, err := os.ReadFile("rbac-deploy.sh")
	require.NoError(t, err)
	require.Contains(t, string(script), `rbac_version "${BASE_DIR}/kata-directvolume/csi-directvol-plugin.yaml" csi-provisioner`)
	require.Contains(t, string(script), "external-provisioner/")
}

func TestCapacityExample(t *testing.T) {
	var sc storageClass
	decodeFile(t, filepath.Join(capacityExampleDir, "csi-storageclass.yaml"), &sc)
	require.Equal(t, "StorageClass", sc.Kind)
	require.Equal(t, driverName, sc.Provisioner)
	require.Equal(t, "WaitForFirstConsumer", sc.VolumeBindingMode)
	require.Equal(t, utils.DirectVolumeTypeName, sc.Parameters[utils.KataContainersDirectVolumeType])
	pool := sc.Parameters[utils.KataContainersDirectCapacityPool]
	require.NoError(t, utils.ValidateCapacityPoolName(pool))

	var cm configMap
	decodeFile(t, filepath.Join(capacityExampleDir, "csi-capacity-configmap.yaml"), &cm)
	require.Equal(t, "ConfigMap", cm.Kind)
	require.Equal(t, capacityConfigMap, cm.Metadata.Name)
	require.Equal(t, namespace, cm.Metadata.Namespace)
	require.NotEmpty(t, cm.Data)
	for node, content := range cm.Data {
		capacity, err := utils.ParseCapacityConfig(content)
		require.NoError(t, err, "node %s", node)
		_, ok := capacity[pool]
		require.True(t, ok, "node %s has no %q pool", node, pool)
	}
}

// fakeKubectl is a kubectl replacement which records the manifests that
// directvol-deploy.sh applies.
const fakeKubectl = `#!/usr/bin/env bash
case "$1 $2" in
"get csistoragecapacities.v1.storage.k8s.io") exit "${FAKE_CAPACITY_API_RC}" ;;
"apply -f") { cat; echo "---"; } >>"${FAKE_APPLIED}" ;;
"get -n") echo 1 ;;
esac
exit 0
`

func runDeployScript(t *testing.T, env ...string) []map[string]any {
	t.Helper()
	if _, err := exec.LookPath("bash"); err != nil {
		t.Skip("bash is not installed")
	}

	bin := t.TempDir()
	require.NoError(t, os.WriteFile(filepath.Join(bin, "kubectl"), []byte(fakeKubectl), 0700))
	applied := filepath.Join(bin, "applied.yaml")

	cmd := exec.Command("bash", "directvol-deploy.sh")
	cmd.Env = append(os.Environ(),
		"PATH="+bin+string(os.PathListSeparator)+os.Getenv("PATH"),
		"FAKE_APPLIED="+applied,
		"FAKE_CAPACITY_API_RC=0",
	)
	cmd.Env = append(cmd.Env, env...)
	out, err := cmd.CombinedOutput()
	require.NoError(t, err, "directvol-deploy.sh failed:\n%s", out)

	content, err := os.ReadFile(applied)
	require.NoError(t, err)
	return decodeDocuments(t, content)
}

func lookup(doc map[string]any, path ...string) any {
	var value any = doc
	for _, key := range path {
		m, ok := value.(map[string]any)
		if !ok {
			return nil
		}
		value = m[key]
	}
	return value
}

func appliedArgs(t *testing.T, docs []map[string]any, containerName string) []string {
	t.Helper()
	for _, doc := range docs {
		if doc["kind"] != "DaemonSet" {
			continue
		}
		containers, _ := lookup(doc, "spec", "template", "spec", "containers").([]any)
		for _, c := range containers {
			m, _ := c.(map[string]any)
			if m["name"] != containerName {
				continue
			}
			var args []string
			list, _ := m["args"].([]any)
			for _, arg := range list {
				args = append(args, arg.(string))
			}
			return args
		}
	}
	require.Failf(t, "missing container", "no %s container was applied", containerName)
	return nil
}

func appliedStorageCapacity(t *testing.T, docs []map[string]any) any {
	t.Helper()
	for _, doc := range docs {
		if doc["kind"] == "CSIDriver" {
			return lookup(doc, "spec", "storageCapacity")
		}
	}
	require.Fail(t, "no CSIDriver was applied")
	return nil
}

func TestDeployScriptCapacityTracking(t *testing.T) {
	capacityFlags := []string{"--enable-capacity", "--capacity-ownerref-level=1", "--capacity-poll-interval=1m"}

	testCases := []struct {
		name        string
		env         []string
		enabled     bool
		pollFlag    string
		driverImage string
	}{
		{name: "API available", enabled: true},
		{name: "API missing", env: []string{"FAKE_CAPACITY_API_RC=1"}},
		{name: "forced off", env: []string{"CAPACITY_TRACKING=false"}},
		{name: "forced on", env: []string{"CAPACITY_TRACKING=true", "FAKE_CAPACITY_API_RC=1"}, enabled: true},
		{
			name:     "poll interval",
			env:      []string{"CAPACITY_POLL_INTERVAL=10s"},
			enabled:  true,
			pollFlag: "--capacity-poll-interval=10s",
		},
		{
			name:        "driver image",
			env:         []string{"KATA_DIRECTVOLUME_REGISTRY=registry.example.com/kata", "KATA_DIRECTVOLUME_TAG=e2e"},
			enabled:     true,
			driverImage: "registry.example.com/kata/kata-directvolume:e2e",
		},
	}

	for _, tc := range testCases {
		t.Run(tc.name, func(t *testing.T) {
			docs := runDeployScript(t, tc.env...)
			provisionerArgs := appliedArgs(t, docs, "csi-provisioner")
			driverArgs := appliedArgs(t, docs, "kata-directvolume")
			// The driver reads its pools in any case.
			require.Contains(t, driverArgs, "--capacity-config-dir=/etc/csi-kata-directvolume/capacity")

			if !tc.enabled {
				require.Nil(t, appliedStorageCapacity(t, docs))
				for _, flag := range capacityFlags {
					require.NotContains(t, provisionerArgs, flag)
				}
				return
			}

			require.Equal(t, true, appliedStorageCapacity(t, docs))
			expected := capacityFlags
			if tc.pollFlag != "" {
				expected = []string{"--enable-capacity", "--capacity-ownerref-level=1", tc.pollFlag}
			}
			for _, flag := range expected {
				require.Contains(t, provisionerArgs, flag)
			}
			if tc.driverImage != "" {
				var image any
				for _, doc := range docs {
					containers, _ := lookup(doc, "spec", "template", "spec", "containers").([]any)
					for _, c := range containers {
						if m, _ := c.(map[string]any); m["name"] == "kata-directvolume" {
							image = m["image"]
						}
					}
				}
				require.Equal(t, tc.driverImage, image)
			}
		})
	}
}

func TestDeployScriptRejectsInvalidSetting(t *testing.T) {
	if _, err := exec.LookPath("bash"); err != nil {
		t.Skip("bash is not installed")
	}
	// The setting is checked before kubectl is used.
	cmd := exec.Command("bash", "directvol-deploy.sh")
	cmd.Env = append(os.Environ(), "CAPACITY_TRACKING=maybe")
	out, err := cmd.CombinedOutput()
	require.Error(t, err)
	require.Contains(t, string(out), "CAPACITY_TRACKING must be auto, true or false")
}
