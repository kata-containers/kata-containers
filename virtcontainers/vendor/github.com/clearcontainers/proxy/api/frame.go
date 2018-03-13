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

package api

import (
	"encoding/json"
)

// Version encodes the proxy protocol version.
//
// List of changes:
//
//   • version 2: initial version released with Clear Containers 3.0
//
//                ⚠⚠⚠ backward incompatible with version 1 ⚠⚠⚠
//
//     List of changes:
//
//       • Changed the frame header to include additional fields: version,
//         header length, type and opcode.
//       • Added a log messages for clients to insert log entries to the
//         consolidated proxy log.
//
//   • version 1: initial version released with Clear Containers 2.1
const Version = 2

// FrameType is the type of frame and is part of the frame header.
type FrameType int

const (
	// TypeCommand is a command from a client to the proxy.
	TypeCommand FrameType = iota
	// TypeResponse is a command response back from the proxy to a client.
	TypeResponse
	// TypeStream is a stream of data from a client to the proxy. Streams
	// are to be forwarded onto the VM agent.
	TypeStream
	// TypeNotification is a notification sent by either the proxy or
	// clients. Notifications are one way only and do not prompt a
	// response.
	TypeNotification
	// TypeMax is the number of types.
	TypeMax
)

const unknown = "unknown"

// String implements Stringer for FrameType.
func (t FrameType) String() string {
	switch t {
	case TypeCommand:
		return "command"
	case TypeResponse:
		return "response"
	case TypeStream:
		return "stream"
	case TypeNotification:
		return "notification"
	default:
		return unknown
	}
}

// Command is the kind of command being sent. In the frame header, Opcode must
// have one of these values when Type is api.TypeCommand.
type Command int

const (
	// CmdRegisterVM registers a new VM/POD.
	CmdRegisterVM Command = iota
	// CmdUnregisterVM unregisters a VM/POD.
	CmdUnregisterVM
	// CmdAttachVM attaches to a registered VM.
	CmdAttachVM
	// CmdHyper sends a hyperstart command through the proxy.
	CmdHyper
	// CmdConnectShim identifies the client as a shim.
	CmdConnectShim
	// CmdDisconnectShim unregisters a shim. DisconnectShim is a bit
	// special and doesn't send a Response back but closes the connection.
	CmdDisconnectShim
	// CmdSignal sends a signal to the process inside the VM. A client
	// needs to be connected as a shim before it can issue that command.
	CmdSignal
	// CmdMax is the number of commands.
	CmdMax
)

// String implements Stringer for Command.
func (t Command) String() string {
	switch t {
	case CmdRegisterVM:
		return "RegisterVM"
	case CmdUnregisterVM:
		return "UnregisterVM"
	case CmdAttachVM:
		return "AttachVM"
	case CmdHyper:
		return "Hyper"
	case CmdConnectShim:
		return "ConnectShim"
	case CmdDisconnectShim:
		return "DisconnectShim"
	case CmdSignal:
		return "Signal"
	default:
		return unknown
	}
}

// Stream is the kind of stream being sent. In the frame header, Opcode must
// have one of the these values when Type is api.TypeStream.
type Stream int

const (
	// StreamStdin is a stream conveying stdin data.
	StreamStdin Stream = iota
	// StreamStdout is a stream conveying stdout data.
	StreamStdout
	// StreamStderr is a stream conveying stderr data.
	StreamStderr
	// StreamLog is a stream conveying structured logs messages. Each Log frame
	// contains a JSON object which fields are the structured log. By convention
	// it would be nice to have a few common fields in log entries to ease
	// post-processing. See the LogEntry payload for details.
	StreamLog
	// StreamMax is the number of stream types.
	StreamMax
)

// String implements Stringer for Stream.
func (s Stream) String() string {
	switch s {
	case StreamStdin:
		return "stdin"
	case StreamStdout:
		return "stdout"
	case StreamStderr:
		return "stderr"
	case StreamLog:
		return "log"
	default:
		return unknown
	}
}

// Notification is the kind of notification being sent. In the frame header,
// Opcode must have one of the these values when Type is api.TypeNotification.
type Notification int

const (
	// NotificationProcessExited is sent to signal a process in the VM has exited.
	NotificationProcessExited = iota
	// NotificationMax is the number of notification types.
	NotificationMax
)

// String implements Stringer for Notification.
func (n Notification) String() string {
	switch n {
	case NotificationProcessExited:
		return "ProcessExited"
	default:
		return unknown
	}
}

// FrameHeader is the header of a Frame.
type FrameHeader struct {
	Version int
	// HeaderLength in the size of the header in bytes (the on-wire
	// HeaderLength is in number of 32-bits words tough).
	HeaderLength  int
	Type          FrameType
	Opcode        int
	PayloadLength int
	InError       bool
}

// Frame is the basic communication unit with the proxy.
type Frame struct {
	Header  FrameHeader
	Payload []byte
}

// NewFrame creates a new Frame with type t, operand op and given payload.
func NewFrame(t FrameType, op int, payload []byte) *Frame {
	return &Frame{
		Header: FrameHeader{
			Version:       Version,
			HeaderLength:  minHeaderLength,
			Type:          t,
			Opcode:        op,
			PayloadLength: len(payload),
		},
		Payload: payload,
	}
}

// NewFrameJSON creates a new Frame with type t, operand op and given payload.
// The payload structure is marshalled into JSON.
func NewFrameJSON(t FrameType, op int, payload interface{}) (*Frame, error) {
	var data []byte

	if payload != nil {
		var err error

		if data, err = json.Marshal(payload); err != nil {
			return nil, err
		}
	}

	return &Frame{
		Header: FrameHeader{
			Version:       Version,
			HeaderLength:  minHeaderLength,
			Type:          t,
			Opcode:        op,
			PayloadLength: len(data),
		},
		Payload: data,
	}, nil
}
