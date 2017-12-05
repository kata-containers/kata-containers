//
// Copyright (c) 2017 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package main

import (
	"fmt"
	"io/ioutil"
	"net"
	"os"
	"path/filepath"
	"strings"

	"github.com/hashicorp/yamux"
	"github.com/mdlayher/vsock"
	"golang.org/x/sys/unix"
)

type channel interface {
	setup() error
	listen() (net.Listener, error)
	teardown() error
}

func newChannel() (channel, error) {
	// Check for vsock support.
	vSockSupported, err := isAFVSockSupported()
	if err != nil {
		return nil, err
	}

	if vSockSupported {
		// Check if vsock socket exists. We want to cover the case
		// where the guest OS can support vsock, but the runtime is
		// still using virtio serial connection.
		exist, err := vSockPathExist()
		if err != nil {
			return nil, err
		}

		if exist {
			return &vSockChannel{}, nil
		}
	}

	return &serialChannel{}, nil
}

type vSockChannel struct {
}

func (c *vSockChannel) setup() error {
	return nil
}

func (c *vSockChannel) listen() (net.Listener, error) {
	l, err := vsock.Listen(vSockPort)
	if err != nil {
		return nil, err
	}

	return l, nil
}

func (c *vSockChannel) teardown() error {
	return nil
}

type serialChannel struct {
	serialConn *os.File
}

func (c *serialChannel) setup() error {
	// Open serial channel.
	file, err := openSerialChannel()
	if err != nil {
		return err
	}

	c.serialConn = file

	return nil
}

func (c *serialChannel) listen() (net.Listener, error) {
	// Initialize Yamux server.
	session, err := yamux.Server(c.serialConn, nil)
	if err != nil {
		return nil, err
	}

	return session, nil
}

func (c *serialChannel) teardown() error {
	return c.serialConn.Close()
}

func isAFVSockSupported() (bool, error) {
	fd, err := unix.Socket(unix.AF_VSOCK, unix.SOCK_STREAM, 0)
	if err != nil {
		// This case is valid. It means AF_VSOCK is not a supported
		// domain on this system.
		if err == unix.EAFNOSUPPORT {
			return false, nil
		}

		return false, err
	}

	if err := unix.Close(fd); err != nil {
		return true, err
	}

	return true, nil
}

func vSockPathExist() (bool, error) {
	if _, err := os.Stat(vSockDevPath); err != nil {
		if os.IsNotExist(err) {
			return false, nil
		}

		return false, err
	}

	return true, nil
}

func findVirtualSerialPath(serialName string) (string, error) {
	dir, err := os.Open(virtIOPath)
	if err != nil {
		return "", err
	}

	defer dir.Close()

	ports, err := dir.Readdirnames(0)
	if err != nil {
		return "", err
	}

	for _, port := range ports {
		path := filepath.Join(virtIOPath, port, "name")
		content, err := ioutil.ReadFile(path)
		if err != nil {
			if os.IsNotExist(err) {
				agentLog.WithField("file", path).Debug("Skip parsing of non-existent file")
				continue
			}
			return "", err
		}

		if strings.Contains(string(content), serialName) == true {
			return filepath.Join(devRootPath, port), nil
		}
	}

	return "", fmt.Errorf("Could not find virtio port %s", serialName)
}

func openSerialChannel() (*os.File, error) {
	serialPath, err := findVirtualSerialPath(serialChannelName)
	if err != nil {
		return nil, err
	}

	file, err := os.OpenFile(serialPath, os.O_RDWR, os.ModeDevice)
	if err != nil {
		return nil, err
	}

	return file, nil
}
