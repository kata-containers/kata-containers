//
// Copyright 2017 The Kubernetes Authors.
// Copyright (c) 2023 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

package main

import (
	"flag"
	"kata-containers/csi-kata-directvolume/pkg/directvolume"
	"kata-containers/csi-kata-directvolume/pkg/spdkrpc"
	"kata-containers/csi-kata-directvolume/pkg/utils"
	"os"
	"path"
	"time"

	"k8s.io/klog/v2"
)

func init() {
	if err := flag.Set("logtostderr", "true"); err != nil {
		klog.Errorln("flag setting failed.")
	}
}

var (
	// Set by the build process
	version = ""
)

func main() {
	cfg := directvolume.Config{
		VendorVersion: version,
	}

	flag.StringVar(&cfg.Endpoint, "endpoint", "unix:///var/run/csi.sock", "CSI endpoint")
	flag.StringVar(&cfg.DriverName, "drivername", "directvolume.csi.katacontainers.io", "name of the driver")
	flag.StringVar(&cfg.StateDir, "statedir", "/csi-persist-data", "directory for storing state information across driver restarts, volumes ")
	flag.StringVar(&cfg.StoragePath, "storagepath", "", "storage path for storing the backend files on host")
	flag.StringVar(&cfg.NodeID, "nodeid", "", "node id")
	flag.Var(&cfg.Capacity, "capacity", "Simulate storage capacity. The parameter is <kind>=<quantity> where <kind> is the value of a 'kind' storage class parameter and <quantity> is the total amount of bytes for that kind. The flag may be used multiple times to configure different kinds.")
	flag.Int64Var(&cfg.MaxVolumeSize, "max-volume-size", 1024*1024*1024*1024, "maximum size of volumes in bytes (inclusive)")
	flag.BoolVar(&cfg.EnableTopology, "enable-topology", true, "Enables PluginCapability_Service_VOLUME_ACCESSIBILITY_CONSTRAINTS capability.")
	flag.StringVar(&cfg.CapacityConfigDir, "capacity-config-dir", "", "directory with per-node capacity pools. If it contains a file named after the node id, that file's pools override the ones from the --capacity flag (same pool names win per-node). The DaemonSet mounts the optional csi-kata-directvol-capacity ConfigMap there.")
	flag.DurationVar(&cfg.SpdkRPCTimeout, "spdk-rpc-timeout", 10*time.Second,
		"timeout for SPDK JSON-RPC requests")
	flag.StringVar(&cfg.SpdkRawPath, "spdk-rawpath", "", "path for spdk rawdisk backing files")
	flag.StringVar(&cfg.SpdkVhostPath, "spdk-vhostpath", "", "path for spdk vhost controller sockets")

	showVersion := flag.Bool("version", false, "Show version.")

	flag.Parse()

	if *showVersion {
		baseName := path.Base(os.Args[0])
		klog.Infof(baseName, version)
		return
	}

	// Per-node capacity pools (#14151): pools from the node's own
	// entry in the capacity-config-dir are merged over the --capacity
	// flag; same-named pools take the per-node value, so one DaemonSet
	// deployment can give asymmetric nodes different quotas, and a
	// node without an entry keeps the flag's pools. Configuration is
	// read once at startup; a ConfigMap change needs a daemonset
	// rollout restart to take effect.
	if cfg.CapacityConfigDir != "" {
		nodePools, found, err := utils.LoadNodeCapacity(cfg.CapacityConfigDir, cfg.NodeID)
		if err != nil {
			klog.Errorf("Failed to load per-node capacity configuration: %s", err.Error())
			os.Exit(1)
		}
		if found {
			if cfg.Capacity == nil {
				cfg.Capacity = utils.Capacity{}
			}
			for pool, quantity := range nodePools {
				cfg.Capacity[pool] = quantity
			}
			klog.Infof("loaded per-node capacity pools from %s: %s", cfg.CapacityConfigDir, cfg.Capacity.String())
		} else {
			klog.Infof("no per-node capacity entry for node %q in %s: volumes are bounded by --capacity pools and the free space of the backing filesystem", cfg.NodeID, cfg.CapacityConfigDir)
		}
	}

	spdkrpc.Init(cfg.SpdkRPCTimeout)

	driver, err := directvolume.NewDirectVolumeDriver(cfg)
	if err != nil {
		klog.Errorf("Failed to initialize driver: %s", err.Error())
		os.Exit(1)
	}

	if err := driver.Run(); err != nil {
		klog.Errorf("Failed to run driver: %s", err.Error())
		os.Exit(1)
	}
}
