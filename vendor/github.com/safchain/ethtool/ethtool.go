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

// Package ethtool  aims to provide a library giving a simple access to the
// Linux SIOCETHTOOL ioctl operations. It can be used to retrieve informations
// from a network device like statistics, driver related informations or
// even the peer of a VETH interface.
package ethtool

import (
	"bytes"
	"encoding/hex"
	"fmt"
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
	ETH_GSTRING_LEN  = 32
	ETH_SS_STATS     = 1
	ETHTOOL_GDRVINFO = 0x00000003
	ETHTOOL_GSTRINGS = 0x0000001b
	ETHTOOL_GSTATS   = 0x0000001d
	// other CMDs from ethtool-copy.h of ethtool-3.5 package
	ETHTOOL_GSET          = 0x00000001 /* Get settings. */
	ETHTOOL_SSET          = 0x00000002 /* Set settings. */
	ETHTOOL_GMSGLVL       = 0x00000007 /* Get driver message level */
	ETHTOOL_SMSGLVL       = 0x00000008 /* Set driver msg level. */
	ETHTOOL_GMODULEINFO   = 0x00000042 /* Get plug-in module information */
	ETHTOOL_GMODULEEEPROM = 0x00000043 /* Get plug-in module eeprom */
	ETHTOOL_GPERMADDR     = 0x00000020
)

// MAX_GSTRINGS maximum number of stats entries that ethtool can
// retrieve currently.
const (
	MAX_GSTRINGS = 1000
	EEPROM_LEN   = 640
	PERMADDR_LEN = 32
)

type ifreq struct {
	ifr_name [IFNAMSIZ]byte
	ifr_data uintptr
}

type ethtoolDrvInfo struct {
	cmd          uint32
	driver       [32]byte
	version      [32]byte
	fw_version   [32]byte
	bus_info     [32]byte
	erom_version [32]byte
	reserved2    [12]byte
	n_priv_flags uint32
	n_stats      uint32
	testinfo_len uint32
	eedump_len   uint32
	regdump_len  uint32
}

type ethtoolGStrings struct {
	cmd        uint32
	string_set uint32
	len        uint32
	data       [MAX_GSTRINGS * ETH_GSTRING_LEN]byte
}

type ethtoolStats struct {
	cmd     uint32
	n_stats uint32
	data    [MAX_GSTRINGS]uint64
}

type ethtoolEeprom struct {
	cmd    uint32
	magic  uint32
	offset uint32
	len    uint32
	data   [EEPROM_LEN]byte
}

type ethtoolModInfo struct {
	cmd        uint32
	tpe        uint32
	eeprom_len uint32
	reserved   [8]uint32
}

type ethtoolPermAddr struct {
	cmd  uint32
	size uint32
	data [PERMADDR_LEN]byte
}

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
	return string(bytes.Trim(info.bus_info[:], "\x00")), nil
}

func (e *Ethtool) ModuleEeprom(intf string) ([]byte, error) {
	eeprom, _, err := e.getModuleEeprom(intf)
	if err != nil {
		return nil, err
	}

	return eeprom.data[:eeprom.len], nil
}

func (e *Ethtool) ModuleEepromHex(intf string) (string, error) {
	eeprom, _, err := e.getModuleEeprom(intf)
	if err != nil {
		return "", err
	}

	return hex.EncodeToString(eeprom.data[:eeprom.len]), nil
}

func (e *Ethtool) DriverInfo(intf string) (ethtoolDrvInfo, error) {
	drvInfo, err := e.getDriverInfo(intf)
	if err != nil {
		return ethtoolDrvInfo{}, err
	}

	return drvInfo, nil
}

func (e *Ethtool) PermAddr(intf string) (string, error) {
	permAddr, err := e.getPermAddr(intf)
	if err != nil {
		return "", err
	}

	if permAddr.data[0] == 0 && permAddr.data[1] == 0 &&
		permAddr.data[2] == 0 && permAddr.data[3] == 0 &&
		permAddr.data[4] == 0 && permAddr.data[5] == 0 {
		return "", nil
	}

	return fmt.Sprintf("%x:%x:%x:%x:%x:%x",
		permAddr.data[0:1],
		permAddr.data[1:2],
		permAddr.data[2:3],
		permAddr.data[3:4],
		permAddr.data[4:5],
		permAddr.data[5:6],
	), nil
}

func (e *Ethtool) getDriverInfo(intf string) (ethtoolDrvInfo, error) {
	drvinfo := ethtoolDrvInfo{
		cmd: ETHTOOL_GDRVINFO,
	}

	var name [IFNAMSIZ]byte
	copy(name[:], []byte(intf))

	ifr := ifreq{
		ifr_name: name,
		ifr_data: uintptr(unsafe.Pointer(&drvinfo)),
	}

	_, _, ep := syscall.Syscall(syscall.SYS_IOCTL, uintptr(e.fd), SIOCETHTOOL, uintptr(unsafe.Pointer(&ifr)))
	if ep != 0 {
		return ethtoolDrvInfo{}, syscall.Errno(ep)
	}

	return drvinfo, nil
}

func (e *Ethtool) getPermAddr(intf string) (ethtoolPermAddr, error) {
	permAddr := ethtoolPermAddr{
		cmd:  ETHTOOL_GPERMADDR,
		size: PERMADDR_LEN,
	}

	var name [IFNAMSIZ]byte
	copy(name[:], []byte(intf))

	ifr := ifreq{
		ifr_name: name,
		ifr_data: uintptr(unsafe.Pointer(&permAddr)),
	}

	_, _, ep := syscall.Syscall(syscall.SYS_IOCTL, uintptr(e.fd), SIOCETHTOOL, uintptr(unsafe.Pointer(&ifr)))
	if ep != 0 {
		return ethtoolPermAddr{}, syscall.Errno(ep)
	}

	return permAddr, nil
}

func (e *Ethtool) getModuleEeprom(intf string) (ethtoolEeprom, ethtoolModInfo, error) {
	modInfo := ethtoolModInfo{
		cmd: ETHTOOL_GMODULEINFO,
	}

	var name [IFNAMSIZ]byte
	copy(name[:], []byte(intf))

	ifr := ifreq{
		ifr_name: name,
		ifr_data: uintptr(unsafe.Pointer(&modInfo)),
	}

	_, _, ep := syscall.Syscall(syscall.SYS_IOCTL, uintptr(e.fd), SIOCETHTOOL, uintptr(unsafe.Pointer(&ifr)))
	if ep != 0 {
		return ethtoolEeprom{}, ethtoolModInfo{}, syscall.Errno(ep)
	}

	eeprom := ethtoolEeprom{
		cmd:    ETHTOOL_GMODULEEEPROM,
		len:    modInfo.eeprom_len,
		offset: 0,
	}

	if modInfo.eeprom_len > EEPROM_LEN {
		return ethtoolEeprom{}, ethtoolModInfo{}, fmt.Errorf("eeprom size: %d is larger than buffer size: %d", modInfo.eeprom_len, EEPROM_LEN)
	}

	ifr.ifr_data = uintptr(unsafe.Pointer(&eeprom))

	_, _, ep = syscall.Syscall(syscall.SYS_IOCTL, uintptr(e.fd), SIOCETHTOOL, uintptr(unsafe.Pointer(&ifr)))
	if ep != 0 {
		return ethtoolEeprom{}, ethtoolModInfo{}, syscall.Errno(ep)
	}

	return eeprom, modInfo, nil
}

// Stats retrieves stats of the given interface name.
func (e *Ethtool) Stats(intf string) (map[string]uint64, error) {
	drvinfo := ethtoolDrvInfo{
		cmd: ETHTOOL_GDRVINFO,
	}

	var name [IFNAMSIZ]byte
	copy(name[:], []byte(intf))

	ifr := ifreq{
		ifr_name: name,
		ifr_data: uintptr(unsafe.Pointer(&drvinfo)),
	}

	_, _, ep := syscall.Syscall(syscall.SYS_IOCTL, uintptr(e.fd), SIOCETHTOOL, uintptr(unsafe.Pointer(&ifr)))
	if ep != 0 {
		return nil, syscall.Errno(ep)
	}

	if drvinfo.n_stats*ETH_GSTRING_LEN > MAX_GSTRINGS*ETH_GSTRING_LEN {
		return nil, fmt.Errorf("ethtool currently doesn't support more than %d entries, received %d", MAX_GSTRINGS, drvinfo.n_stats)
	}

	gstrings := ethtoolGStrings{
		cmd:        ETHTOOL_GSTRINGS,
		string_set: ETH_SS_STATS,
		len:        drvinfo.n_stats,
		data:       [MAX_GSTRINGS * ETH_GSTRING_LEN]byte{},
	}
	ifr.ifr_data = uintptr(unsafe.Pointer(&gstrings))

	_, _, ep = syscall.Syscall(syscall.SYS_IOCTL, uintptr(e.fd), SIOCETHTOOL, uintptr(unsafe.Pointer(&ifr)))
	if ep != 0 {
		return nil, syscall.Errno(ep)
	}

	stats := ethtoolStats{
		cmd:     ETHTOOL_GSTATS,
		n_stats: drvinfo.n_stats,
		data:    [MAX_GSTRINGS]uint64{},
	}

	ifr.ifr_data = uintptr(unsafe.Pointer(&stats))

	_, _, ep = syscall.Syscall(syscall.SYS_IOCTL, uintptr(e.fd), SIOCETHTOOL, uintptr(unsafe.Pointer(&ifr)))
	if ep != 0 {
		return nil, syscall.Errno(ep)
	}

	var result = make(map[string]uint64)
	for i := 0; i != int(drvinfo.n_stats); i++ {
		b := gstrings.data[i*ETH_GSTRING_LEN : i*ETH_GSTRING_LEN+ETH_GSTRING_LEN]
		key := string(bytes.Trim(b, "\x00"))
		result[key] = stats.data[i]
	}

	return result, nil
}

func (e *Ethtool) Close() {
	syscall.Close(e.fd)
}

func NewEthtool() (*Ethtool, error) {
	fd, err := syscall.Socket(syscall.AF_INET, syscall.SOCK_DGRAM, syscall.IPPROTO_IP)
	if err != nil {
		return nil, err
	}

	return &Ethtool{
		fd: int(fd),
	}, nil
}

func BusInfo(intf string) (string, error) {
	e, err := NewEthtool()
	if err != nil {
		return "", err
	}
	defer e.Close()
	return e.BusInfo(intf)
}

func DriverName(intf string) (string, error) {
	e, err := NewEthtool()
	if err != nil {
		return "", err
	}
	defer e.Close()
	return e.DriverName(intf)
}

func Stats(intf string) (map[string]uint64, error) {
	e, err := NewEthtool()
	if err != nil {
		return nil, err
	}
	defer e.Close()
	return e.Stats(intf)
}

func PermAddr(intf string) (string, error) {
	e, err := NewEthtool()
	if err != nil {
		return "", err
	}
	defer e.Close()
	return e.PermAddr(intf)
}
