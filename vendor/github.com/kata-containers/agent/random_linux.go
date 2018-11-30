//
// Copyright (c) 2018 HyperHQ.Inc
//
// SPDX-License-Identifier: Apache-2.0
//

package main

import (
	"fmt"
	"io"
	"os"
	"syscall"
	"unsafe"
)

const (
	rngDev = "/dev/random"

	// include/uapi/linux/random.h
	// RNDADDTOENTCNT _IOW( 'R', 0x01, int )
	// RNDRESEEDCRNG   _IO( 'R', 0x07 )
	iocRNDADDTOENTCNT = 0x40045201
	iocRNDRESEEDCRNG  = 0x5207
)

func reseedRNG(data []byte) error {
	if len(data) == 0 {
		return fmt.Errorf("missing entropy data")
	}

	// Write entropy
	f, err := os.OpenFile(rngDev, os.O_WRONLY, 0)
	if err != nil {
		agentLog.WithError(err).Warn("Could not open rng device")
		return err
	}
	defer f.Close()
	n, err := f.Write(data)
	if err != nil {
		agentLog.WithError(err).Warn("Could not write to rng device")
		return err
	}
	if n < len(data) {
		agentLog.WithError(io.ErrShortWrite).Warn("Short write to rng device")
		return io.ErrShortWrite
	}

	// Add data to the entropy count
	_, _, errNo := syscall.Syscall(syscall.SYS_IOCTL, f.Fd(), iocRNDADDTOENTCNT, uintptr(unsafe.Pointer(&n)))
	if errNo != 0 {
		agentLog.WithError(errNo).Warn("Could not add to rng entropy count, ignoring")
	}

	// Newer kernel supports RNDRESEEDCRNG ioctl to actively kick-off reseed.
	// Let's make use of it if possible.
	_, _, errNo = syscall.Syscall(syscall.SYS_IOCTL, f.Fd(), iocRNDRESEEDCRNG, 0)
	if errNo != 0 {
		agentLog.WithError(errNo).Warn("Could not reseed rng, ignoring")
	}

	return nil
}
