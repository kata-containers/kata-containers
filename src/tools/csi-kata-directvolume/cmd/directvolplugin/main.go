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
	"os"
	"path"

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

	showVersion := flag.Bool("version", false, "Show version.")

	flag.Parse()

	if *showVersion {
		baseName := path.Base(os.Args[0])
		klog.Infof(baseName, version)
		return
	}

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
