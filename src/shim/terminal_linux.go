// +build linux
//
// Copyright (c) 2017 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package main

import (
	"golang.org/x/sys/unix"
)

const (
	termiosIFlagRawTermInvMask = (unix.IGNBRK | unix.BRKINT | unix.PARMRK | unix.ISTRIP | unix.INLCR | unix.IGNCR | unix.ICRNL | unix.IXON)
	termiosOFlagRawTermInvMask = unix.OPOST
	termiosLFlagRawTermInvMask = (unix.ECHO | unix.ECHONL | unix.ICANON | unix.ISIG | unix.IEXTEN)
	termiosCFlagRawTermInvMask = unix.PARENB
	termiosCFlagRawTermMask    = unix.CS8
	termiosCcVMinRawTermVal    = 1
	termiosCcVTimeRawTermVal   = 0
)

func setupTerminal(fd int) (*unix.Termios, error) {
	termios, err := unix.IoctlGetTermios(fd, unix.TCGETS)
	if err != nil {
		return nil, err
	}

	savedTermios := *termios

	// Set the terminal in raw mode
	termios.Iflag &^= termiosIFlagRawTermInvMask
	termios.Oflag &^= termiosOFlagRawTermInvMask
	termios.Lflag &^= termiosLFlagRawTermInvMask
	termios.Cflag &^= termiosCFlagRawTermInvMask
	termios.Cflag |= termiosCFlagRawTermMask
	termios.Cc[unix.VMIN] = termiosCcVMinRawTermVal
	termios.Cc[unix.VTIME] = termiosCcVTimeRawTermVal

	if err := unix.IoctlSetTermios(fd, unix.TCSETS, termios); err != nil {
		return nil, err
	}

	return &savedTermios, nil
}

func restoreTerminal(fd int, termios *unix.Termios) error {
	return unix.IoctlSetTermios(fd, unix.TCSETS, termios)
}
