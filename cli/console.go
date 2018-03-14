// Copyright (c) 2014,2015,2016 Docker, Inc.
// Copyright (c) 2017 Intel Corporation
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//      http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

package main

import (
	"fmt"
	"io"
	"os"
	"syscall"
	"unsafe"

	"golang.org/x/sys/unix"
)

var ptmxPath = "/dev/ptmx"

// Console represents a pseudo TTY.
type Console struct {
	io.ReadWriteCloser

	master    *os.File
	slavePath string
}

// isTerminal returns true if fd is a terminal, else false
func isTerminal(fd uintptr) bool {
	var termios syscall.Termios
	_, _, err := syscall.Syscall(syscall.SYS_IOCTL, fd, syscall.TCGETS, uintptr(unsafe.Pointer(&termios)))
	return err == 0
}

// ConsoleFromFile creates a console from a file
func ConsoleFromFile(f *os.File) *Console {
	return &Console{
		master: f,
	}
}

// NewConsole returns an initialized console that can be used within a container by copying bytes
// from the master side to the slave that is attached as the tty for the container's init process.
func newConsole() (*Console, error) {
	master, err := os.OpenFile(ptmxPath, unix.O_RDWR|unix.O_NOCTTY|unix.O_CLOEXEC, 0)
	if err != nil {
		return nil, err
	}
	if err := saneTerminal(master); err != nil {
		return nil, err
	}
	console, err := ptsname(master)
	if err != nil {
		return nil, err
	}
	if err := unlockpt(master); err != nil {
		return nil, err
	}
	return &Console{
		slavePath: console,
		master:    master,
	}, nil
}

// File returns master
func (c *Console) File() *os.File {
	return c.master
}

// Path to slave
func (c *Console) Path() string {
	return c.slavePath
}

// Read from master
func (c *Console) Read(b []byte) (int, error) {
	return c.master.Read(b)
}

// Write to master
func (c *Console) Write(b []byte) (int, error) {
	return c.master.Write(b)
}

// Close master
func (c *Console) Close() error {
	if m := c.master; m != nil {
		return m.Close()
	}
	return nil
}

func ioctl(fd uintptr, flag, data uintptr) error {
	if _, _, err := unix.Syscall(unix.SYS_IOCTL, fd, flag, data); err != 0 {
		return err
	}
	return nil
}

// unlockpt unlocks the slave pseudoterminal device corresponding to the master pseudoterminal referred to by f.
// unlockpt should be called before opening the slave side of a pty.
func unlockpt(f *os.File) error {
	var u int32
	return ioctl(f.Fd(), unix.TIOCSPTLCK, uintptr(unsafe.Pointer(&u)))
}

// ptsname retrieves the name of the first available pts for the given master.
func ptsname(f *os.File) (string, error) {
	var n int32
	if err := ioctl(f.Fd(), unix.TIOCGPTN, uintptr(unsafe.Pointer(&n))); err != nil {
		return "", err
	}
	return fmt.Sprintf("/dev/pts/%d", n), nil
}

// saneTerminal sets the necessary tty_ioctl(4)s to ensure that a pty pair
// created by us acts normally. In particular, a not-very-well-known default of
// Linux unix98 ptys is that they have +onlcr by default. While this isn't a
// problem for terminal emulators, because we relay data from the terminal we
// also relay that funky line discipline.
func saneTerminal(terminal *os.File) error {
	// Go doesn't have a wrapper for any of the termios ioctls.
	var termios unix.Termios

	if err := ioctl(terminal.Fd(), unix.TCGETS, uintptr(unsafe.Pointer(&termios))); err != nil {
		return fmt.Errorf("ioctl(tty, tcgets): %s", err.Error())
	}

	// Set -onlcr so we don't have to deal with \r.
	termios.Oflag &^= unix.ONLCR

	if err := ioctl(terminal.Fd(), unix.TCSETS, uintptr(unsafe.Pointer(&termios))); err != nil {
		return fmt.Errorf("ioctl(tty, tcsets): %s", err.Error())
	}

	return nil
}
