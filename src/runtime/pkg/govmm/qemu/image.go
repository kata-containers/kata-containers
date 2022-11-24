// Copyright contributors to the Virtual Machine Manager for Go project
//
// SPDX-License-Identifier: Apache-2.0
//

package qemu

import (
	"context"
	"fmt"
	"os"
	"os/exec"
	"path"
	"syscall"
)

// CreateCloudInitISO creates a cloud-init ConfigDrive ISO image.  This is
// useful for configuring newly booted VMs. Before it can create the ISO
// image it needs to create a file tree with the various files that will
// make up the image.  This directory is created under scratchDir and is
// deleted when when the function returns, successfully or otherwise.  ctx is
// a context that can be used to timeout or cancel the image creation.
// isoPath contains the desired path of the ISO image to be created.  The
// userdata and metadata parameters are byte slices that contain the
// ConfigDrive userdata and metadata that will be stored with the ISO image.
// The attrs parameter can be used to control aspects of the newly created
// qemu process, such as the user and group under which it runs.  It may be nil.
func CreateCloudInitISO(ctx context.Context, scratchDir, isoPath string,
	userData, metaData []byte, attr *syscall.SysProcAttr) error {
	configDrivePath := path.Join(scratchDir, "clr-cloud-init")
	dataDirPath := path.Join(configDrivePath, "openstack", "latest")
	metaDataPath := path.Join(dataDirPath, "meta_data.json")
	userDataPath := path.Join(dataDirPath, "user_data")

	defer func() {
		/* #nosec */
		_ = os.RemoveAll(configDrivePath)
	}()

	err := os.MkdirAll(dataDirPath, 0750)
	if err != nil {
		return fmt.Errorf("unable to create config drive directory %s : %v",
			dataDirPath, err)
	}

	err = os.WriteFile(metaDataPath, metaData, 0644)
	if err != nil {
		return fmt.Errorf("unable to create %s : %v", metaDataPath, err)
	}

	err = os.WriteFile(userDataPath, userData, 0644)
	if err != nil {
		return fmt.Errorf("unable to create %s : %v", userDataPath, err)
	}

	cmd := exec.CommandContext(ctx, "xorriso", "-as", "mkisofs", "-R", "-V", "config-2",
		"-o", isoPath, configDrivePath)
	cmd.SysProcAttr = attr
	err = cmd.Run()
	if err != nil {
		return fmt.Errorf("unable to create cloudinit iso image %v", err)
	}

	return nil
}
