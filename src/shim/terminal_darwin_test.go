// Copyright (c) 2017 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package main

import (
	"io/ioutil"
	"os"
	"syscall"
	"testing"

	"github.com/stretchr/testify/assert"
	"golang.org/x/sys/unix"
)

const (
	RawModeErr      = "should be properly set in raw mode"
	FlagRawModeErr  = "flag " + RawModeErr
	ValueRawModeErr = "value " + RawModeErr
)

func newTestTerminal(t *testing.T) (*os.File, error) {
	if os.Getuid() != 0 {
		t.Skip("Skipping this test: Requires to be root")
		return nil, nil
	}

	return os.OpenFile("/dev/tty", os.O_RDWR, os.ModeDevice)
}

func TestSetupTerminalOnNonTerminalFailure(t *testing.T) {
	file, err := ioutil.TempFile("", "tmp")
	assert.Nil(t, err, "Failed to create temporary file")
	defer file.Close()

	_, err = setupTerminal(int(file.Fd()))
	assert.NotNil(t, err, "Should fail because the file is not a terminal")
}

func TestSetupTerminalSuccess(t *testing.T) {
	file, err := newTestTerminal(t)

	if perr, ok := err.(*os.PathError); ok {
		switch perr.Err.(syscall.Errno) {
		case syscall.ENXIO:
			t.Skip("Skipping this test: Failed to open tty, make sure test is running in a tty")
		default:
			t.Fatalf("could not open tty %s", err)
		}
	}

	assert.Nil(t, err, "Failed to create terminal")
	defer file.Close()

	savedTermios, err := setupTerminal(int(file.Fd()))
	assert.Nil(t, err, "Should not fail because the file is a terminal")

	termios, err := unix.IoctlGetTermios(int(file.Fd()), unix.TIOCGETA)
	assert.Nil(t, err, "Failed to get terminal information")
	assert.True(t, (termios.Iflag&termiosIFlagRawTermInvMask) == 0, "Termios I %s", FlagRawModeErr)
	assert.True(t, (termios.Oflag&termiosOFlagRawTermInvMask) == 0, "Termios O %s", FlagRawModeErr)
	assert.True(t, (termios.Lflag&termiosLFlagRawTermInvMask) == 0, "Termios L %s", FlagRawModeErr)
	assert.True(t, (termios.Cflag&termiosCFlagRawTermInvMask) == 0, "Termios C %s", FlagRawModeErr)
	assert.True(t, (termios.Cflag&termiosCFlagRawTermMask) == termiosCFlagRawTermMask, "Termios C %s", FlagRawModeErr)
	assert.True(t, termios.Cc[unix.VMIN] == termiosCcVMinRawTermVal, "Termios CC VMIN %s", ValueRawModeErr)
	assert.True(t, termios.Cc[unix.VTIME] == termiosCcVTimeRawTermVal, "Termios CC VTIME %s", ValueRawModeErr)

	err = restoreTerminal(int(file.Fd()), savedTermios)
	assert.Nil(t, err, "Terminal should be properly restored")
}
