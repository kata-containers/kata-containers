//
// Copyright (c) 2017 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package uevent

import (
	"bufio"
	"io"
	"strings"

	"golang.org/x/sys/unix"
	"google.golang.org/grpc/codes"
	grpcStatus "google.golang.org/grpc/status"
)

const (
	uEventAction    = "ACTION"
	uEventDevPath   = "DEVPATH"
	uEventSubSystem = "SUBSYSTEM"
	uEventSeqNum    = "SEQNUM"
	uEventDevName   = "DEVNAME"
	uEventInterface = "INTERFACE"

	paramDelim = 0x00
)

// ReaderCloser defines a uevent reader/closer. It is an io.ReaderCloser implementation.
type ReaderCloser struct {
	fd int
}

// NewReaderCloser returns an io.ReadCloser handle for uevent.
func NewReaderCloser() (io.ReadCloser, error) {
	nl := unix.SockaddrNetlink{
		Family: unix.AF_NETLINK,
		// Passing Pid as 0 here allows the kernel to take care of assigning
		// it. This allows multiple netlink sockets to be used.
		Pid:    uint32(0),
		Groups: 1,
	}

	fd, err := unix.Socket(unix.AF_NETLINK, unix.SOCK_RAW, unix.NETLINK_KOBJECT_UEVENT)
	if err != nil {
		return nil, err
	}

	if err := unix.Bind(fd, &nl); err != nil {
		return nil, err
	}

	return &ReaderCloser{fd}, nil
}

// Read implements reading function for uevent.
func (r *ReaderCloser) Read(p []byte) (int, error) {
	count, err := unix.Read(r.fd, p)
	if count < 0 && err != nil {
		count = 0
	}
	return count, err
}

// Close implements closing function for uevent.
func (r *ReaderCloser) Close() error {
	return unix.Close(r.fd)
}

// Uevent represents a single uevent.
type Uevent struct {
	Header    string
	Action    string
	DevPath   string
	SubSystem string
	SeqNum    string
	DevName   string
}

// Handler represents a uevent handler.
type Handler struct {
	readerCloser io.ReadCloser
	bufioReader  *bufio.Reader
}

// NewHandler returns a uevent handler.
func NewHandler() (*Handler, error) {
	rdCloser, err := NewReaderCloser()
	if err != nil {
		return nil, err
	}

	return &Handler{
		readerCloser: rdCloser,
		bufioReader:  bufio.NewReader(rdCloser),
	}, nil
}

// Read blocks and returns the next uevent when available.
func (h *Handler) Read() (*Uevent, error) {
	uEv := &Uevent{}

	// Read header first.
	header, err := h.bufioReader.ReadString(paramDelim)
	if err != nil {
		return nil, err
	}

	// Fill uevent header.
	uEv.Header = header

	exitLoop := false

	// Read every parameter as "key=value".
	for !exitLoop {
		keyValue, err := h.bufioReader.ReadString(paramDelim)
		if err != nil {
			return nil, err
		}

		idx := strings.Index(keyValue, "=")
		if idx < 1 {
			return nil, grpcStatus.Errorf(codes.InvalidArgument, "Could not decode uevent: Wrong format %q", keyValue)
		}

		// The key is the first parameter, and the value is the rest
		// without the "=" sign, and without the last character since
		// it is the delimiter.
		key, val := keyValue[:idx], keyValue[idx+1:len(keyValue)-1]

		switch key {
		case uEventAction:
			uEv.Action = val
		case uEventDevPath:
			uEv.DevPath = val
		case uEventSubSystem:
			uEv.SubSystem = val
		case uEventDevName:
			uEv.DevName = val
		case uEventInterface:
			// In case of network interfaces, DevName will be empty since a device node
			// is not created. Instead store the "INTERFACE" field as devName
			uEv.DevName = val
		case uEventSeqNum:
			uEv.SeqNum = val

			// "SEQNUM" signals the uevent is complete.
			exitLoop = true
		}
	}

	return uEv, nil
}

// Close shuts down the uevent handler and closes the underlying netlink
// connection.
func (h *Handler) Close() error {
	return h.readerCloser.Close()
}
