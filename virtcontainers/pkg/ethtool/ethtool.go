/*
 *
 * Licensed to the Apache Software Foundation (ASF) under one
 * or more contributor license agreements.  See the NOTICE file
 * distributed with this work for additional information
 * regarding copyright ownership.  The ASF licenses this file
 * to you under the Apache License, Version 2.0 (the
 * "License"); you may not use this file except in compliance
 * with the License.  You may obtain a copy of the License at
 *
 *  http://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing,
 * software distributed under the License is distributed on an
 * "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
 * KIND, either express or implied.  See the License for the
 * specific language governing permissions and limitations
 * under the License.
 *
 */

package ethtool

import (
	"bytes"
	"syscall"
	"unsafe"
)

// Maximum size of an interface name
const (
	IFNAMSIZ = 16
)

// ioctl ethtool request
const (
	SIOCETHTOOL = 0x8946
)

// ethtool stats related constants.
const (
	ethGstringLen   = 32
	ethtoolGDrvInfo = 0x00000003
)

// maxGtrings maximum number of stats entries that ethtool can
// retrieve currently.
const (
	maxGstrings = 1000
)

type ifreq struct {
	ifrName [IFNAMSIZ]byte
	ifrData uintptr
}

type ethtoolDrvInfo struct {
	cmd         uint32
	driver      [32]byte
	version     [32]byte
	fwVersion   [32]byte
	busInfo     [32]byte
	eromVersion [32]byte
	reserved2   [12]byte
	nPrivFlags  uint32
	nStats      uint32
	testinfoLen uint32
	eedumpLen   uint32
	regdumpLen  uint32
}

type ethtoolGStrings struct {
	cmd       uint32
	stringSet uint32
	len       uint32
	data      [maxGstrings * ethGstringLen]byte
}

type ethtoolStats struct {
	cmd    uint32
	nStats uint32
	data   [maxGstrings]uint64
}

// Ethtool file descriptor.
type Ethtool struct {
	fd int
}

// DriverName returns the driver name of the given interface.
func (e *Ethtool) DriverName(intf string) (string, error) {
	info, err := e.getDriverInfo(intf)
	if err != nil {
		return "", err
	}
	return string(bytes.Trim(info.driver[:], "\x00")), nil
}

// BusInfo returns the bus info of the given interface.
func (e *Ethtool) BusInfo(intf string) (string, error) {
	info, err := e.getDriverInfo(intf)
	if err != nil {
		return "", err
	}
	return string(bytes.Trim(info.busInfo[:], "\x00")), nil
}

func (e *Ethtool) getDriverInfo(intf string) (ethtoolDrvInfo, error) {
	drvinfo := ethtoolDrvInfo{
		cmd: ethtoolGDrvInfo,
	}

	var name [IFNAMSIZ]byte
	copy(name[:], []byte(intf))

	ifr := ifreq{
		ifrName: name,
		ifrData: uintptr(unsafe.Pointer(&drvinfo)),
	}

	_, _, ep := syscall.Syscall(syscall.SYS_IOCTL, uintptr(e.fd), SIOCETHTOOL, uintptr(unsafe.Pointer(&ifr)))
	if ep != 0 {
		return ethtoolDrvInfo{}, syscall.Errno(ep)
	}

	return drvinfo, nil
}

// Close closes the ethtool file descriptor.
func (e *Ethtool) Close() {
	syscall.Close(e.fd)
}

// NewEthtool opens a ethtool socket.
func NewEthtool() (*Ethtool, error) {
	fd, _, err := syscall.RawSyscall(syscall.SYS_SOCKET, syscall.AF_INET, syscall.SOCK_DGRAM, syscall.IPPROTO_IP)
	if err != 0 {
		return nil, syscall.Errno(err)
	}

	return &Ethtool{
		fd: int(fd),
	}, nil
}

// BusInfo returns the bus information of the network device.
func BusInfo(intf string) (string, error) {
	e, err := NewEthtool()
	if err != nil {
		return "", err
	}
	defer e.Close()
	return e.BusInfo(intf)
}

// DriverName returns the driver name of the network interface.
func DriverName(intf string) (string, error) {
	e, err := NewEthtool()
	if err != nil {
		return "", err
	}
	defer e.Close()
	return e.DriverName(intf)
}
