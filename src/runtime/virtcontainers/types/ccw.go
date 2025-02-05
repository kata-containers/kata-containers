// Copyright 2025 IBM
//
// SPDX-License-Identifier: Apache-2.0
//

package types

import (
	"fmt"
	"strconv"
)

// CCW bus ID follow the format <xx>.<d>.<xxxx> [1, p. 11], where
//   - <xx> is the channel subsystem ID, which is always 0 from the guest side, but different from
//     the host side, e.g. 0xfe for virtio-*-ccw [1, p. 435],
//   - <d> is the subchannel set ID, which ranges from 0-3 [2], and
//   - <xxxx> is the device number (0000-ffff; leading zeroes can be omitted,
//     e.g. 3 instead of 0003).
//
// [1] https://www.ibm.com/docs/en/linuxonibm/pdf/lku4dd04.pdf
// [2] https://qemu.readthedocs.io/en/master/system/s390x/css.html
const subchannelSetMax = 3

type CcwDevice struct {
	ssid  uint8
	devno uint16
}

func CcwDeviceFrom(ssid int, devno string) (CcwDevice, error) {
	if ssid < 0 || ssid > subchannelSetMax {
		return CcwDevice{}, fmt.Errorf("Subchannel set ID %d should be in range [0..%d]", ssid, subchannelSetMax)
	}
	v, err := strconv.ParseUint(devno, 16, 16)
	if err != nil {
		return CcwDevice{}, fmt.Errorf("Failed to parse 0x%v as CCW device number: %v", devno, err)
	}
	return CcwDevice{ssid: uint8(ssid), devno: uint16(v)}, nil
}

func (dev CcwDevice) String() string {
	return fmt.Sprintf("0.%x.%04x", dev.ssid, dev.devno)
}
