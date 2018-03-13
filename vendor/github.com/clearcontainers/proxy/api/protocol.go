// Copyright (c) 2016 Intel Corporation
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
	"encoding/binary"
	"encoding/json"
	"errors"
	"fmt"
	"io"
)

// minHeaderLength is the length of the header in the version 2 of protocol.
// It is guaranteed later versions will have a header at least that big.
const minHeaderLength = 12 // in bytes

// A Request is a JSON message sent from a client to the proxy. This message
// embed a payload identified by "id". A payload can have data associated with
// it. It's useful to think of Request as an RPC call with "id" as function
// name and "data" as arguments.
//
// The list of possible payloads are documented in this package.
//
// Each Request has a corresponding Response message sent back from the proxy.
type Request struct {
	ID   string          `json:"id"`
	Data json.RawMessage `json:"data,omitempty"`
}

// A Response is a JSON message sent back from the proxy to a client after a
// Request has been issued. The Response holds the result of the Request,
// including its success state and optional data. It's useful to think of
// Response as the result of an RPC call with ("success", "error") describing
// if the call has been successful and "data" holding the optional results.
type Response struct {
	Success bool                   `json:"success"`
	Error   string                 `json:"error,omitempty"`
	Data    map[string]interface{} `json:"data,omitempty"`
}

// Offsets (in bytes) of frame headers fields.
const (
	versionOffset       = 0
	headerLengthOffset  = 2
	typeOffset          = 6
	flagsOffset         = 6
	opcodeOffset        = 7
	payloadLengthOffset = 8
)

// Size (in bytes) of frame header fields (when larger than 1 byte).
const (
	versionSize       = 2
	payloadLengthSize = 4
)

// Masks needed to extract fields
const (
	typeMask  = 0x0f
	flagsMask = 0xf0
)

func maxOpcodeForFrameType(t FrameType) int {
	switch t {
	default:
		fallthrough
	case TypeCommand:
		return int(CmdMax)
	case TypeResponse:
		return int(CmdMax)
	case TypeStream:
		return int(StreamMax)
	case TypeNotification:
		return int(NotificationMax)
	}
}

// ReadFrame reads a full frame (header and payload) from r.
func ReadFrame(r io.Reader) (*Frame, error) {
	// Read the header.
	buf := make([]byte, minHeaderLength)
	n, err := r.Read(buf)
	if err != nil {
		return nil, err
	}
	if n != minHeaderLength {
		return nil, errors.New("frame: couldn't read the full header")
	}

	// Decode it.
	frame := &Frame{}
	header := &frame.Header
	header.Version = int(binary.BigEndian.Uint16(buf[versionOffset : versionOffset+versionSize]))
	if header.Version < 2 || header.Version > Version {
		return nil, fmt.Errorf("frame: bad version %d", header.Version)
	}
	header.HeaderLength = int(buf[headerLengthOffset]) * 4
	header.Type = FrameType(buf[typeOffset] & typeMask)
	flags := buf[flagsOffset] & flagsMask
	if flags&flagInError != 0 {
		header.InError = true
	}
	if header.Type >= TypeMax {
		return nil, fmt.Errorf("frame: bad type %s", header.Type)
	}
	header.Opcode = int(buf[opcodeOffset])
	if header.Opcode >= maxOpcodeForFrameType(header.Type) {
		return nil, fmt.Errorf("frame: bad opcode (%d) for type %s", header.Opcode,
			header.Type)
	}
	header.PayloadLength = int(binary.BigEndian.Uint32(buf[payloadLengthOffset : payloadLengthOffset+payloadLengthSize]))

	// Read the payload.
	received := 0
	need := header.HeaderLength - minHeaderLength + header.PayloadLength
	payload := make([]byte, need)
	for received < need {
		n, err := r.Read(payload[received:need])
		if err != nil {
			return nil, err
		}

		received += n
	}

	// Skip the bytes part of a bigger header than expected to just keep
	// the payload.
	frame.Payload = payload[header.HeaderLength-minHeaderLength : need]

	return frame, nil
}

const (
	flagInError = 1 << (4 + iota)
)

// WriteFrame writes a frame into w.
//
// Note that frame.Header.PayloadLength dictates the amount of data of
// frame.Payload to write, so frame.Header.Payload must be less or equal to
// len(frame.Payload).
func WriteFrame(w io.Writer, frame *Frame) error {
	header := &frame.Header

	if len(frame.Payload) < header.PayloadLength {
		return fmt.Errorf("frame: bad payload length %d",
			header.PayloadLength)
	}

	// Prepare the header.
	len := minHeaderLength + header.PayloadLength
	buf := make([]byte, len)
	binary.BigEndian.PutUint16(buf[versionOffset:versionOffset+versionSize], uint16(header.Version))
	buf[headerLengthOffset] = byte(header.HeaderLength / 4)
	flags := byte(0)
	if frame.Header.InError {
		flags |= flagInError
	}
	buf[typeOffset] = flags | byte(header.Type)&typeMask
	buf[opcodeOffset] = byte(header.Opcode)
	binary.BigEndian.PutUint32(buf[payloadLengthOffset:payloadLengthOffset+payloadLengthSize],
		uint32(header.PayloadLength))

	// Write payload if needed
	if header.PayloadLength > 0 {
		copy(buf[minHeaderLength:], frame.Payload[0:header.PayloadLength])
	}

	n, err := w.Write(buf)
	if err != nil {
		return err
	}

	if n != len {
		return errors.New("frame: couldn't write frame")
	}

	return nil
}

// WriteCommand is a convenience wrapper around WriteFrame to send commands.
func WriteCommand(w io.Writer, op Command, payload []byte) error {
	return WriteFrame(w, NewFrame(TypeCommand, int(op), payload))
}

// WriteResponse is a convenience wrapper around WriteFrame to send responses.
func WriteResponse(w io.Writer, op Command, inError bool, payload []byte) error {
	frame := NewFrame(TypeResponse, int(op), payload)
	frame.Header.InError = inError
	return WriteFrame(w, frame)
}

// WriteStream is a convenience wrapper around WriteFrame to send stream packets.
func WriteStream(w io.Writer, op Stream, payload []byte) error {
	return WriteFrame(w, NewFrame(TypeStream, int(op), payload))
}

// WriteNotification is a convenience wrapper around WriteFrame to send notifications.
func WriteNotification(w io.Writer, op Notification, payload []byte) error {
	return WriteFrame(w, NewFrame(TypeNotification, int(op), payload))
}
